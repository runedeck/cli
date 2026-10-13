//! The vocabulary a repository's prose may set in italics: every term the
//! ontology defines, or, without an ontology, the entries of `glossary.md`.
//! An ontology term also carries an IRI, so a document that sets the term
//! in italics cites it as `*term* [TAG]` with `[TAG]: <iri>` at the end of
//! the file, and validation proves the citation resolves.
//!
//! The Turtle reader is a statement-shape parser, not an RDF parser. It
//! reads the shape the deck writes: `prefix:Name a <type> ; predicates .`
//! with the terminating period at the end of a line. Anything it cannot
//! read is an error, never a silently missing term.

use super::commands::print_json;
use super::{Error, ErrorKind, relative_display, resolve_spec_root};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

/// The glossary sits beside the capability directories.
pub const GLOSSARY_FILE: &str = "glossary.md";
/// The ontology sits at the repository root unless `deck.yaml` names another file.
pub const DEFAULT_ONTOLOGY_FILE: &str = "ontology/rune.ttl";

/// How the CLI hands the `ontology` path from `deck.yaml` to this crate,
/// which knows nothing about deck manifests.
pub type OntologyLookup = fn(&Path) -> Result<Option<String>, String>;
static ONTOLOGY_LOOKUP: OnceLock<OntologyLookup> = OnceLock::new();

pub fn set_ontology_lookup(lookup: OntologyLookup) -> bool {
    ONTOLOGY_LOOKUP.set(lookup).is_ok()
}

/// Where this repository keeps its ontology, configured or default. A
/// manifest that cannot be read is an error, not the default path.
pub fn ontology_path(repository: &Path) -> Result<PathBuf, Error> {
    let configured = match ONTOLOGY_LOOKUP.get() {
        Some(lookup) => {
            lookup(repository).map_err(|message| Error::new(ErrorKind::Config, message))?
        }
        None => None,
    };
    Ok(repository.join(configured.as_deref().unwrap_or(DEFAULT_ONTOLOGY_FILE)))
}

static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*@prefix\s+([A-Za-z_][\w-]*)?:\s*<([^>]*)>\s*\.")
        .expect("prefix regex is valid")
});
/// One term statement: `prefix:Name a <type> ... .` where the type is a
/// class, a property, or a SKOS concept. The body runs to the first period
/// that ends a line after whitespace or a closing quote, an optional
/// Turtle comment allowed after it. The body keeps that closing quote, so
/// a comment that ends the statement is still a complete literal.
static TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?ms)^([A-Za-z_][\w-]*)?:([\w-]+)\s+(?:a|rdf:type)\s+(rdfs:Class|rdf:Property|owl:Class|owl:ObjectProperty|owl:DatatypeProperty|skos:Concept)\b(.*?(?:\s|"))\.[ \t]*(?:#[^\n]*)?$"#,
    )
    .expect("term regex is valid")
});
static LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"rdfs:label\s+"((?:[^"\\]|\\.)*)""#).expect("label regex is valid")
});
static COMMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"rdfs:comment\s+"((?:[^"\\]|\\.)*)""#).expect("comment regex is valid")
});
static TURTLE_COMMENT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*#[^\n]*$").expect("comment line regex is valid"));
static GLOSSARY_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*-\s+\*\*([^*\n]+?)\*\*\s*:\s*(.*?)\s*$")
        .expect("glossary entry regex is valid")
});

/// What kind of resource a term is. Classes take the plain reference tag
/// when a property shares their name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum TermKind {
    Class,
    Property,
    Concept,
    /// A glossary entry, which has no RDF type.
    Entry,
}

/// One defined term. From a glossary the name is the label and the IRI is empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Term {
    pub name: String,
    pub iri: String,
    pub label: String,
    pub comment: String,
    pub kind: TermKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TermSource {
    /// `ontology/rune.ttl` or the file `deck.yaml` names.
    Ontology,
    /// `<specs root>/glossary.md`, the fallback for a repository without an ontology.
    Glossary,
    /// Neither file exists; the first italic term is an error that names the glossary to create.
    #[default]
    Absent,
}

#[derive(Debug, Default)]
pub struct Terms {
    source: TermSource,
    /// Repository-relative path of the source file, for diagnostics.
    location: String,
    entries: Vec<Term>,
    by_label: BTreeMap<String, usize>,
    by_iri: BTreeMap<String, usize>,
    /// Every IRI prefix a term was declared under.
    namespaces: BTreeSet<String>,
}

