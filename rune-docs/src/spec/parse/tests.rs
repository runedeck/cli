use super::*;

// Direct parser edge coverage for the branches the OpenSpec v1.6.0 oracle
// fixtures do not reach; the golden cases in `spec::openspec_oracle` stay
// the compatibility authority.

#[test]
fn removed_requirements_accept_direct_and_bulleted_forms() {
    let operations = parse_delta(
        "## REMOVED Requirements\n\n### Requirement: Direct form\n\n- `### Requirement: Bulleted form`\n",
    )
    .unwrap();

    assert_eq!(operations.len(), 2);
    let DeltaOperation::Removed { name, .. } = &operations[0] else {
        panic!("expected a removal, got {:?}", operations[0]);
    };
    assert_eq!(name, "Direct form");
    let DeltaOperation::Removed { name, .. } = &operations[1] else {
        panic!("expected a removal, got {:?}", operations[1]);
    };
    assert_eq!(name, "Bulleted form");
}

#[test]
fn renamed_references_accept_bullets_and_backticks() {
    let operations = parse_delta(
        "## RENAMED Requirements\n\n- FROM: `### Requirement: Old name`\n- TO: `### Requirement: New name`\n",
    )
    .unwrap();

    assert_eq!(operations.len(), 1);
    let DeltaOperation::Renamed { from, to, .. } = &operations[0] else {
        panic!("expected a rename, got {:?}", operations[0]);
    };
    assert_eq!(from, "Old name");
    assert_eq!(to, "New name");
}

#[test]
fn delta_section_titles_match_case_insensitively() {
    let operations = parse_delta(
        "## added Requirements\n\n### Requirement: Case test\n\nThe tool SHALL match sections case-insensitively.\n\n#### Scenario: Matches\n\n- WHEN a section is lowercase\n- THEN it still parses\n",
    )
    .unwrap();

    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].kind(), DeltaKind::Added);
}

#[test]
fn duplicate_delta_entries_are_rejected() {
    let issues = parse_delta(
        "## ADDED Requirements\n\n### Requirement: Twice\n\nThe tool SHALL exist.\n\n### Requirement: Twice\n\nThe tool SHALL exist.\n",
    )
    .unwrap_err();

    assert!(
        issues.iter().any(|issue| issue.message.contains("Twice")),
        "no issue names the duplicate: {issues:?}"
    );
}

#[test]
fn rename_collisions_across_sections_are_rejected() {
    let issues = parse_delta(
        "## RENAMED Requirements\n\n- FROM: `### Requirement: Old`\n- TO: `### Requirement: New`\n\n## REMOVED Requirements\n\nRequirement: Old\n",
    )
    .unwrap_err();

    assert!(!issues.is_empty());
}

#[test]
fn additions_without_normative_language_are_rejected() {
    let issues = parse_delta(
        "## ADDED Requirements\n\n### Requirement: Soft wording\n\nThe tool should probably do something.\n",
    )
    .unwrap_err();

    assert!(
        issues
            .iter()
            .any(|issue| issue.message.contains("SHALL") || issue.message.contains("MUST")),
        "no issue names the normative-language rule: {issues:?}"
    );
}

#[test]
fn a_longer_opening_fence_ignores_a_shorter_close() {
    // CommonMark: a fence closes only on a run at least as long as the
    // opener, so the heading stays inside the block and is not a section.
    let operations = parse_delta(
        "## ADDED Requirements\n\n### Requirement: Fenced\n\nThe tool SHALL treat fences correctly.\n\n#### Scenario: Fences\n\n- WHEN a fence opens\n- THEN a shorter run does not close it\n\n````markdown\n```\n## REMOVED Requirements\n````\n",
    )
    .unwrap();

    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].kind(), DeltaKind::Added);
}

#[test]
fn a_root_level_canonical_heading_parses_with_requirements() {
    let canonical = parse_canonical(
        "# Widget Specification\n\n## Requirements\n\n### Requirement: Widget rendering\n\nThe tool SHALL render widgets.\n\n#### Scenario: Renders\n\n- WHEN a widget exists\n- THEN it renders\n",
    )
    .unwrap();

    assert_eq!(canonical.requirement_count(), 1);
}
