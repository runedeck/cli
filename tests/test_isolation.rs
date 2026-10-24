//! No integration test reaches the developer's HOME. `rune install` installs
//! the user's hooks into the harness tables under HOME, so a test that saw
//! the real one would rewrite `~/.claude/settings.json` and
//! `~/.codex/hooks.json`, and every test binary would share one live plan.

mod support;

use std::path::{Path, PathBuf};

/// The HOME this test process inherited from whoever ran it.
fn real_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[test]
fn a_command_from_the_helper_sees_the_scratch_home() {
    let output = support::rune()
        .args(["config", "path"])
        .output()
        .expect("rune config path");
    assert!(output.status.success(), "{output:?}");
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    assert!(
        path.starts_with(support::scratch()),
        "the config lives at {}, outside the scratch HOME {}",
        path.display(),
        support::scratch().display()
    );
    if let Some(home) = real_home().filter(|home| !support::scratch().starts_with(home)) {
        assert!(
            !path.starts_with(&home),
            "a test saw the real HOME {}",
            home.display()
        );
    }
}

/// Every test file that starts the rune binary builds it through
/// `support::rune()`, or names its own HOME.
#[test]
fn every_test_file_isolates_the_rune_it_starts() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&tests).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let starts_rune =
            text.contains("cargo_bin(\"rune\")") || text.contains("CARGO_BIN_EXE_rune");
        if starts_rune && !text.contains("\"HOME\"") {
            offenders.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "these files start rune without support::rune() or a HOME of their own: {offenders:?}"
    );
}
