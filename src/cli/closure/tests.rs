use super::*;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let change = root.join("docs/changes/sample-change");
    fs::create_dir_all(change.join("specs/second-cap-here")).unwrap();
    fs::create_dir_all(change.join("specs/first-cap-here")).unwrap();
    fs::create_dir_all(change.join("specs/unlisted-cap-here")).unwrap();
    fs::write(
        change.join("proposal.md"),
        "---\nadr: docs/changes/sample-change/adr.md\ndecisions: [\"CORE-0002 Second\", \"CORE-0001 First\", \"CORE-0009 Missing\", \"../../outside\"]\n---\n\n# Sample\n\n## What Changes\n\n### New Capabilities\n\n- `second-cap-here`: listed first on purpose.\n- `first-cap-here`: listed second.\n- `second-cap-here`: listed twice.\n\n## Impact\n",
    )
    .unwrap();
    for (cap, scn) in [
        ("first-cap-here", "One Happens"),
        ("second-cap-here", "Two Happens"),
        ("unlisted-cap-here", "Three Happens"),
    ] {
        fs::write(
            change.join("specs").join(cap).join("spec.md"),
            format!("## ADDED Requirements\n\n### Requirement: Thing Holds\n\nMUST.\n\n#### Scenario: {scn}\n\n- **WHEN** a\n- **THEN** b\n"),
        )
        .unwrap();
    }
    fs::write(change.join("adr.md"), "---\ntitle: Draft\n---\n").unwrap();
    fs::write(change.join("tasks.md"), "# Tasks\n").unwrap();
    fs::write(change.join("design.md"), "# Design\n").unwrap();
    let decisions = root.join("docs/decisions");
    fs::create_dir_all(&decisions).unwrap();
    fs::write(
        decisions.join("CORE-0001 First.md"),
        "---\ntitle: First\n---\n",
    )
    .unwrap();
    fs::write(
        decisions.join("CORE-0002 Second.md"),
        "---\ntitle: Second\n---\n",
    )
    .unwrap();
    let ideas = root.join("docs/ideas");
    fs::create_dir_all(&ideas).unwrap();
    fs::write(ideas.join("First Cap Here.svg"), "<svg/>").unwrap();
    fs::write(ideas.join("Unrelated.svg"), "<svg/>").unwrap();
    for (name, change_id) in [("mine", "sample-change"), ("theirs", "other-change")] {
        let proof = root.join("docs/proofs").join(name);
        fs::create_dir_all(&proof).unwrap();
        fs::write(
            proof.join("README.md"),
            format!("---\ntype: proof\nchange: {change_id}\nrecorded: 2026-09-24\ntranscript: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\nscenes: []\n---\n"),
        )
        .unwrap();
    }
    dir
}

#[test]
fn a_change_resolves_in_declared_order() {
    let dir = fixture();
    let root = dir.path();
    let closure = resolve_change(root, "sample-change").expect("closure");
    assert_eq!(
        closure.capabilities,
        ["second-cap-here", "first-cap-here", "unlisted-cap-here"]
    );
    assert_eq!(
        closure.scenarios,
        [
            "second-cap-here#thing-holds/two-happens",
            "first-cap-here#thing-holds/one-happens",
            "unlisted-cap-here#thing-holds/three-happens",
        ]
    );
    let records: Vec<String> = closure
        .records
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        records,
        ["adr.md", "CORE-0002 Second.md", "CORE-0001 First.md"]
    );
    let ideas: Vec<String> = closure
        .ideas
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(ideas, ["First Cap Here.svg"]);
    let proofs: Vec<String> = closure
        .proofs
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        proofs,
        ["mine"],
        "a proof belongs to the change its frontmatter names"
    );
    let rest: Vec<String> = closure
        .rest
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(rest, ["design.md", "tasks.md"]);
    let order = closure.paths();
    assert!(order.first().unwrap().ends_with("proposal.md"));
    assert!(order.last().unwrap().ends_with("tasks.md"));
}

#[test]
fn a_missing_change_is_an_error_that_names_it() {
    let dir = fixture();
    let error = resolve_change(dir.path(), "nonesuch").unwrap_err();
    assert!(error.to_string().contains("nonesuch"));
}

#[test]
fn declared_capabilities_reads_only_the_section() {
    let text = "## What Changes\n\n### New Capabilities\n\n- `a-b-c`: x\n- plain item\n- `d-e-f`: y\n\n### Modified Capabilities\n\n- `g-h-i`: z\n";
    assert_eq!(declared_capabilities(text), ["a-b-c", "d-e-f"]);
    assert!(declared_capabilities("# No section\n").is_empty());
}

#[test]
fn a_change_id_that_is_a_path_is_refused() {
    let dir = fixture();
    let root = dir.path();
    for id in [
        "../decisions",
        "/etc",
        "docs/changes/sample-change",
        "Sample",
    ] {
        let error = resolve_change(root, id).unwrap_err();
        assert!(error.to_string().contains("not a change id"), "{id}");
    }
}
