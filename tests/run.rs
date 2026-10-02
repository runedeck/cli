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

#[cfg(unix)]
fn cursor_run(
    home: &std::path::Path,
    repository: &std::path::Path,
    provider: &std::path::Path,
) -> Command {
    let mut command = rune(home);
    command
        .env_remove("CURSOR_CONFIG_DIR")
        .env_remove("XDG_CONFIG_HOME")
        .args([
            "--json",
            "run",
            "cursor",
            "Reply with pong",
            "--repo",
            repository.to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ]);
    command
}

#[cfg(unix)]
#[test]
fn cursor_run_sends_the_prompt_on_stdin_and_reads_the_result_object() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let capture = home.path().join("capture");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$@" > '{capture}.argv'
cat > '{capture}.stdin'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"result":"pong","usage":{{"inputTokens":9,"outputTokens":2}}}}'
"#,
            capture = capture.display()
        ),
    );

    let output = cursor_run(home.path(), repository.path(), &provider)
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "success");
    assert_eq!(result["tool"], "cursor");
    assert_eq!(result["text"], "pong");
    assert_eq!(result["usage"]["output_tokens"].as_f64(), Some(2.0));

    let argv = std::fs::read_to_string(capture.with_extension("argv")).expect("argv");
    let repository = std::fs::canonicalize(repository.path()).expect("canonical repository");
    let repository = repository.to_str().expect("repository path");
    assert_eq!(
        argv.lines().collect::<Vec<_>>(),
        [
            "-p",
            "--output-format",
            "json",
            "--workspace",
            repository,
            "--trust",
            "--mode",
            "ask",
        ]
    );
    let stdin = std::fs::read_to_string(capture.with_extension("stdin")).expect("stdin");
    assert!(stdin.ends_with("Reply with pong\n"), "{stdin}");
}

#[cfg(unix)]
#[test]
fn fable_at_cursor_selects_fable_without_a_configured_profile() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let capture = home.path().join("capture");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$@" > '{capture}.argv'
cat > /dev/null
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"result":"pong"}}'
"#,
            capture = capture.display()
        ),
    );

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "fable@cursor",
            "Reply with pong",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .env_remove("CURSOR_CONFIG_DIR")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["resolved_model"], "claude-fable-5-1-high");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning"), "{stderr}");
    let argv = std::fs::read_to_string(capture.with_extension("argv")).expect("argv");
    let argv: Vec<&str> = argv.lines().collect();
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["--model", "claude-fable-5-1-high"]),
        "{argv:?}"
    );
    assert_eq!(
        argv.iter()
            .filter(|argument| **argument == "--model")
            .count(),
        1,
        "{argv:?}"
    );
}

#[cfg(unix)]
#[test]
fn cursor_reported_error_fails_the_run() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        r#"#!/bin/sh
cat > /dev/null
printf '%s\n' '{"type":"result","subtype":"error","is_error":true,"result":"model unavailable"}'
"#,
    );

    let output = cursor_run(home.path(), repository.path(), &provider)
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "surface_error");
    assert_eq!(
        result["details"]["message"],
        "cursor-agent reported an error: model unavailable"
    );
}

#[cfg(unix)]
#[test]
fn clean_cursor_run_without_an_api_key_never_starts_the_tool() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let marker = home.path().join("started");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    );

    let output = cursor_run(home.path(), repository.path(), &provider)
        .arg("--clean")
        .env_remove("CURSOR_API_KEY")
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "configuration_error");
    let message = result["details"]["message"].as_str().expect("message");
    assert!(message.contains("CURSOR_API_KEY"), "{message}");
    assert!(!marker.exists());
}

#[cfg(unix)]
#[test]
fn clean_cursor_run_with_an_api_key_moves_home_and_config() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let capture = home.path().join("env");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        &format!(
            r#"#!/bin/sh
cat > /dev/null
printf 'HOME=%s\nCURSOR_CONFIG_DIR=%s\nKEY=%s\n' "$HOME" "$CURSOR_CONFIG_DIR" "$CURSOR_API_KEY" > '{}'
test -d "$CURSOR_CONFIG_DIR" || exit 3
printf '%s\n' '{{"type":"result","is_error":false,"result":"pong"}}'
"#,
            capture.display()
        ),
    );

    let output = cursor_run(home.path(), repository.path(), &provider)
        .arg("--clean")
        .env("CURSOR_API_KEY", "test-cursor-key")
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let seen = std::fs::read_to_string(&capture).expect("environment capture");
    let clean_home = seen
        .lines()
        .find_map(|line| line.strip_prefix("HOME="))
        .expect("HOME line");
    assert_ne!(clean_home, home.path().to_str().expect("home path"));
    assert!(
        seen.contains(&format!("CURSOR_CONFIG_DIR={clean_home}/.cursor")),
        "{seen}"
    );
    assert!(seen.contains("KEY=test-cursor-key"), "{seen}");
}

