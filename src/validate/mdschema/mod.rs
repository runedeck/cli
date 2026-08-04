mod frontmatter;
mod heading;
mod structure;

pub use heading::{Heading, outline};

use super::{Diagnostic, Severity};

/// Check a markdown file against an `.mdschema` definition.
///
/// The schema is a YAML file with three optional sections:
///
/// ```yaml
/// frontmatter:
///     fields:
///         - name: status
///           type: string
///         - name: tags
///           type: array
///           optional: true
///
/// heading_rules:
///     no_skip_levels: true
///     max_depth: 3
///
/// structure:
///     - heading:
///           pattern: "^# .+"
///           regex: true
///       children:
///           - heading:
///                 pattern: "## Context and Problem Statement"
/// ```
///
/// This is a deliberate subset of the `.mdschema` vocabulary, not a complete
/// implementation of it. Checks performed:
/// - Required frontmatter fields exist (fields without `optional: true`)
/// - Heading levels don't skip (h1 -> h3 without h2)
/// - Heading depth doesn't exceed `max_depth`
/// - Required sections (headings matching patterns) are present
///
/// Declared in schemas and never read here, so a schema can promise these and
/// this function will not enforce them:
/// - `optional: true` sections, which are skipped before they are read, so
///   their ordering, their permitted children, and their presence go
///   unchecked. Depth and level continuity still apply to the headings inside
///   them, because those rules walk the file rather than the declared
///   structure.
/// - Section order among siblings
/// - Unexpected sections outside the declared vocabulary
/// - `unique_per_level`
/// - `count`
/// - `allow_additional`
/// - Frontmatter field types
///
/// Standalone `mdschema` enforces the full vocabulary. Callers that need it
/// must dispatch there and treat this function as the reduced fallback; see
/// `cli::validate::mdschema_tool`.
///
/// # Examples
///
/// ```
/// use rune::validate::mdschema;
///
/// let content = "---\nstatus: Draft\n---\n# Title\n\n## Section\n";
/// let schema = "frontmatter:\n    fields:\n        - name: status\n          type: string\nheading_rules:\n    no_skip_levels: true\n    max_depth: 3\n";
/// let diagnostics = mdschema::check(content, "doc.md", schema);
/// assert!(diagnostics.is_empty());
/// ```
pub fn check(file_content: &str, file_path: &str, schema_content: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let Ok(schema): Result<serde_yaml::Value, _> = serde_yaml::from_str(schema_content) else {
        diagnostics.push(Diagnostic {
            file: file_path.to_string(),
            line: None,
            severity: Severity::Error,
            message: "invalid mdschema".to_string(),
        });
        return diagnostics;
    };

    frontmatter::check(file_content, file_path, &schema, &mut diagnostics);
    heading::check(file_content, file_path, &schema, &mut diagnostics);
    structure::check(file_content, file_path, &schema, &mut diagnostics);

    diagnostics
}

#[cfg(test)]
mod tests;
