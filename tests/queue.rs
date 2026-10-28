mod support;
use assert_cmd::Command;
use predicates::prelude::*;

/// Rune with its own HOME, so the postbox is `<home>/.rune/postbox.tsv`,
/// and outside any harness session unless a test sets one.
fn rune(home: &std::path::Path) -> Command {
    let mut command = support::rune();
    command
        .env("HOME", home)
        .env("USER", "owner")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CODEX_THREAD_ID");
    command
}

fn postbox(home: &std::path::Path) -> String {
    std::fs::read_to_string(home.join(".rune/postbox.tsv")).unwrap_or_default()
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
            "post",
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
        .args(["--json", "queue", "post", "codex", "--session", "ouroboros"])
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

    // The ledger records the message as posted, then queued, so a read for
    // the session never hands it out a second time.
    let id = result["id"].as_str().expect("an id");
    let states: Vec<String> = postbox(home.path())
        .lines()
        .filter(|line| line.contains(id))
        .map(|line| line.split('\t').nth(2).unwrap_or_default().to_string())
        .collect();
    assert_eq!(states, ["posted", "queued"]);
    rune(home.path())
        .args(["queue", "read", "--session", "ouroboros"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
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
        .args(["queue", "post", "deep@codex", "--session", "ouroboros"])
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
            "post",
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
        .args(["--json", "queue", "post", "codex", "--session", "ouroboros"])
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
        .args(["queue", "post", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "--dry-run", "Is the deck ready?"])
        .assert()
        .success()
        .stdout(predicate::str::contains("transport: codex queue"))
        .stdout(predicate::str::contains("--message=<message>"))
        .stdout(predicate::str::contains("Is the deck ready?").not());
}

#[test]
fn route_that_names_no_harness_is_refused_before_anything_starts() {
    let home = tempfile::tempdir().expect("home");
    let output = rune(home.path())
        .args([
            "--json",
            "queue",
            "post",
            "ollama",
            "--session",
            "seat",
            "hi",
        ])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(125));
    let result = json_of(&output);
    assert_eq!(result["kind"], "unsupported_harness");
    assert!(postbox(home.path()).is_empty(), "nothing was posted");
}

#[test]
fn message_waits_in_the_postbox_until_its_session_reads_it() {
    let home = tempfile::tempdir().expect("home");
    let output = rune(home.path())
        .args([
            "--json",
            "queue",
            "post",
            "sol@claude",
            "--session",
            "42e43c0b",
        ])
        .arg("Is the deck ready?\nSecond line.")
        .output()
        .expect("post output");
    assert_eq!(output.status.code(), Some(0));
    let posted = json_of(&output);
    assert_eq!(posted["kind"], "posted");
    assert_eq!(posted["transport"], "postbox");
    assert_eq!(posted["sender"], "user:owner");
    let id = posted["id"].as_str().expect("an id").to_string();

    let ledger = postbox(home.path());
    let lines: Vec<&str> = ledger.lines().collect();
    assert_eq!(lines.len(), 2, "the header and one event: {ledger}");
    assert!(lines[0].starts_with("# timestamp\tid\tstate"), "{ledger}");
    let columns: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(
        columns[1..],
        [
            id.as_str(),
            "posted",
            "user:owner",
            "claude",
            "42e43c0b",
            "Is the deck ready?\\nSecond line."
        ]
    );

    rune(home.path())
        .args(["queue", "read", "--session", "42e43c0b"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Message {id} from user:owner at"
        )))
        .stdout(predicate::str::contains("Is the deck ready?\nSecond line."));
    rune(home.path())
        .args(["queue", "read", "--session", "42e43c0b"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("no messages for 42e43c0b"));

    let ledger = postbox(home.path());
    assert_eq!(
        ledger.lines().count(),
        3,
        "the read is a new line: {ledger}"
    );
    assert!(ledger.contains(&format!("\t{id}\tread\tuser:owner\tclaude\t42e43c0b\t")));
}

#[test]
fn read_finds_its_own_session_and_the_sender_names_its_session() {
    let home = tempfile::tempdir().expect("home");
    rune(home.path())
        .env("CODEX_THREAD_ID", "01a0abff")
        .args([
            "queue",
            "post",
            "claude",
            "--session",
            "42e43c0b",
            "--postbox",
        ])
        .arg("From a Codex session")
        .assert()
        .success();
    let output = rune(home.path())
        .env("CLAUDE_CODE_SESSION_ID", "42e43c0b")
        .args(["--json", "queue", "read"])
        .output()
        .expect("read output");
    assert_eq!(output.status.code(), Some(0));
    let read = json_of(&output);
    assert_eq!(read["sessions"], serde_json::json!(["42e43c0b"]));
    assert_eq!(read["messages"][0]["sender"], "codex:01a0abff");
    assert_eq!(read["messages"][0]["message"], "From a Codex session");
}

#[test]
fn read_outside_a_session_needs_the_session_named() {
    let home = tempfile::tempdir().expect("home");
    let output = rune(home.path())
        .args(["--json", "queue", "read"])
        .output()
        .expect("read output");
    assert_eq!(output.status.code(), Some(125));
    assert_eq!(json_of(&output)["kind"], "configuration_error");
}

#[test]
fn list_shows_each_message_in_its_last_state_and_changes_nothing() {
    let home = tempfile::tempdir().expect("home");
    for (session, text) in [("seat-a", "first"), ("seat-b", "second")] {
        rune(home.path())
            .args([
                "queue",
                "post",
                "agy",
                "--session",
                session,
                "--from",
                "council",
            ])
            .arg(text)
            .assert()
            .success();
    }
    rune(home.path())
        .args(["queue", "read", "--session", "seat-a"])
        .assert()
        .success();
    let before = postbox(home.path());

    let output = rune(home.path())
        .args(["--json", "queue", "list"])
        .output()
        .expect("list output");
    let list = json_of(&output);
    let states: Vec<(&str, &str)> = list["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|message| {
            (
                message["session"].as_str().expect("session"),
                message["state"].as_str().expect("state"),
            )
        })
        .collect();
    assert_eq!(states, [("seat-a", "read"), ("seat-b", "posted")]);

    rune(home.path())
        .args(["queue", "list", "--state", "posted"])
        .assert()
        .success()
        .stdout(predicate::str::contains("council -> agy:seat-b  second"))
        .stdout(predicate::str::contains("seat-a").not());
    assert_eq!(postbox(home.path()), before, "list wrote nothing");
}

