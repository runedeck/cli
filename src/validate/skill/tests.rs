//! Stable shell rules that Rune owns in both checking paths.

use super::*;
use crate::validate::mdschema::outline;

fn headings_of(content: &str) -> Vec<Heading> {
    outline(content)
}

#[test]
fn identity_passes_when_all_three_agree() {
    let content = "---\nname: demo-skill\n---\n# demo-skill\n";

    assert!(
        check_identity(
            "demo-skill",
            "demo-skill",
            &headings_of(content),
            "skills/demo-skill/SKILL.md"
        )
        .is_none()
    );
}

#[test]
fn identity_names_all_three_values_when_the_heading_differs() {
    let content = "---\nname: demo-skill\n---\n# wrong-heading\n";
    let diagnostic = check_identity(
        "demo-skill",
        "demo-skill",
        &headings_of(content),
        "skills/demo-skill/SKILL.md",
    )
    .expect("a differing H1 must report");

    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(diagnostic.message.contains("frontmatter name 'demo-skill'"));
    assert!(diagnostic.message.contains("H1 'wrong-heading'"));
    assert!(diagnostic.message.contains("directory 'demo-skill'"));
}

#[test]
fn identity_names_the_directory_when_only_it_differs() {
    let content = "---\nname: demo-skill\n---\n# demo-skill\n";
    let diagnostic = check_identity(
        "demo-skill",
        "somewhere-else",
        &headings_of(content),
        "skills/somewhere-else/SKILL.md",
    )
    .expect("a differing directory must report");

    assert!(diagnostic.message.contains("directory 'somewhere-else'"));
}

#[test]
fn identity_stays_silent_without_exactly_one_top_level_heading() {
    let content = "---\nname: demo-skill\n---\n# demo-skill\n\n# second-heading\n";

    assert!(
        check_identity(
            "demo-skill",
            "demo-skill",
            &headings_of(content),
            "skills/demo-skill/SKILL.md"
        )
        .is_none(),
        "the schema reports duplicate H1s; identity must not pile on"
    );
}

#[test]
fn breadth_stays_silent_at_the_threshold() {
    let content =
        "# demo-skill\n\n## Instructions\n\n### One\n\n### Two\n\n### Three\n\n### Four\n";

    assert!(check_instruction_breadth(&headings_of(content), "SKILL.md").is_none());
}

#[test]
fn breadth_warns_past_the_threshold() {
    let content = "# demo-skill\n\n## Instructions\n\n### One\n\n### Two\n\n### Three\n\n### Four\n\n### Five\n";
    let diagnostic = check_instruction_breadth(&headings_of(content), "SKILL.md")
        .expect("a fifth subsection must warn");

    assert_eq!(diagnostic.severity, Severity::Warning);
    assert!(
        diagnostic
            .message
            .contains("more than 4 direct H3 headings")
    );
}

#[test]
fn breadth_counts_only_subsections_of_instructions() {
    let content = "# demo-skill\n\n## Instructions\n\n### One\n\n## Troubleshooting\n\n### Two\n\n### Three\n\n### Four\n\n### Five\n";

    assert!(
        check_instruction_breadth(&headings_of(content), "SKILL.md").is_none(),
        "subsections under a later H2 belong to that H2"
    );
}

#[test]
fn fenced_headings_count_for_neither_rule() {
    let content = "---\nname: demo-skill\n---\n# demo-skill\n\n## Instructions\n\n```markdown\n# wrong-heading\n### One\n### Two\n### Three\n### Four\n### Five\n```\n";
    let headings = headings_of(content);

    assert!(check_identity("demo-skill", "demo-skill", &headings, "SKILL.md").is_none());
    assert!(check_instruction_breadth(&headings, "SKILL.md").is_none());
}

#[test]
fn tables_report_once_per_block_with_a_line_number() {
    let content = "# demo-skill\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
    let diagnostics = check_no_tables(content, "SKILL.md");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, Some(3));
    assert_eq!(diagnostics[0].severity, Severity::Error);
    assert!(diagnostics[0].message.contains("markdown table"));
}

#[test]
fn tables_inside_fences_are_content_not_structure() {
    let content = "# demo-skill\n\n```markdown\n| a | b |\n| - | - |\n```\n";

    assert!(check_no_tables(content, "SKILL.md").is_empty());
}

#[test]
fn separate_tables_report_separately() {
    let content = "# demo-skill\n\n| a | b |\n| - | - |\n\nprose\n\n| c | d |\n| - | - |\n";
    let diagnostics = check_no_tables(content, "SKILL.md");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, Some(3));
    assert_eq!(diagnostics[1].line, Some(8));
}

#[test]
fn check_collects_both_owned_rules() {
    let content = "---\nname: demo-skill\n---\n# wrong-heading\n\n## Instructions\n\n### One\n\n### Two\n\n### Three\n\n### Four\n\n### Five\n";
    let diagnostics = check("demo-skill", "demo-skill", content, "SKILL.md");

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.severity == Severity::Error));
    assert!(diagnostics.iter().any(|d| d.severity == Severity::Warning));
}
