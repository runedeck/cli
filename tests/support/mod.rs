// Each test crate uses part of this module.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn deck_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck")
}

pub fn copy_deck_fixture(destination: &Path) {
    copy_directory(&deck_fixture(), destination);
}

fn copy_directory(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A HOME that belongs to this test binary alone, made once under cargo's
/// target tmp directory. Its rune config is empty, so `rune install`
/// registers no user hook, and rune keeps its state under it.
pub fn scratch() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = tempfile::Builder::new()
            .prefix("isolated-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .expect("a scratch root under the target tmp directory")
            .keep();
        std::fs::create_dir_all(root.join("home/.config/rune")).expect("a scratch HOME");
        root
    })
}

/// Variables that point rune, or a harness it reads, at a directory
/// outside HOME, or mark a proof scene. A test never inherits them from
/// the developer.
const INHERITED: &[&str] = &[
    "RUNE_STATE_DIR",
    "RUNE_PROOF_SCENE",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_PROJECT_DIR",
    "CODEX_HOME",
    "GROK_SESSION_ID",
];

/// The variables to drop and the one to set for an isolated command. With
/// every `XDG_*` variable and directory override gone, rune's config and
/// state, and the config of jj and git, all follow HOME, so a test that
/// sets a HOME of its own moves all of them with it.
fn isolation() -> (Vec<std::ffi::OsString>, [(&'static str, PathBuf); 1]) {
    let mut removed: Vec<std::ffi::OsString> = std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| key.to_string_lossy().starts_with("XDG_"))
        .collect();
    removed.extend(INHERITED.iter().map(std::ffi::OsString::from));
    (removed, [("HOME", scratch().join("home"))])
}

/// Point a command at the scratch HOME, and drop every inherited `XDG_*`
/// variable and directory override. `rune install` installs the user's
/// hooks from HOME, so a command that reached the real one would rewrite
/// the developer's harness tables. A test may set its own HOME or state
/// directory after this call.
pub fn isolate(command: &mut assert_cmd::Command) -> &mut assert_cmd::Command {
    let (removed, set) = isolation();
    for key in removed {
        command.env_remove(key);
    }
    command.envs(set)
}

/// `isolate` for a `std::process::Command`, such as one a test spawns and
/// waits for itself.
pub fn isolate_process(command: &mut std::process::Command) -> &mut std::process::Command {
    let (removed, set) = isolation();
    for key in removed {
        command.env_remove(key);
    }
    command.envs(set)
}

/// The rune binary's path, for a test that copies it.
pub fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rune"))
}

/// The rune binary, isolated from the developer's HOME, config, and state.
pub fn rune() -> assert_cmd::Command {
    let mut command = assert_cmd::Command::new(binary());
    isolate(&mut command);
    command
}

/// The rune binary as a `std::process::Command`, isolated.
pub fn rune_process() -> std::process::Command {
    let mut command = std::process::Command::new(binary());
    isolate_process(&mut command);
    command
}

/// The rune binary with an empty environment, except the scratch HOME,
/// config, and state. On macOS a process with no HOME still finds the
/// account's home folder, so an empty environment alone is no isolation.
pub fn rune_clean() -> assert_cmd::Command {
    let mut command = assert_cmd::Command::new(binary());
    command.env_clear();
    isolate(&mut command);
    command
}
