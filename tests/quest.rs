//! Integration tests for `rune quest` binding and the `rune add` fallback.
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

fn quest_cmd(home: &Path, quests_root: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .env("HOME", home)
        .env("RUNE_QUESTS", quests_root)
        .arg("quest")
        .args(args)
        .assert()
}

#[test]
fn quest_binds_by_name_under_quests_root() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    fs::create_dir(quests.path().join("inventory")).unwrap();

    quest_cmd(home.path(), quests.path(), &["inventory"])
        .success()
        .stdout(
            predicates::str::contains("quest bound:").and(predicates::str::contains("inventory")),
        );

    let state = fs::read_to_string(home.path().join(".config/rune/state.yaml")).unwrap();
    assert!(
        state.contains("inventory"),
        "state must record the quest: {state}"
    );
}

#[test]
fn quest_slug_resolves_last_segment_and_reports_missing_manifest() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    fs::create_dir(quests.path().join("inventory")).unwrap();

    quest_cmd(home.path(), quests.path(), &["N4M3Z/inventory"])
        .success()
        .stdout(predicates::str::contains("no .rune manifest yet"));
}

#[test]
fn quest_without_argument_shows_binding() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    fs::create_dir(quests.path().join("inventory")).unwrap();

    quest_cmd(home.path(), quests.path(), &["inventory"]).success();
    quest_cmd(home.path(), quests.path(), &[]).success().stdout(
        predicates::str::contains("quest:").and(predicates::str::contains("manifest: none")),
    );
}

#[test]
fn quest_missing_without_clone_suggests_clone() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();

    quest_cmd(home.path(), quests.path(), &["N4M3Z/absent"])
        .failure()
        .stderr(
            predicates::str::contains("not found")
                .and(predicates::str::contains("--clone"))
                .and(predicates::str::contains("github.com/N4M3Z/absent")),
        );
}

#[test]
fn quest_unbind_removes_binding() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    fs::create_dir(quests.path().join("inventory")).unwrap();

    quest_cmd(home.path(), quests.path(), &["inventory"]).success();
    quest_cmd(home.path(), quests.path(), &["--unbind"])
        .success()
        .stdout(predicates::str::contains("quest unbound"));
    quest_cmd(home.path(), quests.path(), &[])
        .success()
        .stdout(predicates::str::contains("no quest bound"));
}

#[test]
fn add_falls_back_to_bound_quest_when_cwd_has_no_manifest() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let quest_dir = quests.path().join("inventory");
    fs::create_dir(&quest_dir).unwrap();

    quest_cmd(home.path(), quests.path(), &["inventory"]).success();

    let deck = deck_fixture().to_string_lossy().into_owned();
    rune()
        .current_dir(elsewhere.path())
        .env("HOME", home.path())
        .env("RUNE_QUESTS", quests.path())
        .args(["add", "science", "--source", &deck])
        .assert()
        .success()
        .stdout(predicates::str::contains("using bound quest"));

    assert!(
        quest_dir.join(".rune").is_file(),
        "manifest must land in the bound quest, not the cwd"
    );
    assert!(!elsewhere.path().join(".rune").exists());
}

#[test]
fn add_prefers_cwd_manifest_over_bound_quest() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    fs::create_dir(quests.path().join("inventory")).unwrap();
    quest_cmd(home.path(), quests.path(), &["inventory"]).success();

    let deck = deck_fixture().to_string_lossy().into_owned();
    fs::write(
        cwd.path().join(".rune"),
        format!("version: 1\nsources:\n  deck:\n    local: {deck}\n"),
    )
    .unwrap();

    rune()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("RUNE_QUESTS", quests.path())
        .args(["add", "science"])
        .assert()
        .success();

    let manifest = fs::read_to_string(cwd.path().join(".rune")).unwrap();
    assert!(
        manifest.contains("science"),
        "cwd manifest must win: {manifest}"
    );
    assert!(
        !quests.path().join("inventory/.rune").exists(),
        "bound quest must stay untouched when cwd has a manifest"
    );
}
