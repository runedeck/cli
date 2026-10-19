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
