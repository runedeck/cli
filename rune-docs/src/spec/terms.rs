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
/// Turtle comment allowed after it.
static TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?ms)^([A-Za-z_][\w-]*)?:([\w-]+)\s+(?:a|rdf:type)\s+(rdfs:Class|rdf:Property|owl:Class|owl:ObjectProperty|owl:DatatypeProperty|skos:Concept)\b(.*?)(?:\s|")\.[ \t]*(?:#[^\n]*)?$"#,
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
        terms
            .entries
            .sort_by(|left, right| left.label.cmp(&right.label));
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

pub fn glossary_output(repository: &Path, specifications: &Path) -> Result<GlossaryOutput, Error> {
    let terms = Terms::load(repository, specifications)?;
    let tags = terms.tags();
    Ok(GlossaryOutput {
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
    })
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
    let mut lines: Vec<String> = output
        .terms
        .iter()
        .map(|term| {
            let mut line = format!("- **{}**: {}", term.label, term.comment);
            if !term.tag.is_empty() {
                line.push_str(" [");
                line.push_str(&term.tag);
                line.push(']');
            }
            line
        })
        .collect();
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
}
