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
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-agy",
        r#"#!/bin/sh
printf '%s\n' '{"response":"provider reply","usage":{"output_tokens":17}}'
"#,
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "agy",
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

#[test]
fn dry_run_reports_a_session_name_the_harness_cannot_take() {
    // agy has no flag for a session name: the plan says so, and nothing is sent.
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "agy",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
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

#[test]
fn dry_run_of_a_prompt_in_argv_prints_the_placeholder() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");

    rune(home.path())
        .args([
            "run",
            "agy",
            "Review this repository",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "--print <prompt> --output-format json",
        ))
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

/// Run the fake agy against `data` as its data directory. The state
/// directory is pinned to `state` so no test touches the real one.
#[cfg(unix)]
fn run_fake_agy(
    home: &std::path::Path,
    data: &std::path::Path,
    state: &std::path::Path,
    json: bool,
    extra: &[&str],
) -> std::process::Output {
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(home, "fake-agy", FAKE_AGY_WITH_ARTIFACTS);
    let mut command = rune(home);
    command
        .env("ANTIGRAVITY_EXECUTABLE_DATA_DIR", data)
        .env("XDG_STATE_HOME", state)
        .env_remove("RUNE_STATE_DIR");
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

#[test]
fn dry_run_names_the_artifacts_destination() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let state = tempfile::tempdir().expect("state");
    let repo = repository.path().to_str().expect("repository path");

    rune(home.path())
        .args([
            "run",
            "agy",
            "Draw an icon",
            "--repo",
            repo,
            "--dry-run",
            "--artifacts-dir",
            "out",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("artifacts_dir: out\n"));

    rune(home.path())
        .env("XDG_STATE_HOME", state.path())
        .env_remove("RUNE_STATE_DIR")
        .args([
            "run",
            "agy",
            "Draw an icon",
            "--repo",
            repo,
            "--dry-run",
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
    assert_eq!(result["workspace"]["colocated"], false, "{result}");
    let change_id = result["workspace"]["change_id"]
        .as_str()
        .expect("change id")
        .to_string();
    let name = result["workspace"]["name"].as_str().expect("name");
    assert!(name.starts_with("run-"), "{result}");

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
    let parent = change_ids(home.path(), repo);

    let children: Vec<std::process::Child> = ["a", "b"]
        .iter()
        .map(|seat| {
            std::process::Command::new(env!("CARGO_BIN_EXE_rune"))
                .env("HOME", home.path())
                .env("CLIPROXY_API_KEY", "test-token")
                .env("XDG_STATE_HOME", state.path())
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
                    provider.to_str().expect("provider path"),
                    "--mode",
                    "workspace-write",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("rune spawns")
        })
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
