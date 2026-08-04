use super::resolve_kind_name;
use rune::provider::ContentKind;

fn deck_ids() -> Vec<String> {
    [
        "development/skills/deslop",
        "development/rules/Deslop",
        "development/skills/version-control",
        "council/skills/convene-council",
        "research/skills/deslop",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[test]
fn unique_name_resolves_to_qualified_id() {
    let id = resolve_kind_name(ContentKind::Skills, "version-control", &deck_ids()).unwrap();
    assert_eq!(id, "development/skills/version-control");
}

#[test]
fn kind_filter_separates_rule_from_skill() {
    let id = resolve_kind_name(ContentKind::Rules, "Deslop", &deck_ids()).unwrap();
    assert_eq!(id, "development/rules/Deslop");
}

#[test]
fn ambiguous_name_lists_every_candidate() {
    let error = resolve_kind_name(ContentKind::Skills, "deslop", &deck_ids()).unwrap_err();
    assert!(error.message().contains("development/skills/deslop"));
    assert!(error.message().contains("research/skills/deslop"));
}

#[test]
fn domain_qualified_name_disambiguates() {
    let id = resolve_kind_name(ContentKind::Skills, "research/deslop", &deck_ids()).unwrap();
    assert_eq!(id, "research/skills/deslop");
}

#[test]
fn full_canonical_id_resolves_as_given() {
    let id = resolve_kind_name(
        ContentKind::Skills,
        "development/skills/deslop",
        &deck_ids(),
    )
    .unwrap();
    assert_eq!(id, "development/skills/deslop");
}

#[test]
fn unknown_name_fails_loudly() {
    let error = resolve_kind_name(ContentKind::Skills, "ghost", &deck_ids()).unwrap_err();
    assert!(error.message().contains("no skills rune named 'ghost'"));
}
