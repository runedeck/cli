use super::*;
use std::path::Path;

#[test]
fn resolve_sidecar_path_appends_provenance_directory() {
    let result = resolve_sidecar_path(Path::new("/home/.claude/rules/UseRTK.md"));
    let result_string = result.to_string_lossy();
    assert!(result_string.contains(rune::manifest::PROVENANCE_DIRECTORY));
}

#[test]
fn resolve_sidecar_path_encodes_full_filename() {
    let result = resolve_sidecar_path(Path::new("/home/.claude/agents/Dev.md"));
    let filename = result.file_name().unwrap().to_string_lossy();
    assert_eq!(filename, "Dev.md.yaml");
}

#[test]
fn resolve_sidecar_path_falls_back_to_legacy_stem_sidecar() {
    let root = tempfile::tempdir().expect("tempdir");
    let rules = root.path().join("rules");
    std::fs::create_dir_all(rules.join(rune::manifest::PROVENANCE_DIRECTORY)).expect("mkdir");
    let file = rules.join("UseRTK.md");
    std::fs::write(&file, "rule body\n").expect("write rule");
    let legacy = rules
        .join(rune::manifest::PROVENANCE_DIRECTORY)
        .join("UseRTK.yaml");
    std::fs::write(&legacy, "provenance: {}\n").expect("write legacy sidecar");
    assert_eq!(resolve_sidecar_path(&file), legacy);
}

#[test]
fn resolve_sidecar_path_preserves_parent_directory() {
    let result = resolve_sidecar_path(Path::new("/project/.claude/rules/UseRTK.md"));
    assert!(result.starts_with("/project/.claude/rules"));
}
