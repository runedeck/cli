use assert_cmd::Command;
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
fn add_creates_minimal_manifest_with_domain_glob_without_installing() {
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
        value["artifacts"]["deck"]["include"][0].as_str(),
        Some("science/**")
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
    assert_eq!(value["artifacts"]["deck"]["cast"].as_str(), Some("science"));
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
