use assert_cmd::Command;
use predicates::prelude::*;

fn rune(home: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("rune").expect("rune binary");
    command
        .env("HOME", home)
        .env("CLIPROXY_API_KEY", "test-token");
    command
}

#[cfg(unix)]
fn executable_provider(home: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let provider = home.join(name);
    std::fs::write(&provider, source).expect("provider");
    let mut permissions = std::fs::metadata(&provider)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&provider, permissions).expect("provider permissions");
    provider
}

#[test]
fn fresh_install_runs_sol_profile_through_claude() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("tool: claude"))
        .stdout(predicate::str::contains("model_id: gpt-5.6-sol"))
        .stdout(predicate::str::contains(
            "ANTHROPIC_BASE_URL=http://127.0.0.1:8317",
        ))
        .stdout(predicate::str::contains("ANTHROPIC_AUTH_TOKEN=<redacted>"));
}

#[test]
fn fresh_install_runs_grok_profile_through_claude() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "grok@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("tool: claude"))
        .stdout(predicate::str::contains("model_id: grok-4.6"))
        .stdout(predicate::str::contains("model_context: 500000"))
        .stdout(predicate::str::contains(
            "ANTHROPIC_SMALL_FAST_MODEL=grok-composer-2.5-fast",
        ))
        .stdout(predicate::str::contains("timeout: none"));
}

#[test]
fn dry_run_reports_binary_and_model_overrides() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let binary = home.path().join("custom-claude");

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
            "--binary",
            binary.to_str().expect("binary path"),
            "--model",
            "custom-model",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "argv: {}",
            binary.display()
        )))
        .stdout(predicate::str::contains("model_id: custom-model"))
        .stdout(predicate::str::contains("model_source: argument"))
        .stdout(predicate::str::contains("model_id: gpt-5.6-sol").not());
}

#[cfg(unix)]
#[test]
fn json_reports_requested_and_resolved_routes() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf 'provider reply\\n'\n",
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["requested_route"], "sol@claude");
    assert_eq!(result["requested_provider"], "sol@claude");
    assert_eq!(result["resolved_route"], "claude");
    assert!(
        result.get("hint").is_none(),
        "a success never hints: {result}"
    );
}

#[cfg(unix)]
#[test]
fn a_model_answer_that_quotes_a_sandbox_denial_earns_no_hint_but_stderr_does() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let repo = repository.path().to_str().expect("repository path");

    let quoting = executable_provider(
        home.path(),
        "fake-claude-quoting",
        "#!/bin/sh\nprintf 'The command failed with Operation not permitted, as expected.\\n'\nexit 1\n",
    );
    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            quoting.to_str().expect("provider path"),
        ])
        .output()
        .expect("run output");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "exit");
    assert!(result["hint"].is_null(), "{result}");

    let denied = executable_provider(
        home.path(),
        "fake-claude-denied",
        "#!/bin/sh\nprintf 'open: Operation not permitted (os error 1)\\n' >&2\nexit 1\n",
    );
    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            denied.to_str().expect("provider path"),
        ])
        .output()
        .expect("run output");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert!(
        result["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("outer sandbox")),
        "{result}"
    );
}

#[cfg(unix)]
#[test]
fn json_keeps_total_usage_unknown_when_input_usage_is_unknown() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let provider = executable_provider(
        home.path(),
        "fake-agy",
        r#"#!/bin/sh
printf '%s\n' '{"response":"provider reply","usage":{"output_tokens":17}}'
"#,
    );

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "agy",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    let usage = result
        .get("usage")
        .and_then(serde_json::Value::as_object)
        .expect("usage object");
    let output_tokens = usage.get("output_tokens").expect("output_tokens key");
    let input_tokens = usage.get("input_tokens").expect("input_tokens key");
    let total_tokens = usage.get("total_tokens").expect("total_tokens key");

    assert_eq!(output_tokens.as_f64(), Some(17.0));
    assert!(input_tokens.is_null());
    assert!(total_tokens.is_null());
}

#[test]
fn dry_run_reports_the_session_name_and_the_flag_that_carries_it() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
            "--session-name",
            "claude/runedeck/mods/council",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "session_name: claude/runedeck/mods/council",
        ))
        .stdout(predicate::str::contains("session_name_flag: --name"));
}

#[test]
fn dry_run_without_a_session_name_says_none() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("session_name: none"))
        .stdout(predicate::str::contains("session_name_flag: --name"));
}

#[cfg(unix)]
#[test]
fn dry_run_reports_a_session_name_the_harness_cannot_take() {
    // agy has no flag for a session name: the plan says so, and nothing is sent.
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "agy",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
            "--mode",
            "workspace-write",
            "--session-name",
            "agy/runedeck/mods/council",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "session_name: agy/runedeck/mods/council",
        ))
        .stdout(predicate::str::contains("session_name_flag: none"));
}

#[test]
fn dry_run_reports_the_answer_only_mode() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
            "--mode",
            "answer-only",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("mode: answer-only"));
}

#[cfg(unix)]
#[test]
fn answer_only_run_starts_claude_with_no_tools() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/argv.txt"
printf 'answer reply\n'
"#,
    );

    rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "answer-only",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("answer reply"));

    let recorded = std::fs::read_to_string(home.path().join("argv.txt")).expect("recorded argv");
    let argv: Vec<&str> = recorded.lines().collect();
    assert!(
        argv.windows(2)
            .any(|pair| pair[0] == "--tools" && pair[1].is_empty()),
        "argv: {argv:?}"
    );
    assert!(!argv.contains(&"Read,Glob,Grep"), "argv: {argv:?}");
}

/// A fake harness that records its environment beside itself and answers.
#[cfg(unix)]
fn env_recording_provider(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    executable_provider(
        home,
        name,
        r#"#!/bin/sh
env > "$(dirname "$0")/env.txt"
printf 'ok\n'
"#,
    )
}

#[cfg(unix)]
fn recorded_env(home: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(home.join("env.txt"))
        .expect("recorded env")
        .lines()
        .map(str::to_string)
        .collect()
}

#[cfg(unix)]
fn has_key(env: &[String], key: &str) -> bool {
    env.iter().any(|line| {
        line.strip_prefix(key)
            .is_some_and(|rest| rest.starts_with('='))
    })
}

#[cfg(unix)]
#[test]
fn desktop_session_variables_do_not_reach_the_child() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = env_recording_provider(home.path(), "fake-claude");

    rune(home.path())
        .env("CLAUDECODE", "1")
        .env("CLAUDE_CODE_ENTRYPOINT", "cli")
        .env("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1")
        .env("CODEX_SANDBOX", "seatbelt")
        .env("CLAUDE_CONFIG_DIR", "/owner/claude")
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));

    let env = recorded_env(home.path());
    for key in [
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
        "CODEX_SANDBOX",
    ] {
        assert!(!has_key(&env, key), "{key} reached the child: {env:?}");
    }
    assert!(env.contains(&"CLAUDE_CONFIG_DIR=/owner/claude".to_string()));
    assert!(env.contains(&"ANTHROPIC_BASE_URL=http://127.0.0.1:8317".to_string()));
    assert!(env.contains(&"ANTHROPIC_MODEL=gpt-5.6-sol".to_string()));
    assert!(env.contains(&"ANTHROPIC_AUTH_TOKEN=test-token".to_string()));
}

#[cfg(unix)]
#[test]
fn native_claude_keeps_the_inherited_endpoint_and_credential() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = env_recording_provider(home.path(), "fake-claude");

    rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .env("ANTHROPIC_AUTH_TOKEN", "gateway-token")
        .args([
            "run",
            "claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .assert()
        .success();

    let env = recorded_env(home.path());
    assert!(env.contains(&"ANTHROPIC_BASE_URL=http://127.0.0.1:9/gateway".to_string()));
    assert!(env.contains(&"ANTHROPIC_AUTH_TOKEN=gateway-token".to_string()));
}

#[cfg(unix)]
#[test]
fn route_endpoint_without_a_credential_withholds_the_inherited_one() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = env_recording_provider(home.path(), "fake-claude");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        "launch:\n  profiles:\n    claude:\n      bare:\n        env:\n          ANTHROPIC_BASE_URL: http://127.0.0.1:9/bare\n",
    )
    .expect("config");

    rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .env("ANTHROPIC_AUTH_TOKEN", "gateway-token")
        .args([
            "run",
            "bare@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .assert()
        .success();

    let env = recorded_env(home.path());
    assert!(env.contains(&"ANTHROPIC_BASE_URL=http://127.0.0.1:9/bare".to_string()));
    assert!(
        !has_key(&env, "ANTHROPIC_AUTH_TOKEN"),
        "the inherited credential reached the route endpoint: {env:?}"
    );
}

#[cfg(unix)]
#[test]
fn failed_run_shows_what_the_harness_wrote_on_both_streams() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        r#"#!/bin/sh
printf '[claude-code:unrecognized_model] {"model":"gpt-5.6-sol"}\n' >&2
printf 'API Error: 400 {"error":{"message":"unknown model"}}\n'
exit 1
"#,
    );
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("process Exited(1)"))
        .stderr(predicate::str::contains("stdout tail:\n  API Error: 400"))
        .stderr(predicate::str::contains(
            "stderr tail:\n  [claude-code:unrecognized_model]",
        ));

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "exit");
    assert_eq!(result["details"]["exit_code"], 1);
    assert!(
        result["details"]["stdout_tail"]
            .as_str()
            .is_some_and(|tail| tail.starts_with("API Error: 400")),
        "{result}"
    );
    assert!(
        result["details"]["stderr_tail"]
            .as_str()
            .is_some_and(|tail| tail.contains("unrecognized_model")),
        "{result}"
    );
    assert!(result["hint"].is_null(), "{result}");
}

#[cfg(unix)]
#[test]
fn timed_out_run_reports_the_partial_output() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf 'partial answer'\nsleep 5\n",
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--timeout",
            "1s",
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(124));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "timeout");
    assert_eq!(result["details"]["timeout_ms"], 1000);
    assert_eq!(result["details"]["stdout_tail"], "partial answer");
}

#[test]
fn dry_run_json_is_one_object_with_the_model_id_and_the_planned_argv() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    let output = rune(home.path())
        .env("CLAUDECODE", "1")
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object");

    assert_eq!(result["ok"], true);
    assert_eq!(result["kind"], "dry_run");
    assert_eq!(result["tool"], "claude");
    assert_eq!(result["model"], "sol");
    assert_eq!(result["model_id"], "gpt-5.6-sol");
    assert_eq!(result["base_url"], "http://127.0.0.1:8317");
    assert_eq!(result["mode"], "read-only");
    assert!(result["timeout"].is_null());
    assert!(result["session_name"].is_null());
    assert_eq!(result["stdin"], "<prompt>");
    let argv: Vec<&str> = result["argv"]
        .as_array()
        .expect("argv array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(argv.contains(&"--print"), "{argv:?}");
    assert!(argv.contains(&"--allowedTools"), "{argv:?}");
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["--model", "gpt-5.6-sol"]),
        "{argv:?}"
    );
    assert!(
        !argv
            .iter()
            .any(|part| part.contains("Review this repository")),
        "{argv:?}"
    );
    assert_eq!(result["env"]["ANTHROPIC_AUTH_TOKEN"], "<redacted>");
    assert_eq!(result["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:8317");
    assert!(
        !output
            .stdout
            .windows(10)
            .any(|window| window == b"test-token")
    );
    let removed: Vec<&str> = result["env_removed"]
        .as_array()
        .expect("env_removed array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(removed.contains(&"CLAUDECODE"), "{removed:?}");
    assert!(result["repository"].as_str().is_some());
}

#[test]
fn dry_run_text_names_the_endpoint_from_the_environment_and_the_removed_keys() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .env("CLAUDECODE", "1")
        .env("CLAUDE_CODE_ENTRYPOINT", "cli")
        .args([
            "run",
            "sol@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("model_id: gpt-5.6-sol"))
        .stdout(predicate::str::contains("base_url: http://127.0.0.1:8317"))
        .stdout(predicate::str::contains("base_url: <none>").not())
        .stdout(predicate::str::contains(
            "--print --output-format text --permission-mode plan",
        ))
        .stdout(predicate::str::contains("stdin: <prompt>"))
        .stdout(predicate::str::contains("env_removed:\n"))
        .stdout(predicate::str::contains("\n  CLAUDECODE\n"))
        .stdout(predicate::str::contains("\n  CLAUDE_CODE_ENTRYPOINT\n"))
        .stdout(predicate::str::contains("Review this repository").not())
        .stdout(predicate::str::contains("test-token").not());
}

#[test]
fn dry_run_says_an_inherited_credential_was_withheld() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        "launch:\n  profiles:\n    claude:\n      bare:\n        env:\n          ANTHROPIC_BASE_URL: http://127.0.0.1:9/bare\n",
    )
    .expect("config");

    rune(home.path())
        .env("ANTHROPIC_AUTH_TOKEN", "gateway-token")
        .args([
            "run",
            "bare@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "ANTHROPIC_AUTH_TOKEN (inherited, withheld: the route sets ANTHROPIC_BASE_URL of the same family)",
        ))
        .stdout(predicate::str::contains("base_url: http://127.0.0.1:9/bare"))
        .stdout(predicate::str::contains("gateway-token").not());
}

