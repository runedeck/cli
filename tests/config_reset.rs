use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn user_config_path(home: &Path) -> PathBuf {
    home.join(".config/rune/config.yaml")
}

fn write_config(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("config parent")).expect("create config directory");
    fs::write(path, content).expect("write config fixture");
}

const SOURCE_CONFIG: &str = "# keep this comment\nvalidate:\n    exclude:\n        - build\nproviders:\n    codex:\n        enabled: false\n    claude:\n        plugin: rune\n";

#[test]
fn reset_removes_one_nested_key_and_keeps_every_other_line() {
    let home = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let config = source.path().join("config.yaml");
    write_config(&config, SOURCE_CONFIG);

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args([
            "--json",
            "config",
            "reset",
            "providers.codex",
            "--scope",
            "source",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["removed"], "providers.codex");

    let after = fs::read_to_string(&config).unwrap();
    assert!(after.contains("# keep this comment"));
    assert!(after.contains("claude:"));
    assert!(!after.contains("codex:"));
    assert!(!after.contains("enabled: false"));
    assert!(after.contains("exclude:"));
}

#[test]
fn reset_writes_a_backup_and_names_the_restore_command() {
    let home = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let config = source.path().join("config.yaml");
    write_config(&config, SOURCE_CONFIG);

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["--json", "config", "reset", "validate", "--scope", "source"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();

    let backup = PathBuf::from(report["backup"].as_str().unwrap());
    assert!(backup.is_file(), "backup file exists");
    assert_eq!(fs::read_to_string(&backup).unwrap(), SOURCE_CONFIG);
    let restore = report["restore"].as_str().unwrap();
    assert!(restore.starts_with("command "), "restore: {restore}");
    assert!(restore.contains(backup.file_name().unwrap().to_str().unwrap()));
}

#[test]
fn reset_unknown_key_fails_without_writes() {
    let home = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let config = source.path().join("config.yaml");
    write_config(&config, SOURCE_CONFIG);

    let output = rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args([
            "--json",
            "config",
            "reset",
            "providers.nope",
            "--scope",
            "source",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "config.unknown_key");
    assert_eq!(fs::read_to_string(&config).unwrap(), SOURCE_CONFIG);
    let entries = fs::read_dir(source.path()).unwrap().count();
    assert_eq!(entries, 1, "no backup on failure");
}

#[test]
fn reset_user_scope_removes_a_top_level_key() {
    let home = tempfile::tempdir().unwrap();
    let config = user_config_path(home.path());
    write_config(&config, "deck: /tmp/deck\nowner: someone\n");

    rune()
        .env("HOME", home.path())
        .args(["config", "reset", "owner", "--scope", "user"])
        .assert()
        .success();

    let after = fs::read_to_string(&config).unwrap();
    assert!(after.contains("deck: /tmp/deck"));
    assert!(!after.contains("owner:"));
}

#[test]
fn reset_handles_two_space_indent() {
    let home = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let config = source.path().join("config.yaml");
    write_config(
        &config,
        "providers:\n  codex:\n    enabled: false\n  claude:\n    plugin: rune\n",
    );

    rune()
        .env("HOME", home.path())
        .current_dir(source.path())
        .args(["config", "reset", "providers.codex", "--scope", "source"])
        .assert()
        .success();

    let after = fs::read_to_string(&config).unwrap();
    assert!(!after.contains("codex:"));
    assert!(after.contains("claude:"));
}
