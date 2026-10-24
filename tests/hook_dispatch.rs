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
