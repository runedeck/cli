use super::*;
use tempfile::TempDir;

fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("SKILL.md"),
        "---\nname: Alpha\ndescription: test\n---\n# Alpha\n",
    )
    .unwrap();
    root
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn valid_digest(root: &Path) -> String {
    let report = inspect_bundle(root);
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    report.digest.unwrap()
}

fn has_problem(report: &BundleInspection, kind: BundleProblemKind, path: &str) {
    assert!(
        report
            .problems
            .iter()
            .any(|problem| problem.kind == kind && problem.path == path),
        "{:?}",
        report.problems
    );
}

#[test]
fn bundle_identity_is_versioned_and_stable() {
    let root = fixture();
    write(root.path(), "refs/Reference.md", b"Reference.\n");
    let first = inspect_bundle(root.path());
    assert_eq!(first.version, BUNDLE_DIGEST_VERSION);
    assert!(first.problems.is_empty());
    assert_eq!(first, inspect_bundle(root.path()));
    assert_eq!(
        first
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["SKILL.md", "refs", "refs/Reference.md"]
    );
}

#[test]
fn companion_bytes_paths_and_binary_changes_invalidate_identity() {
    let root = fixture();
    write(root.path(), "assets/image.bin", &[0, 255]);
    let original = valid_digest(root.path());
    write(root.path(), "assets/image.bin", &[0, 254]);
    let changed = valid_digest(root.path());
    assert_ne!(original, changed);
    fs::rename(
        root.path().join("assets/image.bin"),
        root.path().join("assets/renamed.bin"),
    )
    .unwrap();
    assert_ne!(changed, valid_digest(root.path()));
}

#[test]
fn provenance_content_and_timestamps_do_not_change_identity() {
    let root = fixture();
    let original = valid_digest(root.path());
    write(
        root.path(),
        ".provenance/SKILL.md.yaml",
        b"startedOn: yesterday\n",
    );
    assert_eq!(original, valid_digest(root.path()));
    write(
        root.path(),
        ".provenance/SKILL.md.yaml",
        b"startedOn: today\n",
    );
    assert_eq!(original, valid_digest(root.path()));
    let entrypoint = root.path().join("SKILL.md");
    let bytes = fs::read(&entrypoint).unwrap();
    fs::write(entrypoint, bytes).unwrap();
    assert_eq!(original, valid_digest(root.path()));
}

#[test]
fn build_sidecars_are_excluded_but_authored_yaml_survives() {
    let root = fixture();
    write(
        root.path(),
        "agents/openai.yaml",
        b"interface:\n  display_name: Alpha\n",
    );
    let original = valid_digest(root.path());
    let statement = super::super::generate_statement(
        "codex/skills/Alpha/agents/openai.yaml",
        "abc",
        &[],
        "builder",
        "https://github.com/runedeck/cli/assemble/v1",
        "1",
        "source",
    );
    write(
        root.path(),
        "agents/.provenance/openai.yaml.yaml",
        statement.as_bytes(),
    );
    assert_eq!(original, valid_digest(root.path()));
    assert!(is_build_sidecar(
        &root.path().join("agents/.provenance/openai.yaml.yaml")
    ));
    assert!(!is_build_sidecar(&root.path().join("agents/openai.yaml")));
    write(
        root.path(),
        "agents/openai.yaml.yaml",
        b"authored: content\n",
    );
    assert_ne!(original, valid_digest(root.path()));
}

#[test]
fn provenance_shaped_authored_yaml_always_remains_content() {
    let root = fixture();
    write(root.path(), "data", b"data");
    let authored = super::super::generate_statement(
        "codex/skills/Alpha/data",
        "abc",
        &[],
        "builder",
        "https://github.com/runedeck/cli/assemble/v1",
        "1",
        "source",
    );
    write(root.path(), "data.yaml", authored.as_bytes());
    let generated = super::super::generate_statement(
        "codex/skills/Alpha/data.yaml",
        "def",
        &[],
        "builder",
        "https://github.com/runedeck/cli/assemble/v1",
        "1",
        "source",
    );
    write(
        root.path(),
        ".provenance/data.yaml.yaml",
        generated.as_bytes(),
    );
    assert!(!is_build_sidecar(&root.path().join("data.yaml")));
    assert!(is_build_sidecar(
        &root.path().join(".provenance/data.yaml.yaml")
    ));
    let report = inspect_bundle(root.path());
    assert!(report.entries.iter().any(|entry| entry.path == "data.yaml"));
    assert!(
        !report
            .entries
            .iter()
            .any(|entry| entry.path == ".provenance/data.yaml.yaml")
    );
    fs::remove_file(root.path().join(".provenance/data.yaml.yaml")).unwrap();
    assert_eq!(report.digest, inspect_bundle(root.path()).digest);
    assert!(!is_build_sidecar(&root.path().join("data.yaml")));
}