impl Terms {
    /// The ontology when the repository has one, else the glossary beside
    /// the specifications, else an absent source that names the glossary.
    /// Only a missing ontology file falls back; any other failure to read
    /// or parse it is an error.
    pub fn load(repository: &Path, specifications: &Path) -> Result<Self, Error> {
        let ontology = ontology_path(repository)?;
        let ontology_location = relative_display(repository, &ontology);
        match std::fs::read_to_string(&ontology) {
            Ok(text) => return Self::from_turtle(&text, &ontology_location),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("cannot read ontology {ontology_location}: {error}"),
                ));
            }
        }
        let glossary = specifications.join(GLOSSARY_FILE);
        let location = relative_display(repository, &glossary);
        match std::fs::read_to_string(&glossary) {
            Ok(text) => Ok(Self::from_glossary(&text, location)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                location,
                ..Self::default()
            }),
            Err(error) => Err(Error::new(
                ErrorKind::Io,
                format!("cannot read glossary {location}: {error}"),
            )),
        }
    }

    /// Read every labeled class, property, and concept from Turtle text.
    /// An undeclared prefix, two terms sharing a label (compared without
    /// case), or an ontology with no labeled term is an error.
    pub fn from_turtle(text: &str, location: &str) -> Result<Self, Error> {
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
        let text = TURTLE_COMMENT_LINE.replace_all(text, "");
        let prefixes: BTreeMap<&str, &str> = PREFIX
            .captures_iter(&text)
            .map(|capture| {
                (
                    capture.get(1).map_or("", |prefix| prefix.as_str()),
                    capture.get(2).map_or("", |iri| iri.as_str()),
                )
            })
            .collect();
        let mut terms = Self {
            source: TermSource::Ontology,
            location: location.to_string(),
            ..Self::default()
        };
        for capture in TERM.captures_iter(&text) {
            let prefix = capture.get(1).map_or("", |prefix| prefix.as_str());
            let name = &capture[2];
            let Some(label) = LABEL.captures(&capture[4]) else {
                continue;
            };
            let Some(namespace) = prefixes.get(prefix) else {
                return Err(Error::new(
                    ErrorKind::Parse,
                    format!("{location}: term {prefix}:{name} uses a prefix no @prefix declares"),
                ));
            };
            let kind = match &capture[3] {
                "rdf:Property" | "owl:ObjectProperty" | "owl:DatatypeProperty" => {
                    TermKind::Property
                }
                "skos:Concept" => TermKind::Concept,
                _ => TermKind::Class,
            };
            terms.namespaces.insert((*namespace).to_string());
            let term = Term {
                name: name.to_string(),
                iri: format!("{namespace}{name}"),
                label: unescape(&label[1]),
                comment: COMMENT
                    .captures(&capture[4])
                    .map(|comment| unescape(&comment[1]))
                    .unwrap_or_default(),
                kind,
            };
            let label = normalize(&term.label);
            match terms.by_label.get(&label).copied() {
                Some(index) if terms.entries[index].kind == term.kind => {
                    return Err(Error::new(
                        ErrorKind::Parse,
                        format!(
                            "{location}: {} and {} share the label \"{}\"; labels are compared without case",
                            terms.entries[index].iri, term.iri, term.label
                        ),
                    ));
                }
                // A class and its property may share a label (`Change` and
                // `change`); the italic term is the class, the property is
                // reached through its own tag.
                Some(index) if precedence(term.kind) >= precedence(terms.entries[index].kind) => {}
                _ => {
                    terms.by_label.insert(label, terms.entries.len());
                }
            }
            terms.entries.push(term);
        }
        if terms.entries.is_empty() {
            return Err(Error::new(
                ErrorKind::Parse,
                format!("{location}: no class, property, or concept carries an rdfs:label"),
            ));
        }
        let preferred: Vec<String> = terms
            .by_label
            .values()
            .map(|index| terms.entries[*index].iri.clone())
            .collect();
        // By label, then the class before the property that shares it, then
        // by name, so the order does not depend on the order of the file.
        terms.entries.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| precedence(left.kind).cmp(&precedence(right.kind)))
                .then_with(|| left.name.cmp(&right.name))
        });
        terms.index(&preferred);
        Ok(terms)
    }

    /// Read `- **term**: definition` bullets outside fenced code.
    pub fn from_glossary(text: &str, location: String) -> Self {
        let mut terms = Self {
            source: TermSource::Glossary,
            location,
            ..Self::default()
        };
        let mut fence: Option<(char, usize)> = None;
        for line in text.lines() {
            let trimmed = line.trim_start();
            let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let run = marker.map_or(0, |c| trimmed.chars().take_while(|d| *d == c).count());
            match fence {
                Some((open, width)) if marker == Some(open) && run >= width => {
                    fence = None;
                    continue;
                }
                Some(_) => continue,
                None if run >= 3 => {
                    fence = marker.map(|c| (c, run));
                    continue;
                }
                None => {}
            }
            if let Some(capture) = GLOSSARY_ENTRY.captures(line) {
                let label = capture[1].trim().to_string();
                terms.entries.push(Term {
                    name: label.clone(),
                    iri: String::new(),
                    label,
                    comment: capture[2].to_string(),
                    kind: TermKind::Entry,
                });
            }
        }
        terms.index(&[]);
        terms
    }

    /// Rebuild the lookups after sorting. `preferred` names the IRI that
    /// owns each shared label; every other label maps to its only term.
    fn index(&mut self, preferred: &[String]) {
        self.by_label.clear();
        self.by_iri.clear();
        for (index, term) in self.entries.iter().enumerate() {
            let label = normalize(&term.label);
            if preferred.contains(&term.iri) || !self.by_label.contains_key(&label) {
                self.by_label.insert(label, index);
            }
            if !term.iri.is_empty() {
                self.by_iri.insert(term.iri.clone(), index);
            }
        }
    }

    pub fn source(&self) -> TermSource {
        self.source
    }

    pub fn location(&self) -> &str {
        &self.location
    }

    /// Whether an IRI sits under a namespace the ontology declares terms in.
    pub fn in_namespace(&self, iri: &str) -> bool {
        self.namespaces
            .iter()
            .any(|namespace| iri.starts_with(namespace))
    }

    /// Every term, sorted by label.
    pub fn terms(&self) -> &[Term] {
        &self.entries
    }

    /// A term matches its label exactly or as a plain plural (`trailers`
    /// finds `trailer`, `harnesses` finds `harness`), compared without case.
    pub fn resolve(&self, italic: &str) -> Option<&Term> {
        let normalized = normalize(italic);
        let singulars = [
            Some(normalized.as_str()),
            normalized.strip_suffix('s'),
            normalized.strip_suffix("es"),
        ];
        singulars
            .into_iter()
            .flatten()
            .find_map(|candidate| self.by_label.get(candidate))
            .map(|index| &self.entries[*index])
    }

    pub fn by_iri(&self, iri: &str) -> Option<&Term> {
        self.by_iri.get(iri).map(|index| &self.entries[*index])
    }

    /// The reference tag for each term, keyed by IRI: the local name in
    /// upper case. Markdown reference labels are case-insensitive, so when
    /// two names collide the class keeps the plain tag and every later
    /// term takes `-KEY` suffixes until its tag is unique.
    pub fn tags(&self) -> BTreeMap<String, String> {
        let mut order: Vec<&Term> = self.entries.iter().collect();
        order.sort_by(|left, right| (left.kind, &left.label).cmp(&(right.kind, &right.label)));
        let mut tags = BTreeMap::new();
        let mut taken = BTreeSet::new();
        for term in order {
            let mut tag = term.name.to_uppercase();
            while !taken.insert(tag.clone()) {
                tag.push_str("-KEY");
            }
            tags.insert(term.iri.clone(), tag);
        }
        tags
    }
}