#[test]
fn unknown_profile_with_json_is_one_object_that_lists_the_known_profiles() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "nope@claude",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object");
    assert_eq!(result["ok"], false);
    assert_eq!(result["kind"], "configuration_error");
    assert!(
        result["details"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("profiles: grok, sol")),
        "{result}"
    );
    assert_eq!(
        result["details"]["profiles"],
        serde_json::json!(["grok", "sol"])
    );
}

#[test]
fn a_usage_error_under_json_is_one_object_with_claps_exit_code() {
    let home = tempfile::tempdir().expect("home");
    for (label, args) in [
        (
            "bad mode",
            vec!["--json", "run", "sol@claude", "x", "--mode", "bogus"],
        ),
        (
            "dry-run with check",
            vec!["--json", "run", "sol@claude", "x", "--dry-run", "--check"],
        ),
        ("missing tool", vec!["--json", "run"]),
        (
            "json after the subcommand",
            vec!["run", "sol@claude", "x", "--json", "--mode", "bogus"],
        ),
    ] {
        let output = rune(home.path()).args(&args).output().expect("run output");
        assert_eq!(output.status.code(), Some(2), "{label}");
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
                panic!(
                    "{label}: stdout is not one JSON object ({error}): {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        assert_eq!(result["ok"], false, "{label}");
        assert_eq!(result["kind"], "configuration_error", "{label}");
        assert!(
            result["details"]["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty()),
            "{label}: {result}"
        );
    }
}

#[test]
fn help_keeps_its_own_output_under_json() {
    let home = tempfile::tempdir().expect("home");
    rune(home.path())
        .args(["--json", "run", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("configuration_error").not());
}

#[cfg(unix)]
#[test]
fn dry_run_of_a_prompt_in_argv_prints_the_placeholder() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "agy",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
            "--mode",
            "workspace-write",
        ])
        .assert()
        .success()
        // The read instruction comes first, then the prompt placeholder.
        .stdout(predicate::str::contains(
            "--print Read files with your file tools",
        ))
        .stdout(predicate::str::contains("<prompt> --output-format json"))
        .stdout(predicate::str::contains("--log-file <scratch>/agy.log"))
        .stdout(predicate::str::contains("stdin: none"))
        .stdout(predicate::str::contains("Review this repository").not());
}

#[cfg(unix)]
#[test]
fn clean_grok_run_starts_without_an_authentication_file() {
    // The temporary home has no ~/.grok/auth.json: the route authenticates
    // through its environment, and the clean run starts anyway.
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-grok",
        "#!/bin/sh\nprintf '%s\\n' '{\"text\":\"ok\"}'\n",
    );

    rune(home.path())
        .args([
            "run",
            "grok",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--clean",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"));
}

#[cfg(unix)]
#[test]
fn workspace_write_run_starts_claude_with_the_shell_in_the_sandbox() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/argv.txt"
printf 'built\n'
"#,
    );

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "Build this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("built"));

    let recorded = std::fs::read_to_string(home.path().join("argv.txt")).expect("recorded argv");
    let argv: Vec<&str> = recorded.lines().collect();
    let allowed = argv
        .windows(2)
        .find(|pair| pair[0] == "--allowedTools")
        .map(|pair| pair[1])
        .expect("an --allowedTools value");
    assert!(allowed.ends_with(",NotebookEdit"), "argv: {argv:?}");
    assert!(
        !allowed.split(',').any(|tool| tool == "Bash"),
        "Bash is allowed through the sandbox alone: {argv:?}"
    );
    let settings = argv
        .windows(2)
        .find(|pair| pair[0] == "--settings")
        .map(|pair| pair[1])
        .expect("one --settings value");
    let settings: serde_json::Value = serde_json::from_str(settings).expect("settings JSON");
    assert_eq!(settings["sandbox"]["enabled"], true);
    assert_eq!(settings["sandbox"]["failIfUnavailable"], true);
    assert_eq!(settings["sandbox"]["autoAllowBashIfSandboxed"], true);
    assert_eq!(settings["sandbox"]["allowUnsandboxedCommands"], false);
    assert_eq!(argv.iter().filter(|part| **part == "--settings").count(), 1);
}

#[cfg(unix)]
#[test]
fn failure_output_never_shows_a_credential_value() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        r#"#!/bin/sh
printf 'route %s inherited %s\n' "$ANTHROPIC_AUTH_TOKEN" "$INHERITED_API_KEY"
printf 'route %s inherited %s\n' "$ANTHROPIC_AUTH_TOKEN" "$INHERITED_API_KEY" >&2
exit 1
"#,
    );
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    for json in [false, true] {
        let mut command = rune(home.path());
        command.env("INHERITED_API_KEY", "fake-inherited-secret");
        if json {
            command.arg("--json");
        }
        let output = command
            .args([
                "run",
                "sol@claude",
                "Review",
                "--repo",
                repo,
                "--binary",
                binary,
            ])
            .output()
            .expect("run output");
        assert_eq!(output.status.code(), Some(1));
        let printed = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!printed.contains("test-token"), "{printed}");
        assert!(!printed.contains("fake-inherited-secret"), "{printed}");
        assert!(
            printed.contains("route <redacted> inherited <redacted>"),
            "{printed}"
        );
    }
}

#[test]
fn dry_run_never_shows_a_credential_inside_a_structured_value() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      secretive:\n        args:\n",
            "          - '--settings'\n",
            "          - '{\"env\":{\"MY_TOKEN\":\"fake-settings-secret\"},\"apiKeyHelper\":\"echo fake-helper-secret\",\"hooks\":{\"PreToolUse\":[{\"command\":\"fake-hook-secret\"}]}}'\n",
            "    codex:\n      secretive:\n        args:\n",
            "          - '-c'\n          - 'http_headers.Authorization=\"Bearer fake-header-secret\"'\n",
            "          - '-c'\n          - 'experimental_bearer_token=\"fake-bearer-secret\"'\n",
            "          - '-c'\n          - 'mcp_servers.x.env.TOKEN=\"fake-mcp-secret\"'\n",
        ),
    )
    .expect("config");
    let repo = repository.path().to_str().expect("repository path");

    for route in ["secretive@claude", "secretive@codex"] {
        for json in [false, true] {
            let mut command = rune(home.path());
            if json {
                command.arg("--json");
            }
            let output = command
                .args(["run", route, "Review", "--repo", repo, "--dry-run"])
                .output()
                .expect("run output");
            let printed = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{printed}");
            for secret in [
                "fake-settings-secret",
                "fake-helper-secret",
                "fake-hook-secret",
                "fake-header-secret",
                "fake-bearer-secret",
                "fake-mcp-secret",
            ] {
                assert!(
                    !printed.contains(secret),
                    "{route} json={json} leaked {secret}: {printed}"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn route_credential_or_settings_endpoint_withholds_the_inherited_family_keys() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = env_recording_provider(home.path(), "fake-claude");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n",
            "      tokened:\n        env:\n          ANTHROPIC_AUTH_TOKEN: fake-route-secret\n",
            "      settled:\n        args:\n          - '--settings'\n",
            "          - '{\"env\":{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:9/settings\"}}'\n",
        ),
    )
    .expect("config");
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    // A route credential: the inherited endpoint and the sibling credential leave.
    rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .env("ANTHROPIC_API_KEY", "fake-inherited-secret")
        .args([
            "run",
            "tokened@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .assert()
        .success();
    let env = recorded_env(home.path());
    assert!(env.contains(&"ANTHROPIC_AUTH_TOKEN=fake-route-secret".to_string()));
    assert!(!has_key(&env, "ANTHROPIC_BASE_URL"), "{env:?}");
    assert!(!has_key(&env, "ANTHROPIC_API_KEY"), "{env:?}");

    // An endpoint in the profile's settings env counts as route-set.
    rune(home.path())
        .env("ANTHROPIC_AUTH_TOKEN", "fake-inherited-secret")
        .args([
            "run",
            "settled@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .assert()
        .success();
    let env = recorded_env(home.path());
    assert!(!has_key(&env, "ANTHROPIC_AUTH_TOKEN"), "{env:?}");
}

#[test]
fn dry_run_endpoint_follows_the_effective_child_environment() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        "launch:\n  profiles:\n    claude:\n      tokened:\n        env:\n          ANTHROPIC_AUTH_TOKEN: fake-route-secret\n",
    )
    .expect("config");
    let repo = repository.path().to_str().expect("repository path");

    // A retained inherited endpoint is the one plain claude reaches.
    rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .args(["run", "claude", "x", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/gateway",
        ));
    let output = rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .args(["--json", "run", "claude", "x", "--repo", repo, "--dry-run"])
        .output()
        .expect("run output");
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object");
    assert_eq!(result["base_url"], "http://127.0.0.1:9/gateway");

    // A withheld inherited endpoint is not.
    rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .args(["run", "tokened@claude", "x", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("base_url: <none>"))
        .stdout(predicate::str::contains("fake-route-secret").not());

    // Codex and grok read their own keys.
    rune(home.path())
        .env("OPENAI_BASE_URL", "http://127.0.0.1:9/openai")
        .args(["run", "codex", "x", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/openai",
        ));
    rune(home.path())
        .env("GROK_MODELS_BASE_URL", "http://127.0.0.1:9/grok")
        .args(["run", "grok", "x", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/grok",
        ));

    // The launch dry run shows the same endpoint.
    rune(home.path())
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")
        .args(["launch", "claude", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/gateway",
        ));
}

#[cfg(unix)]
#[test]
fn codex_harness_errors_stay_apart_from_the_stream_tails() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-codex",
        r#"#!/bin/sh
printf '%s\n' '{"type":"error","message":"You have hit your usage limit."}'
printf 'a cache warning\n' >&2
exit 1
"#,
    );
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    let output = rune(home.path())
        .args([
            "--json", "run", "codex", "Review", "--repo", repo, "--binary", binary,
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "exit");
    assert_eq!(
        result["details"]["harness_errors"],
        serde_json::json!(["You have hit your usage limit."])
    );
    assert_eq!(result["details"]["stderr_tail"], "a cache warning");
    assert!(
        result["details"]["stdout_tail"]
            .as_str()
            .is_some_and(|tail| tail.starts_with("{\"type\":\"error\"")),
        "{result}"
    );

    rune(home.path())
        .args(["run", "codex", "Review", "--repo", repo, "--binary", binary])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "harness errors:\n  You have hit your usage limit.",
        ))
        .stderr(predicate::str::contains("stderr tail:\n  a cache warning"));
}

#[test]
fn dry_run_endpoint_prefers_the_settings_env_over_the_process_env() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      settled:\n        env:\n",
            "          ANTHROPIC_BASE_URL: http://127.0.0.1:9/process\n",
            "        args:\n          - '--settings'\n",
            "          - '{\"env\":{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:9/settings\"}}'\n",
        ),
    )
    .expect("config");
    let repo = repository.path().to_str().expect("repository path");

    rune(home.path())
        .args(["run", "settled@claude", "x", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/settings",
        ));
    rune(home.path())
        .args(["launch", "settled@claude", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/settings",
        ));
}

#[test]
fn dry_run_endpoint_prefers_the_settings_env_over_a_middleware_endpoint() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      piped:\n        with:\n          - pxpipe\n",
            "        args:\n          - '--settings'\n",
            "          - '{\"env\":{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:9/settings\"}}'\n",
        ),
    )
    .expect("config");
    let repo = repository.path().to_str().expect("repository path");

    // Claude applies the settings env inside its own process, over the
    // middleware endpoint the process env carries.
    rune(home.path())
        .args(["run", "piped@claude", "x", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/settings",
        ));
    rune(home.path())
        .args(["launch", "piped@claude", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "base_url: http://127.0.0.1:9/settings",
        ));
}

#[cfg(unix)]
#[test]
fn failure_tails_hide_a_header_value_and_a_bearer_token_quoted_alone() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      headed:\n        env:\n",
            "          ANTHROPIC_CUSTOM_HEADERS: 'Authorization: Bearer fake-bearer-token'\n",
            "          ANTHROPIC_AUTH_TOKEN: 'fake token with spaces'\n",
        ),
    )
    .expect("config");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        r"#!/bin/sh
printf 'token fake-bearer-token refused\n'
printf 'auth=fake token with spaces refused\n' >&2
exit 1
",
    );
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "headed@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(1));
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(!printed.contains("fake-bearer-token"), "{printed}");
    assert!(!printed.contains("with spaces"), "{printed}");
    assert!(printed.contains("token <redacted> refused"), "{printed}");
    assert!(printed.contains("auth=<redacted> refused"), "{printed}");
}

