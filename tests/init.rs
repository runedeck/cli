use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn git() -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    command
}

fn skeleton_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/skeleton")
}

fn init(home: &Path, quests: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .env("HOME", home)
        .env_remove("RUNE_TARGETS")
        .env("RUNE_QUESTS", quests)
        .arg("init")
        .args(args)
        .arg("--skeleton")
        .arg(skeleton_fixture())
        .assert()
}

#[test]
fn project_init_composes_layers_and_substitutes_contents_and_names() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("signal-lamp");

    init(
        home.path(),
        quests.path(),
        &[
            "N4M3Z/signal-lamp",
            "--lang",
            "shell",
            "--purpose",
            "tool",
            "--brief",
            "Warns the crew",
        ],
    )
    .success()
    .stdout(predicate::str::contains(
        "layers: base, lang/shell, purpose/tool",
    ))
    .stdout(predicate::str::contains("destination:"))
    .stdout(predicate::str::contains("rune add <deck>"))
    .stdout(predicate::str::contains("rune tui --edit"));

    let makefile = fs::read_to_string(destination.join("Makefile")).unwrap();
    assert!(makefile.contains("NAME := signal-lamp"));
    assert!(makefile.contains("TITLE := Signal Lamp"));
    assert!(makefile.contains("OWNER := N4M3Z"));
    assert!(makefile.contains("BRIEF := Warns the crew"));
    assert!(makefile.contains("UNKNOWN := ${UNCHANGED}"));
    assert!(destination.join("signal-lamp.txt").is_file());
    assert!(destination.join("bin/signal-lamp").is_file());
    assert_eq!(
        fs::read_to_string(destination.join("README.md")).unwrap(),
        "# Signal Lamp\n\nWarns the crew\n"
    );
    assert!(destination.join(".git").exists());
    let hooks_path = git()
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(&destination)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(hooks_path.stdout).unwrap().trim(),
        ".githooks"
    );
    let branch = git()
        .args(["branch", "--show-current"])
        .current_dir(&destination)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(branch.stdout).unwrap().trim(), "main");
    // Under the targets root init runs in workshop mode: layout lands,
    // the first commit stays a human decision.
    let head = git()
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(&destination)
        .output()
        .unwrap();
    assert!(!head.status.success(), "workshop init must not auto-commit");
    for member in ["private", "public", "assets"] {
        assert!(destination.join(member).is_dir(), "missing {member}/");
    }

    #[cfg(unix)]
    for executable in [".githooks/pre-commit", "bin/signal-lamp"] {
        let mode = fs::metadata(destination.join(executable))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "{executable} should be executable");
    }
}

#[cfg(unix)]
#[test]
fn project_init_scaffold_commit_ignores_inherited_git_hooks() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let hooks = home.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let pre_commit = hooks.join("pre-commit");
    fs::write(&pre_commit, "#!/bin/sh\nexit 1\n").unwrap();
    let mut permissions = fs::metadata(&pre_commit).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&pre_commit, permissions).unwrap();
    assert!(
        git()
            .env("HOME", home.path())
            .args([
                "config",
                "--global",
                "core.hooksPath",
                hooks.to_str().unwrap(),
            ])
            .status()
            .unwrap()
            .success()
    );

    let destination = quests.path().join("hook-proof");
    init(
        home.path(),
        quests.path(),
        &["hook-proof", "--lang", "shell", "--purpose", "tool"],
    )
    .success();

    let head = git()
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(destination)
        .output()
        .unwrap();
    assert!(!head.status.success(), "workshop init must not auto-commit");
}