/// Which kind owns a label two terms share: the class, then the concept,
/// then the property.
fn precedence(kind: TermKind) -> u8 {
    match kind {
        TermKind::Class => 0,
        TermKind::Concept => 1,
        TermKind::Property => 2,
        TermKind::Entry => 3,
    }
}

fn normalize(term: &str) -> String {
    term.trim().to_lowercase()
}

/// Undo the Turtle string escapes the deck uses: `\"` and `\\`.
fn unescape(literal: &str) -> String {
    literal.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// `rune spec glossary`: the term list, one `- **label**: comment [TAG]` line
/// per term and the `[TAG]: <iri>` definitions a document copies.
#[derive(Debug, Serialize)]
pub struct GlossaryOutput {
    pub source: String,
    pub terms: Vec<GlossaryTerm>,
}

#[derive(Debug, Serialize)]
pub struct GlossaryTerm {
    pub label: String,
    pub comment: String,
    pub name: String,
    pub iri: String,
    pub kind: TermKind,
    pub tag: String,
}

impl GlossaryOutput {
    fn from_terms(terms: &Terms) -> Self {
        let tags = terms.tags();
        Self {
            source: terms.location().to_string(),
            terms: terms
                .terms()
                .iter()
                .map(|term| GlossaryTerm {
                    label: term.label.clone(),
                    comment: term.comment.clone(),
                    name: term.name.clone(),
                    iri: term.iri.clone(),
                    kind: term.kind,
                    tag: tags.get(&term.iri).cloned().unwrap_or_default(),
                })
                .collect(),
        }
    }
}

pub fn glossary_output(repository: &Path, specifications: &Path) -> Result<GlossaryOutput, Error> {
    Ok(GlossaryOutput::from_terms(&Terms::load(
        repository,
        specifications,
    )?))
}

/// Print the term list, or its JSON form.
pub fn glossary(source: &str, json: bool) -> Result<i32, Error> {
    let spec_root = resolve_spec_root(Path::new(source))?;
    let output = glossary_output(spec_root.repository(), spec_root.specifications())?;
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_glossary(&output));
    }
    Ok(0)
}