#[cfg(unix)]
#[test]
fn a_forwarded_sigterm_ends_the_run_promptly_when_a_setsid_descendant_holds_the_pipes() {
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      boxed:\n        env:\n",
            "          ANTHROPIC_BASE_URL: https://api.example.test\n",
        ),
    )
    .expect("config");
    // The harness reports the proxy's block, then a descendant in its own
    // session keeps every pipe open past the run, and the harness waits.
    // The marker tells the test the harness is up, so the signal reaches
    // a run that is supervising it.
    let marker = home.path().join("harness-started");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!(
            r#"#!/bin/sh
printf 'API Error: 403 {{"error":"Connection blocked by network allowlist","url":"https://api.example.test/v1/messages"}}\n'
if command -v setsid >/dev/null 2>&1; then setsid sleep 10 &
elif command -v python3 >/dev/null 2>&1; then python3 -c 'import os, time; os.setsid(); time.sleep(10)' &
else perl -e 'use POSIX; setsid(); sleep 10' & fi
sleep 0.2
touch '{}'
sleep 10
"#,
            marker.display()
        ),
    );
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    let started = Instant::now();
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_rune"))
        .env("HOME", home.path())
        .env("CLIPROXY_API_KEY", "test-token")
        .args([
            "--json",
            "run",
            "boxed@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("rune spawns");
    while !marker.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the harness never started"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let delivered = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(delivered.success(), "kill failed");
    let output = child.wait_with_output().expect("rune output");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(8),
        "the survivor held the run for {elapsed:?}"
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(143),
        "status {:?} after {elapsed:?}\nstdout: {printed}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(printed.trim()).expect("json result");
    assert_eq!(json["kind"], "signal", "{printed}");
    assert!(
        json["details"]["harness_errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| {
                error
                    .as_str()
                    .is_some_and(|error| error.contains("network allowlist"))
            })),
        "{printed}"
    );
    let hint = json["hint"].as_str().expect("hint");
    assert!(hint.contains("api.example.test"), "{printed}");
    assert!(hint.contains("allowlist"), "{printed}");
}

#[cfg(unix)]
#[test]
fn a_signal_during_the_drain_reports_the_stop_instead_of_a_cut_answer() {
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      boxed:\n        env:\n",
            "          ANTHROPIC_BASE_URL: https://api.example.test\n",
        ),
    )
    .expect("config");
    // The leader exits 0 at once while a group member holds stdout and
    // keeps writing. The member touches the marker after the leader is
    // gone, so the signal reaches a run that is draining, not supervising.
    let marker = home.path().join("leader-gone");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!(
            r"#!/bin/sh
printf 'partial answer sk-fake-drain-secret'
(sleep 0.3; touch '{}'; sleep 10; printf 'the rest') &
exit 0
",
            marker.display()
        ),
    );
    let repo = repository.path().to_str().expect("repository path");
    let binary = provider.to_str().expect("provider path");

    let started = Instant::now();
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_rune"))
        .env("HOME", home.path())
        .env("CLIPROXY_API_KEY", "sk-fake-drain-secret")
        .args([
            "--json",
            "run",
            "boxed@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("rune spawns");
    while !marker.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the leader never exited"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let delivered = std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(delivered.success(), "kill failed");
    let output = child.wait_with_output().expect("rune output");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(8),
        "the member held the run for {elapsed:?}"
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(130),
        "status {:?} after {elapsed:?}\nstdout: {printed}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(printed.trim()).expect("json result");
    assert_eq!(json["kind"], "signal", "{printed}");
    assert_eq!(json["details"]["signal"], 2, "{printed}");
    let tail = json["details"]["stdout_tail"]
        .as_str()
        .expect("stdout tail");
    assert!(tail.contains("partial answer"), "{printed}");
    assert!(!tail.contains("the rest"), "{printed}");
    assert!(!printed.contains("sk-fake-drain-secret"), "{printed}");
}

#[test]
fn dry_run_prints_a_model_name_that_equals_a_credential_value() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let config_dir = home.path().join(".config/rune");
    std::fs::create_dir_all(&config_dir).expect("config directory");
    std::fs::write(
        config_dir.join("config.yaml"),
        concat!(
            "launch:\n  profiles:\n    claude:\n      local:\n        env:\n",
            "          ANTHROPIC_AUTH_TOKEN: ollama\n",
            "          ANTHROPIC_BASE_URL: http://127.0.0.1:11434\n",
            "        args:\n          - '--name'\n          - 'ollama-review'\n",
        ),
    )
    .expect("config");
    let repo = repository.path().to_str().expect("repository path");

    rune(home.path())
        .args([
            "run",
            "local@claude",
            "x",
            "--repo",
            repo,
            "--dry-run",
            "--model",
            "qwen3-ollama:30b",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("--model qwen3-ollama:30b"))
        .stdout(predicate::str::contains("ANTHROPIC_AUTH_TOKEN=<redacted>"))
        .stdout(predicate::str::contains("ANTHROPIC_AUTH_TOKEN=ollama").not());
}

#[test]
fn launch_dry_run_json_is_the_runners_plan_object() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let repo = repository.path().to_str().expect("repository path");

    let launched = rune(home.path())
        .args(["--json", "launch", "sol@claude", "--dry-run"])
        .output()
        .expect("launch output");
    assert!(
        launched.status.success(),
        "{}",
        String::from_utf8_lossy(&launched.stderr)
    );
    let plan: serde_json::Value =
        serde_json::from_slice(&launched.stdout).expect("one JSON object");
    assert_eq!(plan["ok"], true);
    assert_eq!(plan["kind"], "dry_run");
    assert_eq!(plan["tool"], "claude");
    assert_eq!(plan["model"], "sol");
    assert_eq!(plan["model_id"], "gpt-5.6-sol");
    assert_eq!(plan["argv"][0], "claude");
    assert_eq!(plan["env"]["ANTHROPIC_AUTH_TOKEN"], "<redacted>");
    assert_eq!(plan["base_url"], "http://127.0.0.1:8317");
    assert!(plan["env_removed"].is_array(), "{plan}");
    assert!(plan["wrap"].is_array(), "{plan}");
    assert!(plan["pre"].is_array(), "{plan}");
    assert!(
        !launched
            .stdout
            .windows(10)
            .any(|window| window == b"test-token")
    );

    let ran = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "x",
            "--repo",
            repo,
            "--dry-run",
        ])
        .output()
        .expect("run output");
    let runner: serde_json::Value = serde_json::from_slice(&ran.stdout).expect("one JSON object");
    for field in [
        "kind",
        "tool",
        "model",
        "model_id",
        "model_context",
        "model_source",
        "base_url",
        "env",
        "env_removed",
        "inherited_withheld",
        "pre",
    ] {
        assert_eq!(plan[field], runner[field], "{field}");
    }
}

/// A fake agy that leaves an image and its bookkeeping in the data
/// directory the runner names, then answers with the conversation id.
#[cfg(unix)]
const FAKE_AGY_WITH_ARTIFACTS: &str = r#"#!/bin/sh
folder="$ANTIGRAVITY_EXECUTABLE_DATA_DIR/brain/conv-1"
mkdir -p "$folder/scratch" "$folder/.system_generated" "$folder/.user_uploaded"
printf 'jpeg' > "$folder/rune_icon.jpg"
printf '{}' > "$folder/rune_icon.jpg.metadata.json"
printf 'x' > "$folder/scratch/notes.txt"
printf 'x' > "$folder/.system_generated/plan.md"
printf 'x' > "$folder/.user_uploaded/in.png"
printf '%s\n' '{"response":"drawn","conversation_id":"conv-1"}'
"#;

/// Run the fake agy against `data` as its data directory, in
/// workspace-write mode (the one agy takes) on a jj repository. The state
/// directory is pinned to `state` so no test touches the real one.
#[cfg(unix)]
fn run_fake_agy(
    home: &std::path::Path,
    data: &std::path::Path,
    state: &std::path::Path,
    json: bool,
    extra: &[&str],
) -> std::process::Output {
    let repository = jj_repository(home);
    let provider = executable_provider(home, "fake-agy", FAKE_AGY_WITH_ARTIFACTS);
    let mut command = rune_isolated(home, state);
    command.env("ANTIGRAVITY_EXECUTABLE_DATA_DIR", data);
    if json {
        command.arg("--json");
    }
    command.args([
        "run",
        "agy",
        "Draw an icon",
        "--repo",
        repository.path().to_str().expect("repository path"),
        "--binary",
        provider.to_str().expect("provider path"),
        "--mode",
        "workspace-write",
    ]);
    command.args(extra);
    command.output().expect("run output")
}

#[cfg(unix)]
fn artifacts_of(output: &std::process::Output) -> Vec<String> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    result["artifacts"]
        .as_array()
        .expect("artifacts array")
        .iter()
        .map(|value| value.as_str().expect("artifact path").to_string())
        .collect()
}

#[cfg(unix)]
fn canonical(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .expect("canonical path")
        .to_string_lossy()
        .into_owned()
}

#[cfg(unix)]
#[test]
fn agy_run_reports_the_image_it_drew_and_skips_the_bookkeeping() {
    let home = tempfile::tempdir().expect("home");
    let data = tempfile::tempdir().expect("data");
    let state = tempfile::tempdir().expect("state");

    let output = run_fake_agy(home.path(), data.path(), state.path(), true, &[]);

    let image = data.path().join("brain/conv-1/rune_icon.jpg");
    assert_eq!(artifacts_of(&output), [canonical(&image)]);
}

#[cfg(unix)]
#[test]
fn agy_run_prints_one_artifact_line_on_stderr_and_the_answer_on_stdout() {
    let home = tempfile::tempdir().expect("home");
    let data = tempfile::tempdir().expect("data");
    let state = tempfile::tempdir().expect("state");

    let output = run_fake_agy(home.path(), data.path(), state.path(), false, &[]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "drawn\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("artifact: "))
        .collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(
        lines[0].ends_with("/brain/conv-1/rune_icon.jpg"),
        "{stderr}"
    );
    assert!(!stderr.contains("metadata.json"), "{stderr}");
    assert!(!stderr.contains("scratch"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn artifacts_dir_copies_the_image_and_never_overwrites_a_clash() {
    let home = tempfile::tempdir().expect("home");
    let data = tempfile::tempdir().expect("data");
    let state = tempfile::tempdir().expect("state");
    let out = tempfile::tempdir().expect("out");
    std::fs::write(out.path().join("rune_icon.jpg"), "first").expect("first file");

    let output = run_fake_agy(
        home.path(),
        data.path(),
        state.path(),
        true,
        &["--artifacts-dir", out.path().to_str().expect("out path")],
    );

    let copy = out.path().join("rune_icon-1.jpg");
    assert_eq!(artifacts_of(&output), [canonical(&copy)]);
    assert_eq!(std::fs::read_to_string(&copy).expect("copy"), "jpeg");
    assert_eq!(
        std::fs::read_to_string(out.path().join("rune_icon.jpg")).expect("first file"),
        "first"
    );
}

#[cfg(unix)]
#[test]
fn clean_agy_run_keeps_the_image_under_the_state_directory() {
    let home = tempfile::tempdir().expect("home");
    let data = tempfile::tempdir().expect("data");
    let state = tempfile::tempdir().expect("state");
    // The clean state bridges this fake token; nothing under the real home
    // is read.
    std::fs::write(data.path().join("antigravity-oauth-token"), "fake-token").expect("token");

    let output = run_fake_agy(home.path(), data.path(), state.path(), true, &["--clean"]);

    let kept = state.path().join("rune/artifacts/conv-1/rune_icon.jpg");
    assert_eq!(artifacts_of(&output), [canonical(&kept)]);
    assert_eq!(std::fs::read_to_string(&kept).expect("kept copy"), "jpeg");
    // The fake wrote into the clean root, which is gone, not into the source.
    assert!(!data.path().join("brain").exists());
}

#[cfg(unix)]
#[test]
fn grok_run_reports_no_artifacts() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-grok",
        "#!/bin/sh\nprintf '%s\\n' '{\"text\":\"ok\"}'\n",
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "grok",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .output()
        .expect("run output");

    assert_eq!(artifacts_of(&output), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn dry_run_names_the_artifacts_destination() {
    let home = tempfile::tempdir().expect("home");
    let repository = jj_repository(home.path());
    let state = tempfile::tempdir().expect("state");
    let repo = repository.path().to_str().expect("repository path");

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "agy",
            "Draw an icon",
            "--repo",
            repo,
            "--dry-run",
            "--mode",
            "workspace-write",
            "--artifacts-dir",
            "out",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("artifacts_dir: out\n"));

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "agy",
            "Draw an icon",
            "--repo",
            repo,
            "--dry-run",
            "--mode",
            "workspace-write",
            "--clean",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "artifacts_dir: {}/rune/artifacts/<conversation-id>\n",
            state.path().display()
        )));

    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Draw an icon",
            "--repo",
            repo,
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("artifacts_dir: none\n"));
}

