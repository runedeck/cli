use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

/// A change with two scenarios, no proof yet.
fn change_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let change = root.join("docs/changes/sample-change-one");
    fs::create_dir_all(change.join("specs/sample-capability-one")).expect("change dir");
    fs::write(
        change.join("proposal.md"),
        "---\nadr: docs/changes/sample-change-one/adr.md\n---\n\n# Sample\n",
    )
    .expect("proposal");
    fs::write(
        change.join("specs/sample-capability-one/spec.md"),
        "## ADDED Requirements\n\n### Requirement: Thing Holds\n\nMUST.\n\n#### Scenario: Version prints\n\n- **WHEN** a\n- **THEN** b\n\n#### Scenario: Missing tag fails\n\n- **WHEN** a\n- **THEN** b\n",
    )
    .expect("spec");
    dir
}

fn rune() -> Command {
    Command::cargo_bin("rune").expect("binary")
}

#[test]
fn scaffold_then_run_then_check() {
    let dir = change_fixture();
    let root = dir.path();

    rune()
        .args(["proof", "scaffold", "sample-change-one", "--source"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("2 unproven scenes"));
    let readme = root.join("docs/proofs/sample-change-one/README.md");
    let text = fs::read_to_string(&readme).expect("readme");
    assert!(
        text.contains("## sample-capability-one#thing-holds/version-prints\n\n```console\n```")
    );
    assert!(text.contains("kind: unproven"));

    // A second scaffold never overwrites.
    rune()
        .args(["proof", "scaffold", "sample-change-one", "--source"])
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("never overwritten"));

    // Fill one scene with a passing fence and leave the other empty.
    let filled = text.replace(
        "## sample-capability-one#thing-holds/version-prints\n\n```console\n```",
        "## sample-capability-one#thing-holds/version-prints\n\n```console\n$ rune --version\nrune [..] built [..]\n```",
    );
    fs::write(&readme, filled).expect("fill");

    rune()
        .args(["proof", "run", "sample-change-one", "--source"])
        .arg(root)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("version-prints ... ok (check)"))
        .stdout(predicate::str::contains(
            "missing-tag-fails ... unproven: the fence holds no command",
        ))
        .stdout(predicate::str::contains("2 scenes, 1 proven, 1 unproven"));

    let after = fs::read_to_string(&readme).expect("readme after run");
    let fm = rune::proof::parse(Path::new("README.md"), &after).expect("valid frontmatter");
    assert_eq!(fm.scenes[0].kind, rune::proof::Kind::Check);
    assert_eq!(fm.scenes[1].kind, rune::proof::Kind::Unproven);
    let transcript = fs::read_to_string(root.join("docs/proofs/sample-change-one/proof.txt"))
        .expect("transcript");
    assert!(transcript.starts_with("## sample-capability-one#thing-holds/version-prints\nkind: check\n$ rune --version\n  rune "));
    assert!(
        root.join("docs/proofs/sample-change-one/proof.cast")
            .is_file()
    );
    assert_eq!(
        rune::proof::transcript_digest(&root.join("docs/proofs/sample-change-one")).as_deref(),
        Some(fm.transcript.as_str())
    );

    // The graph proves the recorded scene and not the empty one.
    rune()
        .args(["graph", "export", "--source"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "rune:proves <https://runedeck.ai/id/sample-capability-one#thing-holds/version-prints> ;",
        ))
        .stdout(predicate::str::contains(
            "rune:proves <https://runedeck.ai/id/sample-capability-one#thing-holds/missing-tag-fails>",
        ).not());

    // The check holds, then refuses a drifted transcript.
    rune()
        .args(["proof", "run", "sample-change-one", "--check", "--source"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("transcript and head hold"));
    fs::write(
        root.join("docs/proofs/sample-change-one/proof.txt"),
        "drifted\n",
    )
    .expect("drift");
    rune()
        .args(["proof", "run", "sample-change-one", "--check", "--source"])
        .arg(root)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("transcript drifted"))
        .stdout(predicate::str::contains(
            "version-prints is check but has no section",
        ));
}

#[test]
fn a_wrong_expectation_leaves_the_scene_unproven_with_a_diff() {
    let dir = change_fixture();
    let root = dir.path();
    rune()
        .args(["proof", "scaffold", "sample-change-one", "--source"])
        .arg(root)
        .assert()
        .success();
    let readme = root.join("docs/proofs/sample-change-one/README.md");
    let text = fs::read_to_string(&readme).expect("readme").replace(
        "## sample-capability-one#thing-holds/version-prints\n\n```console\n```",
        "## sample-capability-one#thing-holds/version-prints\n\n```console\n$ rune --version\nrune 0.5.0 ([..])\n```",
    ).replace(
        "## sample-capability-one#thing-holds/missing-tag-fails\n\n```console\n```",
        "## sample-capability-one#thing-holds/missing-tag-fails\n\n```console\n$ nonesuch --flag\n```",
    );
    fs::write(&readme, text).expect("fill");
    rune()
        .args(["proof", "run", "sample-change-one", "--source"])
        .arg(root)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("- rune 0.5.0 ([..])"))
        .stdout(predicate::str::contains("+ rune "))
        .stdout(predicate::str::contains(
            "missing-tag-fails ... unproven: command not found: nonesuch",
        ));
    let after = fs::read_to_string(&readme).expect("readme");
    assert_eq!(after.matches("kind: unproven").count(), 2);
}

#[test]
fn scaffold_refuses_a_change_without_scenarios() {
    let dir = tempfile::tempdir().expect("tempdir");
    let change = dir.path().join("docs/changes/bare-change");
    fs::create_dir_all(&change).expect("change dir");
    fs::write(change.join("proposal.md"), "---\nadr: x\n---\n").expect("proposal");
    rune()
        .args(["proof", "scaffold", "bare-change", "--source"])
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("declares no scenario"));
}
