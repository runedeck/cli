use super::*;
use rune::manifest::source_snapshot::inspect;

fn module(root: &Path) {
    fs::create_dir_all(root.join("skills/Alpha")).unwrap();
    fs::write(root.join("module.yaml"), "name: fixture\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("skills/Alpha/SKILL.md"),
        "---\nname: Alpha\ndescription: fixture\n---\n",
    )
    .unwrap();
}

#[test]
fn source_inputs_ignore_generated_roots_and_detect_new_companions() {
    let root = tempfile::tempdir().unwrap();
    module(root.path());
    let original = inspect(&source_inputs(root.path()).unwrap()).unwrap();
    for generated in [
        "build/codex/skills/generated",
        ".agents/skills/generated",
        ".codex/skills/generated",
        ".workspaces/example/file",
        "docs/generated.md",
        ".rune.lock",
    ] {
        let path = root.path().join(generated);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "generated\n").unwrap();
    }
    assert_eq!(
        original,
        inspect(&source_inputs(root.path()).unwrap()).unwrap()
    );
    fs::write(root.path().join("skills/Alpha/New.md"), "new companion\n").unwrap();
    assert_ne!(
        original,
        inspect(&source_inputs(root.path()).unwrap()).unwrap()
    );
}

#[test]
fn source_inputs_bind_builder_effective_defaults_and_model_override() {
    let root = tempfile::tempdir().unwrap();
    module(root.path());
    let default = source_inputs(root.path()).unwrap();
    assert!(
        default
            .iter()
            .any(|input| input.label == "builder:executable"
                && input.required
                && input.path.is_file())
    );
    assert!(
        default
            .iter()
            .any(|input| input.label == "builder:effective_configuration" && input.bytes.is_some())
    );
    let explicit = source_inputs_with_model(root.path(), Some("fixture-model")).unwrap();
    assert_ne!(inspect(&default).unwrap(), inspect(&explicit).unwrap());
}

#[test]
fn source_inputs_bind_selection_and_all_configured_local_sources() {
    let root = tempfile::tempdir().unwrap();
    let consumer = root.path().join("consumer");
    let source = root.path().join("source");
    fs::create_dir(&consumer).unwrap();
    module(&source);
    let manifest = "version: 1\nsources:\n  fixture:\n    local: ../source\nrunes:\n  fixture:\n    skills: [Alpha]\n";
    fs::write(consumer.join(".rune"), manifest).unwrap();
    let original = inspect(&source_inputs(&consumer).unwrap()).unwrap();
    fs::create_dir(source.join("skills/Unselected")).unwrap();
    fs::write(
        source.join("skills/Unselected/SKILL.md"),
        "unselected new skill\n",
    )
    .unwrap();
    assert_ne!(
        original,
        inspect(&source_inputs(&consumer).unwrap()).unwrap()
    );
    let before_selection = inspect(&source_inputs(&consumer).unwrap()).unwrap();
    fs::write(
        consumer.join(".rune"),
        manifest.replace("[Alpha]", "[Alpha, Unselected]"),
    )
    .unwrap();
    assert_ne!(
        before_selection,
        inspect(&source_inputs(&consumer).unwrap()).unwrap()
    );
    fs::remove_dir_all(&source).unwrap();
    assert!(source_inputs(&consumer).is_err());
}

#[test]
fn source_inputs_require_existing_git_cache_without_materializing_it() {
    let root = tempfile::tempdir().unwrap();
    let consumer = root.path().join("consumer");
    let cached = root.path().join("cached");
    fs::create_dir(&consumer).unwrap();
    fs::write(consumer.join(".rune"), format!("version: 1\nsources:\n  fixture:\n    git: https://example.com/owner/deck\n    ref: {}\nrunes: {{}}\n", "a".repeat(40))).unwrap();
    let error = source_inputs_with_cache(&consumer, None, &|_, _, _| None).unwrap_err();
    assert!(error.to_string().contains("no available pinned cache"));
    assert!(!cached.exists());
    module(&cached);
    let observed = source_inputs_with_cache(&consumer, None, &|_, _, label| {
        assert_eq!(label, "fixture");
        Some(cached.clone())
    })
    .unwrap();
    assert!(
        observed
            .iter()
            .any(|input| input.path == cached.canonicalize().unwrap().join("skills"))
    );
}

#[cfg(unix)]
#[test]
fn source_inputs_reject_subpath_symlink_escape() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let consumer = root.path().join("consumer");
    let source = root.path().join("source");
    let outside = root.path().join("outside");
    fs::create_dir(&consumer).unwrap();
    fs::create_dir(&source).unwrap();
    module(&outside);
    symlink(&outside, source.join("inner")).unwrap();
    fs::write(
        consumer.join(".rune"),
        "version: 1\nsources:\n  fixture:\n    local: ../source\n    path: inner\nrunes: {}\n",
    )
    .unwrap();
    let error = source_inputs(&consumer).unwrap_err();
    assert!(error.to_string().contains("escapes its configured root"));
}
