//! The turn wrapper against the real binary and a real jj workspace: the
//! owner's switch and reset, and the dispatcher wrapping a turn on Stop.
//! Each test runs in a scratch HOME, state directory, and workspace under
//! TMPDIR. Skips when jj is missing.

mod support;

use std::ffi::OsString;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command as Process, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serde_json::Value;

/// The wrapper alone, on every event that shows a session in a workspace.
const WRAPPER: &str = "hooks:\n  handlers:\n    - id: turn-wrap\n      exec: [rune-hook-turn-wrap]\n      events: [prompt.before, tool.before, tool.after, turn.finish]\n";

/// The wrapper with room to be held mid-commit.
const PATIENT_WRAPPER: &str = "hooks:\n  events:\n    turn.finish: {budget_ms: 20000}\n  handlers:\n    - id: turn-wrap\n      exec: [rune-hook-turn-wrap]\n      events: [prompt.before, tool.before, tool.after, turn.finish]\n      timeout_ms: 15000\n";

const CLAUDE_PROMPT: [&str; 4] = ["--harness", "claude", "--native-event", "UserPromptSubmit"];
const CLAUDE_WRITE: [&str; 4] = ["--harness", "claude", "--native-event", "PostToolUse"];
const CLAUDE_STOP: [&str; 4] = ["--harness", "claude", "--native-event", "Stop"];

/// The tail of the card of a wrapped bare change on Claude.
const UNDO_TAIL: &str =
    " --into @ -u, or /rewind; the undo conflicts when a later turn changed the same file.";

const RESET_TAIL: &str =
    "; the edits stay in the working copy. Sort it out, then run rune hook wrap reset in it";

struct Scene {
    root: tempfile::TempDir,
}

/// The variables that mark a model session. The tests run inside one,
/// and the switch refuses there.
fn session_marks() -> Vec<OsString> {
    std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| {
            let key = key.to_string_lossy();
            key.starts_with("CLAUDE") || key.starts_with("CODEX") || key == "HARNESS_AUTOMATED"
        })
        .collect()
}

