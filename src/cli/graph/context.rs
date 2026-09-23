//! The deck's JSON-LD context (#70): `ontology/context.jsonld` maps a
//! frontmatter key to a term, and the exporter emits one triple per mapped
//! key. The context owns the meaning of a key; the exporter owns nothing
//! but the record identity and the source path.
//!
//! A term is `{"@id": "prefix:name", "@type": "@id" | "xsd:date" |
//! "@vocab", "@container": "@set"}` or a bare `"prefix:name"` string. A
//! top-level key whose value is an IRI declares a prefix.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::Value;

use rune::error::Error;

use super::{escape, iri, iri_fragment, record_id};

/// How a value under a key becomes a Turtle object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An IRI: a record stem, a `capability#requirement` reference, or a URI.
    Id,
    /// A typed `xsd:date` literal.
    Date,
    /// A `rune:` term named by the value.
    Vocab,
    /// A string literal.
    Text,
}

/// One mapped frontmatter key.
#[derive(Clone, Debug)]
pub struct Term {
    pub prefix: String,
    pub local: String,
    pub kind: Kind,
    pub set: bool,
}

/// Prefix bases the exporter knows without a context. A context that
/// rebinds one of these to another base is refused, because the
/// exporter writes these prefixes itself.
pub const BUILTIN: &[(&str, &str)] = &[
    ("rune", super::NS),
    ("dcterms", super::DCTERMS),
    ("xsd", "http://www.w3.org/2001/XMLSchema#"),
];

/// The loaded context: prefixes and the keys they map.
#[derive(Debug, Default)]
pub struct Context {
    prefixes: BTreeMap<String, String>,
    terms: BTreeMap<String, Term>,
}

/// The prefixes a rendering used, so the header declares only those.
#[derive(Debug, Default)]
pub struct Used {
    prefixes: BTreeSet<String>,
}

impl Used {
    pub fn note(&mut self, prefix: &str) {
        self.prefixes.insert(prefix.to_string());
    }

    /// `@prefix` lines for every used prefix beyond the ones the exporter
    /// always writes, from the context or from the built-in bases, so a
    /// typed date is declared even without a context file.
    pub fn header(&self, context: Option<&Context>, always: &[&str]) -> String {
        let mut out = String::new();
        for prefix in &self.prefixes {
            if always.contains(&prefix.as_str()) {
                continue;
            }
            let base = context
                .and_then(|context| context.prefixes.get(prefix).map(String::as_str))
                .or_else(|| BUILTIN.iter().find(|(p, _)| p == prefix).map(|(_, b)| *b));
            if let Some(base) = base {
                let _ = writeln!(out, "@prefix {prefix}: <{base}> .");
            }
        }
        out
    }
}

