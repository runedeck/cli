use assert_cmd::Command;
use predicates::prelude::*;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

#[test]
fn version_flag_prints_version() {
    rune()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rune"));
}

#[test]
fn help_flag_lists_subcommands() {
    rune()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("assemble"))
        .stdout(predicate::str::contains("copy"))
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("release"));
}

#[test]
fn install_help_shows_flags() {
    rune()
        .args(["install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--json"));
}

#[test]
fn install_nonexistent_path_fails() {
    rune()
        .args(["install", "/nonexistent/path"])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn validate_help_succeeds() {
    rune().args(["validate", "--help"]).assert().success();
}

#[test]
fn assemble_help_succeeds() {
    rune().args(["assemble", "--help"]).assert().success();
}

#[test]
fn copy_help_succeeds() {
    rune().args(["copy", "--help"]).assert().success();
}

#[test]
fn release_help_shows_embed() {
    rune()
        .args(["release", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--embed"));
}

#[test]
fn json_flag_accepted_globally() {
    rune()
        .args(["--json", "install", "--help"])
        .assert()
        .success();
}

#[test]
fn no_args_exits_with_error() {
    rune()
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}
