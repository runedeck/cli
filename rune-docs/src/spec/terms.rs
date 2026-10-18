//! The vocabulary a repository's prose may link: every term the ontology
//! defines. A document marks a term as a reference-style link to the
//! term's generated glossary page, `*[text][TAG]*` at first use and
//! `[text][TAG]` after that, with `[TAG]: <relative page path>` once at
//! the end of the file. Validation proves every link resolves.
//!
//! The Turtle reader is a statement-shape parser, not an RDF parser. It
//! reads the shape the deck writes: `prefix:Name a <type> ; predicates .`
//! with the terminating period at the end of a line. Anything it cannot
//! read is an error, never a silently missing term.

use super::{Error, ErrorKind, relative_display};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

/// The generated pages sit in this directory beside the capability
/// directories: one page per term and an `index.md`.
pub const GLOSSARY_DIR: &str = "glossary";
/// The single glossary file an older render wrote beside the capability
/// directories. `rune spec glossary --write` removes it.
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
/// The status the fixer writes on a term nobody has defined yet.
static DRAFT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:[A-Za-z_][\w-]*)?:status\s+"draft""#).expect("draft regex is valid")
});
static TURTLE_COMMENT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*#[^\n]*$").expect("comment line regex is valid"));

/// What kind of resource a term is. Classes take the plain reference tag
/// and page name when a property shares their label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TermKind {
    Class,
    Property,
    Concept,
}

impl TermKind {
    /// The word the glossary page's `kind:` field carries.
    pub fn name(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Property => "property",
            Self::Concept => "concept",
        }
    }
}

/// One defined term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Term {
    pub name: String,
    pub iri: String,
    pub label: String,
    pub comment: String,
    pub kind: TermKind,
    /// The fixer added this term from a link and nobody has defined it
    /// yet. The validator warns on it and archive refuses it.
    pub draft: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TermSource {
    /// `ontology/rune.ttl` or the file `deck.yaml` names.
    Ontology,
    /// No ontology file exists: the repository defines no terms, and a
    /// reference link is an ordinary link.
    #[default]
    Absent,
}

#[derive(Debug, Default)]
pub struct Terms {
    source: TermSource,
    /// Repository-relative path of the ontology, for diagnostics and the
    /// generated pages.
    location: String,
    /// The directory the glossary pages live in.
    glossary: PathBuf,
    entries: Vec<Term>,
    by_iri: BTreeMap<String, usize>,
    /// Every IRI prefix a term was declared under.
    namespaces: BTreeSet<String>,
    /// Turtle prefix by namespace IRI, for the fixer's draft block.
    prefixes: BTreeMap<String, String>,
    /// Reference tag per entry: the label in upper case, letters only,
    /// with `-KEY` suffixes where a property collides with a class.
    tags: Vec<String>,
    /// The tag an older render gave each entry: the local name in upper
    /// case. Accepted when a page is absent, so existing files resolve.
    name_tags: Vec<String>,
    /// Page file name per entry, `kebab(label).md` with `-key` suffixes
    /// that mirror the tag.
    pages: Vec<String>,
    by_tag: BTreeMap<String, usize>,
}