impl Scene {
    /// A scene with a colocated jj workspace under `repo/`, or `None`
    /// when jj is not installed.
    fn new() -> Option<Self> {
        let present = Process::new("jj")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok();
        if !present {
            eprintln!("skipped: jj is not installed");
            return None;
        }
        let scene = Self {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(scene.home().join(".config/rune")).unwrap();
        std::fs::create_dir_all(scene.path("bin")).unwrap();
        let init = scene
            .jj()
            .args(["git", "init", "--colocate"])
            .arg(scene.repo())
            .output()
            .unwrap();
        assert!(
            init.status.success(),
            "jj git init: {}",
            String::from_utf8_lossy(&init.stderr)
        );
        Some(scene)
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

    fn repo(&self) -> PathBuf {
        self.path("repo")
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

    /// jj with the scene's HOME and an identity of its own.
    fn jj(&self) -> Process {
        let mut command = Process::new("jj");
        command
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("JJ_USER", "Owner")
            .env("JJ_EMAIL", "owner@example.com")
            .env_remove("JJ_CONFIG")
            .stdin(Stdio::null());
        command
    }

    fn run_jj(&self, args: &[&str]) -> String {
        let output = self
            .jj()
            .current_dir(self.repo())
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "jj {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// The description of each change from `@` down to the root, newest
    /// first, `(bare)` for one without.
    fn descriptions(&self) -> Vec<String> {
        self.run_jj(&[
            "log",
            "-r",
            "::@ ~ root()",
            "--no-graph",
            "-T",
            r#"if(description, description.first_line(), "(bare)") ++ "\n""#,
        ])
        .lines()
        .map(str::to_string)
        .collect()
    }

    /// `rune` with the scene's HOME, state, jj identity, and the workspace
    /// as its directory, outside any model session.
    fn rune(&self) -> Command {
        let mut command = support::rune();
        for key in session_marks() {
            command.env_remove(key);
        }
        command
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("RUNE_STATE_DIR", self.state())
            .env("JJ_USER", "Owner")
            .env("JJ_EMAIL", "owner@example.com")
            .env_remove("JJ_CONFIG")
            .current_dir(self.repo());
        command
    }

    /// `rune` as a process the test waits for itself.
    fn rune_process(&self) -> Process {
        let mut command = support::rune_process();
        for key in session_marks() {
            command.env_remove(key);
        }
        command
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("RUNE_STATE_DIR", self.state())
            .env("JJ_USER", "Owner")
            .env("JJ_EMAIL", "owner@example.com")
            .env_remove("JJ_CONFIG")
            .current_dir(self.repo());
        command
    }

    fn write_plan(&self) {
        self.rune()
            .args(["hook", "list", "--write"])
            .assert()
            .success();
    }

    fn switch_on(&self) {
        self.rune().args(["hook", "wrap", "on"]).assert().success();
    }

    fn edit(&self, name: &str, text: &str) {
        std::fs::write(self.repo().join(name), text).unwrap();
    }

    fn slot(&self) -> PathBuf {
        self.repo().join(".jj/turn-wrap.open")
    }

    /// The keys in the slot, none when there is no slot.
    fn keys(&self) -> Vec<String> {
        let Ok(text) = std::fs::read_to_string(self.slot()) else {
            return Vec::new();
        };
        let slot: Value = serde_json::from_str(&text).unwrap();
        slot["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_string())
            .collect()
    }

    /// How the card names `key`: by the harness and the first touch the
    /// slot records for it.
    fn label(&self, key: &str) -> String {
        let text = std::fs::read_to_string(self.slot()).unwrap();
        let slot: Value = serde_json::from_str(&text).unwrap();
        let who = &slot["who"][key];
        let harness = who["harness"]
            .as_str()
            .unwrap_or_else(|| panic!("{key} in {slot}"));
        format!(
            "a {harness} session (first touch {})",
            who["since"].as_str().unwrap()
        )
    }

    /// The card's names for every key in the slot, in key order.
    fn labels(&self) -> String {
        self.keys()
            .iter()
            .map(|key| self.label(key))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The touch markers left by prompts that could not take the lock.
    fn markers(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.repo().join(".jj/turn-wrap.touched")) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// One dispatch: the payload on stdin, the answer back.
    fn dispatch(&self, args: &[&str], payload: &Value) -> Answer {
        let output = self
            .rune()
            .args(["hook", "run"])
            .args(args)
            .write_stdin(payload.to_string())
            .output()
            .unwrap();
        Answer::from(&output)
    }

    fn prompt_payload(&self, session: &str) -> Value {
        serde_json::json!({
            "hook_event_name": "UserPromptSubmit", "session_id": session,
            "cwd": self.repo(), "prompt": "edit a file"
        })
    }

    fn stop_payload(&self, session: &str) -> Value {
        serde_json::json!({
            "hook_event_name": "Stop", "session_id": session,
            "cwd": self.repo(), "stop_hook_active": false
        })
    }

    fn prompt(&self, session: &str) -> Answer {
        self.dispatch(&CLAUDE_PROMPT, &self.prompt_payload(session))
    }

    /// A `PostToolUse` of a Write to `file`, fired from `cwd`: the session
    /// wrote the file, so the test writes it too.
    fn wrote(&self, session: &str, cwd: &std::path::Path, file: &std::path::Path) -> Answer {
        std::fs::write(file, "written\n").unwrap();
        let payload = serde_json::json!({
            "hook_event_name": "PostToolUse", "session_id": session, "cwd": cwd,
            "tool_name": "Write", "tool_input": {"file_path": file}, "tool_response": {}
        });
        self.dispatch(&CLAUDE_WRITE, &payload)
    }

    /// A jj on PATH that sleeps 5 s on every command named in `slow`
    /// while the file `flag` exists, and runs the real jj otherwise.
    fn slow_jj(&self, slow: &str, flag: &std::path::Path) -> OsString {
        let real = String::from_utf8(
            Process::new("sh")
                .args(["-c", "command -v jj"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let shim = self.path("shim");
        std::fs::create_dir_all(&shim).unwrap();
        std::fs::write(
            shim.join("jj"),
            format!(
                "#!/bin/sh\nfor a in \"$@\"; do\n  if [ \"$a\" = {slow} ] && [ -e '{}' ]; then sleep 5; fi\ndone\nexec '{}' \"$@\"\n",
                flag.display(),
                real.trim()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(shim.join("jj"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        std::env::join_paths(
            std::iter::once(shim).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap()
    }

    /// A `PostToolUse` of a Bash tool that ran `command` from `cwd`.
    fn ran(&self, session: &str, cwd: &std::path::Path, command: &str) -> Answer {
        let payload = serde_json::json!({
            "hook_event_name": "PostToolUse", "session_id": session, "cwd": cwd,
            "tool_name": "Bash", "tool_input": {"command": command}, "tool_response": {}
        });
        self.dispatch(&CLAUDE_WRITE, &payload)
    }

    fn stop(&self, session: &str) -> Answer {
        self.dispatch(&CLAUDE_STOP, &self.stop_payload(session))
    }

    /// A prompt and a stop around one edit, as one session's turn.
    fn turn(&self, session: &str, name: &str, text: &str) -> Answer {
        let prompt = self.prompt(session);
        assert_eq!(prompt.code, 0, "{}", prompt.stderr);
        self.edit(name, text);
        self.stop(session)
    }
}

struct Answer {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Answer {
    fn from(output: &std::process::Output) -> Self {
        Self {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// The `systemMessage` of the answer, empty when there is none.
    fn message(&self) -> String {
        serde_json::from_str::<Value>(self.stdout.trim())
            .ok()
            .and_then(|v| v["systemMessage"].as_str().map(str::to_string))
            .unwrap_or_default()
    }
}

/// Wait for a file to appear, within a bound.
fn wait_for(path: &std::path::Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ------------------------------------------------------------ the switch ----

#[test]
fn the_switch_round_trips_and_reads_from_a_subdirectory() {
    let Some(scene) = Scene::new() else { return };
    scene
        .rune()
        .args(["hook", "wrap", "status"])
        .assert()
        .success()
        .stdout("off\n");
    scene
        .rune()
        .args(["hook", "wrap", "on"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("turn wrap on in "));
    assert!(scene.repo().join(".jj/turn-wrap").is_file());
    std::fs::create_dir_all(scene.repo().join("src/deep")).unwrap();
    let output = scene
        .rune()
        .current_dir(scene.repo().join("src/deep"))
        .args(["hook", "wrap", "status", "--json"])
        .output()
        .unwrap();
    let out: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(out["turn_wrap"], "on");
    assert_eq!(
        out["root"].as_str().map(PathBuf::from),
        Some(scene.repo().canonicalize().unwrap())
    );
    scene
        .rune()
        .args(["hook", "wrap", "off"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("turn wrap off in "));
    assert!(!scene.repo().join(".jj/turn-wrap").exists());
    // Off twice is still off.
    scene
        .rune()
        .args(["hook", "wrap", "off"])
        .assert()
        .success();
    scene
        .rune()
        .args(["hook", "wrap", "status"])
        .assert()
        .success()
        .stdout("off\n");
}

#[test]
fn the_switch_refuses_outside_a_jj_workspace() {
    let Some(scene) = Scene::new() else { return };
    let output = scene
        .rune()
        .current_dir(scene.root.path())
        .args(["hook", "wrap", "on", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let out: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(out["code"], "hook.wrap_no_jj");
    assert_eq!(out["fix_command"], "jj git init --colocate");
    assert!(
        !scene.root.path().join(".jj").exists(),
        "the switch made nothing"
    );
}

#[test]
fn the_switch_and_the_reset_refuse_inside_a_model_session() {
    let Some(scene) = Scene::new() else { return };
    for (action, mark) in [("on", "CLAUDECODE"), ("reset", "CODEX_THREAD_ID")] {
        let output = scene
            .rune()
            .env(mark, "1")
            .args(["hook", "wrap", action, "--json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{action}");
        let out: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(out["code"], "hook.wrap_in_session", "{action}");
        let message = out["message"].as_str().unwrap();
        assert!(message.contains(&format!("({mark} is set)")), "{message}");
    }
    assert!(!scene.repo().join(".jj/turn-wrap").exists());
    // Status is a read, and a session may read it.
    scene
        .rune()
        .env("CLAUDECODE", "1")
        .args(["hook", "wrap", "status"])
        .assert()
        .success()
        .stdout("off\n");
}

#[test]
fn the_reset_forgets_every_session_that_touched_the_workspace() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    scene.prompt("s1");
    scene.prompt("s2");
    let keys = scene.keys();
    assert_eq!(keys.len(), 2, "{keys:?}");
    let output = scene
        .rune()
        .args(["hook", "wrap", "reset", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let out: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(out["turn_wrap"], "on");
    assert_eq!(out["reset"], serde_json::json!(keys));
    assert!(!scene.slot().exists());
    assert_eq!(scene.markers(), Vec::<String>::new());
    scene
        .rune()
        .args(["hook", "wrap", "reset"])
        .assert()
        .success()
        .stdout(predicates::str::ends_with(": no session had touched it\n"));
}

// ------------------------------------------------------------- the wrap ----

#[test]
fn a_turn_is_not_wrapped_while_the_switch_is_off() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    let stop = scene.turn("s1", "a.txt", "one\n");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert_eq!(stop.stdout.trim(), "", "{}", stop.stderr);
    assert_eq!(scene.descriptions(), ["(bare)"], "the working copy alone");
    assert!(!scene.slot().exists(), "no slot");
}

#[test]
fn a_turn_of_one_session_is_wrapped_as_one_described_change() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    let prompt = scene.prompt("s1");
    assert_eq!(prompt.code, 0, "{}", prompt.stderr);
    assert_eq!(scene.keys().len(), 1, "the prompt registered the session");
    scene.edit("a.txt", "one\n");
    let stop = scene.stop("s1");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    let message = stop.message();
    assert!(
        message.starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{message} {}",
        stop.stderr
    );
    assert!(message.contains("Undo: jj squash --from "), "{message}");
    assert!(message.ends_with(UNDO_TAIL), "{message}");
    let descriptions = scene.descriptions();
    assert_eq!(descriptions.len(), 2, "{descriptions:?}");
    assert_eq!(
        descriptions[0], "(bare)",
        "a fresh empty working copy on top"
    );
    assert!(
        descriptions[1].starts_with("claude: turn 20"),
        "{descriptions:?}"
    );
    assert!(!scene.slot().exists(), "the wrap forgot the session");
    // The change keeps the owner's author identity.
    let author = scene.run_jj(&["log", "-r", "@-", "--no-graph", "-T", "author.name()"]);
    assert_eq!(author.trim(), "Owner");
    // A second turn with no edit wraps nothing and says nothing.
    scene.prompt("s1");
    let stop = scene.stop("s1");
    assert_eq!(stop.stdout.trim(), "", "{}", stop.stderr);
    assert_eq!(scene.descriptions().len(), 2);
    assert!(!scene.slot().exists(), "an empty turn is forgotten too");
}

#[test]
fn a_turn_on_a_described_working_copy_keeps_the_owners_change() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    scene.edit("owner.txt", "the owner's edit\n");
    scene.run_jj(&["describe", "-m", "feat: the owner's work"]);
    let change = scene.run_jj(&["log", "-r", "@", "--no-graph", "-T", "change_id.short(8)"]);
    let stop = scene.turn("s1", "a.txt", "one\n");
    let message = stop.message();
    assert_eq!(
        message,
        format!(
            "turn-wrap: Turn added to {change} (feat: the owner's work): 1 file changed, 1 insertion(+), 0 deletions(-). Review: jj diff -r {change}. Continue in it: jj squash --from @ --into {change} -u. Undo: jj edit {change}, which drops the empty working copy."
        ),
        "{}",
        stop.stderr
    );
    assert_eq!(scene.descriptions(), ["(bare)", "feat: the owner's work"]);
    let files = scene.run_jj(&["diff", "-r", &change, "--name-only"]);
    assert_eq!(
        files, "a.txt\nowner.txt",
        "both edits stay in the owner's change"
    );
    // The card's undo: the owner is back in the change, and jj drops the
    // empty working copy the wrap made.
    scene.run_jj(&["edit", &change]);
    assert_eq!(scene.descriptions(), ["feat: the owner's work"]);
}

#[test]
fn a_session_that_wrote_a_file_from_another_directory_touched_the_workspace() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    scene.prompt("s1");
    let first = scene.keys().remove(0);
    scene.edit("a.txt", "one\n");
    // The second session prompted elsewhere, with no touch here, and then
    // wrote a file inside this workspace.
    let wrote = scene.wrote("s2", scene.root.path(), &scene.repo().join("b.txt"));
    assert_eq!(wrote.code, 0, "{}", wrote.stderr);
    assert_eq!(wrote.stdout.trim(), "", "{}", wrote.stderr);
    assert_eq!(scene.keys().len(), 2, "{:?}", scene.keys());
    let second = scene.keys().into_iter().find(|k| k != &first).unwrap();
    let stop = scene.stop("s1");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: this session and {} touched the workspace since the last wrap{RESET_TAIL}",
            scene.label(&second)
        )
    );
    assert_eq!(scene.descriptions(), ["(bare)"], "no change was made");
    // A write outside the workspace touches nothing in it.
    scene
        .rune()
        .args(["hook", "wrap", "reset"])
        .assert()
        .success();
    let wrote = scene.wrote("s2", scene.root.path(), &scene.root.path().join("c.txt"));
    assert_eq!(wrote.code, 0, "{}", wrote.stderr);
    assert!(!scene.slot().exists(), "{:?}", scene.keys());
}

#[test]
fn a_session_whose_shell_command_named_the_workspace_touched_it() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    scene.prompt("s1");
    let first = scene.keys().remove(0);
    scene.edit("a.txt", "one\n");
    // The second session ran a shell command from outside, with a `cd`
    // into this workspace, and the command wrote a file here.
    let command = format!("cd {} && printf two > b.txt", scene.repo().display());
    scene.edit("b.txt", "two");
    let ran = scene.ran("s2", scene.root.path(), &command);
    assert_eq!(ran.code, 0, "{}", ran.stderr);
    assert_eq!(ran.stdout.trim(), "", "{}", ran.stderr);
    assert_eq!(scene.keys().len(), 2, "{:?}", scene.keys());
    let second = scene.keys().into_iter().find(|k| k != &first).unwrap();
    let stop = scene.stop("s1");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: this session and {} touched the workspace since the last wrap{RESET_TAIL}",
            scene.label(&second)
        )
    );
    assert_eq!(scene.descriptions(), ["(bare)"], "no change was made");
    // A command that names no path in the workspace touches nothing in it.
    scene
        .rune()
        .args(["hook", "wrap", "reset"])
        .assert()
        .success();
    let ran = scene.ran("s2", scene.root.path(), "cargo test -- wrap");
    assert_eq!(ran.code, 0, "{}", ran.stderr);
    assert!(!scene.slot().exists(), "{:?}", scene.keys());
}

#[test]
fn the_wrapper_hears_every_touch_whatever_events_its_handler_declares() {
    let Some(scene) = Scene::new() else { return };
    // A config that subscribes the wrapper to the stop alone.
    scene.config(
        "hooks:\n  handlers:\n    - id: turn-wrap\n      exec: [rune-hook-turn-wrap]\n      events: [turn.finish]\n",
    );
    scene.write_plan();
    scene.switch_on();
    let prompt = scene.prompt("s1");
    assert_eq!(prompt.code, 0, "{}", prompt.stderr);
    assert_eq!(
        scene.keys().len(),
        1,
        "the prompt registered: {:?}",
        scene.keys()
    );
    scene.edit("a.txt", "one\n");
    let wrote = scene.wrote("s2", scene.root.path(), &scene.repo().join("b.txt"));
    assert_eq!(wrote.code, 0, "{}", wrote.stderr);
    assert_eq!(
        scene.keys().len(),
        2,
        "the write registered: {:?}",
        scene.keys()
    );
    let stop = scene.stop("s1");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert!(
        stop.message().starts_with(
            "turn-wrap: nothing wrapped: this session and a claude session (first touch "
        ) && stop.message().ends_with(RESET_TAIL),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions(), ["(bare)"], "no change was made");
}

#[test]
fn a_slow_stop_handler_cannot_hide_the_card_in_a_shared_workspace() {
    let Some(scene) = Scene::new() else { return };
    // A gate that outlives the whole 2000 ms budget: the dispatcher kills
    // it at its timeout.
    let gate = scene.handler("gate", "cat > /dev/null\nexec sleep 5\n");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: tests\n      exec: [sh, {gate}]\n      events: [turn.finish]\n      order: 20\n      timeout_ms: 5000\n    - id: turn-wrap\n      exec: [rune-hook-turn-wrap]\n      events: [prompt.before, tool.before, tool.after, turn.finish]\n      order: 10\n"
    ));
    scene.write_plan();
    scene.switch_on();
    scene.prompt("s1");
    scene.edit("a.txt", "one\n");
    scene.prompt("s2");
    assert_eq!(scene.keys().len(), 2, "{:?}", scene.keys());
    let stop = scene.stop("s1");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert!(
        stop.stderr.contains("tests failed: timed out"),
        "{}",
        stop.stderr
    );
    // The gate took what it could, and the card still names the reset.
    assert!(
        stop.message().contains("rune hook wrap reset"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions(), ["(bare)"], "no change was made");
}

#[test]
fn a_stop_without_a_session_id_is_nobodys_touch_and_blocks_the_wrap() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    scene.prompt("s1");
    let first = scene.keys().remove(0);
    scene.edit("a.txt", "one\n");
    let payload = serde_json::json!({
        "hook_event_name": "Stop", "cwd": scene.repo(), "stop_hook_active": false
    });
    let stop = scene.dispatch(&CLAUDE_STOP, &payload);
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: the payload names no session, so its touch is nobody's, and {} touched the workspace since the last wrap{RESET_TAIL}",
            scene.label(&first)
        )
    );
    assert_eq!(scene.descriptions(), ["(bare)"]);
    assert_eq!(scene.keys(), [first.clone(), "unknown".to_string()]);
    // The session with an id is not alone with the unknown one.
    let stop = scene.stop("s1");
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: this session and a session without an id touched the workspace since the last wrap{RESET_TAIL}"
        )
    );
    scene
        .rune()
        .args(["hook", "wrap", "reset"])
        .assert()
        .success()
        .stdout(predicates::str::ends_with(format!(
            ": forgot {first}, unknown\n"
        )));
    let stop = scene.stop("s1");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
}

#[test]
fn a_turn_after_a_failed_stop_handler_waits_for_the_next_stop() {
    let Some(scene) = Scene::new() else { return };
    let once = scene.path("failed");
    // A gate that fails at its first firing and passes after.
    let gate = scene.handler(
        "gate",
        &format!(
            "cat > /dev/null\nif [ -e '{}' ]; then printf '{{}}'; else : > '{}'; echo 'tests still running' >&2; exit 1; fi\n",
            once.display(),
            once.display()
        ),
    );
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: tests\n      exec: [sh, {gate}]\n      events: [turn.finish]\n      order: 20\n    - id: turn-wrap\n      exec: [rune-hook-turn-wrap]\n      events: [prompt.before, tool.before, tool.after, turn.finish]\n      order: 10\n"
    ));
    scene.write_plan();
    scene.switch_on();
    let stop = scene.turn("s1", "a.txt", "one\n");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert!(
        stop.stderr
            .contains("rune hook: tests failed: exited 1: tests still running (warn, proceeding)"),
        "{}",
        stop.stderr
    );
    assert_eq!(
        stop.message(),
        "turn-wrap: nothing wrapped: tests failed or timed out before the wrapper, so the turn may be unfinished; the edits stay in the working copy for this session's next turn"
    );
    assert_eq!(scene.descriptions(), ["(bare)"], "nothing committed");
    assert_eq!(scene.keys().len(), 1, "the session stays on record");
    // The next stop, with every handler through, wraps the whole turn.
    scene.edit("a.txt", "one\ntwo\n");
    let stop = scene.stop("s1");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed, 2 insertions(+)"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions().len(), 2);
}

#[test]
fn a_first_touch_keeps_its_key_when_the_snapshot_runs_past_the_timeout() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    let slow = scene.path("slow");
    std::fs::write(&slow, "").unwrap();
    let path = scene.slow_jj("log", &slow);
    // The first prompt's snapshot outlives the wrapper's 1200 ms: the
    // dispatcher kills it, and the harness goes on.
    let started = Instant::now();
    let output = scene
        .rune()
        .env("PATH", &path)
        .args(["hook", "run"])
        .args(CLAUDE_PROMPT)
        .write_stdin(scene.prompt_payload("s1").to_string())
        .output()
        .unwrap();
    let prompt = Answer::from(&output);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(prompt.code, 0, "{}", prompt.stderr);
    assert!(
        prompt.stderr.contains("turn-wrap failed: timed out"),
        "{}",
        prompt.stderr
    );
    assert_eq!(
        scene.keys().len(),
        1,
        "the key went down before the snapshot"
    );
    let state: Value =
        serde_json::from_str(&std::fs::read_to_string(scene.slot()).unwrap()).unwrap();
    assert_eq!(
        state.get("commit"),
        None,
        "the base was lost, not the touch"
    );
    std::fs::remove_file(&slow).unwrap();
    scene.edit("a.txt", "one\n");
    let stop = scene.stop("s1");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions().len(), 2);
}

#[test]
fn a_workspace_two_sessions_touched_is_wrapped_by_neither_until_the_owner_resets() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    scene.prompt("s1");
    let first = scene.keys().remove(0);
    scene.edit("a.txt", "one\n");
    let stop = scene.turn("s2", "b.txt", "two\n");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: this session and {} touched the workspace since the last wrap{RESET_TAIL}",
            scene.label(&first)
        )
    );
    assert_eq!(scene.descriptions(), ["(bare)"], "no change was made");
    assert_eq!(scene.keys().len(), 2, "both sessions stay on record");
    let second = scene.keys().into_iter().find(|k| k != &first).unwrap();
    let stop = scene.stop("s1");
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: this session and {} touched the workspace since the last wrap{RESET_TAIL}",
            scene.label(&second)
        )
    );
    assert_eq!(scene.descriptions(), ["(bare)"]);
    // A session that never prompted here registers at its stop, and is
    // not alone either. The card names each by its harness and first
    // touch, never by its hash.
    let listed = scene.labels();
    let stop = scene.stop("s3");
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: this session and {listed} touched the workspace since the last wrap{RESET_TAIL}"
        )
    );
    assert!(!stop.message().contains(&first), "{}", stop.message());
    assert_eq!(
        scene.keys().len(),
        3,
        "the stop registered the third session"
    );
    let listed = scene.keys().join(", ");
    // The owner sorts the working copy out and resets. The next turn
    // alone wraps everything left in the working copy.
    scene
        .rune()
        .args(["hook", "wrap", "reset"])
        .assert()
        .success()
        .stdout(format!(
            "turn wrap reset in {}: forgot {listed}\n",
            scene.repo().canonicalize().unwrap().display()
        ));
    let stop = scene.turn("s1", "c.txt", "three\n");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 3 files changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions().len(), 2);
}

