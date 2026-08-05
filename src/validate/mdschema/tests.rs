use super::heading::extract_headings;

/// The skills schema declares its H2 sections in shorthand. Reading only the
/// map form skipped them, so a `SKILL.md` with no `Instructions` passed the
/// built-in path.
#[test]
fn shorthand_heading_declarations_are_required_sections() {
    let schema = "structure:\n    - heading:\n          pattern: \"# .+\"\n      children:\n          - heading: \"## Instructions\"\n";

    let missing = super::check("# Title\n\n## Elsewhere\n", "SKILL.md", schema);
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert!(missing[0].message.contains("## Instructions"));

    let present = super::check("# Title\n\n## Instructions\n", "SKILL.md", schema);
    assert!(present.is_empty(), "{present:?}");
}

/// A shorthand heading is text, not a pattern: regex metacharacters in it must
/// not quietly widen what satisfies the section.
#[test]
fn shorthand_heading_declarations_match_literally() {
    let schema = "structure:\n    - heading: \"## C++ Notes\"\n";

    let diagnostics = super::check("# Title\n\n## C++ Notes\n", "doc.md", schema);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn extract_headings_finds_all_levels() {
    let body = "# Title\n\n## Section\n\n### Subsection\n";
    let headings = extract_headings(body);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[1].level, 2);
    assert_eq!(headings[2].level, 3);
}

#[test]
fn extract_headings_skips_backtick_and_tilde_code_fences() {
    let body = "# Real\n\n```\n# Hidden by backticks\n```\n\n~~~markdown\n## Hidden by tildes\n~~~\n\n## Also Real\n";
    let headings = extract_headings(body);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].text, "Real");
    assert_eq!(headings[1].text, "Also Real");
}

#[test]
fn outline_reports_file_line_after_frontmatter() {
    let content = "---\nname: line-test\n---\n\n# line-test\n";
    let headings = super::outline(content);

    assert_eq!(headings[0].line, 5);
    assert_eq!(headings[0].text, "line-test");
}

#[test]
fn extract_headings_empty_body() {
    let headings = extract_headings("");
    assert!(headings.is_empty());
}

#[test]
fn extract_headings_preserves_text() {
    let body = "## Context and Problem Statement\n";
    let headings = extract_headings(body);
    assert_eq!(headings[0].text, "Context and Problem Statement");
}

#[test]
fn check_invalid_schema_returns_error() {
    let diagnostics = super::check("# Doc\n", "test.md", "not: [valid: yaml");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("invalid mdschema"));
}

#[test]
fn check_empty_schema_produces_no_diagnostics() {
    let diagnostics = super::check("# Doc\n", "test.md", "{}");
    assert!(diagnostics.is_empty());
}

#[test]
fn check_optional_frontmatter_not_required() {
    let schema = "frontmatter:\n    fields:\n        - name: tags\n          type: array\n          optional: true\n";
    let content = "---\ntitle: test\n---\n# Doc\n";
    let diagnostics = super::check(content, "test.md", schema);
    assert!(diagnostics.is_empty());
}