impl Terms {
    /// The ontology when the repository has one, else an absent source.
    /// Only a missing ontology file is absent; any other failure to read
    /// or parse it is an error.
    pub fn load(repository: &Path, specifications: &Path) -> Result<Self, Error> {
        let ontology = ontology_path(repository)?;
        let location = relative_display(repository, &ontology);
        let glossary = specifications.join(GLOSSARY_DIR);
        match std::fs::read_to_string(&ontology) {
            Ok(text) => {
                let mut terms = Self::from_turtle(&text, &location)?;
                terms.glossary = glossary;
                Ok(terms)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                location,
                glossary,
                ..Self::default()
            }),
            Err(error) => Err(Error::new(
                ErrorKind::Io,
                format!("cannot read ontology {location}: {error}"),
            )),
        }
    }

    /// Read every labeled class, property, and concept from Turtle text.
    /// An undeclared prefix, two terms of one kind sharing a label
    /// (compared without case), or an ontology with no labeled term is an
    /// error.
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
        let mut by_label: BTreeMap<(String, TermKind), String> = BTreeMap::new();
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
            terms
                .prefixes
                .entry((*namespace).to_string())
                .or_insert_with(|| prefix.to_string());
            let term = Term {
                name: name.to_string(),
                iri: format!("{namespace}{name}"),
                label: unescape(&label[1]),
                comment: COMMENT
                    .captures(&capture[4])
                    .map(|comment| unescape(&comment[1]))
                    .unwrap_or_default(),
                kind,
                draft: DRAFT.is_match(&capture[4]),
            };
            if let Some(other) = by_label.insert((normalize(&term.label), kind), term.iri.clone()) {
                return Err(Error::new(
                    ErrorKind::Parse,
                    format!(
                        "{location}: {other} and {} share the label \"{}\"; labels are compared without case",
                        term.iri, term.label
                    ),
                ));
            }
            terms.entries.push(term);
        }
        if terms.entries.is_empty() {
            return Err(Error::new(
                ErrorKind::Parse,
                format!("{location}: no class, property, or concept carries an rdfs:label"),
            ));
        }
        // By label, then the class before the property that shares it, then
        // by name, so the order does not depend on the order of the file.
        terms.entries.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| precedence(left.kind).cmp(&precedence(right.kind)))
                .then_with(|| left.name.cmp(&right.name))
        });
        terms.index();
        Ok(terms)
    }

    /// Rebuild the lookups, tags, and page names after sorting.
    fn index(&mut self) {
        self.by_iri = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, term)| (term.iri.clone(), index))
            .collect();
        self.tags = unique_tags(&self.entries, |term| tag_base(&term.label));
        self.name_tags = unique_tags(&self.entries, |term| term.name.to_uppercase());
        self.pages = unique_tags(&self.entries, |term| kebab(&term.label))
            .into_iter()
            .map(|page| format!("{}.md", page.replace("-KEY", "-key")))
            .collect();
        self.by_tag.clear();
        for (index, tag) in self.name_tags.iter().enumerate() {
            self.by_tag.insert(tag.clone(), index);
        }
        for (index, tag) in self.tags.iter().enumerate() {
            self.by_tag.insert(tag.clone(), index);
        }
    }

    pub fn source(&self) -> TermSource {
        self.source
    }

    pub fn location(&self) -> &str {
        &self.location
    }

    /// The directory the glossary pages live in.
    pub fn glossary_dir(&self) -> &Path {
        &self.glossary
    }

    /// Whether an IRI sits under a namespace the ontology declares terms in.
    pub fn in_namespace(&self, iri: &str) -> bool {
        self.namespaces
            .iter()
            .any(|namespace| iri.starts_with(namespace))
    }

    /// The Turtle prefix and namespace the fixer writes a draft under: the
    /// namespace of the first term.
    pub fn primary_prefix(&self) -> Option<(&str, &str)> {
        let namespace = self.entries.first().and_then(|term| {
            self.namespaces
                .iter()
                .find(|namespace| term.iri.starts_with(namespace.as_str()))
        })?;
        Some((self.prefixes.get(namespace)?.as_str(), namespace.as_str()))
    }

    /// Every term, sorted by label.
    pub fn terms(&self) -> &[Term] {
        &self.entries
    }

    pub fn by_iri(&self, iri: &str) -> Option<&Term> {
        self.by_iri.get(iri).map(|index| &self.entries[*index])
    }

    /// The term a reference tag names, compared without case: the label
    /// tag first, then the local-name tag an older render used.
    pub fn by_tag(&self, tag: &str) -> Option<&Term> {
        self.by_tag
            .get(&tag.to_uppercase())
            .map(|index| &self.entries[*index])
    }

    /// Whether a local name is taken, compared without case.
    pub fn has_name(&self, name: &str) -> bool {
        self.entries
            .iter()
            .any(|term| term.name.eq_ignore_ascii_case(name))
    }

    /// The reference tag of a term: its label in upper case with every
    /// non-letter removed, `-KEY` suffixed while another term holds it.
    pub fn tag(&self, term: &Term) -> &str {
        self.by_iri
            .get(&term.iri)
            .map_or("", |index| self.tags[*index].as_str())
    }

    /// The file name of a term's glossary page.
    pub fn page_name(&self, term: &Term) -> &str {
        self.by_iri
            .get(&term.iri)
            .map_or("", |index| self.pages[*index].as_str())
    }

    /// The path of a term's glossary page.
    pub fn page_path(&self, term: &Term) -> PathBuf {
        self.glossary.join(self.page_name(term))
    }

    /// The Turtle block the fixer appends for a draft term.
    pub fn draft_block(&self, name: &str, label: &str, origin: &str) -> Option<String> {
        let (prefix, _) = self.primary_prefix()?;
        Some(format!(
            "\n# Draft term. The fixer added it from {origin}; define it or remove the link.\n\
             {prefix}:{name} a rdfs:Class ;\n    rdfs:label \"{}\" ;\n    {prefix}:status \"draft\" .\n",
            escape(label)
        ))
    }
}