/// A jj command under the test's home, with an identity so jj prints no
/// configuration warning. The test fails on a jj failure.
#[cfg(unix)]
fn jj(home: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("jj")
        .env("HOME", home)
        .env("JJ_USER", "Test Runner")
        .env("JJ_EMAIL", "runner@example.test")
        .args(args)
        .output()
        .expect("jj runs");
    assert!(
        output.status.success(),
        "jj {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A colocated jj repository (jj beside a git checkout, as `jj git init`
/// makes one by default) with one tracked file, `README.md`, in the
/// caller's working-copy change.
#[cfg(unix)]
fn jj_repository(home: &std::path::Path) -> tempfile::TempDir {
    jj_repository_with(home, &["git", "init"])
}

/// A jj repository without a git checkout.
#[cfg(unix)]
fn plain_jj_repository(home: &std::path::Path) -> tempfile::TempDir {
    jj_repository_with(home, &["--config", "git.colocate=false", "git", "init"])
}

#[cfg(unix)]
fn jj_repository_with(home: &std::path::Path, init: &[&str]) -> tempfile::TempDir {
    let repository = tempfile::tempdir().expect("repository");
    let path = repository.path().to_str().expect("repository path");
    let mut args = init.to_vec();
    args.push(path);
    jj(home, &args);
    std::fs::write(repository.path().join("README.md"), "base\n").expect("README");
    jj(home, &["-R", path, "util", "snapshot"]);
    repository
}

/// `rune` with its state directory pinned under `state`, so a run's
/// workspaces never touch the real one, and with a jj identity for the jj
/// commands the run makes.
#[cfg(unix)]
fn rune_isolated(home: &std::path::Path, state: &std::path::Path) -> Command {
    let mut command = rune(home);
    command
        .env("XDG_STATE_HOME", state)
        .env_remove("RUNE_STATE_DIR")
        .env("JJ_USER", "Test Runner")
        .env("JJ_EMAIL", "runner@example.test");
    command
}

/// The workspace folders left under the state directory, if any.
#[cfg(unix)]
fn leftover_workspaces(state: &std::path::Path) -> Vec<String> {
    let Ok(repositories) = std::fs::read_dir(state.join("rune/workspaces")) else {
        return Vec::new();
    };
    repositories
        .flatten()
        .flat_map(|repository| std::fs::read_dir(repository.path()).into_iter().flatten())
        .flatten()
        .map(|entry| entry.path().display().to_string())
        .collect()
}

/// The files one change touches, as `jj diff --summary` reports them:
/// the status letter and the file name.
#[cfg(unix)]
fn changed_files(home: &std::path::Path, repository: &str, change_id: &str) -> Vec<String> {
    jj(
        home,
        &["-R", repository, "diff", "-r", change_id, "--summary"],
    )
    .lines()
    .filter_map(|line| {
        let (status, path) = line.split_once(' ')?;
        let name = std::path::Path::new(path.trim()).file_name()?;
        Some(format!("{status} {}", name.to_string_lossy()))
    })
    .collect()
}

/// The names in `jj workspace list`.
#[cfg(unix)]
fn workspace_names(home: &std::path::Path, repository: &str) -> Vec<String> {
    jj(home, &["-R", repository, "workspace", "list"])
        .lines()
        .filter_map(|line| {
            line.split_once(':')
                .map(|(name, _)| name.trim().to_string())
        })
        .collect()
}

/// Every visible change id in the repository, in log order.
#[cfg(unix)]
fn change_ids(home: &std::path::Path, repository: &str) -> Vec<String> {
    jj(
        home,
        &[
            "-R",
            repository,
            "log",
            "-r",
            "all()",
            "--no-graph",
            "-T",
            "change_id ++ \"\\n\"",
        ],
    )
    .lines()
    .map(str::to_string)
    .collect()
}

/// A fake harness that edits one file, creates another, and records the
/// directory it worked in beside itself.
#[cfg(unix)]
const FAKE_EDITOR: &str = r#"#!/bin/sh
pwd -P > "$(dirname "$0")/cwd.txt"
printf 'edited\n' > README.md
printf 'new\n' > NEW.md
printf 'done\n'
"#;

#[cfg(unix)]
#[test]
fn workspace_write_run_edits_its_own_change_and_leaves_the_checkout_alone() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let provider = executable_provider(home.path(), "fake-claude", FAKE_EDITOR);
    let parent = change_ids(home.path(), repo);

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Edit the README",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
            "--session-name",
            "council/sol",
        ])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["text"], "done");
    assert_eq!(result["workspace"]["kept"], true, "{result}");
    assert_eq!(result["workspace"]["conflicted"], false, "{result}");
    assert_eq!(result["workspace"]["colocated"], false, "{result}");
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id")
        .to_string();
    let name = result["workspace"]["name"].as_str().expect("name");
    assert!(name.starts_with("run-"), "{result}");
    // The run's change is the one addition: the copy it started from is gone.
    let after = change_ids(home.path(), repo);
    assert_eq!(after.len(), parent.len() + 1, "{after:?}");
    assert!(after.contains(&change_id), "{after:?}");

    // The caller's checkout is untouched.
    assert_eq!(
        std::fs::read_to_string(repository.path().join("README.md")).expect("README"),
        "base\n"
    );
    assert!(!repository.path().join("NEW.md").exists());

    // The harness worked under the state directory, which holds nothing now.
    let cwd = std::fs::read_to_string(home.path().join("cwd.txt")).expect("recorded cwd");
    assert!(
        cwd.contains("/rune/workspaces/"),
        "the harness ran in {cwd}"
    );
    assert!(!std::path::Path::new(cwd.trim()).exists(), "{cwd} remains");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);

    // One change holds both edits, named after the route and the session,
    // as a child of the caller's working-copy change.
    let mut changed = changed_files(home.path(), repo, &change_id);
    changed.sort();
    assert_eq!(changed, ["A NEW.md", "M README.md"]);
    let description = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            &change_id,
            "--no-graph",
            "-T",
            "description",
        ],
    );
    assert!(description.contains("sol@claude"), "{description}");
    assert!(description.contains("council/sol"), "{description}");
    let parents = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            &format!("{change_id}-"),
            "--no-graph",
            "-T",
            "change_id ++ \"\\n\"",
        ],
    );
    assert_eq!(parents.trim(), parent[0], "{parents}");
}

#[cfg(unix)]
#[test]
fn workspace_write_run_that_changes_nothing_leaves_no_trace() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    // A jj repository without git: the workspace is no git worktree.
    let repository = plain_jj_repository(home.path());
    assert!(!repository.path().join(".git").exists());
    let repo = repository.path().to_str().expect("repository path");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf 'looked only\\n'\n",
    );
    let before = change_ids(home.path(), repo);

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Look around",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["workspace"]["kept"], false, "{result}");
    assert!(result["workspace"]["change_id"].is_string(), "{result}");

    assert_eq!(change_ids(home.path(), repo), before);
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "Look around",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .assert()
        .success()
        .stdout("looked only\n")
        .stderr(predicate::str::contains("workspace: no file changed"));
}

#[cfg(unix)]
#[test]
fn timed_out_workspace_write_run_keeps_its_partial_edit() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf 'partial\\n' > partial.md\nprintf 'working'\nsleep 5\n",
    );

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Write partial.md",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
            "--timeout",
            "1s",
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(124));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "timeout");
    assert_eq!(result["workspace"]["kept"], true, "{result}");
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id");
    assert_eq!(
        changed_files(home.path(), repo, change_id),
        ["A partial.md"]
    );
    assert!(!repository.path().join("partial.md").exists());
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn dry_run_of_a_workspace_write_route_names_the_workspace_and_the_parent() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let parent = change_ids(home.path(), repo);

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "Edit the README",
            "--repo",
            repo,
            "--dry-run",
            "--mode",
            "workspace-write",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "workspace: {}/rune/workspaces/",
            state.path().display()
        )))
        // The harness would start in the workspace, so the plan says so.
        .stdout(predicate::str::contains(format!(
            "repository: {}/rune/workspaces/",
            state.path().display()
        )))
        .stdout(predicate::str::contains(format!(
            "workspace_parent: {}\n",
            parent[0]
        )))
        .stdout(predicate::str::contains("workspace_colocate: false\n"));

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Edit the README",
            "--repo",
            repo,
            "--dry-run",
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(
        result["workspace"]["parent_change_id"], parent[0],
        "{result}"
    );
    assert_eq!(result["workspace"]["colocate"], false, "{result}");
    assert!(
        result["workspace"]["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("run-")),
        "{result}"
    );

    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert!(!state.path().join("rune/workspaces").exists());

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "Look",
            "--repo",
            repo,
            "--dry-run",
            "--mode",
            "read-only",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("workspace: none\n"));
}

#[cfg(unix)]
#[test]
fn dry_run_of_a_subfolder_route_names_the_workspace_in_the_harness_argv() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    // A harness that takes its folder as an argument gets the workspace
    // path there, at the subfolder that matches --repo.
    let subfolder = repository.path().join("crates");
    std::fs::create_dir_all(&subfolder).expect("subfolder");

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "codex",
            "Edit the crate",
            "--repo",
            subfolder.to_str().expect("subfolder path"),
            "--dry-run",
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    let argv: Vec<&str> = result["argv"]
        .as_array()
        .expect("argv array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let cwd = argv
        .windows(2)
        .find(|pair| pair[0] == "-C")
        .map(|pair| pair[1])
        .expect("a -C value");
    assert!(
        cwd.contains("/rune/workspaces/") && cwd.ends_with("/crates"),
        "{argv:?}"
    );
    assert!(!cwd.starts_with(repo), "{argv:?}");
    assert_eq!(result["repository"], cwd, "{result}");
    assert!(!state.path().join("rune/workspaces").exists());
}

#[cfg(unix)]
#[test]
fn git_repository_without_jj_is_colocated_before_an_isolated_run() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = tempfile::tempdir().expect("repository");
    let repo = repository.path().to_str().expect("repository path");
    for args in [
        vec!["init", "-q", repo],
        vec!["-C", repo, "commit", "-q", "--allow-empty", "-m", "base"],
    ] {
        let status = std::process::Command::new("git")
            .env("HOME", home.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.test")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.test")
            .args(&args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }
    std::fs::write(repository.path().join("README.md"), "base\n").expect("README");
    let provider = executable_provider(home.path(), "fake-claude", FAKE_EDITOR);

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Edit the README",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["workspace"]["colocated"], true, "{result}");
    assert_eq!(result["workspace"]["kept"], true, "{result}");
    assert!(repository.path().join(".jj").is_dir());
    assert_eq!(
        std::fs::read_to_string(repository.path().join("README.md")).expect("README"),
        "base\n"
    );
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    // The git side is clean too: no worktree and no branch from the run.
    for (args, expected_lines) in [
        (vec!["-C", repo, "worktree", "list"], 1),
        (vec!["-C", repo, "branch", "--list", "jj-worktree-*"], 0),
    ] {
        let printed = String::from_utf8_lossy(
            &std::process::Command::new("git")
                .args(args)
                .output()
                .expect("git runs")
                .stdout,
        )
        .into_owned();
        assert_eq!(printed.lines().count(), expected_lines, "{printed}");
    }

    // The second run finds jj in place and says nothing about colocation.
    let output = rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "Edit the README",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("colocated"), "{stderr}");
    assert!(stderr.contains("workspace: kept change "), "{stderr}");
}

#[cfg(unix)]
#[test]
fn folder_without_version_control_refuses_a_workspace_write_run() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let folder = tempfile::tempdir().expect("folder");
    let marker = home.path().join("harness-started");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!("#!/bin/sh\ntouch '{}'\nprintf 'ran\\n'\n", marker.display()),
    );
    let path = folder.path().to_str().expect("folder path");
    let binary = provider.to_str().expect("provider path");

    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "Edit",
            "--repo",
            path,
            "--binary",
            binary,
            "--mode",
            "workspace-write",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--mode read-only"))
        .stderr(predicate::str::contains("jj git init --colocate"));
    assert!(!marker.exists(), "the harness started");

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Edit",
            "--repo",
            path,
            "--binary",
            binary,
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "configuration_error");
    assert!(!marker.exists(), "the harness started");
}