#[cfg(unix)]
#[test]
fn harness_failure_passes_its_status_and_its_stderr_through() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), "", "Error: no session named ouroboros", 1);

    let output = rune(home.path())
        .args(["--json", "queue", "post", "codex", "--session", "ouroboros"])
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
fn a_name_resolves_to_the_receipt_thread_and_its_turn_marks_it_read() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    rune(home.path())
        .args(["queue", "post", "codex", "--session", "ouroboros"])
        .args(["--binary", &codex, "Is the deck ready?"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: Codex looks the name ouroboros up page by page",
        ));

    // The turn the queue starts carries the message as its prompt.
    let payload = serde_json::json!({
        "v": 1,
        "harness": "codex",
        "event": "prompt.before",
        "native_event": "UserPromptSubmit",
        "session_id": "019a-ouroboros",
        "prompt": "Is the deck ready?",
        "raw": {},
    });
    let output = rune(home.path())
        .args(["hook", "adapter", "postbox"])
        .write_stdin(payload.to_string())
        .output()
        .expect("adapter output");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(json_of(&output)["effect"], "pass");

    let output = rune(home.path())
        .args(["--json", "queue", "list"])
        .output()
        .expect("list output");
    let message = &json_of(&output)["messages"][0];
    assert_eq!(message["session"], "019a-ouroboros", "{message}");
    assert_eq!(message["state"], "read", "{message}");
}

#[cfg(unix)]
#[test]
fn a_session_uuid_gets_no_name_warning() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), RECEIPT, "", 0);

    rune(home.path())
        .args(["queue", "post", "codex", "--session"])
        .args([
            "01a15844-7b2e-71a0-aa88-b41f3c4622af",
            "--binary",
            &codex,
            "hi",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("warning").not());
}

