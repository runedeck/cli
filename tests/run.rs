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
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-claude",
        r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/argv.txt"
printf 'built\n'
"#,
    );

    rune(home.path())
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