/// Start one seat of a council in the background, with the state
/// directory pinned under `state`.
#[cfg(unix)]
fn spawn_seat(
    home: &std::path::Path,
    state: &std::path::Path,
    repo: &str,
    binary: &str,
    seat: &str,
) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_rune"))
        .env("HOME", home)
        .env("CLIPROXY_API_KEY", "test-token")
        .env("XDG_STATE_HOME", state)
        .env_remove("RUNE_STATE_DIR")
        .env("JJ_USER", "Test Runner")
        .env("JJ_EMAIL", "runner@example.test")
        .env("SEAT", seat)
        .args([
            "--json",
            "run",
            "sol@claude",
            "Write your seat file",
            "--repo",
            repo,
            "--binary",
            binary,
            "--mode",
            "workspace-write",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("rune spawns")
}

#[cfg(unix)]
#[test]
fn two_isolated_runs_at_once_each_keep_their_own_change() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nsleep 1\nprintf '%s\\n' \"$SEAT\" > \"seat-$SEAT.md\"\nprintf 'seat %s\\n' \"$SEAT\"\n",
    );
    let binary = provider.to_str().expect("provider path");
    let parent = change_ids(home.path(), repo);

    let children: Vec<std::process::Child> = ["a", "b"]
        .iter()
        .map(|seat| spawn_seat(home.path(), state.path(), repo, binary, seat))
        .collect();
    let mut ids = Vec::new();
    for child in children {
        let output = child.wait_with_output().expect("rune output");
        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
        assert_eq!(result["workspace"]["kept"], true, "{result}");
        ids.push(
            result["workspace"]["change_id"]
                .as_str()
                .expect("change id")
                .to_string(),
        );
    }
    assert_ne!(ids[0], ids[1]);
    for id in &ids {
        let parents = jj(
            home.path(),
            &[
                "-R",
                repo,
                "log",
                "-r",
                &format!("{id}-"),
                "--no-graph",
                "-T",
                "change_id ++ \"\\n\"",
            ],
        );
        assert_eq!(parents.trim(), parent[0], "{parents}");
    }
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert!(!repository.path().join("seat-a.md").exists());
    assert!(!repository.path().join("seat-b.md").exists());
    // Two run changes and nothing else: no copy survives a run.
    assert_eq!(change_ids(home.path(), repo).len(), parent.len() + 2);
}

#[cfg(unix)]
#[test]
fn caller_keeps_working_while_a_seat_runs_and_a_second_seat_starts() {
    use std::time::Duration;

    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    // Seat a works for a while; seat b is quick. The caller writes its brief
    // and snapshots the checkout between their starts, which rewrites the
    // caller's working-copy change under seat a.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf '%s\\n' \"$SEAT\" > \"seat-$SEAT.md\"\n[ \"$SEAT\" = a ] && touch \"$HOME/seat-a-started\" && sleep 3\nprintf 'seat %s\\n' \"$SEAT\"\n",
    );
    let binary = provider.to_str().expect("provider path");
    let before = change_ids(home.path(), repo);

    let seat_a = spawn_seat(home.path(), state.path(), repo, binary, "a");
    // Seat a's harness marks its start, so the caller's edit comes after
    // seat a recorded the caller's commits.
    let started = std::time::Instant::now();
    while !home.path().join("seat-a-started").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "seat a never started"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    std::fs::write(
        repository.path().join("brief.md"),
        "the caller writes its brief\n",
    )
    .expect("brief");
    jj(home.path(), &["-R", repo, "status"]);
    let seat_b = spawn_seat(home.path(), state.path(), repo, binary, "b");

    let mut ids = Vec::new();
    for (seat, child) in [("a", seat_a), ("b", seat_b)] {
        let output = child.wait_with_output().expect("rune output");
        assert!(
            output.status.success(),
            "seat {seat}: stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
        assert_eq!(result["workspace"]["kept"], true, "seat {seat}: {result}");
        assert_eq!(
            result["workspace"]["conflicted"], false,
            "seat {seat}: {result}"
        );
        // The caller's edit and snapshot under seat a rewrote its
        // working-copy commit, which is the caller's own work and no
        // outside change: the flag stays off for both seats.
        assert_eq!(
            result["workspace"]["outside_changes"], false,
            "seat {seat}: {result}"
        );
        let id = result["workspace"]["change_id"]
            .as_str()
            .expect("change id")
            .to_string();
        assert_eq!(
            changed_files(home.path(), repo, &id),
            [format!("A seat-{seat}.md")]
        );
        ids.push(id);
    }
    assert_ne!(ids[0], ids[1]);
    // Both land on the caller's current change, the one with the brief.
    let current = change_ids(home.path(), repo);
    let caller = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "change_id",
        ],
    );
    for id in &ids {
        let parents = jj(
            home.path(),
            &[
                "-R",
                repo,
                "log",
                "-r",
                &format!("{id}-"),
                "--no-graph",
                "-T",
                "change_id ++ \"\\n\"",
            ],
        );
        assert_eq!(parents.trim(), caller.trim(), "{parents}");
    }
    assert_eq!(current.len(), before.len() + 2, "{current:?}");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
    assert_eq!(
        std::fs::read_to_string(repository.path().join("brief.md")).expect("brief"),
        "the caller writes its brief\n"
    );
}

#[cfg(unix)]
#[test]
fn harness_that_commits_its_own_work_still_leaves_one_change() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let fakes = [
        (
            "fake-jj-committer",
            "#!/bin/sh\nprintf 'harness\\n' > h.txt\njj commit -m 'harness work' >/dev/null 2>&1\nprintf 'done\\n'\n",
        ),
        (
            "fake-git-committer",
            "#!/bin/sh\nprintf 'harness\\n' > h.txt\ngit add h.txt\ngit -c user.name=t -c user.email=t@example.test -c commit.gpgsign=false commit -qm 'harness git work'\nprintf 'done\\n'\n",
        ),
    ];
    for (name, script) in fakes {
        let before = change_ids(home.path(), repo);
        let provider = executable_provider(home.path(), name, script);
        let output = rune_isolated(home.path(), state.path())
            .args([
                "--json",
                "run",
                "sol@claude",
                "Commit your work",
                "--repo",
                repo,
                "--binary",
                provider.to_str().expect("provider path"),
                "--mode",
                "workspace-write",
            ])
            .output()
            .expect("run output");
        assert!(
            output.status.success(),
            "{name}: stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
        assert_eq!(result["workspace"]["kept"], true, "{name}: {result}");
        let change_id = result["workspace"]["change_id"]
            .as_str()
            .expect("change id")
            .to_string();
        assert_eq!(
            changed_files(home.path(), repo, &change_id),
            ["A h.txt"],
            "{name}"
        );
        // One change more, and nothing empty or orphaned from the run.
        let after = change_ids(home.path(), repo);
        assert_eq!(after.len(), before.len() + 1, "{name}: {after:?}");
        let empties = jj(
            home.path(),
            &[
                "-R",
                repo,
                "log",
                "-r",
                "mutable() & empty() & ~@",
                "--no-graph",
                "-T",
                "change_id ++ \"\\n\"",
            ],
        );
        assert_eq!(empties.trim(), "", "{name}: empty changes remain");
        assert_eq!(workspace_names(home.path(), repo), ["default"]);
        assert!(!repository.path().join("h.txt").exists());
    }
}

#[cfg(unix)]
#[test]
fn a_large_new_file_is_recorded_and_ignored_output_goes_with_the_folder() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    std::fs::write(repository.path().join(".gitignore"), "dist/\n").expect(".gitignore");
    jj(home.path(), &["-R", repo, "util", "snapshot"]);
    let before = change_ids(home.path(), repo);

    // Twice jj's default limit for a new file.
    let big = executable_provider(
        home.path(),
        "fake-big",
        "#!/bin/sh\nhead -c 2000000 /dev/zero > big.bin\nprintf 'done\\n'\n",
    );
    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Write a big file",
            "--repo",
            repo,
            "--binary",
            big.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["workspace"]["kept"], true, "{result}");
    assert_eq!(
        result["workspace"]["unrecorded"],
        serde_json::json!([]),
        "{result}"
    );
    assert!(result["workspace"].get("path").is_none(), "{result}");
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id");
    assert_eq!(changed_files(home.path(), repo, change_id), ["A big.bin"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());

    // Output under an ignored folder only: nothing to keep, and the folder
    // goes with it.
    let builder = executable_provider(
        home.path(),
        "fake-builder",
        "#!/bin/sh\nmkdir -p dist\nprintf 'out\\n' > dist/report.html\nprintf 'built\\n'\n",
    );
    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Build",
            "--repo",
            repo,
            "--binary",
            builder.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["workspace"]["kept"], false, "{result}");
    assert_eq!(
        result["workspace"]["unrecorded"],
        serde_json::json!([]),
        "{result}"
    );
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
    assert_eq!(change_ids(home.path(), repo).len(), before.len() + 1);
}

#[cfg(unix)]
#[test]
fn finish_error_after_the_change_is_known_still_names_it() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    // The harness edits a file, then makes the folder above its workspace
    // read-only: the fold works, and the last step, removing the folder,
    // fails.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\npwd -P > \"$(dirname \"$0\")/cwd.txt\"\nprintf 'new\\n' > NEW.md\nchmod 555 \"$(dirname \"$(pwd -P)\")\"\nprintf 'done\\n'\n",
    );

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Edit",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    let cwd = std::fs::read_to_string(home.path().join("cwd.txt")).expect("recorded cwd");
    let folder = std::path::PathBuf::from(cwd.trim());
    let parent = folder.parent().expect("parent").to_path_buf();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    let workspace = &result["workspace"];
    assert_eq!(workspace["kept"], true, "{result}");
    let change_id = workspace["change_id"].as_str().expect("change id");
    assert_eq!(changed_files(home.path(), repo, change_id), ["A NEW.md"]);
    assert!(
        workspace["error"]
            .as_str()
            .is_some_and(|error| error.contains("cannot remove workspace folder")),
        "{result}"
    );
    let reported = std::path::PathBuf::from(workspace["path"].as_str().expect("path"));
    assert_eq!(canonical(&reported), canonical(&folder), "{result}");
    assert!(
        folder.exists(),
        "the folder went although the removal failed"
    );
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn artifact_copy_failure_after_the_run_keeps_the_workspace_object() {
    let home = tempfile::tempdir().expect("home");
    let data = tempfile::tempdir().expect("data");
    let state = tempfile::tempdir().expect("state");
    // A regular file where the artifacts directory should go: the copy
    // cannot create it, after the workspace was finished.
    let blocker = home.path().join("blocker");
    std::fs::write(&blocker, "not a folder").expect("blocker");
    let out = blocker.join("out");

    let output = run_fake_agy(
        home.path(),
        data.path(),
        state.path(),
        true,
        &["--artifacts-dir", out.to_str().expect("out path")],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "configuration_error", "{result}");
    assert!(
        result["details"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("artifacts directory")),
        "{result}"
    );
    assert!(result["workspace"]["change_id"].is_string(), "{result}");
    assert_eq!(result["workspace"]["kept"], false, "{result}");
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn a_sigterm_during_an_isolated_run_still_finishes_the_workspace() {
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let marker = home.path().join("harness-started");
    // The harness edits a file, says it is up, then waits to be stopped.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!(
            "#!/bin/sh\nprintf 'partial\\n' > partial.md\ntouch '{}'\nsleep 10\n",
            marker.display()
        ),
    );

    let started = Instant::now();
    let child = spawn_seat(
        home.path(),
        state.path(),
        repo,
        provider.to_str().expect("provider path"),
        "stopped",
    );
    while !marker.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the harness never started"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let delivered = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(delivered.success(), "kill failed");
    let output = child.wait_with_output().expect("rune output");
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(143),
        "stdout: {printed}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_str(printed.trim()).expect("run JSON");
    assert_eq!(result["kind"], "signal", "{printed}");
    assert_eq!(result["workspace"]["kept"], true, "{printed}");
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id");
    assert_eq!(
        changed_files(home.path(), repo, change_id),
        ["A partial.md"]
    );
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
    assert!(!repository.path().join("partial.md").exists());
}

#[cfg(unix)]
#[test]
fn a_state_directory_inside_the_checkout_refuses_the_isolated_run() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let marker = home.path().join("harness-started");
    let provider = marking_provider(home.path(), "fake-claude", "ran");
    let binary = provider.to_str().expect("provider path");
    // The state directory inside the repository, then a symlink outside it
    // that leads inside.
    let alias = home.path().join("state-alias");
    std::fs::create_dir_all(repository.path().join("state")).expect("state inside");
    std::os::unix::fs::symlink(repository.path().join("state"), &alias).expect("symlink");
    for state_dir in [repository.path().join(".state"), alias] {
        let output = rune_isolated(home.path(), state.path())
            .env("RUNE_STATE_DIR", &state_dir)
            .args([
                "--json",
                "run",
                "sol@claude",
                "Edit",
                "--repo",
                repo,
                "--binary",
                binary,
                "--mode",
                "workspace-write",
            ])
            .output()
            .expect("run output");
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
        assert_eq!(result["kind"], "configuration_error", "{result}");
        assert!(
            result["details"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("would lie inside the checkout")),
            "{result}"
        );
        assert!(!marker.exists(), "the harness started");
        assert_eq!(workspace_names(home.path(), repo), ["default"]);
    }
}

