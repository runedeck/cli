use assert_cmd::Command;
use predicates::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

#[cfg(feature = "tui")]
#[test]
fn tui_snapshot_renders_deck_entry_columns() {
    let deck = format!("{}/tests/support/deck", env!("CARGO_MANIFEST_DIR"));
    rune()
        .args([
            "tui",
            "--snapshot",
            "--source",
            &deck,
            "--section",
            "14",
            "--width",
            "140",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Decks"))
        .stdout(predicate::str::contains("Kinds"))
        .stdout(predicate::str::contains("Runes"))
        .stdout(predicate::str::contains("science"))
        .stdout(predicate::str::contains("writing"))
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("KIND"))
        .stdout(predicate::str::contains("DECK"));
}

#[cfg(feature = "tui")]
#[test]
fn tui_snapshot_renders_deck_casts() {
    let deck = format!("{}/tests/support/deck", env!("CARGO_MANIFEST_DIR"));
    rune()
        .args([
            "tui",
            "--snapshot",
            "--source",
            &deck,
            "--section",
            "15",
            "--drill",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Casts"))
        .stdout(predicate::str::contains("essentials · 2 resolved"));
}

#[cfg(feature = "tui")]
#[test]
fn tui_snapshot_renders_batched_deck_history() {
    let deck = format!("{}/tests/support/deck", env!("CARGO_MANIFEST_DIR"));
    rune()
        .args(["tui", "--snapshot", "--source", &deck, "--section", "16"])
        .assert()
        .success()
        .stdout(predicate::str::contains("History"))
        .stdout(predicate::str::contains("Loading commit history").not());
}

#[test]
fn exec_shell_fixture_round_trips_json() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("module.yaml"), "name: test\n").unwrap();
    let skill = root.path().join("skills/demo");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nexec:\n    script: run.sh\n---\n# Demo\n",
    )
    .unwrap();
    std::fs::write(
        skill.join("run.sh"),
        "read payload\nif [ \"$payload\" != '{\"name\":\"Ada\"}' ]; then exit 8; fi\nprintf '{\"input\":\"%s\",\"arg\":\"%s\"}\\n' \"$INPUT_NAME\" \"$1\"\n",
    )
    .unwrap();

    rune()
        .current_dir(root.path())
        .env("HOME", home.path())
        .args(["exec", "demo", "--json", "{\"name\":\"Ada\"}", "--", "x"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\":true"))
        .stdout(predicate::str::contains("\"input\":\"Ada\""));
}

#[cfg(unix)]
#[test]
fn external_command_from_extension_receives_args_and_exit_code() {
    let cwd = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let extension = tempfile::tempdir().unwrap();
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.yaml"),
        format!("extensions:\n    - {}\n", extension.path().display()),
    )
    .unwrap();
    let script = extension.path().join("rune-hello");
    std::fs::write(
        &script,
        "#!/usr/bin/env bash\nif [ -z \"$RUNE_ROOT\" ]; then exit 7; fi\nprintf 'hello %s %s\\n' \"$1\" \"$2\"\nexit 5\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();

    rune()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .args(["hello", "--name", "x"])
        .assert()
        .code(5)
        .stdout(predicate::str::contains("hello --name x"));
}

#[test]
fn unknown_external_command_exits_two_cleanly() {
    let cwd = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    rune()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .args(["does-not-exist"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "error: unknown command 'rune does-not-exist'",
        ));
}
