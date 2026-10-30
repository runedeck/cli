//! The agent policy as the harness meets it: the real binary, a scratch
//! HOME with a policy file, and a scratch state directory. A refused
//! handler would leave a marker file if it started, so its absence proves
//! the dispatcher never started it.

mod support;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

/// A scratch HOME, state directory, and handler directory under TMPDIR.
struct Scene {
    _root: tempfile::TempDir,
    root: PathBuf,
}

impl Scene {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        // The real path, so the paths the test writes and the paths the
        // policy resolves read alike.
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let scene = Self { _root: temp, root };
        for dir in ["home/.config/rune", "home/work", "home/other", "bin"] {
            std::fs::create_dir_all(scene.path(dir)).unwrap();
        }
        scene
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn home(&self) -> PathBuf {
        self.path("home")
    }

    fn state(&self) -> PathBuf {
        self.path("state")
    }

    fn marker(&self) -> PathBuf {
        self.path("guard-started")
    }

    /// The hook config: a guard that names `guard.deny`, touches the
    /// marker when it starts, and denies every call.
    fn config(&self, policy_key: bool) {
        let guard = self.path("bin/guard.sh");
        std::fs::write(
            &guard,
            format!(
                "cat > /dev/null; touch '{}'; printf '{{\"effect\":\"deny\",\"reason\":\"guard says no\"}}'",
                self.marker().display()
            ),
        )
        .unwrap();
        let key = if policy_key {
            "\n      policy: guard.deny"
        } else {
            ""
        };
        std::fs::write(
            self.home().join(".config/rune/config.yaml"),
            format!(
                "hooks:\n  handlers:\n    - id: guard\n      exec: [sh, {}]\n      events: [tool.before]{key}\n",
                guard.display()
            ),
        )
        .unwrap();
    }

    /// The user policy: `guard.*` everywhere, `guard.deny` revoked under
    /// `~/work`.
    fn policy(&self) {
        std::fs::write(
            self.home().join(".config/rune/policy.yaml"),
            "version: 1\ngrant: [guard.*]\nscopes:\n  - path: ~/work\n    revoke: [guard.deny]\n",
        )
        .unwrap();
    }

    fn rune(&self) -> Command {
        let mut command = support::rune();
        command
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("RUNE_STATE_DIR", self.state())
            .current_dir(&self.root);
        command
    }

    fn write_plan(&self) {
        self.rune()
            .args(["hook", "list", "--write"])
            .assert()
            .success();
    }

    /// Fire Claude's `PreToolUse` in `cwd` for `session` and return the
    /// permission decision, empty when the answer allows.
    fn fire(&self, cwd: &Path, session: &str) -> String {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse", "session_id": session, "cwd": cwd,
            "tool_name": "Bash", "tool_use_id": "t1", "tool_input": {"command": "ls"}
        })
        .to_string();
        let output = self
            .rune()
            .args([
                "hook",
                "run",
                "--harness",
                "claude",
                "--native-event",
                "PreToolUse",
            ])
            .write_stdin(payload)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str::<Value>(stdout.trim())
            .ok()
            .and_then(|v| {
                v["hookSpecificOutput"]["permissionDecision"]
                    .as_str()
                    .map(str::to_string)
            })
            .unwrap_or_default()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.state().join("hooks/run.log")).unwrap_or_default()
    }
}

#[test]
fn a_refused_handler_never_starts_and_the_log_names_the_layer() {
    let scene = Scene::new();
    scene.config(true);
    scene.policy();
    scene.write_plan();
    let decision = scene.fire(&scene.home().join("work"), "s1");
    assert_eq!(decision, "", "the refused guard denied nothing");
    assert!(!scene.marker().exists(), "the refused guard never started");
    let log = scene.log();
    assert!(
        log.contains("guard skipped by policy: guard.deny (scope ~/work)"),
        "{log}"
    );
}

#[test]
fn an_allowed_handler_starts_and_denies() {
    let scene = Scene::new();
    scene.config(true);
    scene.policy();
    scene.write_plan();
    assert_eq!(scene.fire(&scene.home().join("other"), "s1"), "deny");
    assert!(scene.marker().exists());
}

#[test]
fn a_session_switch_file_refuses_the_handler_for_its_session() {
    let scene = Scene::new();
    scene.config(true);
    scene.policy();
    scene.write_plan();
    let switches = scene.state().join("policy/session");
    std::fs::create_dir_all(&switches).unwrap();
    std::fs::write(switches.join("s1.yaml"), "disabled: [guard.*]\n").unwrap();
    assert_eq!(scene.fire(&scene.home().join("other"), "s1"), "");
    assert!(!scene.marker().exists());
    assert!(
        scene
            .log()
            .contains("guard skipped by policy: guard.deny (session s1)"),
        "{}",
        scene.log()
    );
    assert_eq!(scene.fire(&scene.home().join("other"), "s2"), "deny");
    assert!(scene.marker().exists());
}

#[test]
fn a_handler_without_a_policy_key_runs_with_no_policy_file() {
    let scene = Scene::new();
    scene.config(false);
    scene.write_plan();
    assert_eq!(scene.fire(&scene.home().join("work"), "s1"), "deny");
    assert!(scene.marker().exists());
}

#[test]
fn a_missing_policy_file_refuses_a_named_handler() {
    let scene = Scene::new();
    scene.config(true);
    scene.write_plan();
    assert_eq!(scene.fire(&scene.home().join("other"), "s1"), "");
    assert!(!scene.marker().exists());
    assert!(
        scene
            .log()
            .contains("guard skipped by policy: guard.deny (policy file)"),
        "{}",
        scene.log()
    );
}

#[test]
fn the_hook_list_shows_the_policy_name() {
    let scene = Scene::new();
    scene.config(true);
    let output = scene.rune().args(["hook", "list"]).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(
        text.lines()
            .any(|l| l.contains(" guard ") && l.ends_with("warn  guard.deny")),
        "{text}"
    );
}

#[test]
fn the_policy_check_prints_one_line_per_name_and_exits_1_on_a_refusal() {
    let scene = Scene::new();
    scene.policy();
    let output = scene
        .rune()
        .args(["policy", "check", "--path"])
        .arg(scene.home().join("work"))
        .args(["guard.deny", "guard.watch"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        [
            "guard.deny   refused  scope ~/work (revoke guard.deny)",
            "guard.watch  allowed  top level (grant guard.*)",
        ]
    );
    scene
        .rune()
        .args(["policy", "check", "--path"])
        .arg(scene.home().join("other"))
        .arg("guard.deny")
        .assert()
        .code(0);
}

#[test]
fn the_policy_check_reads_the_session_switch() {
    let scene = Scene::new();
    scene.policy();
    let switches = scene.state().join("policy/session");
    std::fs::create_dir_all(&switches).unwrap();
    std::fs::write(switches.join("s1.yaml"), "disabled: [guard.watch]\n").unwrap();
    let output = scene
        .rune()
        .args(["policy", "check", "--session", "s1", "--path"])
        .arg(scene.home().join("other"))
        .arg("guard.watch")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "guard.watch  refused  session s1 (disabled guard.watch)"
    );
}

#[test]
fn the_policy_check_honours_the_environment_switch() {
    let scene = Scene::new();
    scene.policy();
    let output = scene
        .rune()
        .env("RUNE_POLICY_DISABLE", "guard.*")
        .args(["policy", "check", "--path"])
        .arg(scene.home().join("other"))
        .arg("guard.watch")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("refused  RUNE_POLICY_DISABLE (RUNE_POLICY_DISABLE guard.*)")
    );
}