#[test]
fn markdown_links_images_and_reference_definitions_are_checked() {
    let root = fixture();
    write(root.path(), "refs/Space Name.md", b"# Reference\n");
    write(root.path(), "assets/image.bin", &[255]);
    write(root.path(), "SKILL.md", b"# Alpha\n[reference][r]\n![image](assets/image.bin)\n[web](https://example.test/a)\n[anchor](#alpha)\n\n[r]: refs/Space%20Name.md#reference\n");
    valid_digest(root.path());
    fs::remove_file(root.path().join("refs/Space Name.md")).unwrap();
    has_problem(
        &inspect_bundle(root.path()),
        BundleProblemKind::BrokenReference,
        "SKILL.md",
    );
}

#[test]
fn escaping_reference_is_rejected_even_when_destination_exists() {
    let parent = TempDir::new().unwrap();
    let root = parent.path().join("skill");
    fs::create_dir(&root).unwrap();
    fs::write(parent.path().join("outside.md"), "outside").unwrap();
    write(&root, "SKILL.md", b"# Alpha\n[escape](../outside.md)\n");
    has_problem(
        &inspect_bundle(&root),
        BundleProblemKind::EscapingReference,
        "SKILL.md",
    );
}

#[test]
fn inaccessible_root_never_produces_a_complete_digest() {
    let root = TempDir::new().unwrap();
    let report = inspect_bundle(&root.path().join("absent"));
    assert!(report.digest.is_none());
    has_problem(&report, BundleProblemKind::Unreadable, ".");
}

#[test]
fn case_collision_detection_does_not_depend_on_host_filesystem() {
    let root = fixture();
    let mut inventory = Inventory::default();
    inventory.walk(root.path(), Path::new(""));
    let mut duplicate = inventory.entries["SKILL.md"].clone();
    duplicate.path = "skill.md".into();
    inventory.entries.insert(duplicate.path.clone(), duplicate);
    inventory.check(root.path());
    assert!(
        inventory
            .problems
            .iter()
            .any(|problem| problem.kind == BundleProblemKind::CaseCollision
                && problem.path == "skill.md")
    );
}

#[cfg(unix)]
#[test]
fn executable_bits_are_part_of_the_bundle_identity() {
    use std::os::unix::fs::PermissionsExt;
    let root = fixture();
    let script = root.path().join("run.sh");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o644)).unwrap();
    let plain = valid_digest(root.path());
    fs::set_permissions(&script, fs::Permissions::from_mode(0o744)).unwrap();
    let owner_executable = valid_digest(root.path());
    assert_ne!(plain, owner_executable);
    fs::set_permissions(&script, fs::Permissions::from_mode(0o654)).unwrap();
    assert_ne!(owner_executable, valid_digest(root.path()));
}

#[cfg(unix)]
#[test]
fn contained_symlink_target_and_entry_type_change_identity() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    write(root.path(), "refs/a.md", b"same bytes");
    write(root.path(), "refs/b.md", b"same bytes");
    let link = root.path().join("Reference.md");
    symlink("refs/a.md", &link).unwrap();
    let first = valid_digest(root.path());
    assert_eq!(
        contained_symlink_target(root.path(), &link).unwrap(),
        Path::new("refs/a.md")
    );
    fs::remove_file(&link).unwrap();
    symlink("refs/b.md", &link).unwrap();
    let changed = valid_digest(root.path());
    assert_ne!(first, changed);
    fs::remove_file(&link).unwrap();
    fs::write(&link, "same bytes").unwrap();
    assert_ne!(changed, valid_digest(root.path()));
}

#[cfg(unix)]
#[test]
fn contained_directory_aliases_preserve_local_references() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    write(root.path(), "refs/a.md", b"content");
    symlink("refs", root.path().join("alias")).unwrap();
    write(
        root.path(),
        "SKILL.md",
        b"# Alpha\n[reference](alias/a.md)\n",
    );
    let report = inspect_bundle(root.path());
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert!(
        !report
            .entries
            .iter()
            .any(|entry| entry.path == "alias/a.md")
    );
}

#[cfg(unix)]
#[test]
fn escaping_dangling_and_cyclic_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    symlink("../outside", root.path().join("escape")).unwrap();
    symlink("missing", root.path().join("dangling")).unwrap();
    symlink("loop-b", root.path().join("loop-a")).unwrap();
    symlink("loop-a", root.path().join("loop-b")).unwrap();
    symlink(".", root.path().join("tree-cycle")).unwrap();
    let report = inspect_bundle(root.path());
    has_problem(&report, BundleProblemKind::EscapingSymlink, "escape");
    has_problem(&report, BundleProblemKind::BrokenSymlink, "dangling");
    has_problem(&report, BundleProblemKind::CyclicSymlink, "loop-a");
    has_problem(&report, BundleProblemKind::CyclicSymlink, "tree-cycle");
}

#[cfg(unix)]
#[test]
fn mutual_directory_symlinks_are_a_traversal_cycle() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    fs::create_dir(root.path().join("a")).unwrap();
    fs::create_dir(root.path().join("b")).unwrap();
    symlink("../b", root.path().join("a/to-b")).unwrap();
    symlink("../a", root.path().join("b/to-a")).unwrap();
    has_problem(
        &inspect_bundle(root.path()),
        BundleProblemKind::CyclicSymlink,
        "b/to-a",
    );
}