#[cfg(unix)]
#[test]
fn read_only_cursor_run_refuses_unrestricted_approval_before_the_tool_starts() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let marker = home.path().join("started");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        &format!(
            "#!/bin/sh\ncat > /dev/null\ntouch '{}'\nprintf '%s\\n' '{{\"type\":\"result\",\"result\":\"pong\"}}'\n",
            marker.display()
        ),
    );
    let cursor = home.path().join(".cursor");
    std::fs::create_dir_all(&cursor).expect("cursor config directory");
    std::fs::write(
        cursor.join("cli-config.json"),
        r#"{"version":1,"approvalMode":"unrestricted"}"#,
    )
    .expect("cursor config");

    let output = cursor_run(home.path(), repository.path(), &provider)
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "configuration_error");
    let message = result["details"]["message"].as_str().expect("message");
    assert!(message.contains("approvalMode unrestricted"), "{message}");
    assert!(!marker.exists());

    let xdg = home.path().join("xdg");
    std::fs::create_dir_all(xdg.join("cursor")).expect("xdg cursor directory");
    std::fs::write(
        xdg.join("cursor/cli-config.json"),
        r#"{"version":1,"approvalMode":"unrestricted"}"#,
    )
    .expect("xdg cursor config");
    std::fs::remove_file(cursor.join("cli-config.json")).expect("remove home config");
    let output = cursor_run(home.path(), repository.path(), &provider)
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .expect("run output");
    assert_eq!(
        output.status.code(),
        Some(2),
        "XDG_CONFIG_HOME config is read"
    );
    assert!(!marker.exists());

    let output = cursor_run(home.path(), repository.path(), &provider)
        .env("XDG_CONFIG_HOME", &xdg)
        .args(["--mode", "workspace-write"])
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(marker.exists());
}

#[cfg(unix)]
#[test]
fn clean_cursor_run_accepts_a_key_from_the_profile_environment() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let capture = home.path().join("env");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        &format!(
            r#"#!/bin/sh
cat > /dev/null
printf 'KEY=%s\n' "$CURSOR_API_KEY" > '{}'
printf '%s\n' '{{"type":"result","is_error":false,"result":"pong"}}'
"#,
            capture.display()
        ),
    );
    let config = home.path().join(".config/rune");
    std::fs::create_dir_all(&config).expect("config directory");
    std::fs::write(
        config.join("config.yaml"),
        "launch:\n  profiles:\n    cursor:\n      keyed:\n        env:\n          CURSOR_API_KEY:\n            from_env: PROFILE_CURSOR_KEY\n",
    )
    .expect("config");

    let output = rune(home.path())
        .args([
            "--json",
            "run",
            "keyed@cursor",
            "Reply with pong",
            "--clean",
            "--repo",
            repository.path().to_str().expect("repository path"),
            "--binary",
            provider.to_str().expect("provider path"),
        ])
        .env_remove("CURSOR_API_KEY")
        .env("PROFILE_CURSOR_KEY", "profile-cursor-key")
        .output()
        .expect("run output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let seen = std::fs::read_to_string(&capture).expect("environment capture");
    assert!(seen.contains("KEY=profile-cursor-key"), "{seen}");
}

#[cfg(unix)]
#[test]
fn cursor_keychain_failure_carries_the_sandbox_hint() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        r"#!/bin/sh
cat > /dev/null
i=0
while [ $i -lt 8 ]; do
  echo 'ERROR: failed to copy trust settings of system certificate-25291' >&2
  i=$((i + 1))
done
echo 'Error: Authentication required.' >&2
exit 1
",
    );

    let output = cursor_run(home.path(), repository.path(), &provider)
        .env_remove("SANDBOX_RUNTIME")
        .output()
        .expect("run output");
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
    assert_eq!(result["kind"], "exit");
    let stderr = result["details"]["stderr"].as_str().expect("stderr");
    assert!(stderr.starts_with("hint: "), "{stderr}");
    assert!(stderr.contains("Authentication required"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn cursor_login_failure_gets_the_hint_only_inside_a_sandbox() {
    let home = tempfile::tempdir().expect("home");
    let repository = tempfile::tempdir().expect("repository");
    let provider = executable_provider(
        home.path(),
        "fake-cursor-agent",
        "#!/bin/sh\ncat > /dev/null\necho 'Error: Authentication required.' >&2\nexit 1\n",
    );

    for (sandboxed, hinted) in [(true, true), (false, false)] {
        let mut command = cursor_run(home.path(), repository.path(), &provider);
        if sandboxed {
            command.env("SANDBOX_RUNTIME", "1");
        } else {
            command.env_remove("SANDBOX_RUNTIME");
        }
        let output = command.output().expect("run output");
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("run JSON");
        let stderr = result["details"]["stderr"].as_str().expect("stderr");
        assert_eq!(stderr.starts_with("hint: "), hinted, "{stderr}");
    }
}
