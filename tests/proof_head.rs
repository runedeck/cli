//! The proof check finds the head after a signature or a reword rewrote its
//! commit id, by the change id and the tree, and refuses a content edit.
//! Skips when jj or ssh-keygen is missing.
mod support;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use tempfile::TempDir;

const CHANGE: &str = "sample-change-one";

fn tool_present(name: &str, flag: &str) -> bool {
    Process::new(name)
        .arg(flag)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

struct Fixture {
    _home: TempDir,
    root: PathBuf,
    config: PathBuf,
}

impl Fixture {
    /// jj with an isolated config, as the user and as the rune it starts.
    fn jj(&self) -> Process {
        let mut command = Process::new("jj");
        command
            .arg("-R")
            .arg(&self.root)
            .env("JJ_USER", "Model One")
            .env("JJ_EMAIL", "model-one@example.com")
            .env("JJ_CONFIG", &self.config);
        command
    }

    fn run_jj(&self, args: &[&str]) -> String {
        let output = self.jj().args(args).output().expect("jj runs");
        assert!(
            output.status.success(),
            "jj {args:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn rune(&self) -> assert_cmd::Command {
        let mut command = support::rune();
        command
            .env("JJ_USER", "Model One")
            .env("JJ_EMAIL", "model-one@example.com")
            .env("JJ_CONFIG", &self.config);
        command
    }

    fn check(&self) -> assert_cmd::assert::Assert {
        self.rune()
            .args(["proof", "run", CHANGE, "--check", "--source"])
            .arg(&self.root)
            .assert()
    }

    fn readme(&self) -> PathBuf {
        self.root.join("docs/proofs").join(CHANGE).join("README.md")
    }

    /// The change id of the recorded head.
    fn head_change(&self) -> String {
        let text = fs::read_to_string(self.readme()).expect("readme");
        rune::proof::parse(Path::new("README.md"), &text)
            .expect("frontmatter")
            .head_change
            .expect("head_change recorded")
    }
}

/// A colocated jj repository holding one change with a recorded proof: the
/// change in one commit, the proof on top of it, and an empty working copy.
/// jj signs with an ssh key made for the test.
fn fixture() -> Option<Fixture> {
    if !tool_present("jj", "--version") || !tool_present("ssh-keygen", "-?") {
        eprintln!("skipped: jj or ssh-keygen is not installed");
        return None;
    }
    let home = TempDir::new().expect("tempdir");
    let root = home.path().join("repo");
    let key = home.path().join("key");
    let made = Process::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&key)
        .status()
        .expect("ssh-keygen runs");
    assert!(made.success(), "ssh-keygen failed");
    let config = home.path().join("jj.toml");
    fs::write(
        &config,
        format!(
            "[signing]\nbehavior = \"drop\"\nbackend = \"ssh\"\nkey = \"{}\"\n[revset-aliases]\n\"immutable_heads()\" = \"none()\"\n",
            key.display()
        ),
    )
    .expect("jj config");
    let fixture = Fixture {
        _home: home,
        root,
        config,
    };
    let output = Process::new("jj")
        .args(["git", "init", "--colocate"])
        .arg(&fixture.root)
        .env("JJ_CONFIG", &fixture.config)
        .output()
        .expect("jj git init");
    assert!(output.status.success(), "jj git init failed");

    let change = fixture.root.join("docs/changes").join(CHANGE);
    fs::create_dir_all(change.join("specs/sample-capability-one")).expect("change dir");
    fs::write(
        change.join("proposal.md"),
        "---\nadr: docs/changes/sample-change-one/adr.md\n---\n\n# Sample\n",
    )
    .expect("proposal");
    fs::write(
        change.join("specs/sample-capability-one/spec.md"),
        "## ADDED Requirements\n\n### Requirement: Thing Holds\n\nMUST.\n\n#### Scenario: Version prints\n\n- **WHEN** a\n- **THEN** b\n",
    )
    .expect("spec");
    fixture
        .rune()
        .args(["proof", "scaffold", CHANGE, "--source"])
        .arg(&fixture.root)
        .assert()
        .success();
    let readme = fixture.readme();
    let text = fs::read_to_string(&readme).expect("readme").replace(
        "```console\n```",
        "```console\n$ rune --version\nrune [..] built [..]\n```",
    );
    fs::write(&readme, text).expect("fill");
    fixture.run_jj(&["commit", "-m", "the change"]);

    fixture
        .rune()
        .args(["proof", "run", CHANGE, "--source"])
        .arg(&fixture.root)
        .assert()
        .success();
    fixture.run_jj(&["commit", "-m", "the proof"]);
    Some(fixture)
}

#[test]
fn signed_head_holds_by_change_and_tree() {
    let Some(fixture) = fixture() else { return };
    fixture.check().success();
    let change = fixture.head_change();
    fixture.run_jj(&["sign", "-r", &format!("change_id(\"{change}\")")]);
    fixture
        .check()
        .success()
        .stdout(predicate::str::contains("transcript and head hold"));
}

#[test]
fn reworded_head_holds_by_change_and_tree() {
    let Some(fixture) = fixture() else { return };
    let change = fixture.head_change();
    fixture.run_jj(&[
        "describe",
        "-r",
        &format!("change_id(\"{change}\")"),
        "-m",
        "the change, reworded",
    ]);
    fixture
        .check()
        .success()
        .stdout(predicate::str::contains("transcript and head hold"));
}

#[test]
fn content_edit_of_the_head_fails_the_check() {
    let Some(fixture) = fixture() else { return };
    let change = fixture.head_change();
    fs::write(
        fixture.root.join("extra.txt"),
        "edited after the recording\n",
    )
    .expect("edit");
    fixture.run_jj(&["squash", "--into", &format!("change_id(\"{change}\")")]);
    fixture
        .check()
        .code(1)
        .stdout(predicate::str::contains("was rewritten with other content"));
}

#[test]
fn proof_without_change_and_tree_falls_back_to_the_old_commit() {
    let Some(fixture) = fixture() else { return };
    let change = fixture.head_change();
    let readme = fixture.readme();
    let stripped: String = fs::read_to_string(&readme)
        .expect("readme")
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("head_change:") && !line.starts_with("head_tree:"))
        .collect();
    fs::write(&readme, stripped).expect("strip");
    fixture.run_jj(&["sign", "-r", &format!("change_id(\"{change}\")")]);
    fixture
        .check()
        .success()
        .stdout(predicate::str::contains("transcript and head hold"));

    // A head that never existed has nothing to fall back to. Forty zeros
    // would name jj's root commit, an ancestor of everything.
    let text = fs::read_to_string(&readme).expect("readme");
    let head = rune::proof::parse(Path::new("README.md"), &text)
        .expect("frontmatter")
        .head
        .expect("head");
    fs::write(&readme, text.replace(&head, &"deadbeef".repeat(5))).expect("unknown head");
    fixture
        .check()
        .code(1)
        .stdout(predicate::str::contains("no longer resolves"));
}
