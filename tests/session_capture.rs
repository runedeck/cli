//! `rune session capture`, `rune session sweep`, `rune session debt`, the
//! `session-capture` hook handler, and the runner's capture, against
//! fixture homes under a temporary folder. No test reads or writes the
//! developer's own data, config, or harness stores.

mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serde_json::Value;

struct Scene {
    _root: tempfile::TempDir,
    home: PathBuf,
    data: PathBuf,
    state: PathBuf,
}

impl Scene {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("root");
        let home = root.path().join("home");
        std::fs::create_dir_all(home.join(".config/rune")).expect("home");
        Self {
            data: root.path().join("Data"),
            state: root.path().join("state"),
            _root: root,
            home,
        }
    }

    fn config(&self, text: &str) {
        std::fs::write(self.home.join(".config/rune/config.yaml"), text).expect("config");
    }

    /// Rune with this scene's home, data folder, and state directory, and
    /// no `SpecStory`, so the readable copy of Claude Code never runs.
    fn rune(&self) -> Command {
        let mut command = support::rune();
        command
            .env("HOME", &self.home)
            .env("DATA_DIR", &self.data)
            .env("RUNE_STATE_DIR", &self.state)
            .env("SPECSTORY_BIN", "/nonexistent/specstory")
            .env("SESSION_SYNC_LOCK_WAIT", "1");
        command
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.home.join(relative);
        write(&path, text);
        path
    }

    fn debt_open(&self) -> usize {
        std::fs::read_dir(self.state.join("capture-debt")).map_or(0, |entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "json"))
                .count()
        })
    }
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(path, text).expect("write");
}

fn claude_line(cwd: &str, text: &str) -> String {
    format!("{{\"cwd\":\"{cwd}\",\"type\":\"user\",\"message\":\"{text}\"}}\n")
}

