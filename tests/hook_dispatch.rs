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
    // Claude and Codex group hooks under a matcher; Cursor lists them
    // flat, one object per hook.
    for table in [
        claude_settings(scene),
        scene.home().join(".codex/hooks.json"),
        scene.home().join(".cursor/hooks.json"),
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
                let flat = std::slice::from_ref(group);
                let hooks = group["hooks"].as_array().map_or(flat, Vec::as_slice);
                for hook in hooks {
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
    std::fs::create_dir_all(scene.home().join(".codex")).unwrap();
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

// --------------------------------------------------------------- cursor ----

const CURSOR_PRE: [&str; 4] = ["--harness", "cursor", "--native-event", "preToolUse"];

/// Cursor's `preToolUse` payload for its shell tool, as its hooks
/// reference shows it (read 2026-11-02).
fn cursor_shell(cwd: &Path, command: &str) -> String {
    serde_json::json!({
        "hook_event_name": "preToolUse", "conversation_id": "c1", "generation_id": "g1",
        "cwd": cwd, "tool_name": "Shell", "tool_use_id": "t1", "tool_input": {"command": command}
    })
    .to_string()
}

/// The answer's stdout as JSON, or `null` when it is not one object.
fn parsed(answer: &Answer) -> Value {
    serde_json::from_str(answer.stdout.trim()).unwrap_or(Value::Null)
}

fn cursor_table(scene: &Scene) -> PathBuf {
    scene.home().join(".cursor/hooks.json")
}

/// The commands of rune's own entries in a table, grouped or flat.
fn rune_entries(table: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(table) else {
        return Vec::new();
    };
    let value: Value = serde_json::from_str(&text).unwrap();
    let mut found = Vec::new();
    for groups in value["hooks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        for group in groups.as_array().into_iter().flatten() {
            let flat = std::slice::from_ref(group);
            let hooks = group["hooks"].as_array().map_or(flat, Vec::as_slice);
            for hook in hooks {
                let command = hook["command"].as_str().unwrap_or_default();
                if command.contains(" hook run ") {
                    found.push(command.to_string());
                }
            }
        }
    }
    found
}

#[cfg(unix)]
#[test]
fn dcg_judges_cursor_s_shell_tool_through_the_dispatcher() {
    // Cursor names its shell tool `Shell`. The dcg adapter once knew every
    // other harness's name and not that one, so a Cursor `git reset
    // --hard` passed the guard and was answered allow (found 2026-11-02).
    let scene = Scene::new();
    let dcg = scene.path("bin/dcg");
    std::fs::write(
        &dcg,
        "#!/bin/sh\ncat > /dev/null\necho 'dcg: refused' >&2\nexit 2\n",
    )
    .unwrap();
    set_mode(&dcg, 0o755);
    scene.config(DCG_GUARD);
    scene.write_plan();
    let path = format!(
        "{}:{}",
        scene.path("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut command = scene.rune();
    command.env("PATH", &path);
    let answer = dispatch(
        command,
        &CURSOR_PRE,
        &cursor_shell(scene.root.path(), "git reset --hard"),
    );
    let out = parsed(&answer);
    assert_eq!(
        out["permission"], "deny",
        "{} {}",
        answer.stdout, answer.stderr
    );
    assert!(
        out["agent_message"]
            .as_str()
            .is_some_and(|m| m.contains("dcg: refused")),
        "{out}"
    );
    // A tool that is no shell goes to no guard.
    let read = serde_json::json!({
        "hook_event_name": "preToolUse", "conversation_id": "c1", "cwd": scene.root.path(),
        "tool_name": "Read", "tool_use_id": "t2", "tool_input": {"path": "/etc/hosts"}
    })
    .to_string();
    let mut command = scene.rune();
    command.env("PATH", &path);
    let answer = dispatch(command, &CURSOR_PRE, &read);
    assert_eq!(parsed(&answer), serde_json::json!({"permission": "allow"}));
}

#[test]
fn a_cursor_registration_answers_allow_where_it_has_nothing_to_run() {
    // Cursor blocks a permission hook that prints nothing valid, so the
    // paths that proceed in silence on Claude must answer here: a plan
    // that does not load under a warn registration, and a plan that names
    // no handler for the event.
    let scene = Scene::new();
    let payload = cursor_shell(scene.root.path(), "ls");
    let missing = scene.path("nowhere/plan.json").display().to_string();
    let mut args = CURSOR_PRE.to_vec();
    args.extend(["--plan", &missing]);
    let answer = dispatch(scene.rune(), &args, &payload);
    assert_eq!(answer.code, 0);
    assert_eq!(
        parsed(&answer),
        serde_json::json!({"permission": "allow"}),
        "{}",
        answer.stderr
    );
    assert!(
        answer.stderr.contains("no compiled hook plan"),
        "{}",
        answer.stderr
    );
    let capture = scene.handler("capture", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: capture\n      exec: [sh, {capture}]\n      events: [session.end]\n"
    ));
    scene.write_plan();
    let plan = scene.state().join("hooks/plan.json").display().to_string();
    let mut args = CURSOR_PRE.to_vec();
    args.extend(["--plan", &plan]);
    let answer = dispatch(scene.rune(), &args, &payload);
    assert_eq!(
        parsed(&answer),
        serde_json::json!({"permission": "allow"}),
        "{}",
        answer.stderr
    );
    assert!(
        answer.stderr.contains("no handler subscribes"),
        "{}",
        answer.stderr
    );
    // The prompt gate answers in its own shape.
    let answer = dispatch(
        scene.rune(),
        &[
            "--harness",
            "cursor",
            "--native-event",
            "beforeSubmitPrompt",
            "--plan",
            &plan,
        ],
        r#"{"hook_event_name":"beforeSubmitPrompt","conversation_id":"c1","prompt":"hi"}"#,
    );
    assert_eq!(
        parsed(&answer),
        serde_json::json!({"continue": true}),
        "{}",
        answer.stderr
    );
    // Claude still proceeds in silence, as it reads silence.
    let mut args = CLAUDE_PRE.to_vec();
    args.extend(["--plan", &plan]);
    let answer = dispatch(scene.rune(), &args, &claude_bash(scene.root.path(), "ls"));
    assert_eq!(answer.stdout, "");
}

#[test]
fn a_cursor_permission_extension_observed_passively_answers_allow() {
    // `cursor:beforeShellExecution` registers as a passive observer, and
    // Cursor requires `permission` on that event: `{}` would block every
    // shell command.
    let scene = Scene::new();
    let watch = scene.handler("watch", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: watch\n      exec: [sh, {watch}]\n      events: ['cursor:beforeShellExecution', 'cursor:afterShellExecution']\n"
    ));
    scene.write_plan();
    let answer = dispatch(
        scene.rune(),
        &[
            "--harness",
            "cursor",
            "--native-event",
            "beforeShellExecution",
        ],
        r#"{"hook_event_name":"beforeShellExecution","conversation_id":"c1","command":"ls","cwd":"/tmp"}"#,
    );
    assert_eq!(
        parsed(&answer),
        serde_json::json!({"permission": "allow"}),
        "{} {}",
        answer.stdout,
        answer.stderr
    );
    let answer = dispatch(
        scene.rune(),
        &[
            "--harness",
            "cursor",
            "--native-event",
            "afterShellExecution",
        ],
        r#"{"hook_event_name":"afterShellExecution","conversation_id":"c1","command":"ls","output":"","duration":1}"#,
    );
    assert_eq!(parsed(&answer), serde_json::json!({}), "{}", answer.stderr);
}

#[test]
fn an_explicit_provider_never_switches_a_handler() {
    // `rune hook off dcg --provider claude` names a staged hook rune. It
    // once switched the declared handler `dcg` off on every harness and
    // left the rune alone (found 2026-11-02). The scene has no `.rune`, so
    // the rune toggle fails, and the handler must still be on.
    let scene = Scene::new();
    scene.config(DCG_GUARD);
    let output = scene
        .rune()
        .args(["hook", "off", "dcg", "--provider", "claude"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    let switches = scene.state().join("hooks/switches.json");
    assert!(!switches.exists(), "the handler was switched");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!text.contains("dcg off on"), "{text}");
    // `--harness` is the handler's flag.
    scene
        .rune()
        .args(["hook", "off", "dcg", "--harness", "claude"])
        .assert()
        .success();
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(&switches).unwrap()).unwrap();
    assert_eq!(written["off"]["dcg"], serde_json::json!(["claude"]));
}

#[test]
fn a_harness_without_its_folder_is_skipped_and_returns_with_it() {
    // `rune install` and a bare `rune hook install` attempt every
    // registered harness. One whose folder does not exist is skipped and
    // reported, and nothing is made for it: no folder, no table, no lock
    // file. Nothing is remembered, so a later install writes it once the
    // folder exists, and `rune hook remove` is undone by the next install
    // the same way.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config(DCG_GUARD);
    let output = scene
        .rune()
        .args(["--json", "hook", "install"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["skipped_harnesses"],
        serde_json::json!([
            "codex: ~/.codex does not exist",
            "cursor: ~/.cursor does not exist"
        ]),
        "{report}"
    );
    assert_eq!(rune_entries(&claude_settings(&scene)).len(), 1);
    for folder in [".codex", ".cursor"] {
        assert!(
            !scene.home().join(folder).exists(),
            "{folder} was created for a skipped harness"
        );
    }
    // The deck install follows the same rule and reports the same.
    let output = install_module(&scene);
    assert!(output.status.success(), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document["hooks"]["skipped_harnesses"]
            .as_array()
            .map(Vec::len),
        Some(2),
        "{document}"
    );
    assert!(!scene.home().join(".cursor").exists());
    // Once the folder exists, the next install writes the table.
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    scene.rune().args(["hook", "install"]).assert().success();
    assert_eq!(rune_entries(&cursor_table(&scene)).len(), 1);
    assert!(!scene.home().join(".codex").exists());
    // A removal takes rune's entries out and remembers nothing: the
    // next install puts them back.
    scene
        .rune()
        .args(["hook", "remove", "--harness", "cursor"])
        .assert()
        .success();
    assert_eq!(rune_entries(&cursor_table(&scene)), Vec::<String>::new());
    assert_eq!(rune_entries(&claude_settings(&scene)).len(), 1);
    let output = install_module(&scene);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(rune_entries(&cursor_table(&scene)).len(), 1);
}

#[test]
fn an_unselected_harness_s_table_is_neither_read_nor_locked_nor_created() {
    // An install into Cursor alone once read every registered table and
    // locked every table directory, so a garbled Claude `settings.json`
    // stopped `rune hook remove --harness cursor`, and a fresh home
    // gained `~/.claude/` and `~/.codex/` lock files (found 2026-11-02).
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    std::fs::write(claude_settings(&scene), "{ not json").unwrap();
    scene.config(DCG_GUARD);
    scene
        .rune()
        .args(["hook", "install", "--harness", "cursor"])
        .assert()
        .success();
    assert_eq!(rune_entries(&cursor_table(&scene)).len(), 1);
    assert_eq!(
        std::fs::read_to_string(claude_settings(&scene)).unwrap(),
        "{ not json"
    );
    let claude_entries: Vec<String> = std::fs::read_dir(scene.home().join(".claude"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        claude_entries,
        ["settings.json"],
        "no lock file in ~/.claude"
    );
    assert!(
        !scene.home().join(".codex").exists(),
        "an unselected harness's directory was created"
    );
    scene
        .rune()
        .args(["hook", "remove", "--harness", "cursor"])
        .assert()
        .success();
    assert_eq!(rune_entries(&cursor_table(&scene)), Vec::<String>::new());
    assert!(!scene.home().join(".codex").exists());
    // The garbled table still stops the install that selects it.
    let output = scene
        .rune()
        .args(["hook", "install", "--harness", "claude"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
}

#[test]
fn a_switch_rewrites_the_harness_table_at_once() {
    // `rune hook off <id> --harness cursor` leaves the handler out of
    // Cursor's plan and installs that plan into Cursor's table in the same
    // command, so off means off without a second step; Claude's table is
    // not touched. `on` puts the entry back the same way.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    scene.config(DCG_GUARD);
    scene.rune().args(["hook", "install"]).assert().success();
    assert_eq!(rune_entries(&cursor_table(&scene)).len(), 1);
    let claude_before = std::fs::read_to_string(claude_settings(&scene)).unwrap();
    let output = scene
        .rune()
        .args(["--json", "hook", "off", "dcg", "--harness", "cursor"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["off"], serde_json::json!(["cursor"]), "{answer}");
    assert!(
        answer["install"]["written"]
            .as_array()
            .is_some_and(|w| w.iter().any(|e| e == "cursor:preToolUse removed")),
        "{answer}"
    );
    assert_eq!(
        rune_entries(&cursor_table(&scene)),
        Vec::<String>::new(),
        "the switch left the entry in the table"
    );
    assert_eq!(
        std::fs::read_to_string(claude_settings(&scene)).unwrap(),
        claude_before
    );
    scene
        .rune()
        .args(["hook", "on", "dcg", "--harness", "cursor"])
        .assert()
        .success();
    assert_eq!(rune_entries(&cursor_table(&scene)).len(), 1);
    assert!(!scene.state().join("hooks/switches.json").exists());
}

// ------------------------------------------------------------- switches ----

/// Trust the first rune entry of one Codex event the way `/hooks` records
/// it: the sha256 of the hook's identity, keyed by table, event label,
/// group, and handler, appended to the Codex config.
fn trust_codex(scene: &Scene, event: &str, label: &str) {
    use sha2::Digest as _;
    let table = scene.home().join(".codex/hooks.json");
    let value = json_at(&table);
    for (g, group) in value["hooks"][event].as_array().unwrap().iter().enumerate() {
        for (h, hook) in group["hooks"].as_array().unwrap().iter().enumerate() {
            let command = hook["command"].as_str().unwrap();
            if !command.contains(" hook run ") {
                continue;
            }
            let handler = serde_json::json!({
                "type": "command", "command": command,
                "timeout": hook["timeout"].as_u64().unwrap_or(600), "async": false,
            });
            let mut identity = serde_json::json!({"event_name": label, "hooks": [handler]});
            if let Some(matcher) = group["matcher"].as_str() {
                identity["matcher"] = serde_json::json!(matcher);
            }
            let hash = sha2::Sha256::digest(identity.to_string().as_bytes());
            let config = scene.home().join(".codex/config.toml");
            let text = format!(
                "{}[hooks.state.\"{}:{label}:{g}:{h}\"]\ntrusted_hash = \"sha256:{hash:x}\"\n",
                std::fs::read_to_string(&config).unwrap_or_default(),
                table.display()
            );
            std::fs::write(&config, text).unwrap();
            return;
        }
    }
    panic!("no rune entry on {event} to trust");
}

/// Two shell handlers on `tool.before` that each append their id to
/// `ran.log` in the scene, so a firing shows which of them ran.
fn two_recording_handlers(scene: &Scene) -> PathBuf {
    let ran = scene.path("ran.log");
    let a = scene.handler(
        "a",
        &format!(
            "cat > /dev/null; echo a >> {}; printf '{{}}'",
            ran.display()
        ),
    );
    let b = scene.handler(
        "b",
        &format!(
            "cat > /dev/null; echo b >> {}; printf '{{}}'",
            ran.display()
        ),
    );
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: a\n      exec: [sh, {a}]\n      events: [tool.before]\n    - id: b\n      exec: [sh, {b}]\n      events: [tool.before]\n"
    ));
    ran
}

#[cfg(unix)]
#[test]
fn a_codex_switch_holds_where_codex_keeps_the_trusted_entry() {
    // Codex trusts a hook by its command at its slot. `rune hook off a
    // --harness codex` compiles a new generation whose entry Codex does
    // not trust yet, so the install keeps the trusted old entry, which
    // names the old generation, and the dispatcher ran `a` from it as if
    // nothing was switched, while `off` reported success (review,
    // 2026-11-02). A firing now reads the switches beside the generations,
    // so off holds whichever generation the entry names.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".codex")).unwrap();
    let ran = two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    let table = scene.home().join(".codex/hooks.json");
    let old = rune_entries(&table);
    assert_eq!(old.len(), 1, "{old:?}");
    trust_codex(&scene, "PreToolUse", "pre_tool_use");
    let output = scene
        .rune()
        .args(["hook", "off", "a", "--harness", "codex"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("keeps rune's earlier entry"), "{text}");
    // The trusted entry stays, first, and still names the old generation.
    assert_eq!(rune_entries(&table)[0], old[0]);
    // Fired as Codex fires it, the old generation runs `b` alone.
    let answer = dispatch(
        scene.rune(),
        &[
            "--harness",
            "codex",
            "--native-event",
            "PreToolUse",
            "--plan",
            &plan_of(&old[0]),
        ],
        &claude_bash(scene.root.path(), "ls"),
    );
    assert_eq!(answer.code, 0, "{} {}", answer.stdout, answer.stderr);
    assert!(
        answer.stderr.contains("a switched off for codex; not run"),
        "{}",
        answer.stderr
    );
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "b\n");
    // Back on, the same generation runs both again.
    scene
        .rune()
        .args(["hook", "on", "a", "--harness", "codex"])
        .assert()
        .success();
    std::fs::remove_file(&ran).unwrap();
    dispatch(
        scene.rune(),
        &[
            "--harness",
            "codex",
            "--native-event",
            "PreToolUse",
            "--plan",
            &plan_of(&old[0]),
        ],
        &claude_bash(scene.root.path(), "ls"),
    );
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "a\nb\n");
}

#[test]
fn a_switch_waits_for_the_lock_before_it_reads_the_state() {
    // `rune hook off A` once read the switches, then took the lock to
    // write them, so two switches that read before either wrote kept one
    // another's change out (review, 2026-11-02). Here the test holds the
    // lock, a switch starts and waits, and the state changes under the
    // held lock: the switch reads the state after the lock, so it keeps
    // the change and adds its own.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    let hooks = scene.state().join("hooks");
    let lock = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(hooks.join(".rune-install.lock"))
        .unwrap();
    lock.lock().unwrap();
    let trace = scene.path("switch.trace");
    let mut child = rune_in(&scene, &["hook", "off", "a", "--harness", "cursor"], &trace);
    // The switch has entered its wait on the held lock, with the state
    // still unread: its trace names the wait and no `held` for this dir.
    let so_far = waits_for(&mut child, &trace, &hooks);
    assert!(
        !so_far.contains(&format!(
            "held {}\n",
            std::fs::canonicalize(&hooks).unwrap().display()
        )),
        "the switch took the lock the test holds: {so_far}"
    );
    let switches = hooks.join("switches.json");
    std::fs::write(
        &switches,
        "{\"version\": 1, \"off\": {\"b\": [\"cursor\"]}}\n",
    )
    .unwrap();
    lock.unlock().unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
    let state = json_at(&switches);
    assert_eq!(state["off"]["a"], serde_json::json!(["cursor"]), "{state}");
    assert_eq!(
        state["off"]["b"],
        serde_json::json!(["cursor"]),
        "the change made under the lock was lost: {state}"
    );
    assert_eq!(rune_entries(&cursor_table(&scene)), Vec::<String>::new());
}

#[test]
fn a_switch_the_install_refuses_leaves_the_state_as_it_was() {
    // `rune hook off` wrote the switches, then ran the install, with no
    // way back: an install the table refused left the file saying the
    // handler was off while the table still fired it (review, 2026-11-02).
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    // The timeout of rune's own entry changes by hand: a conflict.
    let table = cursor_table(&scene);
    let mut value = json_at(&table);
    value["hooks"]["preToolUse"][0]["timeout"] = serde_json::json!(99);
    std::fs::write(&table, value.to_string()).unwrap();
    let before = std::fs::read_to_string(&table).unwrap();
    let output = scene
        .rune()
        .args(["hook", "off", "a", "--harness", "cursor"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("changed by hand"), "{stderr}");
    assert!(
        !scene.state().join("hooks/switches.json").exists(),
        "the refused switch stayed in the state"
    );
    assert_eq!(std::fs::read_to_string(&table).unwrap(), before);
    let list = scene.rune().args(["hook", "list"]).output().unwrap();
    assert!(
        !String::from_utf8_lossy(&list.stdout).contains("off:"),
        "{}",
        String::from_utf8_lossy(&list.stdout)
    );
    // A switch that was set stays set when a later one is refused.
    let switches = scene.state().join("hooks/switches.json");
    std::fs::write(
        &switches,
        "{\"version\": 1, \"off\": {\"b\": [\"claude\"]}}\n",
    )
    .unwrap();
    scene
        .rune()
        .args(["hook", "off", "a", "--harness", "cursor"])
        .assert()
        .failure();
    let state = json_at(&switches);
    assert_eq!(
        state["off"],
        serde_json::json!({"b": ["claude"]}),
        "{state}"
    );
}

#[test]
fn a_fresh_config_still_reports_the_skipped_harnesses() {
    // With no handler, no generation, and nothing owned, the install
    // returned before it attached the harnesses the folder rule skipped
    // (review, 2026-11-02), so a fresh config on a machine without
    // `~/.cursor` heard nothing of it.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".claude")).unwrap();
    scene.config("hooks:\n  handlers: []\n");
    let output = scene
        .rune()
        .args(["--json", "hook", "install"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["skipped_harnesses"],
        serde_json::json!([
            "codex: ~/.codex does not exist",
            "cursor: ~/.cursor does not exist"
        ]),
        "{report}"
    );
    assert_eq!(report["written"], serde_json::json!([]), "{report}");
    let output = scene.rune().args(["hook", "install"]).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("cursor: skipped, ~/.cursor does not exist"),
        "{text}"
    );
    assert!(text.contains("nothing to register"), "{text}");
    // The deck install carries the same skip report.
    let output = install_module(&scene);
    assert!(output.status.success(), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document["hooks"]["skipped_harnesses"]
            .as_array()
            .map(Vec::len),
        Some(2),
        "{document}"
    );
}

// ----------------------------------------------- switches, round four ----

/// The install lock file of one directory, opened for locking.
fn lock_file(dir: &Path) -> std::fs::File {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".rune-install.lock"))
        .unwrap()
}

/// `rune` as a plain process with the scene's environment, for a run the
/// test waits on while it does something else. Its lock trace goes to
/// `trace`: one `wait <dir>` line before it waits for a directory's lock,
/// one `held <dir>` line once it has it.
fn rune_in(scene: &Scene, args: &[&str], trace: &Path) -> std::process::Child {
    let mut child = support::rune_process();
    child
        .env("HOME", scene.home())
        .env("XDG_CONFIG_HOME", scene.home().join(".config"))
        .env("RUNE_STATE_DIR", scene.state())
        .env("RUNE_HOOK_LOCK_TRACE", trace)
        .current_dir(scene.root.path())
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    child.spawn().unwrap()
}

/// Wait until the child's lock trace says it waits for `dir`'s lock: the
/// child has crossed into its lock call, holding what the trace says it
/// holds and nothing more. A fixed sleep and a child still running proved
/// neither (review, 2026-11-02). Returns the trace so far.
fn waits_for(child: &mut std::process::Child, trace: &Path, dir: &Path) -> String {
    let dir = std::fs::canonicalize(dir).unwrap();
    let line = format!("wait {}\n", dir.display());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let text = std::fs::read_to_string(trace).unwrap_or_default();
        if text.contains(&line) {
            return text;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!(
                "the child ended ({status}) before it waited for {}; trace:\n{text}",
                dir.display()
            );
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!(
                "the child did not wait for {} in ten seconds; trace:\n{text}",
                dir.display()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Wait up to ten seconds for a process, and kill it when it does not
/// end: a deadlock is a failure, not a hang of the test run.
fn ends(child: &mut std::process::Child, what: &str) -> std::process::ExitStatus {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("{what} did not end in ten seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// The handler ids of one native event in the generation a table entry
/// names.
fn planned_handlers(entry: &str, harness: &str, native: &str) -> Vec<String> {
    let plan = json_at(Path::new(&plan_of(entry)));
    plan["harnesses"][harness][native]["handlers"]
        .as_array()
        .map(|handlers| {
            handlers
                .iter()
                .map(|h| h["id"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_switch_takes_the_install_s_whole_lock_set_in_the_install_s_order() {
    // `rune hook off` locked the state directory alone, then its install
    // asked for the tables; a plain install took the tables first and the
    // state directory last. Each could hold what the other waited for,
    // and a file lock waits forever (review, 2026-11-02). The switch now
    // takes the install's whole set in the install's order: here the
    // test holds the Cursor table's directory, which sorts before the
    // state directory, so a waiting switch holds nothing yet, and a plain
    // install waiting beside it ends with it once the directory is free.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    let table_lock = lock_file(&scene.home().join(".cursor"));
    table_lock.lock().unwrap();
    let trace = scene.path("switch.trace");
    let mut switch = rune_in(&scene, &["hook", "off", "a", "--harness", "cursor"], &trace);
    // The switch is at its lock call for the table's directory, the
    // first of its set, and its trace says it holds nothing yet.
    let so_far = waits_for(&mut switch, &trace, &scene.home().join(".cursor"));
    assert!(
        !so_far.contains("held "),
        "the waiting switch holds a directory already, out of the install's order:\n{so_far}"
    );
    let state_lock = lock_file(&scene.state().join("hooks"));
    assert!(
        state_lock.try_lock().is_ok(),
        "the waiting switch holds the state directory, out of the install's order"
    );
    state_lock.unlock().unwrap();
    let install_trace = scene.path("install.trace");
    let mut install = rune_in(&scene, &["hook", "install"], &install_trace);
    waits_for(&mut install, &install_trace, &scene.home().join(".cursor"));
    table_lock.unlock().unwrap();
    assert!(ends(&mut switch, "the switch").success());
    assert!(ends(&mut install, "the install").success());
    assert_eq!(
        planned_handlers(
            &rune_entries(&cursor_table(&scene))[0],
            "cursor",
            "preToolUse"
        ),
        ["b"]
    );
}

#[test]
fn an_install_reads_the_switches_under_its_lock() {
    // `rune hook install` compiled its plan, then took the lock. A switch
    // that landed in between was undone when the install went on to
    // publish the plan it had compiled, and a firing cannot add back a
    // handler its plan lacks (review, 2026-11-02). The install now reads
    // the switches after the lock: here it waits on the lock while `a`
    // is turned back on, and publishes a plan that runs `a`.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    scene
        .rune()
        .args(["hook", "off", "a", "--harness", "cursor"])
        .assert()
        .success();
    let table = cursor_table(&scene);
    assert_eq!(
        planned_handlers(&rune_entries(&table)[0], "cursor", "preToolUse"),
        ["b"]
    );
    let state_lock = lock_file(&scene.state().join("hooks"));
    state_lock.lock().unwrap();
    let trace = scene.path("install.trace");
    let mut install = rune_in(&scene, &["hook", "install"], &trace);
    waits_for(&mut install, &trace, &scene.state().join("hooks"));
    // The switch lifts under the held lock, as `rune hook on` would once
    // it had the lock itself.
    std::fs::remove_file(scene.state().join("hooks/switches.json")).unwrap();
    state_lock.unlock().unwrap();
    assert!(ends(&mut install, "the install").success());
    assert_eq!(
        planned_handlers(&rune_entries(&table)[0], "cursor", "preToolUse"),
        ["a", "b"],
        "the install published the plan it compiled before the lock"
    );
}

#[test]
fn a_listing_that_writes_compiles_under_the_install_s_lock() {
    // `rune hook list --write` compiled with the switches before it took
    // any lock, and `plan::write` locked the plan's directory around the
    // publish alone. A switch that landed between the compile and the
    // write was overwritten by the stale `plan.json`, and a later firing
    // with no `--plan` read it, so a handler turned back on stayed off
    // (review, 2026-11-02). The listing now takes the install's whole
    // lock set before it compiles and holds it through the publish: here
    // it waits on the lock while `a` is turned back on, and writes a
    // plan that runs `a`.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    scene
        .rune()
        .args(["hook", "off", "a", "--harness", "cursor"])
        .assert()
        .success();
    let written = |scene: &Scene| -> Vec<String> {
        let plan = json_at(&scene.state().join("hooks/plan.json"));
        plan["harnesses"]["cursor"]["preToolUse"]["handlers"]
            .as_array()
            .map(|handlers| {
                handlers
                    .iter()
                    .map(|h| h["id"].as_str().unwrap().to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    scene.write_plan();
    assert_eq!(written(&scene), ["b"]);
    let state_lock = lock_file(&scene.state().join("hooks"));
    state_lock.lock().unwrap();
    let trace = scene.path("list.trace");
    let mut listing = rune_in(&scene, &["hook", "list", "--write"], &trace);
    waits_for(&mut listing, &trace, &scene.state().join("hooks"));
    // The switch lifts under the held lock, as `rune hook on` would once
    // it had the lock itself.
    std::fs::remove_file(scene.state().join("hooks/switches.json")).unwrap();
    state_lock.unlock().unwrap();
    assert!(ends(&mut listing, "the listing").success());
    assert_eq!(
        written(&scene),
        ["a", "b"],
        "the listing wrote the plan it compiled before the lock"
    );
}

#[cfg(unix)]
#[test]
fn a_firing_finds_the_switches_beside_a_plans_link_by_its_name() {
    // The firing canonicalized its plan path, so a `plans` that is a link
    // to a directory of another name lost its name, and the switches
    // were looked for beside the target, where nothing is: a switched-off
    // handler ran (review, 2026-11-02). The lookup is lexical now, on the
    // path the registration names.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    let ran = two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    let entry = rune_entries(&cursor_table(&scene)).remove(0);
    let hooks = scene.state().join("hooks");
    // The switch by hand, so the generation the entry names still holds
    // `a`, as a kept entry's would.
    std::fs::write(
        hooks.join("switches.json"),
        "{\"version\": 1, \"off\": {\"a\": [\"cursor\"]}}",
    )
    .unwrap();
    std::fs::rename(hooks.join("plans"), hooks.join("generations")).unwrap();
    std::os::unix::fs::symlink(hooks.join("generations"), hooks.join("plans")).unwrap();
    assert!(plan_of(&entry).contains("/plans/"), "{entry}");
    let answer = dispatch(
        scene.rune(),
        &[
            "--harness",
            "cursor",
            "--native-event",
            "preToolUse",
            "--plan",
            &plan_of(&entry),
        ],
        &cursor_shell(scene.root.path(), "ls"),
    );
    assert!(answer.stdout.contains("\"allow\""), "{}", answer.stdout);
    assert!(
        answer.stderr.contains("a switched off for cursor; not run"),
        "{}",
        answer.stderr
    );
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "b\n");
}

#[test]
fn a_firing_records_the_context_cursor_s_stop_has_no_field_for() {
    // The spec has the firing record the context as not applied where
    // the harness has no field for it; the dispatcher said so on stderr
    // and the record held nothing of it (review, 2026-11-02). The record
    // of the hook that asked for the context now carries `unapplied`,
    // and a record with nothing dropped has no such member, so the
    // readers of the record, jev among them, see every member they saw.
    let scene = Scene::new();
    let jev = scene.handler(
        "jev",
        "cat > /dev/null; printf '{\"context\":[\"jev: one finding\"]}'",
    );
    let quiet = scene.handler("quiet", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: jev\n      exec: [sh, {jev}]\n      events: [tool.after, turn.finish]\n    - id: quiet\n      exec: [sh, {quiet}]\n      events: [turn.finish]\n"
    ));
    scene.write_plan();
    let records = |scene: &Scene| -> Vec<Value> {
        let dir = scene.state().join("hooks/record");
        let mut lines = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            lines.extend(text.lines().map(|line| serde_json::from_str(line).unwrap()));
        }
        lines
    };
    let stop = serde_json::json!({
        "hook_event_name": "stop", "conversation_id": "c1", "workspace_roots": ["/w"],
        "status": "completed", "loop_count": 0
    })
    .to_string();
    let answer = dispatch(
        scene.rune(),
        &["--harness", "cursor", "--native-event", "stop"],
        &stop,
    );
    assert_eq!(answer.stdout.trim(), "{}");
    assert!(
        answer
            .stderr
            .contains("cursor cannot express context on stop; reported, not applied"),
        "{}",
        answer.stderr
    );
    let lines = records(&scene);
    let of = |hook: &str| -> Value {
        lines
            .iter()
            .find(|line| line["hook"] == hook && line["native_event"] == "stop")
            .cloned()
            .unwrap_or_else(|| panic!("no record of {hook}: {lines:?}"))
    };
    assert_eq!(of("jev")["unapplied"], serde_json::json!(["context"]));
    assert!(
        of("quiet").get("unapplied").is_none(),
        "a hook that asked for nothing has no member: {}",
        of("quiet")
    );
    // Every member an older reader knows is still there, in its shape.
    for member in [
        "v",
        "ts",
        "harness",
        "event",
        "native_event",
        "hook",
        "session_id",
        "tool_use_id",
        "cwd",
        "file",
        "duration_ms",
        "exit_status",
        "asked",
        "answer",
        "failure",
        "metrics",
    ] {
        assert!(
            of("jev").get(member).is_some(),
            "{member} missing: {}",
            of("jev")
        );
    }
    // Where Cursor has a field, the context is applied and nothing is stored.
    let after = serde_json::json!({
        "hook_event_name": "postToolUse", "conversation_id": "c1", "workspace_roots": ["/w"],
        "tool_name": "Shell", "tool_input": {"command": "ls"}, "tool_output": "a"
    })
    .to_string();
    let answer = dispatch(
        scene.rune(),
        &["--harness", "cursor", "--native-event", "postToolUse"],
        &after,
    );
    assert!(
        answer.stdout.contains("additional_context"),
        "{}",
        answer.stdout
    );
    let applied = records(&scene)
        .into_iter()
        .find(|line| line["hook"] == "jev" && line["native_event"] == "postToolUse")
        .unwrap();
    assert!(applied.get("unapplied").is_none(), "{applied}");
}

#[test]
fn a_firing_records_what_a_passive_event_s_merge_discarded() {
    // The merge drops a passive event's denial and replacement input
    // before the encoder sees them, and `passive_notes` says so on stderr;
    // only the encoder's drops reached the record, so these were pruned
    // away and the record read `asked: deny, answer: allow` with nothing
    // stored as not applied (review, 2026-11-02). The record of the hook
    // that asked now carries them, and a hook whose effects all reached
    // the harness still has no member.
    let scene = Scene::new();
    let guard = scene.handler(
        "guard",
        "cat > /dev/null; printf '{\"effect\":\"deny\",\"reason\":\"stop\"}'",
    );
    let rewrite = scene.handler(
        "rewrite",
        "cat > /dev/null; printf '{\"input\":{\"command\":\"ls -l\"}}'",
    );
    let quiet = scene.handler("quiet", "cat > /dev/null; printf '{}'");
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: guard\n      exec: [sh, {guard}]\n      events: [session.end]\n    - id: rewrite\n      exec: [sh, {rewrite}]\n      events: [session.end]\n      rewrites: true\n    - id: quiet\n      exec: [sh, {quiet}]\n      events: [session.end]\n"
    ));
    scene.write_plan();
    let end = serde_json::json!({
        "hook_event_name": "sessionEnd", "conversation_id": "c1", "workspace_roots": ["/w"],
        "reason": "user_exit"
    })
    .to_string();
    let answer = dispatch(
        scene.rune(),
        &["--harness", "cursor", "--native-event", "sessionEnd"],
        &end,
    );
    assert_eq!(answer.stdout.trim(), "{}");
    for note in [
        "sessionEnd is passive on cursor; guard's Deny is logged, not applied",
        "sessionEnd is passive on cursor; rewrite's replacement input is logged, not applied",
    ] {
        assert!(answer.stderr.contains(note), "{}", answer.stderr);
    }
    let dir = scene.state().join("hooks/record");
    let mut lines: Vec<Value> = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        lines.extend(text.lines().map(|line| serde_json::from_str(line).unwrap()));
    }
    let of = |hook: &str| -> Value {
        lines
            .iter()
            .find(|line| line["hook"] == hook)
            .cloned()
            .unwrap_or_else(|| panic!("no record of {hook}: {lines:?}"))
    };
    let guard = of("guard");
    assert_eq!(guard["asked"], "deny");
    assert_eq!(guard["answer"], "allow");
    assert_eq!(guard["unapplied"], serde_json::json!(["denial"]), "{guard}");
    assert_eq!(
        of("rewrite")["unapplied"],
        serde_json::json!(["input"]),
        "{}",
        of("rewrite")
    );
    assert!(of("quiet").get("unapplied").is_none(), "{}", of("quiet"));
}

#[test]
fn a_switches_file_that_does_not_read_follows_the_registration_s_policy() {
    // A firing ran its generation as installed when `switches.json` was
    // there and did not parse, so a handler the owner turned off fired
    // again from an entry the install could not replace (review,
    // 2026-11-02). The firing now treats the file as it treats a plan
    // that does not read: a guarded registration denies, and nothing
    // runs. A FIFO at the path reads as no regular file, without a wait.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    let ran = scene.path("ran.log");
    let guard = scene.handler(
        "guard",
        &format!(
            "cat > /dev/null; echo guard >> {}; printf '{{}}'",
            ran.display()
        ),
    );
    scene.config(&format!(
        "hooks:\n  handlers:\n    - id: guard\n      exec: [sh, {guard}]\n      events: [tool.before]\n      on_failure: deny\n      requires: [tool_deny]\n"
    ));
    scene.rune().args(["hook", "install"]).assert().success();
    let entry = rune_entries(&cursor_table(&scene)).remove(0);
    assert!(entry.contains("--on-failure deny"), "{entry}");
    let switches = scene.state().join("hooks/switches.json");
    let fire = |scene: &Scene| {
        dispatch(
            scene.rune(),
            &[
                "--harness",
                "cursor",
                "--native-event",
                "preToolUse",
                "--plan",
                &plan_of(&entry),
                "--on-failure",
                "deny",
            ],
            &cursor_shell(scene.root.path(), "ls"),
        )
    };
    // No file: nothing is off, the guard runs and allows.
    let answer = fire(&scene);
    assert!(answer.stdout.contains("\"allow\""), "{}", answer.stdout);
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "guard\n");
    // A file that does not parse: the registration's policy, no handler.
    std::fs::write(&switches, "{\"version\": 1, \"off\": [").unwrap();
    let answer = fire(&scene);
    assert!(answer.stdout.contains("\"deny\""), "{}", answer.stdout);
    assert!(answer.stderr.contains("switches.json"), "{}", answer.stderr);
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "guard\n");
    // A version this rune does not read, and a member it does not know.
    std::fs::write(&switches, "{\"version\": 2, \"off\": {}}").unwrap();
    assert!(fire(&scene).stdout.contains("\"deny\""));
    std::fs::write(&switches, "{\"version\": 1, \"off\": {}, \"on\": {}}").unwrap();
    assert!(fire(&scene).stdout.contains("\"deny\""));
    #[cfg(unix)]
    {
        std::fs::remove_file(&switches).unwrap();
        let status = std::process::Command::new("mkfifo")
            .arg(&switches)
            .status()
            .unwrap();
        assert!(status.success());
        let started = std::time::Instant::now();
        let answer = fire(&scene);
        assert!(answer.stdout.contains("\"deny\""), "{}", answer.stdout);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the firing waited on the FIFO"
        );
        assert_eq!(std::fs::read_to_string(&ran).unwrap(), "guard\n");
        // A dangling link at the path read as no file, and so as nothing
        // off (review, 2026-11-02): it is something there that is no
        // switches file, and follows the policy like the rest.
        std::fs::remove_file(&switches).unwrap();
        std::os::unix::fs::symlink(scene.state().join("hooks/gone.json"), &switches).unwrap();
        let answer = fire(&scene);
        assert!(answer.stdout.contains("\"deny\""), "{}", answer.stdout);
        assert!(answer.stderr.contains("symbolic link"), "{}", answer.stderr);
        assert_eq!(std::fs::read_to_string(&ran).unwrap(), "guard\n");
    }
}

#[cfg(unix)]
#[test]
fn a_refused_switch_puts_the_switches_file_back_as_it_was() {
    // A refused switch wrote the state back through the serializer: the
    // owner's formatting went, a file whose switches were all lifted was
    // deleted, and a deleted file came back with rune's mode (review,
    // 2026-11-02). The file now goes back byte for byte, with its mode.
    use std::os::unix::fs::PermissionsExt as _;
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    two_recording_handlers(&scene);
    scene.rune().args(["hook", "install"]).assert().success();
    // The timeout of rune's own entry changes by hand: every install is
    // refused until it is put back.
    let table = cursor_table(&scene);
    let mut value = json_at(&table);
    value["hooks"]["preToolUse"][0]["timeout"] = serde_json::json!(99);
    std::fs::write(&table, value.to_string()).unwrap();
    let switches = scene.state().join("hooks/switches.json");
    for original in [
        "{\"off\": {\"b\": [\"claude\"]}, \"version\": 1}\n",
        "{\"version\": 1,\n  \"off\": {}}",
    ] {
        std::fs::write(&switches, original).unwrap();
        std::fs::set_permissions(&switches, std::fs::Permissions::from_mode(0o644)).unwrap();
        scene
            .rune()
            .args(["hook", "off", "a", "--harness", "cursor"])
            .assert()
            .failure();
        assert_eq!(
            std::fs::read_to_string(&switches).unwrap(),
            original,
            "the refused switch changed the file"
        );
        assert_eq!(
            std::fs::metadata(&switches).unwrap().permissions().mode() & 0o777,
            0o644,
            "the refused switch changed the mode"
        );
    }
}

#[test]
fn a_firing_reads_the_switches_beside_its_plan_whatever_the_state_directory_says() {
    // A registration names its plan by path; a firing looked the
    // switches up beside the plan only when the plan was a generation,
    // and otherwise in the state directory the firing's own environment
    // named, so `--plan /A/hooks/plan.json` under `RUNE_STATE_DIR=/B`
    // read B's switches, and a relative plan read `switches.json` from
    // the working directory (review, 2026-11-02). The lookup is now
    // anchored to the plan's canonical path, and a relative one is
    // refused.
    let scene = Scene::new();
    std::fs::create_dir_all(scene.home().join(".cursor")).unwrap();
    let ran = two_recording_handlers(&scene);
    scene.write_plan();
    let hooks = scene.state().join("hooks");
    std::fs::write(
        hooks.join("switches.json"),
        "{\"version\": 1, \"off\": {\"a\": [\"cursor\"]}}\n",
    )
    .unwrap();
    let other = scene.path("other-state");
    std::fs::create_dir_all(&other).unwrap();
    let plan = hooks.join("plan.json");
    let mut command = scene.rune();
    command.env("RUNE_STATE_DIR", &other);
    let answer = dispatch(
        command,
        &[
            "--harness",
            "cursor",
            "--native-event",
            "preToolUse",
            "--plan",
            plan.to_str().unwrap(),
        ],
        &cursor_shell(scene.root.path(), "ls"),
    );
    assert_eq!(answer.code, 0, "{} {}", answer.stdout, answer.stderr);
    assert!(
        answer.stderr.contains("a switched off for cursor; not run"),
        "{}",
        answer.stderr
    );
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "b\n");
    // A relative plan is refused: with no plan read, a registration
    // without a deny guard proceeds, and runs nothing.
    std::fs::remove_file(&ran).unwrap();
    let mut command = scene.rune();
    command.current_dir(&hooks);
    let answer = dispatch(
        command,
        &[
            "--harness",
            "cursor",
            "--native-event",
            "preToolUse",
            "--plan",
            "plan.json",
        ],
        &cursor_shell(scene.root.path(), "ls"),
    );
    assert!(answer.stdout.contains("\"allow\""), "{}", answer.stdout);
    assert!(answer.stderr.contains("relative"), "{}", answer.stderr);
    assert!(!ran.exists(), "a handler ran under a relative plan path");
}