#[test]
fn project_init_commit_excludes_existing_repository_files() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let hooks = destination.path().join("custom-hooks");

    assert!(
        git()
            .args(["init", "-b", "main"])
            .current_dir(destination.path())
            .status()
            .unwrap()
            .success()
    );
    assert!(
        git()
            .args(["config", "core.hooksPath"])
            .arg(&hooks)
            .current_dir(destination.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(
        destination.path().join("private-note.txt"),
        "Not part of the scaffold.\n",
    )
    .unwrap();

    init(
        home.path(),
        quests.path(),
        &[
            &destination.path().to_string_lossy(),
            "--lang",
            "shell",
            "--purpose",
            "tool",
        ],
    )
    .success();

    let committed_files = git()
        .args(["show", "--format=", "--name-only", "HEAD"])
        .current_dir(destination.path())
        .output()
        .unwrap();
    let committed_files = String::from_utf8(committed_files.stdout).unwrap();
    assert!(committed_files.lines().any(|path| path == "Makefile"));
    assert!(
        !committed_files
            .lines()
            .any(|path| path == "private-note.txt")
    );

    let private_note_status = git()
        .args(["status", "--porcelain", "--", "private-note.txt"])
        .current_dir(destination.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(private_note_status.stdout).unwrap(),
        "?? private-note.txt\n"
    );

    let hooks_path = git()
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(destination.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(hooks_path.stdout).unwrap().trim(),
        hooks.to_string_lossy()
    );
}

#[test]
fn project_init_does_not_commit_when_every_template_exists() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("complete-project");

    init(
        home.path(),
        quests.path(),
        &["complete-project", "--lang", "shell", "--purpose", "tool"],
    )
    .success();
    init(
        home.path(),
        quests.path(),
        &[
            &destination.to_string_lossy(),
            "--lang",
            "shell",
            "--purpose",
            "tool",
        ],
    )
    .success();

    let head = git()
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(destination)
        .output()
        .unwrap();
    assert!(!head.status.success());
}

#[test]
fn project_init_repo_is_silent_during_install_freshness_check() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("quiet-main");

    init(
        home.path(),
        quests.path(),
        &["quiet-main", "--lang", "shell", "--purpose", "tool"],
    )
    .success();

    rune()
        .env("HOME", home.path())
        .args(["install", "--source", destination.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("git freshness").not());
}

#[test]
fn project_init_rerun_skips_existing_without_overwriting() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("demo");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("README.md"), "# Hand written\n").unwrap();

    init(
        home.path(),
        quests.path(),
        &["demo", "--lang", "shell", "--purpose", "tool"],
    )
    .success();
    let second = init(
        home.path(),
        quests.path(),
        &["demo", "--lang", "shell", "--purpose", "tool"],
    );
    second
        .success()
        .stdout(predicate::str::contains("already exists"))
        .stdout(predicate::str::contains("skipped"));

    assert_eq!(
        fs::read_to_string(destination.join("README.md")).unwrap(),
        "# Hand written\n"
    );
}

#[test]
fn bare_name_resolves_under_rune_quests_and_quest_flag_binds_it() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();

    init(
        home.path(),
        quests.path(),
        &["demo", "--lang", "shell", "--purpose", "tool", "--quest"],
    )
    .success()
    .stdout(predicate::str::contains("quest: bound to destination"));

    let destination = quests.path().join("demo");
    assert!(destination.join("Makefile").is_file());
    let state = fs::read_to_string(home.path().join(".config/rune/state.yaml")).unwrap();
    assert!(state.contains(&destination.to_string_lossy().to_string()));
}

#[test]
fn existing_directory_is_used_in_place() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();

    init(
        home.path(),
        quests.path(),
        &[
            &destination.path().to_string_lossy(),
            "--lang",
            "shell",
            "--purpose",
            "tool",
        ],
    )
    .success();

    assert!(destination.path().join("Makefile").is_file());
    assert!(
        !quests
            .path()
            .join(destination.path().file_name().unwrap())
            .exists()
    );
}

#[test]
fn module_scaffolder_remains_available_behind_module_flag() {
    let home = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();

    rune()
        .env("HOME", home.path())
        .args(["init", "--module", &destination.path().to_string_lossy()])
        .assert()
        .success();

    assert!(destination.path().join("module.yaml").is_file());
    assert!(destination.path().join(".manifest").is_file());
}

#[test]
fn init_help_matches_snapshot() {
    let output = rune().args(["init", "--help"]).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        include_str!("fixtures/init-help.txt")
    );
}
