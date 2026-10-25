mod support;
use assert_cmd::Command;
use predicates::prelude::*;

fn rune(home: &std::path::Path) -> Command {
    let mut command = support::rune();
    command.env("HOME", home);
    command
}

/// A fake `codex` that records its arguments and environment beside
/// itself, writes `stderr` on stderr, prints `stdout`, and exits `code`.
#[cfg(unix)]
fn fake_codex(home: &std::path::Path, stdout: &str, stderr: &str, code: i32) -> String {
    use std::os::unix::fs::PermissionsExt;

    let path = home.join("fake-codex");
    std::fs::write(
        &path,
        format!(
            r#"#!/bin/sh
dir="$(dirname "$0")"
for argument in "$@"; do printf '%s\n' "$argument"; done > "$dir/args.txt"
env > "$dir/env.txt"
printf '%s' '{stderr}' >&2
printf '%s' '{stdout}'
exit {code}
"#
        ),
    )
    .expect("fake codex");
    let mut permissions = std::fs::metadata(&path)
        .expect("fake codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("fake codex permissions");
    path.to_str().expect("fake codex path").to_string()
}

#[cfg(unix)]
fn recorded(home: &std::path::Path, file: &str) -> Vec<String> {
    std::fs::read_to_string(home.join(file))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn json_of(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not one JSON object ({error}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

const RECEIPT: &str = "Queued message msg-41 for thread 019a-ouroboros.\n";

#[cfg(unix)]
#[test]
fn codex_session_takes_a_queued_message() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    rune(home.path())
        .args([
            "queue",
            "codex",
            "--session",
            "ouroboros",
            "--binary",
            &codex,
        ])
        .arg("Is the deck ready?")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "queued message msg-41 for codex thread 019a-ouroboros",
        ));
    assert_eq!(
        recorded(home.path(), "args.txt"),
        [
            "queue",
            "--thread=ouroboros",
            "--message=Is the deck ready?",
            "--remote=unix://",
        ]
    );
}

#[cfg(unix)]
#[test]
fn json_result_names_the_message_and_the_thread() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    let output = rune(home.path())
        .args(["--json", "queue", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "Is the deck ready?"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(0));
    let result = json_of(&output);
    assert_eq!(result["ok"], true);
    assert_eq!(result["kind"], "queued");
    assert_eq!(result["transport"], "codex queue");
    assert_eq!(result["session"], "ouroboros");
    assert_eq!(result["message_id"], "msg-41");
    assert_eq!(result["thread_id"], "019a-ouroboros");
    assert_eq!(result["exit_code"], 0);
}

#[cfg(unix)]
#[test]
fn profile_environment_applies_and_its_arguments_stay_out() {
    let home = tempfile::tempdir().expect("home");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    codex:\n      deep:\n",
            "        env:\n          CODEX_HOME: /owner/codex-deep\n",
            "        args: ['-m', 'gpt-5.6-sol']\n",
        ),
    )
    .expect("config");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    rune(home.path())
        .env("CODEX_THREAD_ID", "the-calling-session")
        .args(["queue", "deep@codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "Is the deck ready?"])
        .assert()
        .success()
        .stderr(predicate::str::contains("-m gpt-5.6-sol"));
    let args = recorded(home.path(), "args.txt");
    assert!(!args.iter().any(|argument| argument == "-m"), "{args:?}");
    let env = recorded(home.path(), "env.txt");
    assert!(
        env.contains(&"CODEX_HOME=/owner/codex-deep".to_string()),
        "{env:?}"
    );
    assert!(
        !env.iter().any(|line| line.starts_with("CODEX_THREAD_ID=")),
        "{env:?}"
    );
}

#[cfg(unix)]
#[test]
fn message_comes_from_stdin_and_remote_options_pass_on() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    rune(home.path())
        .args([
            "queue",
            "codex",
            "--session",
            "ouroboros",
            "--binary",
            &codex,
        ])
        .args([
            "--remote",
            "wss://host:4500",
            "--remote-auth-token-env",
            "CODEX_TOKEN",
        ])
        .write_stdin("from stdin")
        .assert()
        .success();
    assert_eq!(
        recorded(home.path(), "args.txt"),
        [
            "queue",
            "--thread=ouroboros",
            "--message=from stdin",
            "--remote=wss://host:4500",
            "--remote-auth-token-env=CODEX_TOKEN",
        ]
    );
}

#[cfg(unix)]
#[test]
fn dry_run_shows_the_command_and_starts_nothing() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    let output = rune(home.path())
        .args(["--json", "queue", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "--dry-run", "Is the deck ready?"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Is the deck ready?"), "{stdout}");
    let result = json_of(&output);
    assert_eq!(result["kind"], "dry_run");
    assert_eq!(
        result["argv"],
        serde_json::json!([
            codex,
            "queue",
            "--thread=ouroboros",
            "--message=<message>",
            "--remote=unix://",
        ])
    );
    assert_eq!(result["message_chars"], 18);
    assert!(!home.path().join("args.txt").exists(), "the harness ran");

    rune(home.path())
        .args(["queue", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "--dry-run", "Is the deck ready?"])
        .assert()
        .success()
        .stdout(predicate::str::contains("transport: codex queue"))
        .stdout(predicate::str::contains("--message=<message>"))
        .stdout(predicate::str::contains("Is the deck ready?").not());
}

#[test]
fn harness_without_a_queue_is_refused_before_anything_starts() {
    let home = tempfile::tempdir().expect("home");
    for route in ["sol@claude", "agy", "grok", "opencode", "ollama"] {
        let output = rune(home.path())
            .args(["--json", "queue", route, "--session", "ouroboros", "hi"])
            .output()
            .expect("queue output");
        assert_eq!(output.status.code(), Some(125), "{route}");
        let result = json_of(&output);
        assert_eq!(result["ok"], false, "{route}");
        assert_eq!(result["kind"], "unsupported_harness", "{route}");
        assert_eq!(result["details"]["supported"], serde_json::json!(["codex"]));
    }
}

#[cfg(unix)]
#[test]
fn harness_failure_passes_its_status_and_its_stderr_through() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), "", "Error: no session named ouroboros", 1);

    let output = rune(home.path())
        .args(["--json", "queue", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "Is the deck ready?"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(1));
    let result = json_of(&output);
    assert_eq!(result["kind"], "exit");
    assert!(
        result["details"]["stderr_tail"]
            .as_str()
            .is_some_and(|tail| tail.contains("no session named ouroboros")),
        "{result}"
    );
}

#[cfg(unix)]
#[test]
fn missing_receipt_still_reports_success_with_a_warning() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), "", "", 0);

    let output = rune(home.path())
        .args(["--json", "queue", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "Is the deck ready?"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(0));
    let result = json_of(&output);
    assert_eq!(result["kind"], "queued");
    assert!(result["message_id"].is_null(), "{result}");
    assert!(
        result["warnings"]
            .as_array()
            .is_some_and(|warnings| !warnings.is_empty()),
        "{result}"
    );
}

#[test]
fn usage_error_under_json_is_one_object_with_exit_125() {
    let home = tempfile::tempdir().expect("home");
    let output = rune(home.path())
        .args(["--json", "queue", "codex", "Is the deck ready?"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(125));
    let result = json_of(&output);
    assert_eq!(result["kind"], "configuration_error");
}
