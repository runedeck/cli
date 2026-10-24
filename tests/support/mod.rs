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

/// A HOME, a config directory, and a state directory that belong to this
/// test binary alone, made once under cargo's target tmp directory. They
/// hold no rune config, so `rune install` registers no user hook.
pub fn scratch() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = tempfile::Builder::new()
            .prefix("isolated-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .expect("a scratch root under the target tmp directory")
            .keep();
        for dir in ["home/.config/rune", "state"] {
            std::fs::create_dir_all(root.join(dir)).expect("a scratch directory");
        }
        root
    })
}

/// Variables that point rune, or a harness it reads, at a directory
/// outside HOME. A test never inherits them from the developer.
const INHERITED: &[&str] = &[
    "RUNE_STATE_DIR",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_PROJECT_DIR",
    "CODEX_HOME",
    "GROK_SESSION_ID",
];

/// Point a command at the scratch HOME, config, and state, and drop every
/// inherited `XDG_*` variable and directory override. `rune install`
/// installs the user's hooks from HOME, so a command that reached the
/// real one would rewrite the developer's harness tables. A test may set
/// its own HOME or state directory after this call.
pub fn isolate(command: &mut assert_cmd::Command) -> &mut assert_cmd::Command {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("XDG_") {
            command.env_remove(&key);
        }
    }
    for key in INHERITED {
        command.env_remove(key);
    }
    let root = scratch();
    command
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("home/.config"))
        .env("RUNE_STATE_DIR", root.join("state"))
}

/// The rune binary, isolated from the developer's HOME, config, and state.
pub fn rune() -> assert_cmd::Command {
    let mut command = assert_cmd::Command::cargo_bin("rune").expect("the rune binary");
    isolate(&mut command);
    command
}