pub fn render_glossary(output: &GlossaryOutput) -> String {
    let mut lines: Vec<String> = output.terms.iter().map(entry_line).collect();
    let definitions: Vec<String> = output
        .terms
        .iter()
        .filter(|term| !term.tag.is_empty())
        .map(|term| format!("[{}]: {}", term.tag, term.iri))
        .collect();
    if !definitions.is_empty() {
        lines.push(String::new());
        lines.extend(definitions);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// One `- **label**: comment [TAG]` line. A term without a comment keeps
/// one space before its tag.
fn entry_line(term: &GlossaryTerm) -> String {
    let mut line = format!("- **{}**:", term.label);
    if !term.comment.is_empty() {
        line.push(' ');
        line.push_str(&term.comment);
    }
    if !term.tag.is_empty() {
        line.push_str(" [");
        line.push_str(&term.tag);
        line.push(']');
    }
    line
}

/// The command the glossary header and a drift report tell the reader to run.
const WRITE_COMMAND: &str = "rune spec glossary --write";

/// The glossary file: a header that names the ontology and the write
/// command, then the term list `render_glossary` prints.
pub fn render_glossary_file(output: &GlossaryOutput) -> String {
    let namespace = output
        .terms
        .first()
        .and_then(|term| term.iri.strip_suffix(term.name.as_str()))
        .unwrap_or("https://runedeck.ai/ns#");
    format!(
        "# Glossary\n\n\
         Generated from `{}` by `{WRITE_COMMAND}`. Do not edit this file. Change the ontology.\n\
         A specification sets a term in italics at first use and cites it as `*term* [TERM]` with\n\
         `[TERM]: {namespace}Term` at the end of the file.\n\n{}",
        output.source,
        render_glossary(output)
    )
}

/// Where the glossary file sits and what the rendering looks like, for
/// `--write` and `--check`. The ontology is the only source a file can be
/// rendered from: a repository that reads its terms from a glossary has
/// nothing to render.
struct GlossaryFile {
    path: PathBuf,
    display: String,
    custom: bool,
    output: GlossaryOutput,
}

impl GlossaryFile {
    fn resolve(source: &str, path: Option<&str>) -> Result<Self, Error> {
        let spec_root = resolve_spec_root(Path::new(source))?;
        let repository = spec_root.repository();
        let terms = Terms::load(repository, spec_root.specifications())?;
        if terms.source() != TermSource::Ontology {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "no ontology at {}; the glossary file is rendered from the ontology",
                    relative_display(repository, &ontology_path(repository)?)
                ),
            ));
        }
        let file = match path {
            Some(path) => repository.join(path),
            None => spec_root.specifications().join(GLOSSARY_FILE),
        };
        Ok(Self {
            display: relative_display(repository, &file),
            path: file,
            custom: path.is_some(),
            output: GlossaryOutput::from_terms(&terms),
        })
    }

    fn write_command(&self) -> String {
        if self.custom {
            format!("{WRITE_COMMAND} {}", self.display)
        } else {
            WRITE_COMMAND.to_string()
        }
    }
}

/// What `--write` reports.
#[derive(Debug, Serialize)]
pub struct GlossaryWriteOutput {
    pub path: String,
    pub terms: usize,
}

/// Write the glossary file from the ontology, creating its directory.
pub fn write_glossary_output(
    source: &str,
    path: Option<&str>,
) -> Result<GlossaryWriteOutput, Error> {
    let file = GlossaryFile::resolve(source, path)?;
    let io_error = |error: std::io::Error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", file.display),
        )
    };
    if let Some(parent) = file.path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    std::fs::write(&file.path, render_glossary_file(&file.output)).map_err(io_error)?;
    Ok(GlossaryWriteOutput {
        path: file.display,
        terms: file.output.terms.len(),
    })
}

