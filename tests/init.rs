use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn skeleton_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/skeleton")
}

fn init(home: &Path, quests: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .env("HOME", home)
        .env("RUNE_QUESTS", quests)
        .arg("init")
        .args(args)
        .arg("--skeleton")
        .arg(skeleton_fixture())
        .assert()
}

fn init_embedded(home: &Path, quests: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .env("HOME", home)
        .env("RUNE_QUESTS", quests)
        .arg("init")
        .args(args)
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
    .stdout(predicate::str::contains("layers: base, shell, tool"))
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
        fs::read_to_string(destination.join("verbatim.txt")).unwrap(),
        "Literal ${NAME} and ${{ github.ref }} expressions.\n"
    );
    assert_eq!(
        fs::read_to_string(destination.join("README.md")).unwrap(),
        "# Signal Lamp\n\nWarns the crew\n"
    );
    let copier_answers = fs::read_to_string(destination.join("answers.yaml")).unwrap();
    assert!(copier_answers.contains("BRIEF: Warns the crew"));
    assert!(copier_answers.contains("NAME: signal-lamp"));
    assert!(copier_answers.contains("OWNER: N4M3Z"));
    assert!(copier_answers.contains("TITLE: Signal Lamp"));
    assert!(copier_answers.contains("_commit:"));
    assert!(copier_answers.contains("_src_path:"));
    assert!(!destination.join("answers.yaml.jinja").exists());
    assert!(destination.join(".git").exists());
    let hooks_path = std::process::Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(&destination)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(hooks_path.stdout).unwrap().trim(),
        ".githooks"
    );
    let branch = std::process::Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args(["branch", "--show-current"])
        .current_dir(&destination)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(branch.stdout).unwrap().trim(), "main");
    // Under the targets root init runs in workshop mode: layout lands,
    // the first commit stays a human decision.
    let head = std::process::Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
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
        std::process::Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
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

    let head = std::process::Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(destination)
        .output()
        .unwrap();
    assert!(!head.status.success(), "workshop init must not auto-commit");
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
fn with_templates_compose_flat_layers_in_order() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("ordered");

    init(
        home.path(),
        quests.path(),
        &["ordered", "--with", "shell,tool"],
    )
    .success()
    .stdout(predicate::str::contains("layers: base, shell, tool"));

    assert_eq!(
        fs::read_to_string(destination.join("selection.txt")).unwrap(),
        "Tool template.\n"
    );
}

#[test]
fn unknown_template_fails_before_writing() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("unknown");

    init(
        home.path(),
        quests.path(),
        &["unknown", "--with", "missing"],
    )
    .failure()
    .stderr(predicate::str::contains("available templates: shell, tool"));

    assert!(!destination.exists());
}

#[test]
fn noninteractive_init_without_templates_applies_base_only() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("base-only");

    init(home.path(), quests.path(), &["base-only"])
        .success()
        .stdout(predicate::str::contains("layers: base"));

    assert!(destination.join("Makefile").is_file());
    assert!(!destination.join("selection.txt").exists());
    assert!(!destination.join("bin/base-only").exists());
}

#[test]
fn retrofit_appends_missing_gitignore_entries_once() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    fs::write(destination.path().join(".gitignore"), "custom/\n").unwrap();
    let destination_path = destination.path().to_string_lossy();

    init(
        home.path(),
        quests.path(),
        &[&destination_path, "--with", "shell,tool"],
    )
    .success();
    init(
        home.path(),
        quests.path(),
        &[&destination_path, "--with", "shell,tool"],
    )
    .success();

    assert_eq!(
        fs::read_to_string(destination.path().join(".gitignore")).unwrap(),
        "custom/\ndist/\n# layer: tool\n.cache/\n"
    );
}

#[test]
fn embedded_init_writes_tagged_copier_metadata_offline() {
    let home = tempfile::tempdir().unwrap();
    let quests = tempfile::tempdir().unwrap();
    let destination = quests.path().join("offline-copy");

    init_embedded(
        home.path(),
        quests.path(),
        &[
            "offline-copy",
            "--with",
            "shell,tool",
            "--brief",
            "Works offline",
        ],
    )
    .success();

    let copier_answers = fs::read_to_string(destination.join("answers.yaml")).unwrap();
    assert!(copier_answers.contains("BRIEF: Works offline"));
    assert!(copier_answers.contains("NAME: offline-copy"));
    assert!(copier_answers.contains("_commit: v0.5.0"));
    assert!(copier_answers.contains("_src_path: https://github.com/runedeck/skeleton.git"));
    assert!(!destination.join("AGENTS.md.jinja").exists());
    let agents = fs::read_to_string(destination.join("AGENTS.md")).unwrap();
    assert!(agents.contains("Offline Copy"));
    let workflow = fs::read_to_string(destination.join(".github/workflows/quality.yaml")).unwrap();
    assert!(workflow.contains("${{ github.ref }}"));
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
