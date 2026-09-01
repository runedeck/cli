use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

/// A consumer root with one local module source carrying two rules and one
/// skill, plus a commented `.rune` staging all of them.
fn consumer_fixture(root: &Path) {
    let module = root.join("module");
    fs::create_dir_all(module.join("rules")).unwrap();
    fs::create_dir_all(module.join("skills/Deslop")).unwrap();
    fs::write(module.join("module.yaml"), "name: mod\n").unwrap();
    fs::write(module.join("rules/Style.md"), "Use the active voice.\n").unwrap();
    fs::write(module.join("rules/Naming.md"), "Use one name per thing.\n").unwrap();
    fs::write(
        module.join("skills/Deslop/SKILL.md"),
        include_str!("fixtures/toggle/skill.md"),
    )
    .unwrap();
    fs::write(
        root.join(".rune"),
        include_str!("fixtures/toggle/consumer.rune"),
    )
    .unwrap();
}

/// A consumer root whose source is a deck, so the bare kind listing works.
fn deck_fixture(root: &Path) {
    let deck = root.join("deck");
    fs::create_dir_all(deck.join("runes/core/rules")).unwrap();
    fs::write(
        deck.join("deck.yaml"),
        include_str!("fixtures/toggle/deck.yaml"),
    )
    .unwrap();
    fs::write(
        deck.join("runes/core/module.yaml"),
        "name: core\nversion: 0.1.0\ndescription: test domain\n",
    )
    .unwrap();
    fs::write(
        deck.join("runes/core/rules/Style.md"),
        "Use the active voice.\n",
    )
    .unwrap();
    fs::write(
        deck.join("runes/core/rules/Naming.md"),
        "Use one name per thing.\n",
    )
    .unwrap();
    fs::write(
        root.join(".rune"),
        include_str!("fixtures/toggle/deck-consumer.rune"),
    )
    .unwrap();
}

#[test]
fn toggle_off_excludes_one_provider_and_preserves_bytes() {
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());
    let before = fs::read_to_string(root.path().join(".rune")).unwrap();

    rune()
        .current_dir(root.path())
        .args(["rule", "off", "Style", "--provider", "claude"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rules/Style off for claude"));

    let after = fs::read_to_string(root.path().join(".rune")).unwrap();
    assert!(after.contains("# consumer manifest (comment must survive toggles)"));
    assert!(after.contains("# staged selection comment"));
    assert!(after.contains("version: 3"));
    assert!(after.contains("exclude: [rules/Style]"));
    let unchanged_lines: Vec<&str> = before
        .lines()
        .filter(|line| !line.starts_with("version:"))
        .collect();
    for line in unchanged_lines {
        assert!(after.contains(line), "line lost: {line}");
    }
}

#[test]
fn toggle_off_without_provider_covers_every_enabled_provider() {
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());

    let output = rune()
        .current_dir(root.path())
        .args(["--json", "rule", "off", "Naming"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let toggled = report["toggled"].as_array().unwrap();
    assert!(toggled.len() >= 2, "expected several providers: {report}");
}

#[test]
fn toggle_on_restores_and_removes_the_overlay() {
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());

    rune()
        .current_dir(root.path())
        .args(["rule", "off", "Style", "--provider", "claude"])
        .assert()
        .success();
    rune()
        .current_dir(root.path())
        .args(["rule", "on", "Style", "--provider", "claude"])
        .assert()
        .success();

    let after = fs::read_to_string(root.path().join(".rune")).unwrap();
    assert!(!after.contains("exclude:"));
    assert!(!after.contains("providers:"));
}

#[test]
fn toggle_of_an_unstaged_rune_fails_loudly() {
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());
    let before = fs::read_to_string(root.path().join(".rune")).unwrap();

    let output = rune()
        .current_dir(root.path())
        .args(["--json", "rule", "off", "Typo", "--provider", "claude"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "toggle.unknown_rune");
    assert_eq!(
        fs::read_to_string(root.path().join(".rune")).unwrap(),
        before,
        "a failed toggle writes nothing"
    );
}

#[test]
fn unknown_provider_fails_with_a_structured_error() {
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());

    let output = rune()
        .current_dir(root.path())
        .args(["--json", "rule", "off", "Style", "--provider", "nope"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "provider.unknown");
    assert_eq!(error["fix_command"], "rune provider");
}

#[test]
fn install_prunes_a_toggled_off_deployment() {
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());
    let target = root.path();

    rune()
        .current_dir(root.path())
        .args(["install", "--provider", "claude"])
        .assert()
        .success();
    let deployed = target.join(".claude/rules/Style.md");
    assert!(deployed.is_file(), "first install deploys the rule");

    rune()
        .current_dir(root.path())
        .args(["rule", "off", "Style", "--provider", "claude"])
        .assert()
        .success();
    rune()
        .current_dir(root.path())
        .args(["install", "--provider", "claude"])
        .assert()
        .success();

    assert!(!deployed.exists(), "toggled-off rule must leave the tree");
    let trash = target.join(".claude/.trash");
    assert!(trash.is_dir(), "prune quarantines into .trash");
    assert!(
        target.join(".claude/rules/Naming.md").is_file(),
        "other rules stay deployed"
    );
}

#[test]
fn list_shows_the_provider_matrix() {
    let root = tempfile::tempdir().unwrap();
    deck_fixture(root.path());

    rune()
        .current_dir(root.path())
        .args(["rule", "off", "Style", "--provider", "claude"])
        .assert()
        .success();

    rune()
        .current_dir(root.path())
        .args(["rule"])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude:off"))
        .stdout(predicate::str::contains("claude:on"));
}