pub fn write_glossary(source: &str, path: Option<&str>, json: bool) -> Result<i32, Error> {
    let output = write_glossary_output(source, path)?;
    if json {
        print_json(&output)?;
    } else {
        println!("wrote {}", output.path);
    }
    Ok(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GlossaryStatus {
    Current,
    Stale,
    Missing,
}

/// One term a drift report names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlossaryEntry {
    pub label: String,
    pub tag: String,
}

impl GlossaryEntry {
    fn render(&self) -> String {
        if self.tag.is_empty() {
            self.label.clone()
        } else {
            format!("{} [{}]", self.label, self.tag)
        }
    }
}

/// What `--check` reports: the terms the file lacks, the terms it holds
/// that the ontology no longer defines, and the terms whose line differs.
#[derive(Debug, Serialize)]
pub struct GlossaryCheckOutput {
    pub path: String,
    pub status: GlossaryStatus,
    pub write_command: String,
    pub missing: Vec<GlossaryEntry>,
    pub extra: Vec<GlossaryEntry>,
    pub changed: Vec<GlossaryEntry>,
}

impl GlossaryCheckOutput {
    pub fn is_current(&self) -> bool {
        self.status == GlossaryStatus::Current
    }
}

static ENTRY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- \*\*(?P<label>[^*\n]+)\*\*:(?:.*?)(?: \[(?P<tag>[^\[\]\s]+)\])?$")
        .expect("entry line regex is valid")
});

/// The entry lines of a glossary text, keyed by tag, or by label for a
/// line that carries no tag.
fn entry_lines(text: &str) -> Vec<(String, GlossaryEntry, &str)> {
    text.lines()
        .filter_map(|line| {
            let capture = ENTRY_LINE.captures(line)?;
            let entry = GlossaryEntry {
                label: capture["label"].to_string(),
                tag: capture
                    .name("tag")
                    .map_or_else(String::new, |tag| tag.as_str().to_string()),
            };
            let key = if entry.tag.is_empty() {
                format!("label:{}", entry.label)
            } else {
                format!("tag:{}", entry.tag)
            };
            Some((key, entry, line))
        })
        .collect()
}

/// Name the terms whose entry lines differ between the rendering and the file.
fn compare_glossary(output: &mut GlossaryCheckOutput, rendered: &str, current: &str) {
    let wanted = entry_lines(rendered);
    let found = entry_lines(current);
    let found_by_key: BTreeMap<&str, &str> = found
        .iter()
        .map(|(key, _, line)| (key.as_str(), *line))
        .collect();
    let wanted_keys: BTreeSet<&str> = wanted.iter().map(|(key, _, _)| key.as_str()).collect();
    for (key, entry, line) in &wanted {
        match found_by_key.get(key.as_str()) {
            None => output.missing.push(entry.clone()),
            Some(existing) if existing != line => output.changed.push(entry.clone()),
            Some(_) => {}
        }
    }
    output.extra = found
        .iter()
        .filter(|(key, _, _)| !wanted_keys.contains(key.as_str()))
        .map(|(_, entry, _)| entry.clone())
        .collect();
}

/// Render the glossary in memory and compare it with the file, byte for byte.
pub fn check_glossary_output(
    source: &str,
    path: Option<&str>,
) -> Result<GlossaryCheckOutput, Error> {
    let file = GlossaryFile::resolve(source, path)?;
    let rendered = render_glossary_file(&file.output);
    let mut output = GlossaryCheckOutput {
        path: file.display.clone(),
        status: GlossaryStatus::Current,
        write_command: file.write_command(),
        missing: Vec::new(),
        extra: Vec::new(),
        changed: Vec::new(),
    };
    let current = match std::fs::read_to_string(&file.path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            output.status = GlossaryStatus::Missing;
            return Ok(output);
        }
        Err(error) => {
            return Err(Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", file.display),
            ));
        }
    };
    if current != rendered {
        output.status = GlossaryStatus::Stale;
        compare_glossary(&mut output, &rendered, &current);
    }
    Ok(output)
}

