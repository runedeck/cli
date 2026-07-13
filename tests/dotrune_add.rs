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

fn add(consumer_root: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .current_dir(consumer_root)
        .arg("add")
        .args(args)
        .assert()
}

fn install(consumer_root: &Path) -> assert_cmd::assert::Assert {
    rune()
        .args([
            "install",
            "--source",
            consumer_root.to_str().unwrap(),
            "--target",
            consumer_root.to_str().unwrap(),
        ])
        .assert()
}

#[test]
fn add_creates_minimal_manifest_with_deck_token_without_installing() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    let output = add(consumer.path(), &["science", "--source", &deck]).success();

    let manifest = fs::read_to_string(consumer.path().join(".rune")).unwrap();
    let value: serde_yaml::Value = serde_yaml::from_str(&manifest).unwrap();
    assert_eq!(value["version"].as_u64(), Some(1));
    assert_eq!(
        value["sources"]["deck"]["local"].as_str(),
        Some(deck.as_str())
    );
    assert_eq!(
        value["runes"]["deck"]["include"][0].as_str(),
        Some("science")
    );
    assert!(!consumer.path().join(".claude").exists());
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("rune install --source"), "{stdout}");
}

#[test]
fn add_is_idempotent_and_cast_is_stored_by_name() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    add(consumer.path(), &["--cast", "science", "--source", &deck]).success();
    let first = fs::read(consumer.path().join(".rune")).unwrap();
    add(consumer.path(), &["--cast", "science"]).success();
    let second = fs::read(consumer.path().join(".rune")).unwrap();

    assert_eq!(
        first, second,
        "idempotent add must leave .rune byte-identical"
    );
    let value: serde_yaml::Value = serde_yaml::from_slice(&second).unwrap();
    assert_eq!(value["runes"]["deck"]["casts"][0].as_str(), Some("science"));
}

#[test]
fn add_then_install_then_drift_is_clean() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();
    add(consumer.path(), &["science", "--source", &deck]).success();

    install(consumer.path()).success();
    let output = rune()
        .args([
            "drift",
            "--source",
            consumer.path().to_str().unwrap(),
            "--target",
            consumer.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("identical") || stdout.contains("No drift"),
        "{stdout}"
    );
}

#[test]
fn add_uses_rune_deck_when_source_is_omitted() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    rune()
        .current_dir(consumer.path())
        .env("RUNE_DECK", &deck)
        .args(["add", "science"])
        .assert()
        .success();

    let manifest: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(consumer.path().join(".rune")).unwrap()).unwrap();
    assert_eq!(
        manifest["sources"]["deck"]["local"].as_str(),
        Some(deck.as_str())
    );
}

#[test]
fn add_prefers_existing_single_source_over_rune_deck() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();
    add(consumer.path(), &["science", "--source", &deck]).success();

    rune()
        .current_dir(consumer.path())
        .env("RUNE_DECK", "/does/not/exist")
        .args(["add", "writing"])
        .assert()
        .success();

    let manifest: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(consumer.path().join(".rune")).unwrap()).unwrap();
    assert_eq!(
        manifest["sources"]["deck"]["local"].as_str(),
        Some(deck.as_str())
    );
}

#[test]
fn add_accepts_canonical_three_segment_id() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    add(
        consumer.path(),
        &["science/skills/OnlyScience", "--source", &deck],
    )
    .success();

    let manifest: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(consumer.path().join(".rune")).unwrap()).unwrap();
    assert_eq!(
        manifest["runes"]["deck"]["include"][0].as_str(),
        Some("science/skills/OnlyScience")
    );
}

#[test]
fn add_rejects_ambiguous_short_form_listing_candidates() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    add(consumer.path(), &["science/SharedName", "--source", &deck])
        .failure()
        .stderr(
            predicates::str::contains("ambiguous")
                .and(predicates::str::contains("science/agents/SharedName"))
                .and(predicates::str::contains("science/skills/SharedName")),
        );

    assert!(
        !consumer.path().join(".rune").exists(),
        "a rejected add must not write .rune"
    );
}

#[test]
fn add_rejects_unknown_rune_at_add_time() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    add(
        consumer.path(),
        &["science/DoesNotExist", "--source", &deck],
    )
    .failure()
    .stderr(predicates::str::contains("not found"));
}

#[test]
fn add_accepts_comma_separated_runes_and_casts() {
    let consumer = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    add(
        consumer.path(),
        &["OnlyScience,OnlyWriting", "--source", &deck],
    )
    .success();
    add(consumer.path(), &["--cast", "science,essentials"]).success();

    let manifest: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(consumer.path().join(".rune")).unwrap()).unwrap();
    assert_eq!(
        manifest["runes"]["deck"]["include"][0].as_str(),
        Some("OnlyScience")
    );
    assert_eq!(
        manifest["runes"]["deck"]["include"][1].as_str(),
        Some("OnlyWriting")
    );
    assert_eq!(
        manifest["runes"]["deck"]["casts"][0].as_str(),
        Some("science")
    );
    assert_eq!(
        manifest["runes"]["deck"]["casts"][1].as_str(),
        Some("essentials")
    );
}

#[test]
fn add_without_any_source_names_all_resolution_options() {
    let consumer = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    rune()
        .current_dir(consumer.path())
        .env("HOME", home.path())
        .env_remove("RUNE_DECK")
        .args(["add", "science"])
        .assert()
        .failure()
        .stderr(
            predicates::str::contains("--source")
                .and(predicates::str::contains("RUNE_DECK"))
                .and(predicates::str::contains("rune config set deck")),
        );
}

#[test]
fn config_set_deck_supplies_add_source() {
    let consumer = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let deck = deck_fixture().to_string_lossy().into_owned();

    rune()
        .env("HOME", home.path())
        .args(["config", "set", "deck", &deck])
        .assert()
        .success();
    rune()
        .current_dir(consumer.path())
        .env("HOME", home.path())
        .env_remove("RUNE_DECK")
        .args(["add", "science"])
        .assert()
        .success();

    let manifest: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(consumer.path().join(".rune")).unwrap()).unwrap();
    assert_eq!(
        manifest["sources"]["deck"]["local"].as_str(),
        Some(deck.as_str())
    );
}
