//! The hook record stays on the machine: a populated record under the state
//! dir reaches no release tarball, and no command outside the hook module
//! reads the record directory at all.

mod support;

use std::path::Path;

use assert_cmd::Command;

/// A sentinel the record carries, as a path, so a leak is one grep away.
const SENTINEL: &str = "/vault/private/AMBER-sentinel-note.md";

fn populated_state() -> tempfile::TempDir {
    let state = tempfile::tempdir().unwrap();
    let record = state.path().join("hooks").join("record");
    std::fs::create_dir_all(&record).unwrap();
    std::fs::write(
        record.join("2026-10.jsonl"),
        format!(
            r#"{{"v":1,"ts":"2026-10-16T00:10:16.912Z","harness":"claude","event":"tool.after","native_event":"PostToolUse","hook":"lint-on-write","session_id":"s","tool_use_id":"toolu_1","cwd":"/w","file":"{SENTINEL}","duration_ms":12,"exit_status":0,"asked":"deny","answer":"deny","failure":null,"metrics":{{"rules":{{"Core.NoEmDash":1}},"cause":["Core.NoEmDash"]}}}}
"#
        ),
    )
    .unwrap();
    state
}

fn rune(state: &Path) -> Command {
    let mut command = support::rune();
    command.env("RUNE_STATE_DIR", state);
    command
}

#[test]
fn a_release_carries_no_record_line() {
    let state = populated_state();
    let deck = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(deck.path());
    rune(state.path())
        .args([
            "release",
            "science",
            "--source",
            deck.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let dist = deck.path().join("runes/science/dist");
    let mut tarballs = 0;
    for entry in std::fs::read_dir(&dist).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "gz") {
            tarballs += 1;
            let bytes = std::fs::read(&path).unwrap();
            let listing = Command::new("tar")
                .args(["-tzf", path.to_str().unwrap()])
                .output()
                .unwrap();
            let names = String::from_utf8_lossy(&listing.stdout);
            assert!(!names.contains("hooks/record"), "{names}");
            // Decompressed content, in case a record line was embedded in another file.
            let content = Command::new("tar")
                .args(["-xzOf", path.to_str().unwrap()])
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&content.stdout);
            assert!(
                !text.contains(SENTINEL),
                "a record line reached {}",
                path.display()
            );
            assert_ne!(bytes.len(), 0);
        }
    }
    assert!(tarballs > 0, "the release wrote no tarball");
    // The record itself read back unchanged through the one reader it has.
    let listing = rune(state.path())
        .args(["hook", "record"])
        .assert()
        .success();
    assert!(String::from_utf8_lossy(&listing.get_output().stdout).contains(SENTINEL));
}

#[test]
fn a_proof_run_carries_no_record_line() {
    let state = populated_state();
    let repo = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proof/filled-good");
    copy_tree(&fixture, repo.path());
    let output = rune(state.path())
        .args([
            "proof",
            "run",
            "sample-change-one",
            "--pause",
            "0",
            "--source",
            repo.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(SENTINEL));
    let mut leaked = Vec::new();
    for entry in walkdir(repo.path()) {
        if let Ok(text) = std::fs::read_to_string(&entry)
            && text.contains(SENTINEL)
        {
            leaked.push(entry);
        }
    }
    assert!(leaked.is_empty(), "{leaked:?}");
}

/// A scene that reads the record without naming a fixture state sees an
/// empty one: the runner gives every scene its own state directory.
#[test]
fn a_proof_scene_reads_its_own_state_and_never_the_live_record() {
    let state = populated_state();
    let repo = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proof/filled-good");
    copy_tree(&fixture, repo.path());
    let readme = repo.path().join("docs/proofs/sample-change-one/README.md");
    let text = std::fs::read_to_string(&readme).unwrap().replace(
        "$ rune --version\nrune [..] built [..]",
        "$ rune hook record --json",
    );
    assert!(text.contains("$ rune hook record --json"), "{text}");
    std::fs::write(&readme, text).unwrap();
    let output = rune(state.path())
        .args([
            "proof",
            "run",
            "sample-change-one",
            "--pause",
            "0",
            "--source",
            repo.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut leaked = Vec::new();
    for entry in walkdir(repo.path()) {
        if let Ok(text) = std::fs::read_to_string(&entry)
            && text.contains(SENTINEL)
        {
            leaked.push(entry);
        }
    }
    assert!(leaked.is_empty(), "{leaked:?}");
}

/// Only the hook module names the record directory in the source tree, so
/// release, bench, proof, and sign cannot read it by construction.
#[test]
fn only_the_hook_module_reads_the_record_directory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();
    for path in walkdir(&root.join("src")) {
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let inside_hook = path.starts_with(root.join("src").join("cli").join("hook"));
        if inside_hook {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // `hook::record_command` is the one entry point and may be named by
        // the command table; the directory and its readers may not.
        if text.contains("hooks/record")
            || text.contains("hook::record::")
            || text.contains("record::dir")
            || text.contains("record::load")
        {
            offenders.push(path);
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walkdir(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in walkdir(from) {
        let target = to.join(entry.strip_prefix(from).unwrap());
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(&entry, &target).unwrap();
    }
}
