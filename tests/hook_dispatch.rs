//! The dispatcher and the installer as the harness meets them: the real
//! binary, a scratch HOME, and a scratch state directory. Each test is one
//! scenario a review proved with a script against the earlier code.

mod support;

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

    /// `rune` with the scene's HOME and state, and none of the owner's
    /// directory variables.
    fn rune(&self) -> Command {
        let mut command = support::rune();
        command
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("RUNE_STATE_DIR", self.state())
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

/// The plan generation a registered command names after `--plan`.
fn plan_of(command: &str) -> String {
    let words: Vec<&str> = command.split_whitespace().collect();
    words[words.iter().position(|w| *w == "--plan").unwrap() + 1].to_string()
}

/// The plan generations under a state directory, sorted.
fn generations(state: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(state.join("hooks/plans"))
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    found.sort();
    found
}

/// Every plan generation the Claude and Codex tables of a scene name.
fn named_generations(scene: &Scene) -> Vec<PathBuf> {
    let mut named = Vec::new();
    for table in [
        claude_settings(scene),
        scene.home().join(".codex/hooks.json"),
    ] {
        let Ok(text) = std::fs::read_to_string(&table) else {
            continue;
        };
        let value: Value = serde_json::from_str(&text).unwrap();
        for groups in value["hooks"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
        {
            for group in groups.as_array().into_iter().flatten() {
                for hook in group["hooks"].as_array().into_iter().flatten() {
                    let command = hook["command"].as_str().unwrap_or_default();
                    if command.contains(" hook run ") {
                        named.push(PathBuf::from(plan_of(command)));
                    }
                }
            }
        }
    }
    named.sort();
    named.dedup();
    named
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
    std::fs::copy(support::binary(), &copy).unwrap();
    let mut second = Command::new(&copy);
    support::isolate(&mut second)
        .env("HOME", scene.home())
        .env("XDG_CONFIG_HOME", scene.home().join(".config"))
        .env("RUNE_STATE_DIR", scene.state())
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
fn emptying_the_handlers_takes_rune_s_entries_back_and_keeps_the_owner_s() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    std::fs::write(
        claude_settings(&scene),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/owner/stop"}]}]}}"#,
    )
    .unwrap();
    scene.config(DCG_GUARD);
    scene.rune().args(["hook", "install"]).assert().success();
    scene.config("hooks:\n  handlers: []\n");
    let check = scene
        .rune()
        .args(["hook", "install", "--check"])
        .output()
        .unwrap();
    assert!(!check.status.success(), "the old registrations are drift");
    scene.rune().args(["hook", "install"]).assert().success();
    let settings = json_at(&claude_settings(&scene));
    assert!(settings["hooks"].get("PreToolUse").is_none(), "{settings}");
    assert_eq!(
        settings["hooks"]["Stop"][0]["hooks"][0]["command"],
        "/owner/stop"
    );
    // No registration is left. Each generation stays for a session that
    // loaded the old hooks, and the manifest records when it went.
    assert_eq!(named_generations(&scene), Vec::<PathBuf>::new());
    let kept = generations(&scene.state());
    assert_ne!(kept, Vec::<PathBuf>::new());
    assert_eq!(retired(&scene), kept);
    scene
        .rune()
        .args(["hook", "install", "--check"])
        .assert()
        .success();
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// The generations the manifest records as no longer named, sorted.
fn retired(scene: &Scene) -> Vec<PathBuf> {
    let manifest = json_at(&scene.state().join("hooks/manifest.json"));
    let mut found: Vec<PathBuf> = manifest["retired"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|record| {
            assert!(
                record["since"].as_str().is_some_and(|s| s.ends_with('Z')),
                "{record}"
            );
            PathBuf::from(record["generation"].as_str().unwrap())
        })
        .collect();
    found.sort();
    found
}

/// The command of the Claude `PreToolUse` registration.
fn pre_tool_command(scene: &Scene) -> String {
    json_at(&claude_settings(scene))["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A scene whose config guards `tool.before` with `dcg` under `deny`,
/// installed, and the observer config that gives `dcg`'s id to a handler
/// that allows every call.
fn guarded_scene() -> (Scene, String) {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    let guard = scene.handler(
        "guard",
        r#"cat > /dev/null; printf '{"effect":"deny","reason":"no rm"}'"#,
    );
    let watch = scene.handler("watch", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: dcg\n      exec: [sh, {guard}]\n      events: [tool.before]\n      on_failure: deny\n"
    ));
    scene.rune().args(["hook", "install"]).assert().success();
    let observer = format!(
        "hooks:\n  handlers:\n    - id: dcg\n      exec: [sh, {watch}]\n      events: [tool.before]\n"
    );
    (scene, observer)
}

/// What the registered `command` answers for `rm -rf /tmp/x`.
fn registration_answers(scene: &Scene, command: &str) -> String {
    let mut args = CLAUDE_PRE.to_vec();
    let plan = plan_of(command);
    args.extend(["--plan", &plan, "--on-failure", "deny"]);
    let answer = dispatch(
        scene.rune(),
        &args,
        &claude_bash(scene.root.path(), "rm -rf /tmp/x"),
    );
    decision(&answer)
}

/// The window of an install: the old registration names its own plan
/// generation, so a new config cannot change what it runs. Here the
/// install of the new config fails after it wrote its generations, the
/// window held open, and the old registration still denies with its old
/// plan. The failed install leaves the manifest as it was.
#[cfg(unix)]
#[test]
fn an_old_registration_reads_its_own_generation_when_an_install_fails() {
    let (scene, observer) = guarded_scene();
    let old = pre_tool_command(&scene);
    let old_text = std::fs::read_to_string(plan_of(&old)).unwrap();
    let manifest = scene.state().join("hooks/manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest).unwrap();
    // An observer reuses the guard's id, and the manifest's directory is
    // read-only, so the install writes its generations, then fails to
    // stage the manifest.
    scene.config(&observer);
    let hooks = scene.state().join("hooks");
    set_mode(&hooks, 0o555);
    let failed = scene.rune().args(["hook", "install"]).output().unwrap();
    set_mode(&hooks, 0o755);
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(!failed.status.success());
    assert!(stderr.contains(".manifest.json.rune-new-"), "{stderr}");
    assert_eq!(
        pre_tool_command(&scene),
        old,
        "the table still holds the old registration"
    );
    assert_eq!(std::fs::read_to_string(plan_of(&old)).unwrap(), old_text);
    assert_eq!(registration_answers(&scene, &old), "deny");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), manifest_text);
    // The failed install's generations wait for the next install.
    let named = named_generations(&scene);
    let on_disk = generations(&scene.state());
    assert!(named.iter().all(|g| on_disk.contains(g)), "{on_disk:?}");
    assert!(on_disk.len() > named.len(), "{on_disk:?}");
}

/// An edit that clears the handlers of a guarded registration's
/// generation leaves a valid plan, but not the bytes the file's name is
/// the digest of, so the registration's `deny` answers, and `--check`
/// names the file.
#[cfg(unix)]
#[test]
fn a_generation_edited_after_install_falls_to_the_registration_s_policy() {
    let (scene, _) = guarded_scene();
    let command = pre_tool_command(&scene);
    let generation = plan_of(&command);
    let mut plan = json_at(Path::new(&generation));
    plan["harnesses"]["claude"]["PreToolUse"]["handlers"] = serde_json::json!([]);
    std::fs::write(&generation, serde_json::to_string_pretty(&plan).unwrap()).unwrap();
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &generation, "--on-failure", "deny"]);
    let answer = dispatch(scene.rune(), &args, &claude_bash(scene.root.path(), "ls"));
    assert_eq!(decision(&answer), "deny", "{}", answer.stderr);
    assert!(
        answer.stderr.contains("changed after install"),
        "{}",
        answer.stderr
    );
    let check = scene
        .rune()
        .args(["hook", "install", "--check"])
        .output()
        .unwrap();
    assert!(!check.status.success());
    assert!(
        String::from_utf8_lossy(&check.stdout).contains("was changed after install"),
        "{}",
        String::from_utf8_lossy(&check.stdout)
    );
    // An install puts the generation back, and the guard denies again.
    scene.rune().args(["hook", "install"]).assert().success();
    assert_eq!(registration_answers(&scene, &command), "deny");
}

/// A session that loaded its hooks before a reinstall keeps calling the
/// old `--plan` path. The reinstall changes the plan and the table, and
/// the old registration still answers from its old generation, which the
/// manifest records as no longer named.
#[cfg(unix)]
#[test]
fn an_old_registration_answers_from_its_generation_after_a_reinstall() {
    let (scene, observer) = guarded_scene();
    let old = pre_tool_command(&scene);
    let old_text = std::fs::read_to_string(plan_of(&old)).unwrap();
    scene.config(&observer);
    scene.rune().args(["hook", "install"]).assert().success();
    let new = pre_tool_command(&scene);
    assert_ne!(
        plan_of(&new),
        plan_of(&old),
        "the table names a new generation"
    );
    assert_eq!(registration_answers(&scene, &new), "");
    assert_eq!(std::fs::read_to_string(plan_of(&old)).unwrap(), old_text);
    assert_eq!(registration_answers(&scene, &old), "deny");
    // Every generation is named or recorded as no longer named, the old
    // Claude one among them.
    let retired = retired(&scene);
    assert!(
        retired.contains(&PathBuf::from(plan_of(&old))),
        "{retired:?}"
    );
    let mut every = named_generations(&scene);
    every.extend(retired);
    every.sort();
    assert_eq!(every, generations(&scene.state()));
}

/// `rune install --json` of a one-rule module into a scratch target.
fn install_module(scene: &Scene) -> std::process::Output {
    install_module_with(scene, &[])
}

/// `install_module` with more variables for rune.
fn install_module_with(scene: &Scene, env: &[(&str, PathBuf)]) -> std::process::Output {
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
    let mut command = scene.rune();
    command.envs(env.iter().map(|(name, value)| (*name, value)));
    command
        .args([
            "install",
            "--source",
            source.to_str().unwrap(),
            "--target",
            target.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap()
}

/// A proof runner's scene marker: the runner's process id and HOME. This
/// test process stands in for the runner, so the marker is live.
fn scene_marker(scene: &Scene, pid: u32, home: &Path) -> PathBuf {
    let marker = scene.path("scene-marker");
    std::fs::write(&marker, format!("{pid}\n{}\n", home.display())).unwrap();
    marker
}

#[test]
fn rune_install_in_a_proof_scene_with_the_runner_s_home_reports_the_hooks_skipped() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    let marker = scene_marker(&scene, std::process::id(), &scene.home());
    let output = install_module_with(&scene, &[("RUNE_PROOF_SCENE", marker)]);
    assert!(output.status.success(), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        document["hooks"]["skipped"]
            .as_str()
            .is_some_and(|why| why.contains("a proof scene runs with the runner's HOME")),
        "{document}"
    );
    assert!(!claude_settings(&scene).exists(), "no table was written");
}

#[test]
fn a_stale_or_bare_proof_marker_skips_nothing() {
    // The variable of the old marker, a marker file that is gone, and a
    // marker whose runner has exited are no proof scene: install runs.
    let mut gone = std::process::Command::new("true").spawn().unwrap();
    let dead = gone.id();
    gone.wait().unwrap();
    for case in ["old variable", "missing file", "exited runner"] {
        let scene = Scene::new();
        std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
        scene.config(DCG_GUARD);
        let env: Vec<(&str, PathBuf)> = match case {
            "old variable" => vec![("RUNE_PROOF_LIVE_HOME", scene.home())],
            "missing file" => vec![("RUNE_PROOF_SCENE", scene.path("no-such-marker"))],
            _ => vec![(
                "RUNE_PROOF_SCENE",
                scene_marker(&scene, dead, &scene.home()),
            )],
        };
        let output = install_module_with(&scene, &env);
        assert!(output.status.success(), "{case}: {output:?}");
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            document["hooks"].get("skipped").is_none(),
            "{case}: {document}"
        );
        assert!(
            claude_settings(&scene).exists(),
            "{case}: the table was written"
        );
    }
}

#[test]
fn a_failed_hook_install_fails_rune_install() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".codex")).unwrap();
    std::fs::write(scene.home().join(".codex/hooks.json"), "{ not json").unwrap();
    scene.config(DCG_GUARD);
    let output = install_module(&scene);
    assert!(!output.status.success(), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        document["errors"]
            .as_array()
            .is_some_and(|e| e.iter().any(|line| line
                .as_str()
                .is_some_and(|l| l.contains("hook tables not written")))),
        "{document}"
    );
}

