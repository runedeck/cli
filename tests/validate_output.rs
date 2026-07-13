use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn write_valid_module(root: &std::path::Path) {
    std::fs::write(
        root.join("module.yaml"),
        "name: validate-output\nversion: 0.1.0\ndescription: test module\nevents: []\n",
    )
    .unwrap();
    std::fs::write(root.join("defaults.yaml"), "{}\n").unwrap();
    std::fs::write(root.join("README.md"), "# Validate output\n").unwrap();
    std::fs::write(root.join("LICENSE"), "test license\n").unwrap();
    std::fs::write(root.join(".manifest"), "{}\n").unwrap();
    std::fs::create_dir(root.join("rules")).unwrap();
    std::fs::write(root.join("rules/Good.md"), "A compact valid rule.\n").unwrap();
}

#[test]
fn validate_prints_compact_per_item_lines_and_summary() {
    let module = tempfile::tempdir().unwrap();
    write_valid_module(module.path());

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ module.yaml"))
        .stdout(predicate::str::contains("✓ README.md"))
        .stdout(predicate::str::contains("✓ rules/Good.md"))
        .stdout(predicate::str::contains("checked"))
        .stdout(predicate::str::contains("0 warnings"))
        .stdout(predicate::str::contains("0 errors"))
        .stdout(predicate::str::contains("  ok ").not())
        .stdout(predicate::str::contains("MISSING").not());
}

#[test]
fn validate_prints_warning_items_without_failing() {
    let module = tempfile::tempdir().unwrap();
    write_valid_module(module.path());
    std::fs::remove_file(module.path().join(".manifest")).unwrap();

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("⚡ .manifest"))
        .stdout(predicate::str::contains("1 warning"))
        .stdout(predicate::str::contains("0 errors"));
}

#[test]
fn validate_prints_failures_as_items_and_keeps_exit_one() {
    let module = tempfile::tempdir().unwrap();
    write_valid_module(module.path());
    std::fs::remove_file(module.path().join("README.md")).unwrap();

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("✗ README.md"))
        .stdout(predicate::str::contains("1 error"))
        .stdout(predicate::str::contains("missing required file: README.md"));
}

#[test]
fn validate_json_is_an_unpolluted_action_result() {
    let module = tempfile::tempdir().unwrap();
    write_valid_module(module.path());

    let output = rune()
        .args(["--json", "validate", "--source"])
        .arg(module.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout is exactly JSON");
    assert_eq!(json["errors"], serde_json::json!([]));
    assert_eq!(json["warnings"], serde_json::json!([]));
    assert_eq!(json["installed"], serde_json::json!([]));
}