#[cfg(unix)]
#[test]
fn a_subfolder_repo_starts_the_harness_in_the_matching_subfolder_of_the_workspace() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    std::fs::create_dir_all(repository.path().join("crates/core")).expect("subfolder");
    std::fs::write(repository.path().join("crates/core/lib.rs"), "// base\n").expect("lib");
    jj(home.path(), &["-R", repo, "util", "snapshot"]);
    let provider = executable_provider(home.path(), "fake-claude", FAKE_EDITOR);
    let subfolder = repository.path().join("crates/core");

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Edit the crate",
            "--repo",
            subfolder.to_str().expect("subfolder path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    let cwd = std::fs::read_to_string(home.path().join("cwd.txt")).expect("recorded cwd");
    assert!(
        cwd.contains("/rune/workspaces/"),
        "the harness ran in {cwd}"
    );
    assert!(
        cwd.trim().ends_with("/crates/core"),
        "the harness ran in {cwd}"
    );
    assert_ne!(cwd.trim(), canonical(&subfolder));
    // The edits landed under the subfolder of the run's change.
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id");
    let summary = jj(
        home.path(),
        &["-R", repo, "diff", "-r", change_id, "--summary"],
    );
    assert!(summary.contains("crates/core/NEW.md"), "{summary}");
    assert!(summary.contains("crates/core/README.md"), "{summary}");
    assert!(!subfolder.join("NEW.md").exists());
}

/// One harness that rewrites history inside the workspace, run on a fresh
/// repository: the result and the repository afterwards.
#[cfg(unix)]
fn run_rewriting_harness(
    home: &std::path::Path,
    state: &std::path::Path,
    repository: &std::path::Path,
    name: &str,
    script: &str,
) -> serde_json::Value {
    let provider = executable_provider(home, name, script);
    let output = rune_isolated(home, state)
        .env("CALLER", repository)
        .args([
            "--json",
            "run",
            "sol@claude",
            "go",
            "--repo",
            repository.to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{name}: stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("run JSON")
}

#[cfg(unix)]
#[test]
fn a_harness_that_squashes_into_its_base_still_leaves_its_work_in_one_change() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    std::fs::write(repository.path().join("README.md"), "line one\nline two\n").expect("README");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);
    let before = change_ids(home.path(), repo);

    for (name, script, expected) in [
        (
            "fake-squash-u",
            "#!/bin/sh\nprintf 'harness\\n' > NEW.md\njj squash --use-destination-message >/dev/null 2>&1\nprintf 'done\\n'\n",
            vec!["A NEW.md"],
        ),
        (
            "fake-squash-m",
            "#!/bin/sh\nprintf 'harness\\n' > NEW.md\njj squash -m 'harness work' >/dev/null 2>&1\nprintf 'done\\n'\n",
            vec!["A NEW.md"],
        ),
        // absorb moves the edited line into the copy, the only mutable
        // ancestor the workspace has: the caller's commits are out of its
        // reach, and the fold still keeps the line.
        (
            "fake-absorb",
            "#!/bin/sh\nprintf 'harness\\n' > NEW.md\nprintf 'line one\\nline two HARNESS\\n' > README.md\njj absorb >/dev/null 2>&1\nprintf 'done\\n'\n",
            vec!["A NEW.md", "M README.md"],
        ),
    ] {
        let result =
            run_rewriting_harness(home.path(), state.path(), repository.path(), name, script);
        let workspace = &result["workspace"];
        assert_eq!(workspace["kept"], true, "{name}: {result}");
        assert_eq!(workspace["outside_changes"], false, "{name}: {result}");
        assert!(workspace["error"].is_null(), "{name}: {result}");
        let change_id = workspace["change_id"].as_str().expect("change id");
        assert_eq!(
            changed_files(home.path(), repo, change_id),
            expected,
            "{name}"
        );
        // The caller's own commits are untouched, and only the run's change
        // was added.
        let base = jj(
            home.path(),
            &[
                "-R",
                repo,
                "file",
                "show",
                "-r",
                "description(exact:\"base\\n\")",
                "root:\"README.md\"",
            ],
        );
        assert_eq!(base, "line one\nline two\n", "{name}");
        assert_eq!(workspace_names(home.path(), repo), ["default"]);
        assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
        jj(home.path(), &["-R", repo, "abandon", change_id]);
        assert_eq!(change_ids(home.path(), repo), before, "{name}");
    }
}

/// A harness that rewrites one of the caller's commits by id. The shared
/// store lets it: the workspace's ancestors stop at the copy, so absorb
/// and squash cannot reach the caller's line, but an explicit revision can.
#[cfg(unix)]
const FAKE_REWRITER: &str = "#!/bin/sh\nprintf 'harness\\n' > NEW.md\njj describe -r 'description(exact:\"base\\n\")' -m 'rewritten by the harness' >/dev/null 2>&1\nprintf 'done\\n'\n";

#[cfg(unix)]
#[test]
fn a_harness_that_rewrites_a_caller_commit_is_reported_with_the_operation() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);

    let result = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-rewriter",
        FAKE_REWRITER,
    );
    let workspace = &result["workspace"];
    assert_eq!(workspace["outside_changes"], true, "{result}");
    let operation = workspace["operation_id"].as_str().expect("operation id");
    assert!(operation.len() > 8, "{result}");
    // The run's change still carries the edit, and the rewrite stands for
    // the owner to judge: the runner reports, it does not revert.
    assert_eq!(workspace["kept"], true, "{result}");
    let change_id = workspace["change_id"].as_str().expect("change id");
    assert_eq!(changed_files(home.path(), repo, change_id), ["A NEW.md"]);
    let rewritten = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            "@-",
            "--no-graph",
            "-T",
            "description",
        ],
    );
    assert_eq!(rewritten.trim(), "rewritten by the harness");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    let op_log = jj(
        home.path(),
        &["-R", repo, "op", "log", "--no-graph", "-T", "id ++ \"\\n\""],
    );
    assert!(
        op_log.lines().any(|line| line.trim() == operation),
        "{op_log}"
    );

    // The text report, on a repository whose base commit is still there to
    // rewrite.
    let second = jj_repository(home.path());
    let second_repo = second.path().to_str().expect("repository path");
    jj(home.path(), &["-R", second_repo, "commit", "-m", "base"]);
    rune_isolated(home.path(), state.path())
        .args([
            "run",
            "sol@claude",
            "go",
            "--repo",
            second_repo,
            "--binary",
            home.path()
                .join("fake-rewriter")
                .to_str()
                .expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("commits outside the run changed"))
        .stderr(predicate::str::contains("jj op log from operation"));
}

