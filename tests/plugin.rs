use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn write_plugin(home: &Path, name: &str, events: &str, script: &str) {
    let directory = home.join(".config/rune/plugins").join(name);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("plugin.yaml"),
        format!("name: {name}\ndescription: test plugin\nexec: run.sh\nevents: [{events}]\n"),
    )
    .unwrap();
    let exec = directory.join("run.sh");
    fs::write(&exec, script).unwrap();
    fs::set_permissions(&exec, fs::Permissions::from_mode(0o755)).unwrap();
}

fn consumer_fixture(root: &Path) {
    let module = root.join("module");
    fs::create_dir_all(module.join("rules")).unwrap();
    fs::write(module.join("module.yaml"), "name: mod\n").unwrap();
    fs::write(module.join("rules/Style.md"), "Use the active voice.\n").unwrap();
    fs::write(
        root.join(".rune"),
        "version: 1\nsources:\n    mod:\n        local: module\nrunes:\n    mod:\n        rules: [Style]\n",
    )
    .unwrap();
}

#[test]
fn plugin_list_shows_manifests_and_events() {
    let home = tempfile::tempdir().unwrap();
    write_plugin(home.path(), "syncer", "post-install", "#!/bin/sh\nexit 0\n");
    write_plugin(home.path(), "notifier", "post-install", "#!/bin/sh\nexit 0\n");

    let output = rune()
        .env("HOME", home.path())
        .args(["--json", "plugin", "list"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let plugins = report["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 2);
    assert_eq!(plugins[0]["name"], "notifier");
    assert_eq!(plugins[1]["events"][0], "post-install");
}

#[test]
fn install_fires_the_post_install_event() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());
    write_plugin(
        home.path(),
        "recorder",
        "post-install",
        "#!/bin/sh\ncat > event.json\n",
    );

    rune()
        .current_dir(root.path())
        .env("HOME", home.path())
        .args(["install", "--provider", "claude"])
        .assert()
        .success();

    let event = fs::read_to_string(
        home.path()
            .join(".config/rune/plugins/recorder/event.json"),
    )
    .expect("the plugin received the event");
    let payload: serde_json::Value = serde_json::from_str(&event).unwrap();
    assert_eq!(payload["event"], "post-install");
    assert!(payload["deployed"].as_u64().unwrap() >= 1);
    let source = payload["source"].as_str().unwrap();
    assert!(source.starts_with('/'), "source is resolved: {source}");
}

#[test]
fn failing_plugin_cannot_break_the_install() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    consumer_fixture(root.path());
    write_plugin(home.path(), "broken", "post-install", "#!/bin/sh\nexit 7\n");

    let output = rune()
        .current_dir(root.path())
        .env("HOME", home.path())
        .args(["install", "--provider", "claude"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("broken"), "warning names the plugin: {stderr}");
    assert!(root.path().join(".claude/rules/Style.md").is_file());
}

#[test]
fn escaping_executable_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let directory = home.path().join(".config/rune/plugins/escape");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("plugin.yaml"),
        "name: escape\nexec: ../../../../bin/sh\nevents: [post-install]\n",
    )
    .unwrap();

    let output = rune()
        .env("HOME", home.path())
        .args(["--json", "plugin", "list"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["plugins"].as_array().unwrap().len(), 0);
    let invalid = report["invalid"].as_array().unwrap();
    assert_eq!(invalid.len(), 1);
    assert!(invalid[0].as_str().unwrap().contains("escapes"));
}
