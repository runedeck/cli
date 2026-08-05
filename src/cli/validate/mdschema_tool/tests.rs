use super::*;

const SAMPLE_OUTPUT: &str = "\
/deck/runes/core/skills/Probe/SKILL.md
  ✗ 1:1 [frontmatter] Frontmatter field 'user-invocable' should be a boolean
  ⚠ 6:3 [structure] Required element \"## Workflows\" not found within \"Probe\"
  ℹ 6:3 [structure] Required element \"## Sources\" not found within \"Probe\"

✗ Found 3 violation(s) in 1 file(s)
";

#[test]
fn parses_severities_positions_and_skips_the_summary_line() {
    let diagnostics = parse_violations(SAMPLE_OUTPUT, "skills/Probe/SKILL.md");

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(diagnostics[0].severity, Severity::Error);
    assert_eq!(diagnostics[0].line, Some(1));
    assert_eq!(
        diagnostics[0].message,
        "[frontmatter] Frontmatter field 'user-invocable' should be a boolean"
    );
    assert_eq!(diagnostics[1].severity, Severity::Warning);
    assert_eq!(diagnostics[1].line, Some(6));
    assert_eq!(diagnostics[2].severity, Severity::Warning);
    assert!(
        diagnostics
            .iter()
            .all(|d| d.file == "skills/Probe/SKILL.md")
    );
}

#[test]
fn clean_output_yields_no_diagnostics() {
    let diagnostics = parse_violations("✓ No violations found\n", "doc.md");
    assert!(diagnostics.is_empty());
}