fn json_of(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not one JSON object ({error}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn wait_for(path: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    path.exists()
}

#[test]
fn capture_copies_a_claude_session_into_the_data_folder() {
    let scene = Scene::new();
    scene.write(
        ".claude/projects/-tmp-repo/abc.jsonl",
        &claude_line("/tmp/repo", "hi"),
    );
    scene.write(".claude/projects/-tmp-repo/abc/subagents/one.jsonl", "{}\n");
    scene.write(".claude/plans/plan.md", "# plan\n");

    let output = scene
        .rune()
        .args([
            "--json",
            "session",
            "capture",
            "--harness",
            "claude",
            "--session",
            "abc",
            "--source",
            "hook",
        ])
        .output()
        .expect("capture");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = json_of(&output);
    assert_eq!(result["outcome"], "copied", "{result}");
    assert_eq!(result["files"], 2, "{result}");
    assert!(
        scene
            .data
            .join("Sessions/claude/-tmp-repo/abc.jsonl")
            .is_file()
    );
    assert!(
        scene
            .data
            .join("Sessions/claude/-tmp-repo/abc/subagents/one.jsonl")
            .is_file()
    );
    assert!(scene.data.join("Plans/claude/plan.md").is_file());
    assert_eq!(scene.debt_open(), 0);
    assert!(scene.state.join("session-capture.log").is_file());
}

#[test]
fn a_session_the_config_excludes_is_skipped_without_debt() {
    let scene = Scene::new();
    scene.config("capture:\n  exclude:\n    paths: [\"~/work/**\"]\n    sessions: [listed]\n");
    let work = scene.home.join("work/app");
    std::fs::create_dir_all(&work).unwrap();
    scene.write(
        ".claude/projects/-home-work-app/w1.jsonl",
        &claude_line(&work.display().to_string(), "work"),
    );
    scene.write(
        ".claude/projects/-tmp-own/listed.jsonl",
        &claude_line("/tmp/own", "listed"),
    );
    scene.write(
        ".claude/projects/-tmp-own/kept.jsonl",
        &claude_line("/tmp/own", "kept"),
    );

    let output = scene
        .rune()
        .current_dir(&work)
        .args([
            "--json",
            "session",
            "capture",
            "--harness",
            "claude",
            "--session",
            "w1",
        ])
        .output()
        .expect("capture");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(json_of(&output)["outcome"], "skipped");

    let output = scene
        .rune()
        .args(["--json", "session", "sweep"])
        .output()
        .expect("sweep");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report = json_of(&output);
    assert_eq!(report["harnesses"]["claude"]["skipped"], 2, "{report}");
    assert!(
        scene
            .data
            .join("Sessions/claude/-tmp-own/kept.jsonl")
            .is_file()
    );
    assert!(
        !scene
            .data
            .join("Sessions/claude/-tmp-own/listed.jsonl")
            .exists()
    );
    assert!(!scene.data.join("Sessions/claude/-home-work-app").exists());
    assert_eq!(scene.debt_open(), 0);
}

/// Every file under `root`, as a path below it, sorted.
#[cfg(unix)]
fn files_under(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(relative) = path.strip_prefix(root) {
                found.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    found
}

/// A stand-in for `sqlite3 <db> ".backup '<file>'"` that copies the
/// database, so the test needs no `sqlite3` on the host.
#[cfg(unix)]
fn fake_sqlite3(scene: &Scene) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = scene.home.join("fake-sqlite3");
    std::fs::write(
        &path,
        "#!/bin/sh\ntarget=${2#.backup \\'}\ncp \"$1\" \"${target%\\'}\"\n",
    )
    .expect("fake sqlite3");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("permissions");
    path
}

#[cfg(unix)]
#[test]
fn the_sweep_copies_no_file_of_an_excluded_session_and_backs_none_up() {
    let scene = Scene::new();
    scene.config(
        "capture:\n  exclude:\n    paths: [\"~/work/**\"]\n    sessions: [private-session]\n",
    );
    let work = scene.home.join("work/app");
    std::fs::create_dir_all(&work).unwrap();
    let rollout = |cwd: &str| {
        ruzstd::encoding::compress_to_vec(
            format!("{{\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{cwd}\"}}}}\n").as_bytes(),
            ruzstd::encoding::CompressionLevel::Fastest,
        )
    };
    let day = scene.home.join(".codex/sessions/2026/11/17");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::write(
        day.join("rollout-2026-11-17-11111111-2222-3333-4444-555555555555.jsonl.zst"),
        rollout(&work.display().to_string()),
    )
    .unwrap();
    std::fs::write(
        day.join("rollout-2026-11-17-99999999-2222-3333-4444-555555555555.jsonl.zst"),
        rollout("/tmp/own"),
    )
    .unwrap();
    scene.write(
        ".gemini/antigravity-cli/conversations/private-session.db",
        "private",
    );
    scene.write(
        ".gemini/antigravity-cli/conversations/own-session.db",
        "own",
    );

    let output = scene
        .rune()
        .env("SQLITE3_BIN", fake_sqlite3(&scene))
        .args(["--json", "session", "sweep"])
        .output()
        .expect("sweep");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report = json_of(&output);
    assert_eq!(report["harnesses"]["codex"]["files"], 1, "{report}");
    assert_eq!(report["harnesses"]["codex"]["skipped"], 1, "{report}");
    assert_eq!(report["harnesses"]["antigravity"]["files"], 1, "{report}");
    assert_eq!(report["harnesses"]["antigravity"]["skipped"], 1, "{report}");
    assert_eq!(report["backups"], 1, "{report}");
    assert!(report.get("backup_error").is_none(), "{report}");

    let day_folder = format!(
        "SessionVersions/antigravity/{}-daily",
        backup_day(&scene.data)
    );
    assert_eq!(
        files_under(&scene.data),
        [
            format!("{day_folder}/own-session.db"),
            "Sessions/antigravity/own-session.db".to_string(),
            "Sessions/codex/2026/11/17/rollout-2026-11-17-99999999-2222-3333-4444-555555555555.jsonl.zst"
                .to_string(),
        ],
        "the data folder holds the sessions that are not excluded, and no other file"
    );
    assert_eq!(scene.debt_open(), 0);
}

/// The day in the name of the one daily backup folder of Antigravity.
#[cfg(unix)]
fn backup_day(data: &Path) -> String {
    let versions = data.join("SessionVersions/antigravity");
    let mut days: Vec<String> = std::fs::read_dir(&versions)
        .expect("the backup folder")
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .strip_suffix("-daily")
                .map(str::to_string)
        })
        .collect();
    assert_eq!(days.len(), 1, "one day folder under {}", versions.display());
    days.remove(0)
}

