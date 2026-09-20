use super::*;

const CLEAN: &str = "# Changelog\n\nAll notable changes to this project are recorded here.\n\n## [Unreleased]\n\n### Added\n\n- Add `rune draft` for runes under construction (#61)\n\n## [0.5.0] - 2026-07-17\n\nFirst release with the sealed ceremony.\n\n### Changed\n\n- **Breaking:** Rename `rune sign queue` to `rune sign submit` for merge requests\n\n### Fixed\n\n- Fix the manifest fingerprint after a provenance move\n";

fn rules(content: &str) -> Vec<String> {
    lint(content)
        .errors
        .iter()
        .map(|error| error.split(": ").nth(1).unwrap_or_default().to_string())
        .collect()
}

#[test]
fn a_clean_changelog_has_no_errors() {
    let report = lint(CLEAN);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.releases, 2);
    assert_eq!(report.entries, 3);
}

#[test]
fn a_paragraph_entry_is_an_error_by_length_and_by_wrapping() {
    let wall = format!("- {}", "the command does this and that, ".repeat(10));
    let content = CLEAN.replace("- Fix the manifest fingerprint after a provenance move", &wall);
    assert!(rules(&content).contains(&"entry-length".to_string()), "{:?}", lint(&content).errors);

    let wrapped = CLEAN.replace(
        "- Fix the manifest fingerprint after a provenance move",
        "- Fix the manifest fingerprint\n  after a provenance move",
    );
    assert!(rules(&wrapped).contains(&"entry-wrapped".to_string()));
}

#[test]
fn entries_start_with_a_verb_and_carry_no_encoded_prefix() {
    let content = CLEAN.replace("- Fix the manifest", "- fix: the manifest");
    let found = rules(&content);
    assert!(found.contains(&"entry-prefix".to_string()), "{found:?}");
    assert!(found.contains(&"entry-verb".to_string()), "{found:?}");
    let content = CLEAN.replace("- Fix the manifest", "- The manifest");
    assert!(rules(&content).contains(&"entry-verb".to_string()));
    let content = CLEAN.replace("- Fix the manifest", "- `rune sign` fixes the manifest");
    assert!(rules(&content).contains(&"entry-verb".to_string()));
}

#[test]
fn groups_are_known_unique_and_ordered() {
    let content = CLEAN.replace("### Fixed", "### Bugs");
    assert!(rules(&content).contains(&"group-name".to_string()));
    let content = CLEAN.replace("### Fixed\n\n- Fix", "### Added\n\n- Add");
    assert!(rules(&content).contains(&"group-order".to_string()));
    let content = CLEAN.replace("### Fixed\n\n- Fix", "### Changed\n\n- Change");
    assert!(rules(&content).contains(&"group-repeated".to_string()));
}

#[test]
fn releases_are_shaped_and_newest_first() {
    let content = CLEAN.replace("## [0.5.0] - 2026-07-17", "## 0.5.0 (2026-07-17)");
    assert!(rules(&content).contains(&"release-heading".to_string()));
    let content = format!("{CLEAN}\n## [0.6.0] - 2026-08-01\n\n### Added\n\n- Add a thing\n");
    assert!(rules(&content).contains(&"release-order".to_string()));
    let content = CLEAN.replacen("## [Unreleased]\n\n### Added\n\n- Add `rune draft` for runes under construction (#61)\n\n", "", 1)
        + "\n## [Unreleased]\n\n### Added\n\n- Add a thing\n";
    assert!(rules(&content).contains(&"unreleased-first".to_string()));
}

#[test]
fn a_release_holds_one_notice_and_no_other_prose() {
    let content = CLEAN.replace(
        "First release with the sealed ceremony.\n",
        "First release with the sealed ceremony.\n\nA second paragraph of blog.\n",
    );
    assert!(rules(&content).contains(&"release-prose".to_string()));
    let content = CLEAN.replace("### Fixed\n\n- Fix", "### Fixed\n\nSome prose here.\n\n- Fix");
    assert!(rules(&content).contains(&"prose-in-group".to_string()));
}

#[test]
fn an_absent_changelog_is_not_an_error() {
    let root = tempfile::tempdir().unwrap();
    let report = check(root.path()).unwrap();
    assert!(!report.present);
    assert!(report.errors.is_empty());
}