pub fn render_glossary_check(output: &GlossaryCheckOutput) -> String {
    use std::fmt::Write as _;
    let mut text = match output.status {
        GlossaryStatus::Current => return format!("{} is current\n", output.path),
        GlossaryStatus::Missing => {
            return format!(
                "{} is missing: run `{}`\n",
                output.path, output.write_command
            );
        }
        GlossaryStatus::Stale => {
            format!("{} is stale: run `{}`\n", output.path, output.write_command)
        }
    };
    let rows = [
        ("missing", &output.missing),
        ("extra", &output.extra),
        ("changed", &output.changed),
    ];
    let mut listed = false;
    for (kind, entries) in rows {
        for entry in entries {
            listed = true;
            let _ = writeln!(text, "  {kind:<8} {}", entry.render());
        }
    }
    if !listed {
        text.push_str("  the header or the definitions differ\n");
    }
    text
}

pub fn check_glossary(source: &str, path: Option<&str>, json: bool) -> Result<i32, Error> {
    let output = check_glossary_output(source, path)?;
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_glossary_check(&output));
    }
    Ok(i32::from(!output.is_current()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TURTLE: &str = r#"@prefix rune: <https://runedeck.ai/ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .

# rdfs:label "Fake" in a comment line is not a term.
rune:Change a rdfs:Class ;
    rdfs:label "change" ;
    rdfs:comment "One spec-driven unit of work." .

rune:change a rdf:Property ;
    rdfs:label "change key" ;
    rdfs:comment "The change a record belongs to." .

rune:Falsifiability a skos:Concept ; skos:inScheme rune:principles ;
    rdfs:label "falsifiability" ;
    rdfs:comment "A requirement states a check that can fail; \"nothing can fail\" is not a requirement." .

rune:Unlabeled a rdfs:Class ;
    rdfs:comment "No label, so prose cannot name it." .

rune:Tight a rdfs:Class ; rdfs:label "tight" ; rdfs:comment "Ends without a space before the period.". # trailing comment
"#;

    fn terms() -> Terms {
        Terms::from_turtle(TURTLE, "ontology/rune.ttl").expect("valid ontology")
    }

    #[test]
    fn turtle_terms_carry_iri_label_kind_and_unescaped_comment() {
        let terms = terms();
        assert_eq!(terms.source(), TermSource::Ontology);
        assert!(terms.in_namespace("https://runedeck.ai/ns#Anything"));
        let labels: Vec<_> = terms
            .terms()
            .iter()
            .map(|term| term.label.as_str())
            .collect();
        assert_eq!(labels, ["change", "change key", "falsifiability", "tight"]);
        let concept = terms.resolve("Falsifiability").expect("skos concept");
        assert_eq!(concept.kind, TermKind::Concept);
        assert_eq!(concept.iri, "https://runedeck.ai/ns#Falsifiability");
        assert!(concept.comment.contains("\"nothing can fail\""));
        assert_eq!(
            terms.resolve("changes").map(|term| term.name.as_str()),
            Some("Change")
        );
        assert_eq!(
            terms.resolve("change key").map(|term| term.kind),
            Some(TermKind::Property)
        );
        assert_eq!(
            terms.resolve("tight").map(|term| term.comment.as_str()),
            Some("Ends without a space before the period.")
        );
        assert!(terms.resolve("unlabeled").is_none());
        assert!(terms.resolve("fake").is_none());
    }

    #[test]
    fn a_property_sharing_a_class_name_takes_the_key_suffix_by_kind() {
        let tags = terms().tags();
        assert_eq!(tags["https://runedeck.ai/ns#Change"], "CHANGE");
        assert_eq!(tags["https://runedeck.ai/ns#change"], "CHANGE-KEY");
        let lowercase_class = Terms::from_turtle(
            "@prefix ex: <https://example.org/> .\nex:thing a rdfs:Class ; rdfs:label \"thing\" .\nex:Thing a rdf:Property ; rdfs:label \"thing key\" .\nex:THING a rdf:Property ; rdfs:label \"thing ref\" .\n",
            "o.ttl",
        )
        .unwrap();
        let tags = lowercase_class.tags();
        assert_eq!(tags["https://example.org/thing"], "THING");
        assert_eq!(tags["https://example.org/Thing"], "THING-KEY");
        assert_eq!(tags["https://example.org/THING"], "THING-KEY-KEY");
    }

    #[test]
    fn an_undeclared_prefix_a_duplicate_label_and_an_empty_ontology_are_errors() {
        let undeclared = Terms::from_turtle("ex:A a rdfs:Class ; rdfs:label \"a\" .\n", "o.ttl");
        assert!(
            undeclared
                .unwrap_err()
                .message()
                .contains("prefix no @prefix declares")
        );
        let duplicate = Terms::from_turtle(
            "@prefix ex: <https://example.org/> .\nex:A a rdfs:Class ; rdfs:label \"Thing\" .\nex:B a rdfs:Class ; rdfs:label \"thing\" .\n",
            "o.ttl",
        );
        assert!(
            duplicate
                .unwrap_err()
                .message()
                .contains("share the label \"thing\"")
        );
        let shared = Terms::from_turtle(
            "@prefix ex: <https://example.org/> .\nex:change a rdf:Property ; rdfs:label \"change\" .\nex:Change a rdfs:Class ; rdfs:label \"Change\" .\n",
            "o.ttl",
        )
        .unwrap();
        assert_eq!(
            shared.resolve("changes").map(|term| term.iri.as_str()),
            Some("https://example.org/Change")
        );
        assert_eq!(shared.tags()["https://example.org/change"], "CHANGE-KEY");
        let empty = Terms::from_turtle("@prefix ex: <https://example.org/> .\n", "o.ttl");
        assert!(
            empty
                .unwrap_err()
                .message()
                .contains("no class, property, or concept")
        );
    }

    #[test]
    fn glossary_entries_keep_their_definitions_and_skip_fenced_examples() {
        let terms = Terms::from_glossary(
            "# Glossary\n\n- **Trailer**: a key-value footer line.\n- plain bullet\n\n```markdown\n- **example**: not an entry.\n```\n",
            "docs/specs/glossary.md".into(),
        );
        assert_eq!(terms.source(), TermSource::Glossary);
        let trailer = terms.resolve("trailers").expect("plural finds singular");
        assert_eq!(trailer.comment, "a key-value footer line.");
        assert!(trailer.iri.is_empty());
        assert!(terms.resolve("example").is_none());
    }

    #[test]
    fn rendered_glossary_lists_terms_then_definitions() {
        let terms = terms();
        let tags = terms.tags();
        let output = GlossaryOutput {
            source: terms.location().to_string(),
            terms: terms
                .terms()
                .iter()
                .map(|term| GlossaryTerm {
                    label: term.label.clone(),
                    comment: term.comment.clone(),
                    name: term.name.clone(),
                    iri: term.iri.clone(),
                    kind: term.kind,
                    tag: tags[&term.iri].clone(),
                })
                .collect(),
        };
        let text = render_glossary(&output);
        assert!(text.starts_with("- **change**: One spec-driven unit of work. [CHANGE]\n"));
        assert!(text.contains("- **change key**: The change a record belongs to. [CHANGE-KEY]\n"));
        assert!(text.contains("\n[CHANGE-KEY]: https://runedeck.ai/ns#change\n"));
        assert!(text.ends_with("[TIGHT]: https://runedeck.ai/ns#Tight\n"));
    }

    /// A repository root with the ontology and an empty `docs/specs`.
    fn ontology_repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("ontology")).unwrap();
        std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
        std::fs::write(root.path().join("ontology/rune.ttl"), TURTLE).unwrap();
        root
    }

    fn source(root: &tempfile::TempDir) -> &str {
        root.path().to_str().unwrap()
    }

    #[test]
    fn two_writes_from_one_ontology_give_the_same_bytes() {
        let root = ontology_repository();
        let glossary = root.path().join("docs/specs/glossary.md");

        let first = write_glossary_output(source(&root), None).unwrap();
        let first_bytes = std::fs::read(&glossary).unwrap();
        let second = write_glossary_output(source(&root), None).unwrap();
        let second_bytes = std::fs::read(&glossary).unwrap();

        assert_eq!(first.path, "docs/specs/glossary.md");
        assert_eq!(first.terms, 4);
        assert_eq!(second.terms, 4);
        assert_eq!(first_bytes, second_bytes);
        let text = String::from_utf8(first_bytes).unwrap();
        assert!(text.starts_with(
            "# Glossary\n\nGenerated from `ontology/rune.ttl` by `rune spec glossary --write`. Do not edit this file. Change the ontology.\n"
        ));
        assert!(text.contains("`[TERM]: https://runedeck.ai/ns#Term` at the end of the file.\n\n- **change**: One spec-driven unit of work. [CHANGE]\n"));
        assert!(text.ends_with("\n[TIGHT]: https://runedeck.ai/ns#Tight\n"));
    }

    #[test]
    fn a_written_glossary_checks_current_at_the_default_and_a_custom_path() {
        let root = ontology_repository();
        write_glossary_output(source(&root), None).unwrap();
        let check = check_glossary_output(source(&root), None).unwrap();
        assert_eq!(check.status, GlossaryStatus::Current);
        assert!(check.is_current());
        assert_eq!(
            render_glossary_check(&check),
            "docs/specs/glossary.md is current\n"
        );

        write_glossary_output(source(&root), Some("docs/terms.md")).unwrap();
        let custom = check_glossary_output(source(&root), Some("docs/terms.md")).unwrap();
        assert_eq!(custom.path, "docs/terms.md");
        assert!(custom.is_current());
        assert_eq!(
            custom.write_command,
            "rune spec glossary --write docs/terms.md"
        );
    }

    #[test]
    fn an_edited_term_line_is_named_as_drift() {
        let root = ontology_repository();
        let glossary = root.path().join("docs/specs/glossary.md");
        write_glossary_output(source(&root), None).unwrap();
        let edited = std::fs::read_to_string(&glossary)
            .unwrap()
            .replace(
                "- **change key**: The change a record belongs to. [CHANGE-KEY]",
                "- **change key**: Edited by hand. [CHANGE-KEY]",
            )
            .replace(
                "- **tight**: Ends without a space before the period. [TIGHT]\n",
                "",
            )
            + "- **stray**: Not in the ontology. [STRAY]\n";
        std::fs::write(&glossary, edited).unwrap();

        let check = check_glossary_output(source(&root), None).unwrap();
        assert_eq!(check.status, GlossaryStatus::Stale);
        assert_eq!(
            check.changed,
            [GlossaryEntry {
                label: "change key".into(),
                tag: "CHANGE-KEY".into()
            }]
        );
        assert_eq!(
            check.missing,
            [GlossaryEntry {
                label: "tight".into(),
                tag: "TIGHT".into()
            }]
        );
        assert_eq!(
            check.extra,
            [GlossaryEntry {
                label: "stray".into(),
                tag: "STRAY".into()
            }]
        );
        assert_eq!(
            render_glossary_check(&check),
            "docs/specs/glossary.md is stale: run `rune spec glossary --write`\n  missing  tight [TIGHT]\n  extra    stray [STRAY]\n  changed  change key [CHANGE-KEY]\n"
        );
    }

    #[test]
    fn a_header_edit_is_drift_without_a_term() {
        let root = ontology_repository();
        let glossary = root.path().join("docs/specs/glossary.md");
        write_glossary_output(source(&root), None).unwrap();
        let edited = std::fs::read_to_string(&glossary)
            .unwrap()
            .replace("# Glossary", "# Terms");
        std::fs::write(&glossary, edited).unwrap();

        let check = check_glossary_output(source(&root), None).unwrap();
        assert_eq!(check.status, GlossaryStatus::Stale);
        assert_eq!(check.missing, []);
        assert_eq!(check.extra, []);
        assert_eq!(check.changed, []);
        assert!(
            render_glossary_check(&check).ends_with("  the header or the definitions differ\n")
        );
    }

    #[test]
    fn a_missing_glossary_file_is_reported_with_its_path() {
        let root = ontology_repository();
        let check = check_glossary_output(source(&root), None).unwrap();
        assert_eq!(check.status, GlossaryStatus::Missing);
        assert!(!check.is_current());
        assert_eq!(
            render_glossary_check(&check),
            "docs/specs/glossary.md is missing: run `rune spec glossary --write`\n"
        );
    }

    #[test]
    fn a_repository_without_an_ontology_cannot_render_the_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
        std::fs::write(
            root.path().join("docs/specs/glossary.md"),
            "# Glossary\n\n- **scan**: a full read.\n",
        )
        .unwrap();
        let error = write_glossary_output(source(&root), None).unwrap_err();
        assert!(error.message().contains("no ontology at ontology/rune.ttl"));
        assert!(check_glossary_output(source(&root), None).is_err());
    }

    #[test]
    fn a_term_without_a_comment_keeps_one_space_before_its_tag() {
        let terms = Terms::from_turtle(
            "@prefix ex: <https://example.org/> .\nex:Bare a rdfs:Class ; rdfs:label \"bare\" .\n",
            "o.ttl",
        )
        .unwrap();
        let output = GlossaryOutput::from_terms(&terms);
        assert_eq!(
            render_glossary(&output),
            "- **bare**: [BARE]\n\n[BARE]: https://example.org/Bare\n"
        );
        assert!(render_glossary_file(&output).contains("`[TERM]: https://example.org/Term`"));
    }
}