impl Context {
    /// Load `<root>/ontology/context.jsonld`. `None` when the file is
    /// absent; an error when it is present and malformed.
    pub fn load(root: &Path) -> Result<Option<Self>, Error> {
        let path = root.join("ontology").join("context.jsonld");
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path).map_err(|error| Error::io(error.to_string()))?;
        let document: Value = serde_json::from_str(&text)
            .map_err(|error| Error::parse(format!("{}: {error}", path.display())))?;
        let Some(Value::Object(entries)) = document.get("@context") else {
            return Err(Error::parse(format!(
                "{}: no @context object",
                path.display()
            )));
        };
        let mut context = Self::default();
        for (key, value) in entries {
            if key.starts_with('@') {
                continue;
            }
            if let Value::String(base) = value
                && (base.contains("://") || base.starts_with("urn:"))
            {
                if base
                    .chars()
                    .any(|c| c.is_whitespace() || matches!(c, '<' | '>' | '"'))
                {
                    return Err(Error::parse(format!(
                        "{}: prefix {key} has an invalid base {base}",
                        path.display()
                    )));
                }
                if let Some((_, builtin)) = BUILTIN.iter().find(|(p, _)| *p == key)
                    && builtin != base
                {
                    return Err(Error::parse(format!(
                        "{}: prefix {key} is {builtin} and cannot be rebound to {base}",
                        path.display()
                    )));
                }
                context.prefixes.insert(key.clone(), base.clone());
                continue;
            }
            match Self::term(value) {
                Ok(Some(term)) => {
                    context.terms.insert(key.clone(), term);
                }
                Ok(None) => {}
                Err(reason) => {
                    return Err(Error::parse(format!(
                        "{}: key {key}: {reason}",
                        path.display()
                    )));
                }
            }
        }
        for term in context.terms.values() {
            if !context.prefixes.contains_key(&term.prefix) {
                return Err(Error::parse(format!(
                    "{}: prefix {} is not declared",
                    path.display(),
                    term.prefix
                )));
            }
        }
        Ok(Some(context))
    }

    /// The term for one context entry, or `None` for an entry that is not
    /// a key mapping.
    fn term(value: &Value) -> Result<Option<Term>, String> {
        let (id, kind, set) = match value {
            Value::String(id) => (id.as_str(), Kind::Text, false),
            Value::Object(fields) => {
                let id = match fields.get("@id") {
                    None => return Ok(None),
                    Some(Value::String(id)) => id.as_str(),
                    Some(other) => return Err(format!("@id is not a string: {other}")),
                };
                let kind = match fields.get("@type") {
                    None => Kind::Text,
                    Some(Value::String(kind)) => match kind.as_str() {
                        "@id" => Kind::Id,
                        "@vocab" => Kind::Vocab,
                        "xsd:date" => Kind::Date,
                        other => return Err(format!("unsupported @type {other}")),
                    },
                    Some(other) => return Err(format!("@type is not a string: {other}")),
                };
                let set = match fields.get("@container") {
                    None => false,
                    Some(Value::String(container)) if container == "@set" => true,
                    Some(other) => return Err(format!("unsupported @container {other}")),
                };
                (id, kind, set)
            }
            other => return Err(format!("not a term: {other}")),
        };
        let Some((prefix, local)) = id.split_once(':') else {
            return Err(format!("term {id} has no prefix"));
        };
        if prefix.is_empty()
            || local.is_empty()
            || !prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            || !local
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            || !local.chars().next().is_some_and(char::is_alphabetic)
        {
            return Err(format!("term {id} is not a prefixed name"));
        }
        Ok(Some(Term {
            prefix: prefix.to_string(),
            local: local.to_string(),
            kind,
            set,
        }))
    }

    /// The mapped keys, in key order.
    pub fn terms(&self) -> impl Iterator<Item = (&String, &Term)> {
        self.terms.iter()
    }

    /// Whether the context declares a prefix.
    pub fn declares(&self, prefix: &str) -> bool {
        self.prefixes.contains_key(prefix)
    }
}

