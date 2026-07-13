use assert_cmd::Command;
use predicates::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

#[test]
fn version_flag_prints_version() {
    let expected = format!(
        "rune {} ({}) built {}\n",
        env!("CARGO_PKG_VERSION"),
        env!("RUNE_BUILD_COMMIT"),
        env!("RUNE_BUILD_TIME")
    );

    rune().arg("--version").assert().success().stdout(expected);
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
        .stdout(predicate::str::contains("review"))
        .stdout(predicate::str::contains("release"));
}

#[test]
fn root_help_spellings_render_the_custom_page() {
    for argument in ["--help", "-h", "help"] {
        rune()
            .arg(argument)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Deck toolkit for AI harnesses: your runes, deployed.",
            ))
            .stdout(predicate::str::contains("Quick start:"));
    }
}

#[test]
fn review_export_matches_agent_ready_golden_file() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), "alpha\nbeta\ngamma\n").unwrap();
    std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(
        root.path().join(".rune-comments.yaml"),
        "version: 1\ncomments:\n  - module: rune\n    path: src/main.rs\n    line: 1\n    kind: praise\n    text: clear entry point\n  - module: rune\n    path: src/lib.rs\n    line: 2\n    end_line: 3\n    kind: issue\n    text: simplify the branch\n",
    )
    .unwrap();

    rune()
        .args([
            "review",
            "export",
            "--target",
            root.path().to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout(include_str!("fixtures/review-export.md"));
}

#[test]
fn review_defaults_to_bound_quest_when_cwd_has_no_comments() {
    let home = tempfile::tempdir().unwrap();
    let quest = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        quest.path().join(".rune-comments.yaml"),
        "version: 1\ncomments:\n  - module: rune\n    path: src/lib.rs\n    line: 2\n    kind: note\n    text: bound quest comment\n",
    )
    .unwrap();
    let state_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::fs::write(
        state_dir.join("state.yaml"),
        format!(
            "quest: {}\nquests:\n  - {}\n",
            quest.path().display(),
            quest.path().display()
        ),
    )
    .unwrap();

    rune()
        .current_dir(elsewhere.path())
        .env("HOME", home.path())
        .args(["review", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bound quest comment"));
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

#[cfg(not(feature = "tui"))]
#[test]
fn no_args_exits_with_error() {
    rune().assert().failure().stderr(predicate::str::contains(
        "Deck toolkit for AI harnesses: your runes, deployed.",
    ));
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
fn tui_edit_snapshot_keeps_action_footer_visible() {
    let deck = format!("{}/tests/support/deck", env!("CARGO_MANIFEST_DIR"));
    let home = tempfile::tempdir().unwrap();
    rune()
        .env("HOME", home.path())
        .args([
            "tui",
            "--snapshot",
            "--edit",
            "--source",
            &deck,
            "--width",
            "120",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Space toggle"))
        .stdout(predicate::str::contains("n/p deck"))
        .stdout(predicate::str::contains("I install"))
        .stdout(predicate::str::contains("q quit"));
}

#[cfg(feature = "tui")]
#[test]
fn tui_code_snapshot_renders_raw_agent_and_edit_footer() {
    let deck = format!("{}/tests/support/deck", env!("CARGO_MANIFEST_DIR"));
    rune()
        .args([
            "tui",
            "--snapshot",
            "--source",
            &deck,
            "--section",
            "3",
            "--drill",
            "2",
            "--tab",
            "code",
            "--width",
            "160",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Descriptive fixture placeholder for an agent.",
        ))
        .stdout(predicate::str::contains("c comment"))
        .stdout(predicate::str::contains("e edit"))
        .stdout(predicate::str::contains("E $EDITOR"))
        .stdout(predicate::str::contains("o override"))
        .stdout(predicate::str::contains("source unavailable").not());
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
