//! Release notes for a release with no merged pull request.
//!
//! `scripts/compile-release-notes` reads the release note entries of every
//! pull request merged since the last release. A release whose commits
//! were pushed straight to `main` has none, and the notes then come from
//! the changelog section of the requested tag. These tests run the script
//! in a fixture repository and prove that it prints that section only.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const CHANGELOG: &str = "\
# Changelog

[0.7.0]: https://example.com/compare/v0.6.0...v0.7.0
[0.6.0]: https://example.com/releases/tag/v0.6.0

## [Unreleased]

### Added

- An entry that no release holds yet.

## [0.7.0] - 2026-11-16

### Added

- Add the first new thing.
- Add the second new thing.

### Fixed

- Fix the old thing.

## [0.6.0] - 2026-10-26

### Added

- An entry of the release before.
";

const EXPECTED: &str = "\
# v0.7.0

### Added

- Add the first new thing.
- Add the second new thing.

### Fixed

- Fix the old thing.
";

fn git(repository: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .current_dir(repository)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `mapfile` needs bash 4. macOS ships 3.2 as `/bin/bash`, and a machine
/// without a newer bash on PATH cannot run the script at all.
fn bash_has_mapfile() -> bool {
    Command::new("bash")
        .args(["-c", "type mapfile"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Two releases on a history with no merge commit: `v0.6.0` on the first
/// commit, and the changelog above on the second.
fn fixture(changelog: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("fixture repository");
    let repository = directory.path();
    git(repository, &["init", "-q", "-b", "main"]);
    fs::write(repository.join("CHANGELOG.md"), "# Changelog\n").expect("first changelog");
    git(repository, &["add", "CHANGELOG.md"]);
    git(repository, &["commit", "-q", "-m", "release 0.6.0"]);
    git(repository, &["tag", "v0.6.0"]);
    fs::write(repository.join("CHANGELOG.md"), changelog).expect("second changelog");
    git(repository, &["commit", "-q", "-am", "release 0.7.0"]);
    git(repository, &["tag", "v0.7.0"]);
    directory
}

/// Run the script for `tag`. A `gh` that leaves a file when it starts
/// stands first on PATH, so a test can prove that no API call was made.
fn compile(repository: &Path, tag: &str) -> Output {
    let tools = repository.join(".fake-tools");
    fs::create_dir_all(&tools).expect("tool folder");
    let fake_gh = tools.join("gh");
    fs::write(
        &fake_gh,
        format!(
            "#!/bin/sh\ntouch '{}'\nexit 1\n",
            gh_marker(repository).display()
        ),
    )
    .expect("fake gh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_gh, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let mut path = tools.into_os_string();
    if let Some(inherited) = std::env::var_os("PATH") {
        path.push(":");
        path.push(inherited);
    }
    Command::new("bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/compile-release-notes"))
        .arg(tag)
        .current_dir(repository)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env("GITHUB_REPOSITORY", "example/fixture")
        .env("PATH", path)
        .output()
        .expect("bash runs scripts/compile-release-notes")
}

/// The file the fake `gh` leaves when the script calls it.
fn gh_marker(repository: &Path) -> std::path::PathBuf {
    repository.join(".gh-was-called")
}

#[test]
fn notes_are_the_changelog_section_of_the_requested_tag() {
    if !bash_has_mapfile() {
        eprintln!("skipped: bash on PATH has no mapfile");
        return;
    }
    let repository = fixture(CHANGELOG);
    let output = compile(repository.path(), "v0.7.0");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let notes = String::from_utf8_lossy(&output.stdout);
    assert_eq!(notes, EXPECTED);
    assert!(
        !gh_marker(repository.path()).exists(),
        "the script called the API for a release with no merged pull request"
    );
    for foreign in [
        "no release holds yet",
        "release before",
        "Unreleased",
        "0.6.0",
        "example.com",
    ] {
        assert!(
            !notes.contains(foreign),
            "the notes hold `{foreign}`:\n{notes}"
        );
    }
}

#[test]
fn a_tag_without_a_changelog_section_reports_no_user_facing_change() {
    if !bash_has_mapfile() {
        eprintln!("skipped: bash on PATH has no mapfile");
        return;
    }
    let repository =
        fixture(&CHANGELOG.replace("## [0.7.0] - 2026-11-16", "## [0.7.1] - 2026-11-16"));
    let output = compile(repository.path(), "v0.7.0");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "# v0.7.0\n\n- No user-facing changes.\n"
    );
}

#[test]
fn a_longer_version_does_not_match_a_shorter_heading() {
    if !bash_has_mapfile() {
        eprintln!("skipped: bash on PATH has no mapfile");
        return;
    }
    // `## [0.7.0]` must not answer for a tag named v0.7.
    let repository = fixture(CHANGELOG);
    git(repository.path(), &["tag", "v0.7.00"]);
    let output = compile(repository.path(), "v0.7.00");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "# v0.7.00\n\n- No user-facing changes.\n"
    );
}