/// Which kind owns a label two terms share: the class, then the concept,
/// then the property.
fn precedence(kind: TermKind) -> u8 {
    match kind {
        TermKind::Class => 0,
        TermKind::Concept => 1,
        TermKind::Property => 2,
    }
}

/// One tag per entry, in entry order. Markdown reference labels are
/// case-insensitive, so when two bases collide the class keeps the plain
/// tag and every later term takes `-KEY` suffixes until its tag is unique.
fn unique_tags(entries: &[Term], base: impl Fn(&Term) -> String) -> Vec<String> {
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|left, right| {
        (precedence(entries[*left].kind), &entries[*left].label)
            .cmp(&(precedence(entries[*right].kind), &entries[*right].label))
    });
    let mut tags = vec![String::new(); entries.len()];
    let mut taken = BTreeSet::new();
    for index in order {
        let term = &entries[index];
        let mut tag = base(term);
        if tag.is_empty() {
            tag = term.name.to_uppercase();
        }
        while !taken.insert(tag.to_uppercase()) {
            tag.push_str("-KEY");
        }
        tags[index] = tag;
    }
    tags
}

/// The label in upper case with every non-letter removed:
/// `analysis tool` is `ANALYSISTOOL`, `Occam's razor` is `OCCAMSRAZOR`.
fn tag_base(label: &str) -> String {
    label
        .chars()
        .filter(char::is_ascii_alphabetic)
        .flat_map(char::to_uppercase)
        .collect()
}