#[test]
fn a_prompt_during_another_sessions_wrap_leaves_a_touch_the_wrap_cannot_miss() {
    let Some(scene) = Scene::new() else { return };
    scene.config(PATIENT_WRAPPER);
    scene.write_plan();
    scene.switch_on();
    // A jj that stops inside `commit` until the test lets it go, so the
    // wrapper holds the lock with the check done and the commit pending.
    let real = String::from_utf8(
        Process::new("sh")
            .args(["-c", "command -v jj"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let started = scene.path("started");
    let go = scene.path("go");
    let shim = scene.path("shim");
    std::fs::create_dir_all(&shim).unwrap();
    std::fs::write(
        shim.join("jj"),
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do\n  if [ \"$a\" = commit ]; then\n    : > '{}'\n    while [ ! -e '{}' ]; do sleep 0.01; done\n  fi\ndone\nexec '{}' \"$@\"\n",
            started.display(),
            go.display(),
            real.trim()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(shim.join("jj"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(shim).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    scene.prompt("s1");
    scene.edit("a.txt", "one\n");
    let mut wrapping = scene
        .rune_process()
        .env("PATH", path)
        .args(["hook", "run"])
        .args(CLAUDE_STOP)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wrapping
        .stdin
        .take()
        .unwrap()
        .write_all(scene.stop_payload("s1").to_string().as_bytes())
        .unwrap();
    wait_for(&started);
    // The second session's prompt cannot take the lock in time. It
    // leaves its touch as a marker instead, and the harness goes on.
    let prompt = scene.prompt("s2");
    assert_eq!(prompt.code, 0, "{}", prompt.stderr);
    assert_eq!(prompt.stdout.trim(), "", "{}", prompt.stderr);
    let markers = scene.markers();
    assert_eq!(markers.len(), 1, "{markers:?}");
    assert!(markers[0].starts_with("claude-"), "{markers:?}");
    std::fs::write(&go, "").unwrap();
    let stop = Answer::from(&wrapping.wait_with_output().unwrap());
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions().len(), 2);
    assert_eq!(scene.markers(), markers, "the wrap left the touch alone");
    // The next check folds the marker in: the second session is alone,
    // and its turn wraps its own edit.
    scene.edit("b.txt", "two\n");
    let stop = scene.stop("s2");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions().len(), 3);
    assert_eq!(scene.markers(), Vec::<String>::new());
    assert_eq!(scene.run_jj(&["diff", "-r", "@-", "--name-only"]), "b.txt");
}

#[test]
fn a_locked_slot_wraps_nothing_and_a_prompt_leaves_its_touch() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    let lock = scene
        .repo()
        .canonicalize()
        .unwrap()
        .join(".jj/turn-wrap.lock");
    let held = std::fs::File::create(&lock).unwrap();
    held.try_lock().unwrap();
    let stop = scene.turn("s1", "a.txt", "one\n");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    assert_eq!(
        stop.message(),
        format!(
            "turn-wrap: nothing wrapped: another wrapper held {} for 400 ms",
            lock.display()
        ),
        "{}",
        stop.stderr
    );
    assert_eq!(scene.descriptions(), ["(bare)"]);
    assert_eq!(scene.markers().len(), 1, "{:?}", scene.markers());
    assert!(!scene.slot().exists());
    drop(held);
    // With the lock free, the marker counts as the session's touch.
    let stop = scene.stop("s1");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.markers(), Vec::<String>::new());
}

#[test]
fn a_malformed_slot_wraps_nothing_until_the_owner_resets() {
    let Some(scene) = Scene::new() else { return };
    scene.config(WRAPPER);
    scene.write_plan();
    scene.switch_on();
    std::fs::write(scene.slot(), "{not json").unwrap();
    let stop = scene.turn("s1", "a.txt", "one\n");
    assert_eq!(stop.code, 0, "{}", stop.stderr);
    let message = stop.message();
    assert!(
        message.starts_with(&format!(
            "turn-wrap: nothing wrapped: {} is not a turn-wrap slot: ",
            scene.slot().canonicalize().unwrap().display()
        )),
        "{message}"
    );
    assert!(
        message.ends_with("; sort out the working copy, then run rune hook wrap reset"),
        "{message}"
    );
    assert_eq!(scene.descriptions(), ["(bare)"]);
    assert_eq!(
        scene.markers().len(),
        1,
        "the prompt's touch is kept beside the broken slot"
    );
    scene
        .rune()
        .args(["hook", "wrap", "reset"])
        .assert()
        .success()
        .stdout(predicates::str::ends_with(": no session had touched it\n"));
    assert!(!scene.slot().exists());
    let stop = scene.turn("s1", "a.txt", "one\ntwo\n");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
}

#[test]
fn a_continued_turn_is_wrapped_only_when_it_finally_stops() {
    let Some(scene) = Scene::new() else { return };
    let once = scene.path("steered");
    let steer = scene.handler(
        "steer",
        &format!(
            "cat > /dev/null\nif [ -e '{}' ]; then printf '{{}}'; else : > '{}'; printf '{{\"effect\":\"continue_turn\",\"reason\":\"more\"}}'; fi\n",
            once.display(),
            once.display()
        ),
    );
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: steer\n      exec: [sh, {steer}]\n      events: [turn.finish]\n      order: 200\n    - id: turn-wrap\n      exec: [rune-hook-turn-wrap]\n      events: [prompt.before, turn.finish]\n      order: 10\n"
    ));
    scene.write_plan();
    scene.switch_on();
    let stop = scene.turn("s1", "a.txt", "one\n");
    let out: Value = serde_json::from_str(stop.stdout.trim()).unwrap();
    assert_eq!(out["decision"], "block", "{}", stop.stderr);
    assert_eq!(out.get("systemMessage"), None, "{out}");
    assert_eq!(
        scene.descriptions(),
        ["(bare)"],
        "the turn goes on, nothing wrapped"
    );
    assert_eq!(scene.keys().len(), 1, "the session stays on record");
    scene.edit("a.txt", "one\ntwo\n");
    let stop = scene.stop("s1");
    assert!(
        stop.message()
            .starts_with("turn-wrap: Turn wrapped: 1 file changed, 2 insertions(+)"),
        "{} {}",
        stop.message(),
        stop.stderr
    );
    assert_eq!(scene.descriptions().len(), 2);
}