#[cfg(unix)]
#[test]
fn a_refused_name_hints_at_the_uuid_codex_matched() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(
        home.path(),
        "",
        "Error: Cannot verify a unique session label across server pages; matching session UUID: 01a15844-7b2e-71a0-aa88-b41f3c4622af",
        1,
    );
    let post = ["queue", "post", "codex", "--session", "Proton/ai-demo"];

    let output = rune(home.path())
        .arg("--json")
        .args(post)
        .args(["--binary", &codex, "hi"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json_of(&output)["hint"],
        "Codex matched the name to session 01a15844-7b2e-71a0-aa88-b41f3c4622af: post with --session 01a15844-7b2e-71a0-aa88-b41f3c4622af"
    );
    rune(home.path())
        .args(post)
        .args(["--binary", &codex, "hi"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "hint: Codex matched the name to session 01a15844-7b2e-71a0-aa88-b41f3c4622af",
        ));
    assert!(postbox(home.path()).is_empty(), "nothing was recorded");
}

#[cfg(unix)]
#[test]
fn missing_receipt_still_reports_success_with_a_warning() {
    let home = tempfile::tempdir().expect("home");
    let codex = fake_codex(home.path(), "", "", 0);

    let output = rune(home.path())
        .args(["--json", "queue", "post", "codex", "--session", "ouroboros"])
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
        .args(["--json", "queue", "post", "codex", "Is the deck ready?"])
        .output()
        .expect("queue output");
    assert_eq!(output.status.code(), Some(125));
    let result = json_of(&output);
    assert_eq!(result["kind"], "configuration_error");
}

#[test]
fn postbox_hook_adapter_adds_the_message_to_a_claude_prompt() {
    let home = tempfile::tempdir().expect("home");
    rune(home.path())
        .args([
            "queue",
            "post",
            "claude",
            "--session",
            "42e43c0b",
            "--from",
            "codex:01a0abff",
        ])
        .arg("Is the deck ready?")
        .assert()
        .success();

    let payload = serde_json::json!({
        "v": 1,
        "harness": "claude",
        "event": "prompt.before",
        "native_event": "UserPromptSubmit",
        "session_id": "42e43c0b",
        "raw": {},
    });
    let output = rune(home.path())
        .args(["hook", "adapter", "postbox"])
        .write_stdin(payload.to_string())
        .output()
        .expect("adapter output");
    assert_eq!(output.status.code(), Some(0));
    let result = json_of(&output);
    let context = result["context"][0].as_str().expect("context");
    assert!(context.contains("from codex:01a0abff"), "{result}");
    assert!(context.contains("Is the deck ready?"), "{result}");
    assert!(postbox(home.path()).contains("\tread\tcodex:01a0abff\tclaude\t42e43c0b\t"));
}

#[cfg(unix)]
#[test]
fn the_named_remote_token_never_shows_in_a_dry_run_or_a_failure() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().expect("home");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    codex:\n      remote:\n",
            "        env:\n          REMOTE_ACCESS: fake-remote-token-value\n",
        ),
    )
    .expect("config");
    let codex = home.path().join("fake-codex");
    std::fs::write(
        &codex,
        "#!/bin/sh\nprintf 'token %s refused\\n' \"$REMOTE_ACCESS\" >&2\nexit 1\n",
    )
    .expect("fake codex");
    std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755))
        .expect("fake codex permissions");
    let codex = codex.to_str().expect("fake codex path");
    let post = [
        "queue",
        "post",
        "remote@codex",
        "--session",
        "ouroboros",
        "--remote",
        "wss://host:4500",
        "--remote-auth-token-env",
        "REMOTE_ACCESS",
        "--binary",
    ];

    for json in [false, true] {
        let mut command = rune(home.path());
        if json {
            command.arg("--json");
        }
        let output = command
            .args(post)
            .args([codex, "--dry-run", "hi"])
            .output()
            .expect("dry run");
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("fake-remote-token-value"), "{stdout}");
        assert!(stdout.contains("<redacted>"), "{stdout}");
    }

    let output = rune(home.path())
        .arg("--json")
        .args(post)
        .args([codex, "hi"])
        .output()
        .expect("failed post");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("fake-remote-token-value"), "{stdout}");
    assert!(stdout.contains("token <redacted> refused"), "{stdout}");
}
