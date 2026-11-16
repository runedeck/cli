//! The sentinel: a test command never reaches the developer's data folder.
//!
//! The session capture copies harness sessions into `DATA_DIR`, and
//! `rune run` and `rune launch` start it. A developer who exports
//! `DATA_DIR` would get the fixture sessions of every such test in the
//! real data folder. `support::rune()` drops the variables that move the
//! capture, and this test proves it: it runs itself again with those
//! variables set to sentinel folders, as a developer's shell would have
//! them, and nothing may arrive there.
//!
//! Run this test before the full suite. When it fails, the full suite
//! must not run on a machine whose shell exports `DATA_DIR`.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Set for the inner run: the scratch root both halves share.
const INNER: &str = "RUNE_CAPTURE_SENTINEL_ROOT";
const TEST: &str = "a_test_command_never_reaches_the_developers_data_folder";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("parent folder");
    std::fs::write(path, text).expect("fixture file");
}

/// Every file under `root`, as paths relative to it.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path.strip_prefix(root).expect("under root").to_path_buf());
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_test_command_never_reaches_the_developers_data_folder() {
    if let Some(root) = std::env::var_os(INNER) {
        inner(Path::new(&root));
        return;
    }

    let scratch = tempfile::tempdir().expect("scratch root");
    let root = scratch.path();
    let developer = root.join("developer");
    let data = developer.join("Data");
    std::fs::create_dir_all(&data).expect("the developer's data folder");
    // A session in the store the developer's variable names. A capture
    // that read the variable would copy it.
    write(
        &developer.join("antigravity/conversations/sentinel.db"),
        "developer session",
    );
    // Tools the developer's variables name. Each leaves a file when it runs.
    for tool in ["specstory", "sqlite3"] {
        let path = developer.join(tool);
        write(
            &path,
            &format!(
                "#!/bin/sh\ntouch '{}'\n",
                developer.join(format!("{tool}-ran")).display()
            ),
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("executable");
        }
    }

    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", TEST, "--nocapture", "--test-threads", "1"])
        .env(INNER, root)
        .env("DATA_DIR", &data)
        .env(
            "ANTIGRAVITY_EXECUTABLE_DATA_DIR",
            developer.join("antigravity"),
        )
        .env("SPECSTORY_BIN", developer.join("specstory"))
        .env("SQLITE3_BIN", developer.join("sqlite3"))
        .env("SESSION_SYNC_LOCK_WAIT", "0")
        .output()
        .expect("the inner run starts");
    assert!(
        output.status.success(),
        "the inner run failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        files_under(&data),
        Vec::<PathBuf>::new(),
        "a test command wrote into the developer's data folder"
    );
    for tool in ["specstory", "sqlite3"] {
        assert!(
            !developer.join(format!("{tool}-ran")).exists(),
            "a test command ran the developer's {tool}"
        );
    }
    let copied = files_under(&root.join("home/Data"));
    assert!(
        copied.contains(&PathBuf::from("Sessions/claude/-tmp-repo/abc.jsonl")),
        "the capture did not land under the test's own HOME: {copied:?}"
    );
    assert!(
        !copied
            .iter()
            .any(|path| path.to_string_lossy().contains("sentinel")),
        "the capture read the developer's Antigravity store: {copied:?}"
    );
}

/// The inner half runs with the developer's variables in its environment.
/// It captures and sweeps a fixture home through `support::rune()`, the way
/// every other test starts rune.
fn inner(root: &Path) {
    let home = root.join("home");
    write(&home.join(".config/rune/config.yaml"), "");
    write(
        &home.join(".claude/projects/-tmp-repo/abc.jsonl"),
        "{\"cwd\":\"/tmp/repo\",\"type\":\"user\",\"message\":\"hi\"}\n",
    );
    for arguments in [
        vec![
            "session",
            "capture",
            "--harness",
            "claude",
            "--session",
            "abc",
        ],
        vec!["session", "sweep"],
    ] {
        // The system folders hold no `specstory`, so the readable copy,
        // which would run the developer's own install, does not start.
        let output = support::rune()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin")
            .args(&arguments)
            .output()
            .expect("rune runs");
        assert!(
            output.status.success(),
            "rune {arguments:?} failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
