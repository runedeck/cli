//! Export the deck artifact graph as Turtle (DECK-0010).
//!
//! The exporter walks decision records under `docs/decisions/` and rule
//! files under `runes/*/rules/`, mints one IRI per identifier under
//! `https://runedeck.dev/id/`, and prints Turtle on stdout. SHACL
//! validation consumes the output:
//!
//! ```sh
//! rune graph export | rudof shacl-validate -s ontology/shapes.ttl -
//! ```
//!
//! Every input file contributes one `rune:sourcePath` triple, so two
//! files that claim one identifier merge into one node with two source
//! paths, and the duplicate becomes a visible cardinality violation.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use clap::Subcommand;

use rune::error::Error;
use rune::parse::{frontmatter_list, frontmatter_value};

const NS: &str = "https://runedeck.dev/ns#";
const ID: &str = "https://runedeck.dev/id/";
const DCTERMS: &str = "http://purl.org/dc/terms/";

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
    let mut out = String::new();
    let _ = writeln!(out, "@prefix rune:    <{NS}> .");
    let _ = writeln!(out, "@prefix dcterms: <{DCTERMS}> .");
    out.push('\n');
    render_decisions(root, &mut out)?;
    render_rules(root, &mut out)?;
    Ok(out)
}

/// Emit one `rune:DecisionRecord` per record file.
fn render_decisions(root: &Path, out: &mut String) -> Result<(), Error> {
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
        let content = fs::read_to_string(&path).map_err(|error| Error::io(error.to_string()))?;
        let node = iri(&identifier);
        let _ = writeln!(out, "{node} a rune:DecisionRecord ;");
        let _ = writeln!(out, "    dcterms:identifier \"{}\" ;", escape(&identifier));
        if let Some(title) = frontmatter_value(&content, "title") {
            let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(&title));
        }
        for related in related_ids(&content) {
            let _ = writeln!(out, "    dcterms:relation {} ;", iri(&related));
        }
        let _ = writeln!(
            out,
            "    rune:sourcePath \"{}\" .\n",
            escape(&relative(root, &path))
        );
    }
    Ok(())
}

/// Emit one `rune:Rule` per rule file, with its verdict node when the
/// frontmatter carries a `metadata.verdict` pointer.
fn render_rules(root: &Path, out: &mut String) -> Result<(), Error> {
    let runes = root.join("runes");
    if !runes.is_dir() {
        return Ok(());
    }
    let mut modules: Vec<_> = fs::read_dir(&runes)
        .map_err(|error| Error::io(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    modules.sort();
    for module in modules {
        let rules_dir = module.join("rules");
        if !rules_dir.is_dir() {
            continue;
        }
        for path in sorted_markdown(&rules_dir)? {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let content =
                fs::read_to_string(&path).map_err(|error| Error::io(error.to_string()))?;
            let node = iri(stem);
            let _ = writeln!(out, "{node} a rune:Rule ;");
            let title = frontmatter_value(&content, "title").unwrap_or_else(|| stem.to_string());
            let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(&title));
            if let Some(verdict) = frontmatter_value(&content, "metadata.verdict") {
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
fn sorted_markdown(dir: &Path) -> Result<Vec<std::path::PathBuf>, Error> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .map_err(|error| Error::io(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    Ok(files)
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
fn related_ids(content: &str) -> Vec<String> {
    let Some(joined) = frontmatter_list(content, "related") else {
        return Vec::new();
    };
    joined
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim().trim_matches('"');
            record_id(&format!("{entry}.md")).or_else(|| record_id(entry))
        })
        .collect()
}

/// A full IRI reference for an identifier under the `/id/` namespace.
fn iri(identifier: &str) -> String {
    let mut encoded = String::new();
    for byte in identifier.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    format!("<{ID}{encoded}>")
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
    use super::{escape, iri, record_id};

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
        assert_eq!(iri("DECK-0005"), "<https://runedeck.dev/id/DECK-0005>");
        assert_eq!(iri("a b"), "<https://runedeck.dev/id/a%20b>");
    }

    #[test]
    fn escape_guards_literal_delimiters() {
        assert_eq!(escape("a \"b\" \\c"), "a \\\"b\\\" \\\\c");
    }
}
