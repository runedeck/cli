//! Integration tests for `rune target` binding and the `rune add` fallback.
//! Each test isolates HOME so the state file lands in a tempdir.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::fs;
use std::path::{Path, PathBuf};

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn deck_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck")
}

fn target_cmd(home: &Path, targets_root: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .env("HOME", home)
        .env("RUNE_TARGETS", targets_root)
        .arg("target")
        .args(args)
        .assert()
}

#[test]
fn quest_alias_and_legacy_env_still_bind() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();

    rune()
        .env("HOME", home.path())
        .env("RUNE_QUESTS", targets.path())
        .args(["quest", "inventory"])
        .assert()
        .success()
        .stdout(predicates::str::contains("bound target 'inventory'"));
}

#[test]
fn legacy_quest_state_key_still_resolves_the_binding() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    let bound = targets.path().join("inventory");
    fs::create_dir(&bound).unwrap();
    let state_dir = home.path().join(".config/rune");
    fs::create_dir_all(&state_dir).unwrap();
    fs::write(
        state_dir.join("state.yaml"),
        format!("quest: {}\n", bound.display()),
    )
    .unwrap();

    target_cmd(home.path(), targets.path(), &[])
        .success()
        .stdout(predicates::str::contains("target:").and(predicates::str::contains("inventory")));
}

#[test]
fn target_binds_by_name_under_quests_root() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"])
        .success()
        .stdout(predicates::str::contains("bound target 'inventory'"))
        .stdout(predicates::str::contains("next: rune add <deck-or-rune>"))
        .stdout(predicates::str::contains("state.yaml").not());

    let state = fs::read_to_string(home.path().join(".config/rune/state.yaml")).unwrap();
    assert!(
        state.contains("inventory"),
        "state must record the target: {state}"
    );
    assert!(
        state.contains("targets:"),
        "state must record history: {state}"
    );
}

#[test]
fn target_list_marks_the_active_binding() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();
    fs::create_dir(targets.path().join("signals")).unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"]).success();
    target_cmd(home.path(), targets.path(), &["signals"]).success();
    target_cmd(home.path(), targets.path(), &["--list"])
        .success()
        .stdout(predicates::str::contains("* ").and(predicates::str::contains("signals")))
        .stdout(predicates::str::contains("\n  ").and(predicates::str::contains("inventory")));
}

#[test]
fn target_dash_switches_to_the_previous_binding() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();
    fs::create_dir(targets.path().join("signals")).unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"]).success();
    target_cmd(home.path(), targets.path(), &["signals"]).success();
    target_cmd(home.path(), targets.path(), &["-"])
        .success()
        .stdout(predicates::str::contains("bound target 'inventory'"));
    target_cmd(home.path(), targets.path(), &["--list"])
        .success()
        .stdout(predicates::str::contains("* ").and(predicates::str::contains("inventory")))
        .stdout(predicates::str::contains("\n  ").and(predicates::str::contains("signals")));
}

#[test]
fn target_history_deduplicates_and_caps_at_ten() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    for index in 0..11 {
        fs::create_dir(targets.path().join(format!("quest-{index}"))).unwrap();
        target_cmd(home.path(), targets.path(), &[&format!("quest-{index}")]).success();
    }
    target_cmd(home.path(), targets.path(), &["quest-5"]).success();

    let state = fs::read_to_string(home.path().join(".config/rune/state.yaml")).unwrap();
    let state: serde_yaml::Value = serde_yaml::from_str(&state).unwrap();
    let history = state["targets"].as_sequence().unwrap();
    assert_eq!(history.len(), 10);
    assert_eq!(
        history[0].as_str(),
        Some(
            std::fs::canonicalize(targets.path().join("quest-5"))
                .unwrap()
                .to_str()
                .unwrap()
        )
    );
    assert_eq!(
        history
            .iter()
            .filter(|quest| quest.as_str() == history[0].as_str())
            .count(),
        1
    );
}

#[test]
fn target_state_tolerates_unknown_and_mistyped_history_fields() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();
    let state_dir = home.path().join(".config/rune");
    fs::create_dir_all(&state_dir).unwrap();
    fs::write(
        state_dir.join("state.yaml"),
        "quest: 42\nquests: not-a-list\nfuture-field: preserved\n",
    )
    .unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"]).success();
    let state = fs::read_to_string(state_dir.join("state.yaml")).unwrap();
    assert!(state.contains("future-field: preserved"));
    assert!(state.contains("targets:"));
}

#[test]
fn target_slug_resolves_last_segment_and_reports_missing_manifest() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();

    target_cmd(home.path(), targets.path(), &["N4M3Z/inventory"])
        .success()
        .stdout(predicates::str::contains("no .rune manifest yet"));
}

#[test]
fn target_without_argument_shows_binding() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"]).success();
    target_cmd(home.path(), targets.path(), &[])
        .success()
        .stdout(
            predicates::str::contains("target:").and(predicates::str::contains("manifest: none")),
        );
}

#[test]
fn target_missing_without_clone_suggests_clone() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();

    target_cmd(home.path(), targets.path(), &["N4M3Z/absent"])
        .failure()
        .stderr(
            predicates::str::contains("not found")
                .and(predicates::str::contains("--clone"))
                .and(predicates::str::contains("github.com/N4M3Z/absent")),
        );
}

#[test]
fn target_unbind_removes_binding() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"]).success();
    target_cmd(home.path(), targets.path(), &["--unbind"])
        .success()
        .stdout(predicates::str::contains("target unbound"));
    target_cmd(home.path(), targets.path(), &[])
        .success()
        .stdout(predicates::str::contains("no target bound"));
}

#[test]
fn add_falls_back_to_bound_quest_when_cwd_has_no_manifest() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let target_dir = targets.path().join("inventory");
    fs::create_dir(&target_dir).unwrap();

    target_cmd(home.path(), targets.path(), &["inventory"]).success();

    let deck = deck_fixture().to_string_lossy().into_owned();
    rune()
        .current_dir(elsewhere.path())
        .env("HOME", home.path())
        .env("RUNE_TARGETS", targets.path())
        .args(["add", "science", "--source", &deck])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("staged")
                .and(predicates::str::contains("inventory"))
                .and(predicates::str::contains("next:")),
        );

    assert!(
        target_dir.join(".rune").is_file(),
        "manifest must land in the bound target, not the cwd"
    );
    assert!(!elsewhere.path().join(".rune").exists());
}

#[test]
fn add_prefers_cwd_manifest_over_bound_quest() {
    let home = tempfile::tempdir().unwrap();
    let targets = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    fs::create_dir(targets.path().join("inventory")).unwrap();
    target_cmd(home.path(), targets.path(), &["inventory"]).success();

    let deck = deck_fixture().to_string_lossy().into_owned();
    fs::write(
        cwd.path().join(".rune"),
        format!("version: 1\nsources:\n  deck:\n    local: {deck}\n"),
    )
    .unwrap();

    rune()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("RUNE_TARGETS", targets.path())
        .args(["add", "science"])
        .assert()
        .success();

    let manifest = fs::read_to_string(cwd.path().join(".rune")).unwrap();
    assert!(
        manifest.contains("science"),
        "cwd manifest must win: {manifest}"
    );
    assert!(
        !targets.path().join("inventory/.rune").exists(),
        "bound target must stay untouched when cwd has a manifest"
    );
}
