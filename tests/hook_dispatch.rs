//! The dispatcher and the installer as the harness meets them: the real
//! binary, a scratch HOME, and a scratch state directory. Each test is one
//! scenario a review proved with a script against the earlier code.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

/// A scratch HOME, state directory, and handler directory under TMPDIR.
struct Scene {
    root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        let scene = Self {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(scene.home().join(".config/rune")).unwrap();
        std::fs::create_dir_all(scene.path("bin")).unwrap();
        scene
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }

    fn home(&self) -> PathBuf {
        self.path("home")
    }

    fn state(&self) -> PathBuf {
        self.path("state")
    }

    fn config(&self, yaml: &str) {
        std::fs::write(self.home().join(".config/rune/config.yaml"), yaml).unwrap();
    }

    /// A shell handler under `bin/`, its path for an `exec` list.
    fn handler(&self, name: &str, body: &str) -> String {
        let path = self.path(&format!("bin/{name}.sh"));
        std::fs::write(&path, body).unwrap();
        path.display().to_string()
    }

    /// `rune` with the scratch HOME and state, and none of the owner's
    /// directory variables.
    fn rune(&self) -> Command {
        let mut command = Command::cargo_bin("rune").unwrap();
        command
            .env("HOME", self.home())
            .env("RUNE_STATE_DIR", self.state())
            .env_remove("XDG_STATE_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CLAUDE_PROJECT_DIR")
            .env_remove("GROK_SESSION_ID")
            .current_dir(self.root.path());
        command
    }

    fn write_plan(&self) {
        self.rune()
            .args(["hook", "list", "--write"])
            .assert()
            .success();
    }
}

/// One dispatch: the payload on stdin, the answer back.
struct Answer {
    code: i32,
    stdout: String,
    stderr: String,
}