#[test]
fn a_failed_copy_exits_one_and_records_debt_that_debt_lists() {
    let scene = Scene::new();
    scene.write(".codex/sessions/2026/10/01/rollout-a.jsonl", "{}\n");
    write(&scene.data, "a file where the data folder should be");

    let output = scene
        .rune()
        .args([
            "session",
            "capture",
            "--harness",
            "codex",
            "--session",
            "rollout-a",
            "--status",
            "0",
        ])
        .output()
        .expect("capture");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(scene.debt_open(), 1);

    let output = scene
        .rune()
        .args(["--json", "session", "debt"])
        .output()
        .expect("debt");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let listed = json_of(&output);
    assert_eq!(listed["open"][0]["harness"], "codex", "{listed}");
    assert_eq!(listed["open"][0]["session"], "rollout-a", "{listed}");
    assert_eq!(listed["open"][0]["source"], "runner", "{listed}");

    let text = scene
        .rune()
        .args(["session", "debt"])
        .output()
        .expect("debt text");
    assert!(
        String::from_utf8_lossy(&text.stdout).contains("open    "),
        "{text:?}"
    );
}

#[test]
fn a_session_without_a_file_and_an_unknown_harness_are_told_apart() {
    let scene = Scene::new();
    scene.write(".claude/projects/p/x.jsonl", "{}\n");
    let output = scene
        .rune()
        .args([
            "--json",
            "session",
            "capture",
            "--harness",
            "claude",
            "--session",
            "probe",
        ])
        .output()
        .expect("capture");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(json_of(&output)["outcome"], "no-session-file");
    assert_eq!(scene.debt_open(), 0);

    let output = scene
        .rune()
        .args(["session", "capture", "--harness", "opencode"])
        .output()
        .expect("capture");
    assert_eq!(output.status.code(), Some(125), "{output:?}");

    let output = scene
        .rune()
        .args([
            "session",
            "capture",
            "--harness",
            "claude",
            "--session",
            "../x",
        ])
        .output()
        .expect("capture");
    assert_eq!(output.status.code(), Some(125), "{output:?}");
}

#[test]
fn the_hook_handler_starts_a_detached_capture_and_answers_at_once() {
    let scene = Scene::new();
    let cwd = scene.home.join("repo");
    std::fs::create_dir_all(&cwd).unwrap();
    scene.write(
        ".claude/projects/-home-repo/s9.jsonl",
        &claude_line(&cwd.display().to_string(), "hi"),
    );
    let payload = serde_json::json!({
        "v": 1, "harness": "claude", "event": "session.end", "native_event": "SessionEnd",
        "session_id": "s9", "cwd": cwd,
        "raw": {"hook_event_name": "SessionEnd", "session_id": "s9"}
    });
    let output = scene
        .rune()
        .args(["hook", "adapter", "session-capture"])
        .write_stdin(payload.to_string())
        .output()
        .expect("adapter");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).expect("handler result");
    assert!(result.is_object(), "{result}");
    assert!(
        wait_for(&scene.data.join("Sessions/claude/-home-repo/s9.jsonl")),
        "the detached capture copied the session"
    );
}

