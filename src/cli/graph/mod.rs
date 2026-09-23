//! Export the deck artifact graph as Turtle (DECK-0010).
//!
//! The exporter walks decision records under `docs/decisions/`, rule
//! files under `runes/*/rules/`, changes under `docs/changes/`, and
//! capabilities under `docs/specs/`, mints one IRI per identifier under
//! `https://runedeck.ai/id/`, and prints Turtle on stdout. SHACL
//! validation consumes the output:
//!
//! ```sh
//! rune graph export | rudof shacl-validate -s ontology/shapes.ttl -
//! ```
//!
//! Every input file contributes one `rune:sourcePath` triple, so two
//! files that claim one identifier merge into one node with two source
//! paths, and the duplicate becomes a visible cardinality violation.
//!
//! The deck's `ontology/context.jsonld` says what each frontmatter key
//! means ([`context`]); the exporter emits one triple per mapped key and
//! drops the rest. Without the context file the output is the identity,
//! title, relation, and source path of each node, as before.

mod context;
mod lifecycle;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use rune::error::Error;
use rune::parse::{frontmatter_value, split_frontmatter};

use context::{Context, Used};

const NS: &str = "https://runedeck.ai/ns#";
const ID: &str = "https://runedeck.ai/id/";
const DCTERMS: &str = "http://purl.org/dc/terms/";
/// Keys the exporter emits itself, so the context does not emit them twice.
const OWN_KEYS: &[&str] = &["title", "related"];

/// Artifact graph actions.
#[derive(Subcommand)]
pub enum GraphAction {
    /// Emit the artifact graph as Turtle on stdout
    Export {
        /// Deck root to export from. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },
}

/// Run a graph action.
pub fn execute(action: &GraphAction) -> Result<i32, Error> {
    match action {
        GraphAction::Export { source } => {
            let turtle = render(Path::new(source))?;
            print!("{turtle}");
            Ok(0)
        }
    }
}

/// Render the artifact graph for a deck root.
fn render(root: &Path) -> Result<String, Error> {
    let context = Context::load(root)?;
    let mut body = String::new();
    let mut used = Used::default();
    render_decisions(root, context.as_ref(), &mut body, &mut used)?;
    render_rules(root, &mut body)?;
    lifecycle::render_changes(root, context.as_ref(), &mut body, &mut used)?;
    lifecycle::render_capabilities(root, &mut body)?;
    let mut out = String::new();
    let _ = writeln!(out, "@prefix rune:    <{NS}> .");
    let _ = writeln!(out, "@prefix dcterms: <{DCTERMS}> .");
    out.push_str(&used.header(context.as_ref(), &["rune", "dcterms"]));
    out.push('\n');
    out.push_str(&body);
    Ok(out)
}

/// Emit one `rune:DecisionRecord` per record file.
fn render_decisions(
    root: &Path,
    context: Option<&Context>,
    out: &mut String,
    used: &mut Used,
) -> Result<(), Error> {
    let dir = root.join("docs").join("decisions");
    if !dir.is_dir() {
        return Ok(());
    }
    for path in sorted_markdown(&dir)? {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(identifier) = record_id(name) else {
            continue;
        };
        let text = fs::read_to_string(&path).map_err(|error| Error::io(error.to_string()))?;
        let node = iri(&identifier);
        render_record(
            &node,
            Some(&identifier),
            &text,
            &relative(root, &path),
            &[],
            context,
            out,
            used,
        );
    }
    Ok(())
}

/// One decision record node: the identity triples the exporter owns,
/// the triples the context maps, any extra lines, then the source path.
#[allow(clippy::too_many_arguments)]
fn render_record(
    node: &str,
    identifier: Option<&str>,
    text: &str,
    source_path: &str,
    extra: &[String],
    context: Option<&Context>,
    out: &mut String,
    used: &mut Used,
) {
    let _ = writeln!(out, "{node} a rune:DecisionRecord ;");
    if let Some(identifier) = identifier {
        let _ = writeln!(out, "    dcterms:identifier \"{}\" ;", escape(identifier));
    }
    if let Some(title) = frontmatter_value(text, "title") {
        let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(&title));
    }
    for related in related_ids(text) {
        let _ = writeln!(out, "    dcterms:relation {} ;", iri(&related));
    }
    if let Some(context) = context {
        for (key, term) in context.terms() {
            if OWN_KEYS.contains(&key.as_str()) {
                continue;
            }
            let values = if term.set {
                list_values(text, key)
            } else {
                frontmatter_value(text, key).into_iter().collect()
            };
            for value in values {
                let object = context::object(&value, term, context, used);
                used.note(&term.prefix);
                let _ = writeln!(out, "    {}:{} {object} ;", term.prefix, term.local);
            }
        }
    }
    for line in extra {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "    rune:sourcePath \"{}\" .\n", escape(source_path));
}

/// Emit one `rune:Rule` per rule file, with its verdict node when the
/// frontmatter carries a `metadata.verdict` pointer.
fn render_rules(root: &Path, out: &mut String) -> Result<(), Error> {
    let runes = root.join("runes");
    if !runes.is_dir() {
        return Ok(());
    }
    for module in sorted_dirs(&runes)? {
        let rules_dir = module.join("rules");
        if !rules_dir.is_dir() {
            continue;
        }
        for path in sorted_markdown(&rules_dir)? {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let text = fs::read_to_string(&path).map_err(|error| Error::io(error.to_string()))?;
            let node = iri(stem);
            let _ = writeln!(out, "{node} a rune:Rule ;");
            let title = frontmatter_value(&text, "title").unwrap_or_else(|| stem.to_string());
            let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(&title));
            if let Some(verdict) = frontmatter_value(&text, "metadata.verdict") {
                let verdict_node = iri(&format!("{stem}.verdict"));
                let _ = writeln!(out, "    rune:verdict {verdict_node} ;");
                let _ = writeln!(
                    out,
                    "    rune:sourcePath \"{}\" .\n",
                    escape(&relative(root, &path))
                );
                let _ = writeln!(out, "{verdict_node} a rune:Verdict ;");
                let _ = writeln!(out, "    dcterms:identifier \"{}\" .\n", escape(&verdict));
            } else {
                let _ = writeln!(
                    out,
                    "    rune:sourcePath \"{}\" .\n",
                    escape(&relative(root, &path))
                );
            }
        }
    }
    Ok(())
}