/// A local name a Turtle prefixed name accepts: a letter first, then
/// letters, digits, `-`, `_`, or `.`.
fn is_local_name(value: &str) -> bool {
    value.chars().next().is_some_and(char::is_alphabetic)
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The Turtle object for one value under a term.
pub fn object(value: &str, term: &Term, context: &Context, used: &mut Used) -> String {
    match term.kind {
        Kind::Text => format!("\"{}\"", escape(value)),
        Kind::Date => {
            used.note("xsd");
            format!("\"{}\"^^xsd:date", escape(value))
        }
        Kind::Vocab => {
            if is_local_name(value) {
                used.note("rune");
                return format!("rune:{value}");
            }
            if let Some((prefix, local)) = value.split_once(':')
                && context.declares(prefix)
                && is_local_name(local)
            {
                used.note(prefix);
                return format!("{prefix}:{local}");
            }
            format!("\"{}\"", escape(value))
        }
        Kind::Id if term.prefix == "rune" && term.local == "change" => {
            if has_scheme(value) {
                reference(value)
            } else {
                super::iri_change(value)
            }
        }
        Kind::Id => reference(value),
    }
}

/// An IRI for an `@id` value: a URI stays a URI, a record stem becomes
/// the record's IRI, `capability#slug` becomes the requirement's IRI, and
/// anything else is minted under `/id/` as it is written.
pub fn reference(value: &str) -> String {
    if has_scheme(value) {
        // Percent-encode what an IRI reference never holds raw, and keep
        // the rest as written: one `#` is a fragment, a second is the
        // author's error and the parser reports it.
        let mut cleaned = String::with_capacity(value.len());
        for c in value.chars() {
            if c.is_control()
                || c.is_whitespace()
                || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`')
            {
                for byte in c.to_string().bytes() {
                    let _ = write!(cleaned, "%{byte:02X}");
                }
            } else {
                cleaned.push(c);
            }
        }
        return format!("<{cleaned}>");
    }
    if let Some(identifier) = record_id(&format!("{value}.md")) {
        return iri(&identifier);
    }
    if let Some((base, fragment)) = value.split_once('#') {
        return iri_fragment(base, fragment);
    }
    iri(value)
}

/// Whether a value starts with a URI scheme such as `https:` or
/// `obsidian:`. A record stem has spaces and a `-`, never a scheme.
pub fn has_scheme(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    first
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && rest.chars().next().is_some_and(|c| !c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::{Context, Kind, Term, Used, has_scheme, object, reference};

    fn term(prefix: &str, local: &str, kind: Kind) -> Term {
        Term {
            prefix: prefix.to_string(),
            local: local.to_string(),
            kind,
            set: false,
        }
    }

    #[test]
    fn scheme_detection_separates_uris_from_stems() {
        assert!(has_scheme("https://example.org/x"));
        assert!(has_scheme("obsidian://open?vault=Atlas&file=Canvas"));
        assert!(has_scheme("urn:runedeck:import:x"));
        assert!(has_scheme("HTTPS://Example.org/x"));
        assert!(!has_scheme("CORE-0010 Adopt Records"));
        assert!(!has_scheme("capability#slug"));
        assert!(!has_scheme("Title: with colon"));
    }

    #[test]
    fn references_take_their_shape_from_the_value() {
        assert_eq!(
            reference("CORE-0010 Adopt Architecture Decision Records"),
            "<https://runedeck.ai/id/CORE-0010>"
        );
        assert_eq!(
            reference("markdown-first-authoring#markdown-system-language"),
            "<https://runedeck.ai/id/markdown-first-authoring#markdown-system-language>"
        );
        assert_eq!(
            reference("obsidian://open?vault=Atlas&file=Library%2FCanvas"),
            "<obsidian://open?vault=Atlas&file=Library%2FCanvas>"
        );
        assert_eq!(reference("@N4M3Z"), "<https://runedeck.ai/id/%40N4M3Z>");
        assert_eq!(
            reference("https://e.org/a b<c>"),
            "<https://e.org/a%20b%3Cc%3E>"
        );
    }

    #[test]
    fn objects_follow_the_term_kind() {
        let mut used = Used::default();
        let context = Context::default();
        let vocab = term("rune", "status", Kind::Vocab);
        assert_eq!(
            object("accepted", &vocab, &context, &mut used),
            "rune:accepted"
        );
        assert_eq!(
            object(
                "2026-02-19",
                &term("schema", "dateCreated", Kind::Date),
                &context,
                &mut used
            ),
            "\"2026-02-19\"^^xsd:date"
        );
        assert_eq!(
            object(
                "a \"b\"",
                &term("dcterms", "title", Kind::Text),
                &context,
                &mut used
            ),
            "\"a \\\"b\\\"\""
        );
        assert_eq!(
            object("not a token", &vocab, &context, &mut used),
            "\"not a token\""
        );
        assert_eq!(object("-dash", &vocab, &context, &mut used), "\"-dash\"");
        assert_eq!(
            object(
                "my-change",
                &term("rune", "change", Kind::Id),
                &context,
                &mut used
            ),
            "<https://runedeck.ai/id/change/my-change>"
        );
    }
}