#[test]
fn the_hook_handler_exits_zero_when_the_capture_cannot_run() {
    let scene = Scene::new();
    write(&scene.data, "not a folder");
    scene.write(".claude/projects/p/s1.jsonl", "{}\n");
    let payload = serde_json::json!({
        "v": 1, "harness": "claude", "event": "session.end", "native_event": "SessionEnd",
        "session_id": "s1", "cwd": scene.home, "raw": {}
    });
    let output = scene
        .rune()
        .args(["hook", "adapter", "session-capture"])
        .write_stdin(payload.to_string())
        .output()
        .expect("adapter");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let deadline = Instant::now() + Duration::from_secs(10);
    while scene.debt_open() == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        scene.debt_open(),
        1,
        "the detached capture recorded its miss"
    );
}

#[cfg(unix)]
fn fake_grok(scene: &Scene, session_dir: &Path, exit: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = scene.home.join("fake-grok");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nmkdir -p '{dir}'\nprintf '%s\\n' '{{\"type\":\"user\",\"content\":\"hi\"}}' > '{dir}/chat_history.jsonl'\nprintf '%s\\n' '{{\"text\":\"ok\"}}'\nexit {exit}\n",
            dir = session_dir.display()
        ),
    )
    .expect("fake grok");
    let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("permissions");
    path
}

#[cfg(unix)]
#[test]
fn the_runner_captures_the_session_and_keeps_the_harness_exit_status() {
    let scene = Scene::new();
    let repository = tempfile::tempdir().expect("repository");
    let session_dir = scene.home.join(".grok/sessions/%2Ftmp%2Frepo/run-1");
    let binary = fake_grok(&scene, &session_dir, 0);
    let output = scene
        .rune()
        .env("CLIPROXY_API_KEY", "test-token")
        .args([
            "--json",
            "run",
            "grok",
            "say hi",
            "--repo",
            repository.path().to_str().expect("repository"),
            "--binary",
            binary.to_str().expect("binary"),
        ])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = json_of(&output);
    assert_eq!(result["ok"], true, "{result}");
    assert!(
        scene
            .data
            .join("Sessions/grok/%2Ftmp%2Frepo/run-1/chat_history.jsonl")
            .is_file(),
        "the run's session was captured"
    );
    assert!(
        scene
            .data
            .join("Transcripts/grok/%2Ftmp%2Frepo/run-1.md")
            .is_file(),
        "the run's session was rendered"
    );
}

#[cfg(unix)]
#[test]
fn a_failed_capture_after_a_run_is_a_warning_and_debt_not_an_exit_status() {
    let scene = Scene::new();
    let repository = tempfile::tempdir().expect("repository");
    let session_dir = scene.home.join(".grok/sessions/%2Ftmp%2Frepo/run-2");
    let binary = fake_grok(&scene, &session_dir, 0);
    write(&scene.data, "not a folder");
    let output = scene
        .rune()
        .env("CLIPROXY_API_KEY", "test-token")
        .args([
            "--json",
            "run",
            "grok",
            "say hi",
            "--repo",
            repository.path().to_str().expect("repository"),
            "--binary",
            binary.to_str().expect("binary"),
        ])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(json_of(&output)["ok"], true);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("session capture"),
        "{output:?}"
    );
    assert_eq!(scene.debt_open(), 1);
}

#[cfg(unix)]
#[test]
fn a_clean_run_is_not_captured() {
    let scene = Scene::new();
    let repository = tempfile::tempdir().expect("repository");
    let session_dir = scene.home.join(".grok/sessions/%2Ftmp%2Frepo/run-3");
    let binary = fake_grok(&scene, &session_dir, 0);
    let output = scene
        .rune()
        .env("CLIPROXY_API_KEY", "test-token")
        .args([
            "--json",
            "run",
            "grok",
            "say hi",
            "--clean",
            "--repo",
            repository.path().to_str().expect("repository"),
            "--binary",
            binary.to_str().expect("binary"),
        ])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        !scene.data.join("Sessions/grok").exists(),
        "a clean run is not captured"
    );
    assert_eq!(scene.debt_open(), 0);
}