#[cfg(unix)]
#[test]
fn a_caller_that_squashes_during_the_run_does_not_stall_the_workspace() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    std::fs::write(repository.path().join("README.md"), "line one\nline two\n").expect("README");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);
    // The caller edits and squashes into its parent while the harness
    // works: the copy sits on the immutable base, so nothing rewrites it.
    let result = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-caller-squash",
        "#!/bin/sh\nprintf 'harness\\n' > NEW.md\nprintf 'line one\\nline two CALLER\\n' > \"$CALLER/README.md\"\njj -R \"$CALLER\" squash >/dev/null 2>&1\nprintf 'done\\n'\n",
    );
    let workspace = &result["workspace"];
    assert_eq!(workspace["kept"], true, "{result}");
    assert_eq!(workspace["outside_changes"], true, "{result}");
    let change_id = workspace["change_id"].as_str().expect("change id");
    assert_eq!(changed_files(home.path(), repo, change_id), ["A NEW.md"]);
    let parents = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            &format!("{change_id}-"),
            "--no-graph",
            "-T",
            "change_id ++ \"\\n\"",
        ],
    );
    let caller = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "change_id",
        ],
    );
    assert_eq!(parents.trim(), caller.trim(), "{parents}");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn a_harness_that_leaves_the_copy_keeps_the_folder_and_builds_no_change() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    // An immutable trunk under the caller's unpushed feature commit.
    jj(home.path(), &["-R", repo, "commit", "-m", "trunk"]);
    jj(
        home.path(),
        &["-R", repo, "bookmark", "create", "main", "-r", "@-"],
    );
    jj(
        home.path(),
        &[
            "-R",
            repo,
            "config",
            "set",
            "--repo",
            "revset-aliases.\"immutable_heads()\"",
            "main",
        ],
    );
    std::fs::write(repository.path().join("feature.md"), "feature work\n").expect("feature");
    jj(
        home.path(),
        &["-R", repo, "commit", "-m", "caller feature (unpushed)"],
    );
    // The harness starts its own change from trunk: a change built from
    // its files would delete the caller's feature.
    let result = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-trunk",
        "#!/bin/sh\njj new main -m 'harness: start from main' >/dev/null 2>&1\nprintf 'harness\\n' > NEW.md\nprintf 'done\\n'\n",
    );
    let workspace = &result["workspace"];
    assert_eq!(workspace["outside_changes"], true, "{result}");
    assert!(workspace["kept"].is_null(), "{result}");
    assert!(workspace["change_id"].is_null(), "{result}");
    assert!(
        workspace["error"]
            .as_str()
            .is_some_and(|error| error.contains("does not stand on the run's copy")),
        "{result}"
    );
    let folder = std::path::PathBuf::from(workspace["path"].as_str().expect("path"));
    assert_eq!(
        std::fs::read_to_string(folder.join("NEW.md")).expect("the harness's file stays"),
        "harness\n"
    );
    let feature = jj(
        home.path(),
        &[
            "-R",
            repo,
            "file",
            "show",
            "-r",
            "@-",
            "root:\"feature.md\"",
        ],
    );
    assert_eq!(feature, "feature work\n");
    let summary = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            "mutable()",
            "--no-graph",
            "--summary",
            "-T",
            "description.first_line() ++ \"\\n\"",
        ],
    );
    assert!(
        !summary
            .lines()
            .any(|line| line.starts_with("D ") && line.contains("feature.md")),
        "{summary}"
    );
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn a_harness_that_abandons_the_copy_keeps_the_folder() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);
    // Abandoning the copy moves the workspace's change off the run's
    // lineage: nothing descends from the copy any more, so no change is
    // built, and the harness's file stays in the folder.
    let result = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-abandon-parent",
        "#!/bin/sh\nprintf 'harness\\n' > NEW.md\njj abandon @- >/dev/null 2>&1\nprintf 'done\\n'\n",
    );
    let workspace = &result["workspace"];
    assert_eq!(workspace["outside_changes"], true, "{result}");
    assert!(workspace["kept"].is_null(), "{result}");
    assert!(
        workspace["error"]
            .as_str()
            .is_some_and(|error| error.contains("does not stand on the run's copy")),
        "{result}"
    );
    let folder = std::path::PathBuf::from(workspace["path"].as_str().expect("path"));
    assert_eq!(
        std::fs::read_to_string(folder.join("NEW.md")).expect("the harness's file stays"),
        "harness\n"
    );
    assert_eq!(
        std::fs::read_to_string(repository.path().join("README.md")).expect("README"),
        "base\n"
    );
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn a_harness_that_edits_the_callers_change_abandons_nothing_of_anyone_else() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);
    let first = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-seat-one",
        "#!/bin/sh\nprintf 'one\\n' > SEAT1.md\nprintf 'done\\n'\n",
    );
    let seat_one = first["workspace"]["change_id"]
        .as_str()
        .expect("seat one change")
        .to_string();
    // The second harness checks out the caller's working-copy commit in
    // its workspace and writes beside it.
    let second = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-seat-two",
        "#!/bin/sh\njj edit 'default@' >/dev/null 2>&1\nprintf 'evil\\n' > EVIL.md\nprintf 'done\\n'\n",
    );
    let workspace = &second["workspace"];
    assert_eq!(workspace["outside_changes"], true, "{second}");
    assert!(workspace["kept"].is_null(), "{second}");
    assert!(
        workspace["error"]
            .as_str()
            .is_some_and(|error| error.contains("does not stand on the run's copy")),
        "{second}"
    );
    // Seat one's change is still there, the caller's checkout is as it
    // was, and the caller's commands still run (the checkout is not stale).
    let visible = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            &format!("change_id({seat_one}) & ::visible_heads()"),
            "--no-graph",
            "-T",
            "change_id ++ \"\\n\"",
        ],
    );
    assert_eq!(visible.trim(), seat_one);
    let status = jj(home.path(), &["-R", repo, "status"]);
    assert!(!status.contains("EVIL"), "{status}");
    assert!(!repository.path().join("EVIL.md").exists());
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn a_caller_branch_a_harness_rebased_onto_its_work_is_not_abandoned() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);
    // A mutable side branch of the caller's, beside its working copy.
    jj(
        home.path(),
        &["-R", repo, "new", "root()", "-m", "side branch"],
    );
    std::fs::write(repository.path().join("side.md"), "side\n").expect("side");
    jj(
        home.path(),
        &["-R", repo, "new", "description(exact:\"base\\n\")"],
    );
    // The harness rebases that branch onto its own work: the branch then
    // stands on the copy, but it existed before the run.
    let result = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-rebaser",
        "#!/bin/sh\nprintf 'harness\\n' > NEW.md\njj rebase -r 'description(exact:\"side branch\\n\")' -o @ >/dev/null 2>&1\nprintf 'done\\n'\n",
    );
    let workspace = &result["workspace"];
    assert_eq!(workspace["kept"], true, "{result}");
    assert_eq!(workspace["outside_changes"], true, "{result}");
    assert!(workspace["error"].is_null(), "{result}");
    let change_id = workspace["change_id"].as_str().expect("change id");
    assert_eq!(changed_files(home.path(), repo, change_id), ["A NEW.md"]);
    let side = jj(
        home.path(),
        &[
            "-R",
            repo,
            "log",
            "-r",
            "description(exact:\"side branch\\n\") & ::visible_heads()",
            "--no-graph",
            "--summary",
            "-T",
            "description.first_line() ++ \"\\n\"",
        ],
    );
    assert!(side.contains("side branch"), "{side}");
    assert!(side.contains("side.md"), "{side}");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn a_caller_that_describes_its_change_during_the_run_is_reported() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    jj(home.path(), &["-R", repo, "commit", "-m", "base"]);
    // A new description on the caller's working-copy change is a jj
    // command of the caller's, unlike an edit: it is reported.
    let result = run_rewriting_harness(
        home.path(),
        state.path(),
        repository.path(),
        "fake-caller-describe",
        "#!/bin/sh\nprintf 'harness\\n' > NEW.md\njj -R \"$CALLER\" describe -m 'caller renamed its change' >/dev/null 2>&1\nprintf 'done\\n'\n",
    );
    let workspace = &result["workspace"];
    assert_eq!(workspace["outside_changes"], true, "{result}");
    assert_eq!(workspace["kept"], true, "{result}");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn a_colored_jj_configuration_does_not_break_the_run() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let config = home.path().join("jj.toml");
    std::fs::write(
        &config,
        "ui.color = \"always\"\ntemplates.commit_summary = '\"<custom summary>\"'\n",
    )
    .expect("jj config");
    let provider = executable_provider(home.path(), "fake-claude", FAKE_EDITOR);
    let before = change_ids(home.path(), repo);

    let output = rune_isolated(home.path(), state.path())
        .env("JJ_CONFIG", &config)
        .args([
            "--json",
            "run",
            "sol@claude",
            "go",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert!(output.status.success(), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["workspace"]["kept"], true, "{result}");
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id");
    assert!(
        change_id.chars().all(|c| c.is_ascii_lowercase()),
        "{result}"
    );
    assert_eq!(change_ids(home.path(), repo).len(), before.len() + 1);
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
}

#[cfg(unix)]
#[test]
fn a_sigint_to_the_process_group_during_the_finish_lets_the_finish_complete() {
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let repo = repository.path().to_str().expect("repository path");
    let marker = home.path().join("harness-done");
    // Many files make the finish snapshot take a while: Ctrl-C arrives in
    // the middle of it.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!(
            "#!/bin/sh\nmkdir many\ni=0\nwhile [ $i -lt 20000 ]; do echo $i > many/f$i.txt; i=$((i+1)); done\ntouch '{}'\nprintf 'done\\n'\n",
            marker.display()
        ),
    );

    let started = Instant::now();
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_rune"))
        .env("HOME", home.path())
        .env("CLIPROXY_API_KEY", "test-token")
        .env("XDG_STATE_HOME", state.path())
        .env_remove("RUNE_STATE_DIR")
        .env("JJ_USER", "Test Runner")
        .env("JJ_EMAIL", "runner@example.test")
        .args([
            "--json",
            "run",
            "sol@claude",
            "go",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .expect("rune spawns");
    while !marker.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the harness never finished"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    std::thread::sleep(Duration::from_millis(500));
    let delivered = std::process::Command::new("kill")
        .args(["-INT", "--", &format!("-{}", child.id())])
        .status()
        .expect("kill runs");
    assert!(delivered.success(), "kill failed");
    let output = child.wait_with_output().expect("rune output");
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(130),
        "stdout: {printed}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_str(printed.trim()).expect("run JSON");
    assert_eq!(result["kind"], "success", "{printed}");
    assert_eq!(result["stopped_by_signal"], 2, "{printed}");
    assert_eq!(result["workspace"]["kept"], true, "{printed}");
    assert!(result["workspace"]["error"].is_null(), "{printed}");
    assert_eq!(workspace_names(home.path(), repo), ["default"]);
    assert_eq!(leftover_workspaces(state.path()), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn prompt_file_resolves_against_the_callers_directory_in_an_isolated_run() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    let briefs = tempfile::tempdir().expect("briefs");
    std::fs::write(briefs.path().join("brief.md"), "The brief text.\n").expect("brief");
    let provider = executable_provider(home.path(), "fake-claude", "#!/bin/sh\ncat\n");

    rune_isolated(home.path(), state.path())
        .current_dir(briefs.path())
        .args([
            "run",
            "sol@claude",
            "--prompt-file",
            "brief.md",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("The brief text."));
}

/// A fake harness that leaves a marker beside itself when it starts, so a
/// refusal "before anything starts" is checked, not assumed.
#[cfg(unix)]
fn marking_provider(home: &std::path::Path, name: &str, reply: &str) -> std::path::PathBuf {
    executable_provider(
        home,
        name,
        &format!(
            "#!/bin/sh\ntouch \"$(dirname \"$0\")/harness-started\"\nprintf '%s\\n' '{reply}'\n"
        ),
    )
}

#[cfg(unix)]
fn confirmation_of(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not one JSON object ({error}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// A repository folder whose own name is not hidden: the output check
/// judges hidden folders from the closest permitted root.
#[cfg(unix)]
fn plain_repository() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("rune-run-repo-")
        .tempdir()
        .expect("repository")
}

#[cfg(unix)]
#[test]
fn output_file_holds_the_result_and_stdout_confirms_it() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let repo = repository.path().to_str().expect("repository path");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf 'provider reply\\n'\n",
    );
    let binary = provider.to_str().expect("provider path");
    let seat = repository.path().join("council/seat.json");
    std::fs::create_dir_all(seat.parent().expect("parent")).expect("council");
    std::fs::write(&seat, "stale").expect("stale file");

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
            "--output",
        ])
        .arg(&seat)
        .output()
        .expect("run output");
    assert!(output.status.success(), "{output:?}");
    let confirmation = confirmation_of(&output);
    assert_eq!(confirmation["ok"], true, "{confirmation}");
    assert_eq!(confirmation["kind"], "success", "{confirmation}");
    assert_eq!(confirmation["exit_code"], 0, "{confirmation}");
    assert_eq!(confirmation["output"], canonical(&seat), "{confirmation}");
    assert_eq!(confirmation.as_object().expect("object").len(), 4);
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&seat).expect("seat file"))
            .expect("one JSON object in the file");
    assert_eq!(written["kind"], "success", "{written}");
    assert_eq!(written["text"], "provider reply", "{written}");
    let mode = std::fs::metadata(&seat)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let left: Vec<String> = std::fs::read_dir(seat.parent().expect("parent"))
        .expect("council")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(left, ["seat.json"], "a temporary file stays behind");

    // Without --json the file holds the answer text, stdout stays empty,
    // and stderr names the file.
    let text = repository.path().join("seat.txt");
    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repo,
            "--binary",
            binary,
            "--output",
        ])
        .arg(&text)
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains(format!(
            "output: {}\n",
            canonical(&text)
        )));
    assert_eq!(
        std::fs::read_to_string(&text).expect("text file"),
        "provider reply\n"
    );
}

#[cfg(unix)]
#[test]
fn timed_out_run_with_an_output_file_writes_the_timeout_object() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nprintf 'partial answer'\nprintf 'still going\\n' >&2\nsleep 5\n",
    );
    let seat = repository.path().join("seat.json");

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--timeout",
            "1s",
            "--output",
        ])
        .arg(&seat)
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(124));
    let confirmation = confirmation_of(&output);
    assert_eq!(confirmation["ok"], false, "{confirmation}");
    assert_eq!(confirmation["kind"], "timeout", "{confirmation}");
    assert_eq!(confirmation["exit_code"], 124, "{confirmation}");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&seat).expect("seat file"))
            .expect("one JSON object in the file");
    assert_eq!(written["kind"], "timeout", "{written}");
    assert_eq!(
        written["details"]["stdout_tail"], "partial answer",
        "{written}"
    );
    assert_eq!(
        written["details"]["stderr_tail"], "still going",
        "{written}"
    );
}

#[cfg(unix)]
#[test]
fn a_protected_folder_named_as_the_repository_opens_no_file_there() {
    let home = tempfile::tempdir().expect("home");
    let provider = marking_provider(home.path(), "fake-claude", "ran");
    let binary = provider.to_str().expect("provider path");
    let marker = home.path().join("harness-started");
    let claude = home.path().join(".claude");
    std::fs::create_dir_all(&claude).expect(".claude");
    let settings = claude.join("settings.json");
    std::fs::write(&settings, "{\"owner\":true}").expect("settings");
    let zshrc = home.path().join(".zshrc");
    std::fs::write(&zshrc, "# owner\n").expect("zshrc");

    // `--repo` names the protected folder, or the home itself: neither is
    // the caller's repository, and neither widens where a file may go.
    for (repo, output) in [(&claude, &settings), (&home.path().to_path_buf(), &zshrc)] {
        let result = rune(home.path())
            .args(["--json", "run", "sol@claude", "Review", "--repo"])
            .arg(repo)
            .args(["--binary", binary, "--output"])
            .arg(output)
            .output()
            .expect("run output");
        assert_eq!(result.status.code(), Some(2), "{result:?}");
        let json = confirmation_of(&result);
        assert_eq!(json["kind"], "configuration_error", "{json}");
        assert!(
            !marker.exists(),
            "the harness started for {}",
            output.display()
        );
    }
    assert_eq!(
        std::fs::read_to_string(&settings).expect("settings"),
        "{\"owner\":true}"
    );
    assert_eq!(std::fs::read_to_string(&zshrc).expect("zshrc"), "# owner\n");
}

#[cfg(unix)]
#[test]
fn output_path_in_a_protected_place_is_refused_before_the_harness_starts() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let repo = repository.path().to_str().expect("repository path");
    let provider = marking_provider(home.path(), "fake-claude", "ran");
    let binary = provider.to_str().expect("provider path");
    let marker = home.path().join("harness-started");

    std::fs::create_dir_all(home.path().join(".claude")).expect(".claude");
    std::fs::create_dir_all(repository.path().join(".git")).expect(".git");
    let real = repository.path().join("real.json");
    std::fs::write(&real, "{}").expect("real");
    let link = repository.path().join("link.json");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let protected = [
        home.path().join(".claude/settings.json"),
        repository.path().join(".git/hooks.json"),
        link.clone(),
    ];

    for path in &protected {
        let output = rune(home.path())
            .args([
                "--json",
                "run",
                "sol@claude",
                "Review",
                "--repo",
                repo,
                "--binary",
                binary,
                "--output",
            ])
            .arg(path)
            .output()
            .expect("run output");
        assert_eq!(output.status.code(), Some(2), "{}", path.display());
        let result = confirmation_of(&output);
        assert_eq!(result["kind"], "configuration_error", "{result}");
        assert!(
            !marker.exists(),
            "the harness started for {}",
            path.display()
        );

        rune(home.path())
            .args([
                "run",
                "sol@claude",
                "Review",
                "--repo",
                repo,
                "--binary",
                binary,
                "--output",
            ])
            .arg(path)
            .assert()
            .code(2)
            .stdout("")
            .stderr(predicate::str::contains("fatal:"));
        assert!(
            !marker.exists(),
            "the harness started for {}",
            path.display()
        );
    }
    assert!(!home.path().join(".claude/settings.json").exists());
    assert!(!repository.path().join(".git/hooks.json").exists());
    assert_eq!(std::fs::read_to_string(&real).expect("real"), "{}");
}

#[cfg(unix)]
#[test]
fn usage_error_with_an_output_file_writes_nothing() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let seat = repository.path().join("seat.json");

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--mode",
            "bogus",
            "--output",
        ])
        .arg(&seat)
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(2));
    let result = confirmation_of(&output);
    assert_eq!(result["kind"], "configuration_error", "{result}");
    assert!(!seat.exists(), "a usage error wrote the file");
}

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_symlink_during_the_run_moves_no_write() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let out = repository.path().join("out");
    std::fs::create_dir_all(&out).expect("out");
    let claude = home.path().join(".claude");
    std::fs::create_dir_all(&claude).expect(".claude");
    // The harness removes the accepted folder and puts a symlink to the
    // protected folder in its place.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!(
            "#!/bin/sh\nrm -rf '{out}'\nln -s '{claude}' '{out}'\nprintf 'payload\\n'\n",
            out = out.display(),
            claude = claude.display()
        ),
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--output",
        ])
        .arg(out.join("settings.json"))
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(74), "{output:?}");
    let result = confirmation_of(&output);
    assert_eq!(result["kind"], "success", "{result}");
    assert_eq!(result["text"], "payload", "{result}");
    assert!(
        !claude.join("settings.json").exists(),
        "the write followed the symlink"
    );
    assert_eq!(
        std::fs::read_dir(&claude).expect("claude").count(),
        0,
        "the protected folder gained a file"
    );
}