#[test]
fn rune_install_json_prints_one_document_with_the_hook_report_inside() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    let output = install_module(&scene);
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

#[test]
fn a_proof_scene_with_the_runner_s_home_installs_no_hook() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    let marker = scene_marker(&scene, std::process::id(), &scene.home());
    let mut command = scene.rune();
    command
        .env("RUNE_PROOF_SCENE", &marker)
        .args(["hook", "install"]);
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("a proof scene runs with the runner's HOME"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!claude_settings(&scene).exists(), "no table was written");
    assert!(!scene.state().join("hooks/plan.json").exists());
    // A scene that names a HOME of its own installs as usual.
    let marker = scene_marker(&scene, std::process::id(), &scene.path("elsewhere"));
    let mut command = scene.rune();
    command
        .env("RUNE_PROOF_SCENE", &marker)
        .args(["hook", "install"])
        .assert()
        .success();
    assert!(claude_settings(&scene).exists());
}

#[test]
fn a_relative_state_directory_registers_an_absolute_plan_path() {
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    let mut command = scene.rune();
    command
        .env("RUNE_STATE_DIR", "state")
        .args(["hook", "install"])
        .assert()
        .success();
    let settings = json_at(&claude_settings(&scene));
    let rendered = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_string();
    let words: Vec<&str> = rendered.split_whitespace().collect();
    let plan = words[words.iter().position(|w| *w == "--plan").unwrap() + 1];
    assert!(Path::new(plan).is_absolute(), "{rendered}");
    assert!(Path::new(plan).is_file(), "{rendered}");
    // The second install finds the state directory under two names, the
    // relative one and the absolute one, and still takes its lock once.
    let mut again = scene.rune();
    again
        .env("RUNE_STATE_DIR", "state")
        .args(["hook", "install"])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success();
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
    let plan = plan_of(entry["command"].as_str().unwrap());
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &plan]);
    let started = std::time::Instant::now();
    let answer = dispatch(
        scene.rune(),
        &args,
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
fn a_valid_plan_without_the_registered_event_follows_the_registration() {
    let scene = Scene::new();
    let capture = scene.handler("capture", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: capture\n      exec: [sh, {capture}]\n      events: [session.end]\n"
    ));
    scene.write_plan();
    let plan = scene.state().join("hooks/plan.json").display().to_string();
    let payload = claude_bash(scene.root.path(), "rm -rf /tmp/x");
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &plan, "--on-failure", "deny"]);
    let answer = dispatch(scene.rune(), &args, &payload);
    assert_eq!(decision(&answer), "deny", "{}", answer.stderr);
    assert!(
        answer.stderr.contains("names no handler for PreToolUse"),
        "{}",
        answer.stderr
    );
    // An unguarded registration still proceeds and says why.
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &plan]);
    let answer = dispatch(scene.rune(), &args, &payload);
    assert_eq!(answer.stdout, "");
    assert!(
        answer.stderr.contains("no handler subscribes"),
        "{}",
        answer.stderr
    );
}