/// The page file stem of a label: lower case, apostrophes dropped, every
/// run outside `a-z0-9` one hyphen, hyphens trimmed at both ends.
pub fn kebab(label: &str) -> String {
    let mut result = String::with_capacity(label.len());
    for character in label.to_lowercase().chars() {
        if matches!(character, '\'' | '\u{2019}') {
            continue;
        }
        if character.is_ascii_alphanumeric() {
            result.push(character);
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    result.trim_matches('-').to_string()
}

/// A local name for a draft term: the link text in `UpperCamelCase`, with
/// apostrophes dropped (`Occam's razor` is `OccamsRazor`).
pub fn upper_camel(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut start = true;
    for character in text.chars() {
        if matches!(character, '\'' | '\u{2019}') {
            continue;
        }
        if character.is_alphanumeric() {
            if start {
                result.extend(character.to_uppercase());
                start = false;
            } else {
                result.push(character);
            }
        } else {
            start = true;
        }
    }
    result
}

fn normalize(term: &str) -> String {
    term.trim().to_lowercase()
}

/// Undo the Turtle string escapes the deck uses: `\"` and `\\`.
fn unescape(literal: &str) -> String {
    literal.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// Apply the Turtle string escapes the deck uses.
fn escape(literal: &str) -> String {
    literal.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::spec) const TURTLE: &str = r#"@prefix rune: <https://runedeck.ai/ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .

# rdfs:label "Fake" in a comment line is not a term.
rune:Change a rdfs:Class ;
    rdfs:label "change" ;
    rdfs:comment "One spec-driven unit of work." .

rune:change a rdf:Property ;
    rdfs:label "change" ;
    rdfs:comment "The change a record belongs to." .

rune:Falsifiability a skos:Concept ; skos:inScheme rune:principles ;
    rdfs:label "falsifiability" ;
    rdfs:comment "A requirement states a check that can fail; \"nothing can fail\" is not a requirement." .

rune:Unlabeled a rdfs:Class ;
    rdfs:comment "No label, so prose cannot name it." .

rune:OccamsRazor a skos:Concept ;
    rdfs:label "Occam's razor" ;
    rdfs:comment "Of two instructions that pass the same check, the shorter wins." .

rune:Stage a rdfs:Class ;
    rdfs:label "lifecycle stage" ;
    rdfs:comment "Where a change sits." .

rune:Widget a rdfs:Class ;
    rdfs:label "widget" ;
    rune:status "draft" .

rune:Tight a rdfs:Class ; rdfs:label "tight" ; rdfs:comment "Ends without a space before the period.". # trailing comment
"#;

    fn terms() -> Terms {
        Terms::from_turtle(TURTLE, "ontology/rune.ttl").expect("valid ontology")
    }

    #[test]
    fn turtle_terms_carry_iri_label_kind_draft_and_unescaped_comment() {
        let terms = terms();
        assert_eq!(terms.source(), TermSource::Ontology);
        assert!(terms.in_namespace("https://runedeck.ai/ns#Anything"));
        assert_eq!(
            terms.primary_prefix(),
            Some(("rune", "https://runedeck.ai/ns#"))
        );
        let labels: Vec<_> = terms
            .terms()
            .iter()
            .map(|term| term.label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "Occam's razor",
                "change",
                "change",
                "falsifiability",
                "lifecycle stage",
                "tight",
                "widget"
            ]
        );
        let concept = terms
            .by_iri("https://runedeck.ai/ns#Falsifiability")
            .expect("skos concept");
        assert_eq!(concept.kind, TermKind::Concept);
        assert!(concept.comment.contains("\"nothing can fail\""));
        assert!(!concept.draft);
        let widget = terms.by_tag("widget").expect("draft term");
        assert!(widget.draft);
        assert_eq!(widget.comment, "");
        assert_eq!(
            terms.by_tag("TIGHT").map(|term| term.comment.as_str()),
            Some("Ends without a space before the period.")
        );
        assert!(terms.by_tag("unlabeled").is_none());
        assert!(terms.by_tag("fake").is_none());
        assert!(terms.has_name("stage"));
    }

    #[test]
    fn tags_follow_the_label_and_a_colliding_property_takes_the_key_suffix() {
        let terms = terms();
        let class = terms.by_iri("https://runedeck.ai/ns#Change").unwrap();
        let property = terms.by_iri("https://runedeck.ai/ns#change").unwrap();
        assert_eq!(terms.tag(class), "CHANGE");
        assert_eq!(terms.tag(property), "CHANGE-KEY");
        assert_eq!(terms.page_name(class), "change.md");
        assert_eq!(terms.page_name(property), "change-key.md");
        let razor = terms.by_iri("https://runedeck.ai/ns#OccamsRazor").unwrap();
        assert_eq!(terms.tag(razor), "OCCAMSRAZOR");
        assert_eq!(terms.page_name(razor), "occams-razor.md");
        let stage = terms.by_iri("https://runedeck.ai/ns#Stage").unwrap();
        assert_eq!(terms.tag(stage), "LIFECYCLESTAGE");
        assert_eq!(terms.page_name(stage), "lifecycle-stage.md");
        // The older name-based tag still resolves, so existing files keep working.
        assert_eq!(
            terms.by_tag("STAGE").map(|t| t.name.as_str()),
            Some("Stage")
        );
        assert_eq!(
            terms.by_tag("change-key").map(|t| t.kind),
            Some(TermKind::Property)
        );
        assert_eq!(
            terms.by_tag("Change").map(|t| t.kind),
            Some(TermKind::Class)
        );
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
        let empty = Terms::from_turtle("@prefix ex: <https://example.org/> .\n", "o.ttl");
        assert!(
            empty
                .unwrap_err()
                .message()
                .contains("no class, property, or concept")
        );
    }

    #[test]
    fn kebab_and_upper_camel_follow_the_page_and_draft_rules() {
        assert_eq!(kebab("Occam's razor"), "occams-razor");
        assert_eq!(kebab("analysis tool"), "analysis-tool");
        assert_eq!(kebab("  UTF-8 / encoding!"), "utf-8-encoding");
        assert_eq!(upper_camel("analysis tool"), "AnalysisTool");
        assert_eq!(upper_camel("Occam's razor"), "OccamsRazor");
        assert_eq!(upper_camel("UTF-8 encoding"), "UTF8Encoding");
    }

    #[test]
    fn a_draft_block_names_the_prefix_the_label_and_the_origin() {
        let block = terms()
            .draft_block("Gadget", "gad\"get", "docs/x.md:3")
            .unwrap();
        assert_eq!(
            block,
            "\n# Draft term. The fixer added it from docs/x.md:3; define it or remove the link.\nrune:Gadget a rdfs:Class ;\n    rdfs:label \"gad\\\"get\" ;\n    rune:status \"draft\" .\n"
        );
        let reread = Terms::from_turtle(&format!("{TURTLE}{block}"), "o.ttl").unwrap();
        let gadget = reread.by_tag("GADGET").unwrap();
        assert!(gadget.draft);
        assert_eq!(gadget.label, "gad\"get");
    }
}
