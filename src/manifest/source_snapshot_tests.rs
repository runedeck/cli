use super::*;

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("skills/Alpha/claude")).unwrap();
    fs::write(
        root.path().join("skills/Alpha/SKILL.md"),
        "---\nname: Alpha\ndescription: fixture\n---\nRead [guide](Guide.md).\n",
    )
    .unwrap();
    fs::write(root.path().join("skills/Alpha/Guide.md"), "Guide\n").unwrap();
    root
}

fn inputs(root: &Path) -> Vec<SourceInput> {
    vec![
        SourceInput::required("skills", root.join("skills")),
        SourceInput::optional("configuration", root.join("config.yaml")),
    ]
}

#[test]
fn source_snapshot_is_deterministic_and_binds_optional_absence() {
    let root = fixture();
    let original = inspect(&inputs(root.path())).unwrap();
    let mut reversed = inputs(root.path());
    reversed.reverse();
    assert_eq!(original, inspect(&reversed).unwrap());
    fs::write(root.path().join("config.yaml"), "providers: {}\n").unwrap();
    assert_ne!(original, inspect(&inputs(root.path())).unwrap());
    fs::remove_file(root.path().join("config.yaml")).unwrap();
    assert_eq!(original, inspect(&inputs(root.path())).unwrap());
}

#[test]
fn source_snapshot_detects_new_removed_and_changed_authored_content() {
    let root = fixture();
    let original = inspect(&inputs(root.path())).unwrap();
    for relative in [
        "skills/Alpha/New.bin",
        "skills/Alpha/claude/SKILL.md",
        "skills/Beta/SKILL.md",
    ] {
        let path = root.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, [0, 127, 255]).unwrap();
        assert_ne!(
            original,
            inspect(&inputs(root.path())).unwrap(),
            "{relative}"
        );
        fs::remove_file(&path).unwrap();
        if relative.starts_with("skills/Beta/") {
            fs::remove_dir(path.parent().unwrap()).unwrap();
        }
        assert_eq!(original, inspect(&inputs(root.path())).unwrap());
    }
    fs::write(root.path().join("skills/Alpha/Guide.md"), "changed\n").unwrap();
    assert_ne!(original, inspect(&inputs(root.path())).unwrap());
    fs::remove_file(root.path().join("skills/Alpha/Guide.md")).unwrap();
    assert_ne!(original, inspect(&inputs(root.path())).unwrap());
}

#[test]
fn source_snapshot_ignores_reserved_metadata_but_keeps_authored_build_named_companions() {
    let root = fixture();
    let original = inspect(&inputs(root.path())).unwrap();
    for name in [
        ".git",
        ".jj",
        ".provenance",
        ".workspaces",
        ".worktrees",
        "__pycache__",
    ] {
        let path = root.path().join("skills/Alpha").join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("generated"), "generated\n").unwrap();
    }
    assert_eq!(original, inspect(&inputs(root.path())).unwrap());
    fs::create_dir(root.path().join("skills/Alpha/build")).unwrap();
    fs::write(
        root.path().join("skills/Alpha/build/reference.md"),
        "Authored companion\n",
    )
    .unwrap();
    assert_ne!(original, inspect(&inputs(root.path())).unwrap());
}

#[test]
fn source_snapshot_fails_on_missing_required_roots_and_duplicate_labels() {
    let root = fixture();
    assert!(
        inspect(&[SourceInput::required(
            "missing",
            root.path().join("missing")
        )])
        .is_err()
    );
    let input = SourceInput::required("duplicate", root.path().join("skills"));
    assert!(inspect(&[input.clone(), input]).is_err());
    assert!(inspect(&[]).is_err());
}

#[test]
fn source_snapshot_binds_inline_effective_configuration() {
    let first = inspect(&[SourceInput::value("effective", b"configuration one")]).unwrap();
    let second = inspect(&[SourceInput::value("effective", b"configuration two")]).unwrap();
    assert_ne!(first, second);
    assert_eq!(first.roots, vec!["effective"]);
}

#[cfg(unix)]
#[test]
fn source_snapshot_binds_modes_and_contained_symlink_targets() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = fixture();
    let original = inspect(&inputs(root.path())).unwrap();
    let guide = root.path().join("skills/Alpha/Guide.md");
    let old_mode = fs::metadata(&guide).unwrap().permissions();
    fs::set_permissions(&guide, fs::Permissions::from_mode(0o755)).unwrap();
    assert_ne!(original, inspect(&inputs(root.path())).unwrap());
    fs::set_permissions(&guide, old_mode).unwrap();
    assert_eq!(original, inspect(&inputs(root.path())).unwrap());
    let alias = root.path().join("skills/Alpha/claude/Guide.md");
    symlink("../Guide.md", &alias).unwrap();
    let linked = inspect(&inputs(root.path())).unwrap();
    fs::remove_file(&alias).unwrap();
    symlink("../SKILL.md", &alias).unwrap();
    assert_ne!(linked, inspect(&inputs(root.path())).unwrap());
    fs::remove_file(&alias).unwrap();
    symlink("../../../outside", &alias).unwrap();
    assert!(inspect(&inputs(root.path())).is_err());
}

#[test]
fn selected_bundle_map_requires_complete_referenced_content() {
    let root = fixture();
    let selected = selected_skill_bundles(root.path()).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected["skills/Alpha"].len(), 64);
    fs::remove_file(root.path().join("skills/Alpha/Guide.md")).unwrap();
    assert!(selected_skill_bundles(root.path()).is_err());
}

#[test]
fn source_record_validates_versions_digests_and_selected_paths() {
    let root = fixture();
    let record = SourceSnapshotRecord::new(
        inspect(&inputs(root.path())).unwrap(),
        selected_skill_bundles(root.path()).unwrap(),
    );
    record.validate().unwrap();
    let encoded = serde_json::to_string(&record).unwrap();
    assert!(!encoded.contains(&root.path().display().to_string()));
    assert_eq!(
        record,
        serde_json::from_str::<SourceSnapshotRecord>(&encoded).unwrap()
    );
    for path in [
        "../skills/Alpha",
        "/skills/Alpha",
        "skills/../Alpha",
        "skills",
    ] {
        let mut invalid = record.clone();
        invalid.selected_skills = BTreeMap::from([(path.into(), "a".repeat(64))]);
        assert!(invalid.validate().is_err(), "{path}");
    }
    let mut invalid = record;
    invalid.source.digest = "not a digest".into();
    assert!(invalid.validate().is_err());
}