#[cfg(unix)]
#[test]
fn an_output_folder_moved_during_the_run_leaves_the_planted_file_alone() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let out = repository.path().join("out");
    std::fs::create_dir_all(&out).expect("out");
    let moved = repository.path().join("out.old");
    let planted = "{\"ok\":true,\"kind\":\"success\",\"text\":\"LGTM, no issues\"}";
    // The harness moves the accepted folder away, makes a new folder at
    // the old path, plants a result there, and fails.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!(
            "#!/bin/sh\nmv '{out}' '{moved}'\nmkdir '{out}'\nprintf '%s\\n' '{planted}' > '{out}/seat.json'\necho 'real answer: three blockers' >&2\nexit 3\n",
            out = out.display(),
            moved = moved.display()
        ),
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--output",
        ])
        .arg(out.join("seat.json"))
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(74), "{output:?}");
    // stdout carries the full result, never a confirmation that names the
    // planted file.
    let result = confirmation_of(&output);
    assert_eq!(result["kind"], "exit", "{result}");
    assert!(result.get("output").is_none(), "{result}");
    assert!(
        result["details"]["stderr_tail"]
            .as_str()
            .is_some_and(|tail| tail.contains("three blockers")),
        "{result}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not the file the runner wrote"),
        "{output:?}"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("seat.json")).expect("planted file"),
        format!("{planted}\n")
    );
    // The runner's own file went into the moved folder, so nothing is lost.
    let written = std::fs::read_to_string(moved.join("seat.json")).expect("written file");
    assert!(written.contains("\"kind\":\"exit\""), "{written}");
}

#[cfg(unix)]
#[test]
fn result_file_that_cannot_be_written_prints_the_full_result_and_exits_74() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let folder = repository.path().join("out");
    std::fs::create_dir_all(&folder).expect("out");
    // The harness makes the output folder read-only while it runs, so the
    // path passed the check and the final write fails.
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        "#!/bin/sh\nchmod 555 \"$RUNE_TEST_OUT\"\nprintf 'provider reply\\n'\n",
    );
    let seat = folder.join("seat.json");

    let output = rune(home.path())
        .env("RUNE_TEST_OUT", &folder)
        .args([
            "--json",
            "run",
            "sol@claude",
            "Review",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--output",
        ])
        .arg(&seat)
        .output()
        .expect("run output");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(output.status.code(), Some(74), "{output:?}");
    let result = confirmation_of(&output);
    assert_eq!(result["kind"], "success", "{result}");
    assert_eq!(result["text"], "provider reply", "{result}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot write result file"),
        "{output:?}"
    );
    assert!(!seat.exists());
}

#[cfg(unix)]
#[test]
fn read_only_agy_run_is_refused_and_names_workspace_write() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let repo = repository.path().to_str().expect("repository path");
    let provider = marking_provider(home.path(), "fake-agy", "{\"response\":\"ran\"}");
    let binary = provider.to_str().expect("provider path");
    let marker = home.path().join("harness-started");

    // Read-only is the default mode and the explicit one: both refuse.
    for mode in [&[][..], &["--mode", "read-only"][..]] {
        let output = rune(home.path())
            .args([
                "--json", "run", "agy", "Review", "--repo", repo, "--binary", binary,
            ])
            .args(mode)
            .output()
            .expect("run output");
        assert_eq!(output.status.code(), Some(2), "{mode:?}");
        let result = confirmation_of(&output);
        assert_eq!(result["kind"], "configuration_error", "{result}");
        let message = result["details"]["message"].as_str().expect("message");
        assert!(
            message.contains("agy cannot enforce read-only access"),
            "{message}"
        );
        assert!(message.contains("--mode workspace-write"), "{message}");
        assert!(!marker.exists(), "the harness started");
    }
    rune(home.path())
        .args(["run", "agy", "Review", "--repo", repo, "--binary", binary])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot enforce read-only access"))
        .stderr(predicate::str::contains("--mode workspace-write"));
    assert!(!marker.exists(), "the harness started");
}

#[cfg(unix)]
#[test]
fn agy_soft_denial_in_its_log_names_the_denied_tool_in_the_hint() {
    let home = tempfile::tempdir().expect("home");
    let state = tempfile::tempdir().expect("state");
    let repository = jj_repository(home.path());
    // The fake writes agy's own soft-deny line, glog prefix included, to the
    // log file the runner named, and answers with no response.
    let provider = executable_provider(
        home.path(),
        "fake-agy",
        r#"#!/bin/sh
log=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--log-file" ]; then log="$2"; shift; fi
  shift
done
printf '%s\n' 'I1022 12:32:09.920149    1293 tool_confirmation_manager.go:212] Print mode: soft-denying tool confirmation "RunCommand" at step 5' > "$log"
printf '%s\n' '{"response":""}'
"#,
    );

    let output = rune_isolated(home.path(), state.path())
        .args([
            "--json",
            "run",
            "agy",
            "List the files",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
            "--mode",
            "workspace-write",
        ])
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(1));
    let result = confirmation_of(&output);
    assert_eq!(result["kind"], "surface_error", "{result}");
    let hint = result["hint"].as_str().expect("hint");
    assert!(hint.contains("shell command"), "{hint}");
    assert!(hint.contains("headless"), "{hint}");
    assert_eq!(
        result["details"]["harness_errors"],
        serde_json::json!(["Print mode: soft-denying tool confirmation \"RunCommand\" at step 5"]),
        "{result}"
    );
}

#[test]
fn grok_check_covers_an_inherited_endpoint_and_asks_for_models_once() {
    let home = tempfile::tempdir().expect("home");
    // The endpoint is set in the shell, ends in /v1, and answers nothing.
    let output = rune(home.path())
        .env("GROK_MODELS_BASE_URL", "http://127.0.0.1:9/v1")
        .env("GROK_CODE_XAI_API_KEY", "fake-inherited-key")
        .args(["--json", "run", "grok", "x", "--check"])
        .output()
        .expect("check output");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("check JSON");
    let endpoints = result["endpoints"].as_array().expect("endpoints");
    assert_eq!(endpoints.len(), 1, "{result}");
    assert_eq!(
        endpoints[0]["base_url"], "http://127.0.0.1:9/v1",
        "{result}"
    );
    assert_eq!(
        endpoints[0]["credential_key"], "GROK_CODE_XAI_API_KEY",
        "{result}"
    );
    let reason = endpoints[0]["reason"].as_str().expect("reason");
    assert!(reason.contains("http://127.0.0.1:9/v1/models"), "{reason}");
    assert!(!reason.contains("/v1/v1/"), "{reason}");
    assert!(!reason.contains("fake-inherited-key"), "{reason}");

    rune(home.path())
        .env("GROK_MODELS_BASE_URL", "http://127.0.0.1:9/v1")
        .args(["run", "grok", "x", "--check"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains(
            "endpoint: http://127.0.0.1:9/v1/models\n",
        ));
}

#[cfg(unix)]
#[test]
fn grok_echoing_the_value_of_its_key_variable_leaves_no_trace_in_any_output() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let repo = repository.path().to_str().expect("repository path");
    std::fs::create_dir_all(home.path().join(".grok")).expect(".grok");
    // The key map names a variable whose name looks like no credential.
    std::fs::write(
        home.path().join(".grok/config.toml"),
        "[model.\"grok-4.5\"]\nenv_key = \"GROK_SEAT_HANDLE\"\n",
    )
    .expect("grok configuration");
    let value = "plain-handle-value-7788-with-no-secret-look";
    // The fake grok echoes the value in its answer and on stderr.
    let provider = executable_provider(
        home.path(),
        "fake-grok",
        "#!/bin/sh\nprintf '%s\\n' \"{\\\"text\\\":\\\"the handle is $GROK_SEAT_HANDLE\\\"}\"\nprintf 'stderr sees %s\\n' \"$GROK_SEAT_HANDLE\" >&2\n",
    );
    let binary = provider.to_str().expect("provider path");
    let seat = repository.path().join("seat.json");

    let output = rune(home.path())
        .env("GROK_SEAT_HANDLE", value)
        .args([
            "--json", "run", "grok", "Review", "--repo", repo, "--binary", binary, "--output",
        ])
        .arg(&seat)
        .output()
        .expect("run output");
    assert!(output.status.success(), "{output:?}");
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!printed.contains(value), "{printed}");
    assert!(printed.contains("stderr sees <redacted>"), "{printed}");
    let written = std::fs::read_to_string(&seat).expect("seat file");
    assert!(!written.contains(value), "{written}");
    let result: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
    assert_eq!(result["text"], "the handle is <redacted>", "{written}");

    rune(home.path())
        .env("GROK_SEAT_HANDLE", value)
        .args(["run", "grok", "Review", "--repo", repo, "--binary", binary])
        .assert()
        .success()
        .stdout("the handle is <redacted>\n")
        .stdout(predicate::str::contains(value).not())
        .stderr(predicate::str::contains(value).not());
}

/// An ordinary coding answer passes the redaction unchanged: the answer
/// loses only a known credential value, never a `key: value` shape or a
/// URL parameter.
#[cfg(unix)]
#[test]
fn an_ordinary_answer_passes_the_redaction_unchanged() {
    let home = tempfile::tempdir().expect("home");
    let repository = plain_repository();
    let repo = repository.path().to_str().expect("repository path");
    let answer = concat!(
        "Primary key: user_id\n",
        "The lookup uses `let key = cache_key(&path);` and then `map.get(&key)`.\n",
        "In settings.py, set SECRET_KEY = os.environ[\"DJANGO_SECRET\"] and keep it out of git.\n",
        "Hotkey: Ctrl+Shift+P\n",
        "Docs: https://example.com/api?token=abc123&page=2\n",
    );
    let text = home.path().join("answer.txt");
    std::fs::write(&text, answer).expect("answer");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        &format!("#!/bin/sh\ncat '{}'\n", text.display()),
    );
    rune(home.path())
        .args([
            "run",
            "sol@claude",
            "go",
            "--repo",
            repo,
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .assert()
        .success()
        .stdout(answer);
}

#[cfg(unix)]
#[test]
fn clean_grok_run_keeps_only_the_model_key_map() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let repo = repository.path().to_str().expect("repository path");
    std::fs::create_dir_all(home.path().join(".grok")).expect(".grok");
    std::fs::write(
        home.path().join(".grok/config.toml"),
        concat!(
            "[cli]\nauto_update = true\n\n",
            "[model.\"grok-4.5\"]\nenv_key = \"FAKE_GROK_KEY\"\n\n",
            "[permission]\nallow = [\"Bash\"]\n\n",
            "[sandbox]\nmode = \"workspace\"\n\n",
            "[shell_environment_policy]\ninherit = \"all\"\n\n",
            "[telemetry]\nenabled = true\n\n",
            "[ui]\ntheme = \"dark\"\n",
        ),
    )
    .expect("grok configuration");
    // The fake grok copies the configuration it sees under its HOME beside
    // itself and answers.
    let provider = executable_provider(
        home.path(),
        "fake-grok",
        r#"#!/bin/sh
cp "$HOME/.grok/config.toml" "$(dirname "$0")/seen.toml"
printf '%s\n' '{"text":"answered"}'
"#,
    );
    let binary = provider.to_str().expect("provider path");

    rune(home.path())
        .args([
            "run", "grok", "Review", "--repo", repo, "--binary", binary, "--clean",
        ])
        .assert()
        .success()
        .stdout("answered\n");
    let seen = std::fs::read_to_string(home.path().join("seen.toml")).expect("seen config");
    let seen: toml::Value = toml::from_str(&seen).expect("valid TOML");
    assert_eq!(
        seen["model"]["grok-4.5"]["env_key"].as_str(),
        Some("FAKE_GROK_KEY")
    );
    for dropped in [
        "cli",
        "permission",
        "sandbox",
        "shell_environment_policy",
        "telemetry",
        "ui",
    ] {
        assert!(
            seen.get(dropped).is_none(),
            "{dropped} reached the clean run"
        );
    }

    rune(home.path())
        .args([
            "run",
            "grok",
            "Review",
            "--repo",
            repo,
            "--dry-run",
            "--clean",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "clean_config_keys: model.grok-4.5.env_key\n",
        ))
        .stdout(predicate::str::contains("FAKE_GROK_KEY").not());
    rune(home.path())
        .args(["run", "grok", "Review", "--repo", repo, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("clean_config_keys: none\n"));
}