fn dispatch(mut command: Command, args: &[&str], payload: &str) -> Answer {
    let output = command
        .args(["hook", "run"])
        .args(args)
        .write_stdin(payload)
        .output()
        .unwrap();
    Answer {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn claude_bash(cwd: &Path, command: &str) -> String {
    serde_json::json!({
        "hook_event_name": "PreToolUse", "session_id": "s1", "cwd": cwd,
        "tool_name": "Bash", "tool_use_id": "t1", "tool_input": {"command": command}
    })
    .to_string()
}

fn decision(answer: &Answer) -> String {
    serde_json::from_str::<Value>(answer.stdout.trim())
        .ok()
        .and_then(|v| {
            v["hookSpecificOutput"]["permissionDecision"]
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_default()
}

const CLAUDE_PRE: [&str; 4] = ["--harness", "claude", "--native-event", "PreToolUse"];

fn denying_guard(scene: &Scene) {
    let guard = scene.handler(
        "guard",
        "cat > /dev/null; printf '{\"effect\":\"deny\",\"reason\":\"guard says no\"}'",
    );
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: guard\n      exec: [sh, {guard}]\n      events: [tool.before]\n      on_failure: deny\n"
    ));
}

// -------------------------------------------------------------- install ----

const DCG_GUARD: &str = "hooks:\n  handlers:\n    - id: dcg\n      exec: [rune-hook-dcg]\n      events: [tool.before]\n      on_failure: deny\n";

fn claude_settings(scene: &Scene) -> PathBuf {
    scene.home().join(".claude/settings.json")
}

fn json_at(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn an_unreadable_codex_table_stops_the_install_before_anything_is_written() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    std::fs::create_dir_all(scene.home().join(".codex")).unwrap();
    let dcg = scene.home().join(".local/bin/dcg").display().to_string();
    let before = format!(
        r#"{{"hooks":{{"PreToolUse":[{{"matcher":"Bash","hooks":[{{"type":"command","command":"{dcg}"}}]}}]}}}}"#
    );
    std::fs::write(claude_settings(&scene), &before).unwrap();
    std::fs::write(scene.home().join(".codex/hooks.json"), "{ not json").unwrap();
    scene.config(DCG_GUARD);
    let output = scene.rune().args(["hook", "install"]).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("hooks.json is not JSON"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(claude_settings(&scene)).unwrap(),
        before,
        "the Claude table is untouched"
    );
    assert!(!scene.state().join("hooks/manifest.json").exists());
    assert!(!scene.state().join("hooks/plan.json").exists());
}

#[cfg(unix)]
#[test]
fn install_writes_through_a_linked_settings_file_and_keeps_its_mode_and_order() {
    use std::os::unix::fs::PermissionsExt as _;
    let scene = Scene::new();
    let dotfiles = scene.home().join("dotfiles/claude-settings.json");
    std::fs::create_dir_all(dotfiles.parent().unwrap()).unwrap();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    std::fs::write(
        &dotfiles,
        r#"{"permissions": {"allow": ["Bash(git status)"]}, "model": "opus", "env": {"API_TOKEN": "x"}}"#,
    )
    .unwrap();
    std::fs::set_permissions(&dotfiles, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&dotfiles, claude_settings(&scene)).unwrap();
    scene.config(DCG_GUARD);
    scene.rune().args(["hook", "install"]).assert().success();
    let link = claude_settings(&scene);
    assert!(link.is_symlink(), "the link stays a link");
    let mode = std::fs::metadata(&dotfiles).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let text = std::fs::read_to_string(&dotfiles).unwrap();
    let order: Vec<usize> = ["\"permissions\"", "\"model\"", "\"env\"", "\"hooks\""]
        .iter()
        .map(|key| text.find(key).unwrap_or(usize::MAX))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{text}");
    assert!(json_at(&dotfiles)["hooks"]["PreToolUse"].is_array());
}

#[cfg(unix)]
#[test]
fn an_install_from_a_second_copy_of_rune_keeps_one_dispatcher_per_event() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    scene.rune().args(["hook", "install"]).assert().success();
    let copy = scene.path("second/rune");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::copy(assert_cmd::cargo::cargo_bin("rune"), &copy).unwrap();
    let mut second = Command::new(&copy);
    second
        .env("HOME", scene.home())
        .env("RUNE_STATE_DIR", scene.state())
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .args(["hook", "install"])
        .assert()
        .success();
    let settings = json_at(&claude_settings(&scene));
    let commands: Vec<String> = settings["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["hooks"].as_array().unwrap().clone())
        .map(|h| h["command"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(commands.len(), 1, "{commands:?}");
    assert!(
        commands[0].contains("/second/rune hook run"),
        "{commands:?}"
    );
}

#[test]
fn rune_install_json_prints_one_document_with_the_hook_report_inside() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    let source = scene.path("module");
    std::fs::create_dir_all(source.join("rules")).unwrap();
    std::fs::write(
        source.join("module.yaml"),
        "name: hook-json-fixture\nversion: 0.1.0\ndescription: hook json fixture\nevents: []\n",
    )
    .unwrap();
    std::fs::write(source.join("defaults.yaml"), "").unwrap();
    std::fs::write(source.join("rules/Fixture.md"), "a rule\n").unwrap();
    let target = scene.path("target");
    std::fs::create_dir_all(&target).unwrap();
    let output = scene
        .rune()
        .args([
            "install",
            "--source",
            source.to_str().unwrap(),
            "--target",
            target.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|error| panic!("not one JSON document ({error}): {stdout}"));
    assert!(
        document["hooks"]["written"]
            .as_array()
            .is_some_and(|w| w.iter().any(|e| e == "claude:PreToolUse")),
        "{document}"
    );
}

// ------------------------------------------------------------- adapters ----

#[test]
fn dcg_judges_the_codex_shell_tools() {
    if std::process::Command::new("dcg")
        .arg("--version")
        .output()
        .is_err()
    {
        return; // dcg is not installed on this machine
    }
    let scene = Scene::new();
    for (tool, input) in [
        (
            "exec_command",
            serde_json::json!({"cmd": "git reset --hard"}),
        ),
        (
            "shell_command",
            serde_json::json!({"command": "git reset --hard"}),
        ),
    ] {
        let payload = serde_json::json!({
            "v": 1, "harness": "codex", "event": "tool.before", "native_event": "PreToolUse",
            "cwd": scene.root.path(), "tool": {"name": tool, "id": null, "input": input, "response": null},
            "raw": {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input}
        });
        let output = scene
            .rune()
            .args(["hook", "adapter", "dcg"])
            .write_stdin(payload.to_string())
            .output()
            .unwrap();
        let result: Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        assert_eq!(
            result["effect"],
            "deny",
            "{tool}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

// ---------------------------------------------------- the event deadline ----

#[cfg(unix)]
#[test]
fn a_guard_that_ignores_sigterm_is_denied_inside_the_harness_deadline() {
    let scene = Scene::new();
    let slow = scene.handler("slow", "trap '' TERM; cat > /dev/null; sleep 20");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: guard\n      exec: [sh, {slow}]\n      events: [tool.before]\n      timeout_ms: 2500\n      on_failure: deny\n"
    ));
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.rune().args(["hook", "install"]).assert().success();
    let settings: Value = serde_json::from_str(
        &std::fs::read_to_string(scene.home().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    let entry = &settings["hooks"]["PreToolUse"][0]["hooks"][0];
    let deadline = std::time::Duration::from_secs(entry["timeout"].as_u64().unwrap());
    assert_eq!(entry["onFailure"], "block", "{entry}");
    let started = std::time::Instant::now();
    let answer = dispatch(
        scene.rune(),
        &CLAUDE_PRE,
        &claude_bash(scene.root.path(), "rm -rf /tmp/x"),
    );
    let took = started.elapsed();
    assert_eq!(decision(&answer), "deny", "{}", answer.stderr);
    assert!(
        took < deadline,
        "the answer took {took:?}, past the {deadline:?} the harness waits"
    );
}

// ------------------------------------------- the plan that does not load ----

#[test]
fn a_missing_plan_denies_when_the_registration_carries_a_deny_guard() {
    let scene = Scene::new();
    denying_guard(&scene);
    let payload = claude_bash(scene.root.path(), "rm -rf /tmp/x");
    let missing = scene.path("nowhere/plan.json").display().to_string();
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &missing, "--on-failure", "deny"]);
    let answer = dispatch(scene.rune(), &args, &payload);
    assert_eq!(answer.code, 0);
    assert_eq!(decision(&answer), "deny", "{}", answer.stderr);
    // A registration without a deny guard proceeds and says why.
    let answer = dispatch(scene.rune(), &CLAUDE_PRE, &payload);
    assert_eq!(answer.stdout, "");
    assert!(
        answer.stderr.contains("no compiled hook plan"),
        "{}",
        answer.stderr
    );
}

#[test]
fn a_truncated_plan_denies_under_a_deny_registration() {
    let scene = Scene::new();
    denying_guard(&scene);
    scene.write_plan();
    let plan = scene.state().join("hooks/plan.json");
    std::fs::write(&plan, "{\"version\":1,\"harn").unwrap();
    let plan = plan.display().to_string();
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &plan, "--on-failure", "deny"]);
    let answer = dispatch(
        scene.rune(),
        &args,
        &claude_bash(scene.root.path(), "rm -rf /tmp/x"),
    );
    assert_eq!(decision(&answer), "deny", "{}", answer.stderr);
    assert!(
        answer.stderr.contains("is not a hook plan"),
        "{}",
        answer.stderr
    );
}

#[test]
fn an_inherited_grok_variable_does_not_switch_the_guard_off() {
    let scene = Scene::new();
    denying_guard(&scene);
    scene.write_plan();
    let mut command = scene.rune();
    command.env("GROK_SESSION_ID", "inherited");
    let answer = dispatch(
        command,
        &CLAUDE_PRE,
        &claude_bash(scene.root.path(), "rm -rf /tmp/x"),
    );
    assert_eq!(decision(&answer), "deny", "{}", answer.stderr);
    // Grok's own payload on the Claude registration still runs nothing.
    let grok = r#"{"hookEventName":"pre_tool_use","hook_event_name":"PreToolUse","sessionId":"g1","toolName":"Bash","toolInput":{"command":"rm -rf /tmp/x"}}"#;
    let answer = dispatch(scene.rune(), &CLAUDE_PRE, grok);
    assert_eq!(answer.stdout, "");
    assert!(answer.stderr.contains("fired by grok"), "{}", answer.stderr);
}

#[test]
fn the_registration_finds_the_plan_under_another_state_directory() {
    let scene = Scene::new();
    denying_guard(&scene);
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    std::fs::write(scene.home().join(".claude/settings.json"), "{}").unwrap();
    let xdg = scene.path("xdg");
    let mut install = scene.rune();
    install
        .env_remove("RUNE_STATE_DIR")
        .env("XDG_STATE_HOME", &xdg)
        .args(["hook", "install"])
        .assert()
        .success();
    assert!(xdg.join("rune/hooks/plan.json").is_file());
    let settings: Value = serde_json::from_str(
        &std::fs::read_to_string(scene.home().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    let rendered = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_string();
    // The harness runs the rendered command with neither variable set.
    let words: Vec<&str> = rendered.split_whitespace().collect();
    assert_eq!(words[1..3], ["hook", "run"], "{rendered}");
    let mut command = scene.rune();
    command.env_remove("RUNE_STATE_DIR");
    let output = command
        .args(&words[1..])
        .write_stdin(claude_bash(scene.root.path(), "rm -rf /tmp/x"))
        .output()
        .unwrap();
    let answer = Answer {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };
    assert_eq!(decision(&answer), "deny", "{rendered}: {}", answer.stderr);
    assert!(answer.stdout.contains("guard says no"));
}