/// Markdown files of a directory, sorted by name for stable output.
fn sorted_markdown(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .map_err(|error| Error::io(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    Ok(files)
}

/// Subdirectories of a directory, sorted by name, hidden ones skipped.
fn sorted_dirs(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut dirs: Vec<_> = fs::read_dir(dir)
        .map_err(|error| Error::io(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('.'))
        })
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// The record identifier prefix of a file name: `DECK-0005 Title.md`
/// yields `DECK-0005`.
fn record_id(name: &str) -> Option<String> {
    let prefix = name.split_whitespace().next()?;
    let prefix = prefix.strip_suffix(".md").unwrap_or(prefix);
    let (letters, digits) = prefix.split_once('-')?;
    if letters.is_empty() || digits.len() != 4 {
        return None;
    }
    if !letters.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(prefix.to_string())
}

/// Record identifiers named by the `related:` frontmatter list.
fn related_ids(text: &str) -> Vec<String> {
    list_values(text, "related")
        .iter()
        .filter_map(|entry| record_id(&format!("{entry}.md")).or_else(|| record_id(entry)))
        .collect()
}

/// The items of a frontmatter list key, each as written: an inline
/// `[a, b]` list, a block list, or one scalar. A scalar that contains a
/// comma stays one item; the YAML parser owns the split.
fn list_values(text: &str, key: &str) -> Vec<String> {
    let Some((yaml, _)) = split_frontmatter(text) else {
        return Vec::new();
    };
    let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str::<serde_yaml::Value>(yaml)
    else {
        return Vec::new();
    };
    match map.get(serde_yaml::Value::String(key.to_string())) {
        Some(serde_yaml::Value::Sequence(items)) => items.iter().filter_map(scalar).collect(),
        Some(value) => scalar(value).into_iter().collect(),
        None => Vec::new(),
    }
}

/// A YAML scalar as text; a nested mapping or sequence is not a value.
fn scalar(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(text) => Some(text.clone()),
        serde_yaml::Value::Number(number) => Some(number.to_string()),
        serde_yaml::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// A full IRI reference for an identifier under the `/id/` namespace.
fn iri(identifier: &str) -> String {
    format!("<{ID}{}>", encode(identifier))
}

/// A full IRI reference for a change under `/id/change/`, apart from the
/// records and capabilities that share the bare `/id/` space.
fn iri_change(identifier: &str) -> String {
    format!("<{ID}change/{}>", encode(identifier))
}

/// A full IRI reference for `identifier#fragment` under `/id/`. An IRI
/// has one fragment, so a scenario under its requirement is
/// `requirement-slug/scenario-slug`, and the `/` is kept as written.
fn iri_fragment(identifier: &str, fragment: &str) -> String {
    let fragment = fragment
        .split('/')
        .map(encode)
        .collect::<Vec<_>>()
        .join("/");
    format!("<{ID}{}#{fragment}>", encode(identifier))
}

/// Percent-encode everything outside the unreserved set.
fn encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

/// The path of a file relative to the deck root, with forward slashes.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Escape a Turtle string literal.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{escape, iri, iri_change, iri_fragment, list_values, record_id};

    #[test]
    fn record_id_accepts_the_record_prefix() {
        assert_eq!(
            record_id("DECK-0005 Artifact Lifecycle.md").as_deref(),
            Some("DECK-0005")
        );
        assert_eq!(record_id("DECK-0005.md").as_deref(), Some("DECK-0005"));
        assert_eq!(record_id("readme.md"), None);
        assert_eq!(record_id("DECK-05 Short.md"), None);
    }

    #[test]
    fn iri_percent_encodes_reserved_bytes() {
        assert_eq!(iri("DECK-0005"), "<https://runedeck.ai/id/DECK-0005>");
        assert_eq!(iri("a b"), "<https://runedeck.ai/id/a%20b>");
        assert_eq!(
            iri_change("enforce-record-shape"),
            "<https://runedeck.ai/id/change/enforce-record-shape>"
        );
        assert_eq!(
            iri_fragment("cap", "req/scene one"),
            "<https://runedeck.ai/id/cap#req/scene%20one>"
        );
    }

    #[test]
    fn escape_guards_literal_delimiters() {
        assert_eq!(escape("a \"b\" \\c"), "a \\\"b\\\" \\\\c");
    }

    #[test]
    fn list_values_keep_each_item_whole() {
        let inline = "---\nrelated: [\"A-0001 One, two\", \"B-0002 Three\"]\n---\n";
        assert_eq!(
            list_values(inline, "related"),
            vec!["A-0001 One, two", "B-0002 Three"]
        );
        let block = "---\nupstream:\n    - x#y\n    - \"https://e.org/a,b\"\n---\n";
        assert_eq!(
            list_values(block, "upstream"),
            vec!["x#y", "https://e.org/a,b"]
        );
        let scalar = "---\nchange: one-change\n---\n";
        assert_eq!(list_values(scalar, "change"), vec!["one-change"]);
        assert!(list_values("---\nchange: []\n---\n", "change").is_empty());
    }
}
