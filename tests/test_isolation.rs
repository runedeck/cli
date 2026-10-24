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

/// The config path a command reports, which lives under its HOME.
fn config_path(output: &std::process::Output) -> PathBuf {
    assert!(output.status.success(), "{output:?}");
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

#[test]
fn every_helper_command_sees_the_scratch_home() {
    let outputs = [
        (
            "rune",
            support::rune().args(["config", "path"]).output().unwrap(),
        ),
        (
            "rune_process",
            support::rune_process()
                .args(["config", "path"])
                .output()
                .unwrap(),
        ),
        (
            "rune_clean",
            support::rune_clean()
                .args(["config", "path"])
                .output()
                .unwrap(),
        ),
    ];
    for (helper, output) in outputs {
        let path = config_path(&output);
        assert!(
            path.starts_with(support::scratch()),
            "{helper}: the config lives at {}, outside the scratch HOME {}",
            path.display(),
            support::scratch().display()
        );
        if let Some(home) = real_home().filter(|home| !support::scratch().starts_with(home)) {
            assert!(
                !path.starts_with(&home),
                "{helper}: a test saw the real HOME {}",
                home.display()
            );
        }
    }
}

/// Each way to start rune, or to empty the environment of a command, is
/// checked at its place: outside `tests/support/`, no test file builds the
/// binary itself or clears an environment, so every invocation goes
/// through a helper that isolates it last.
#[test]
fn no_test_file_starts_rune_outside_the_support_module() {
    const STARTS: &[&str] = &["cargo_bin(", "CARGO_BIN_EXE_rune", ".env_clear()"];
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&tests).unwrap().flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.extension().is_none_or(|e| e != "rs") || name == "test_isolation.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (number, line) in text.lines().enumerate() {
            if let Some(start) = STARTS.iter().find(|start| line.contains(*start)) {
                offenders.push(format!("{name}:{}: {start}", number + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "start rune through support::rune(), rune_process(), or rune_clean(): {offenders:?}"
    );
}