/// The `unsupported` lines of `rune --json hook install --dry-run` in the
/// scene, with `xdg` as `XDG_CONFIG_HOME`.
fn unsupported_under(scene: &Scene, xdg: &Path) -> Vec<String> {
    let mut command = scene.rune();
    command.env("XDG_CONFIG_HOME", xdg);
    let output = command
        .args(["--json", "hook", "install", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    report["unsupported"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_harness_under_the_xdg_config_root_has_its_gap_named() {
    // `opencode debug paths` names `$XDG_CONFIG_HOME/opencode` as the
    // config directory under that variable. Install read `~/.config`
    // alone, so a home with the variable set reported no gap for an
    // OpenCode set up there (found 2026-10-26).
    let scene = Scene::new();
    let capture = scene.handler("capture", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: capture\n      exec: [sh, {capture}]\n      events: [session.end]\n"
    ));
    let xdg = scene.path("xdg");
    std::fs::create_dir_all(xdg.join("opencode")).unwrap();
    assert_eq!(
        unsupported_under(&scene, &xdg),
        ["opencode has no session.end for capture"]
    );
    // The scene's own root, `~/.config`, holds no harness.
    assert_eq!(
        unsupported_under(&scene, &scene.home().join(".config")),
        Vec::<String>::new()
    );
    std::fs::create_dir_all(scene.home().join(".config/opencode")).unwrap();
    assert_eq!(
        unsupported_under(&scene, &scene.home().join(".config")),
        ["opencode has no session.end for capture"]
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
    assert!(
        generations(&xdg.join("rune")).len() > 1,
        "one generation per registration"
    );
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
