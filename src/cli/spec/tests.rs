use super::*;
use crate::cli::spec_root::changes_root;
use std::fs;
use tempfile::TempDir;

#[test]
fn root_choice_notes_without_writing_when_non_interactive() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("openspec/changes")).unwrap();

    offer_root_choice(&root.path().to_string_lossy(), false).unwrap();

    assert!(!root.path().join("config.yaml").exists());
    assert!(
        changes_root(root.path())
            .unwrap()
            .ends_with("openspec/changes")
    );
}

#[test]
fn root_choice_leaves_an_unreadable_config_untouched() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("openspec/changes")).unwrap();
    let malformed = "spec: [unclosed\n";
    fs::write(root.path().join("config.yaml"), malformed).unwrap();

    offer_root_choice(&root.path().to_string_lossy(), false).unwrap();

    assert_eq!(
        fs::read_to_string(root.path().join("config.yaml")).unwrap(),
        malformed
    );
}

#[test]
fn root_choice_skips_configured_and_leaves_ambiguous_trees_unselected() {
    let configured = TempDir::new().unwrap();
    fs::create_dir_all(configured.path().join("openspec/changes")).unwrap();
    let config = "spec:\n    root: openspec\n";
    fs::write(configured.path().join("config.yaml"), config).unwrap();
    offer_root_choice(&configured.path().to_string_lossy(), false).unwrap();
    assert_eq!(
        fs::read_to_string(configured.path().join("config.yaml")).unwrap(),
        config
    );

    let native = TempDir::new().unwrap();
    fs::create_dir_all(native.path().join("docs/changes")).unwrap();
    fs::create_dir_all(native.path().join("openspec/changes")).unwrap();
    offer_root_choice(&native.path().to_string_lossy(), false).unwrap();
    assert!(!native.path().join("config.yaml").exists());
    let error = changes_root(native.path()).unwrap_err();
    assert!(error.message().contains("both docs/ and openspec/"));
}

#[test]
fn root_choice_migrates_openspec_ownership_under_docs() {
    let root = TempDir::new().unwrap();
    let proposal = root.path().join("openspec/changes/add-widget/proposal.md");
    let specification = root.path().join("openspec/specs/widget/spec.md");
    fs::create_dir_all(proposal.parent().unwrap()).unwrap();
    fs::create_dir_all(specification.parent().unwrap()).unwrap();
    fs::write(&proposal, "proposal\n").unwrap();
    fs::write(&specification, "specification\n").unwrap();
    install_hooks();

    apply_root_choice(root.path(), &root.path().to_string_lossy(), "2", true).unwrap();

    let merged = crate::cli::config::load_merged_config(root.path()).unwrap();
    assert_eq!(
        rune::yaml::yaml_list(&merged, "spec.root").as_deref(),
        Some("docs")
    );
    assert_eq!(
        fs::read_to_string(root.path().join("docs/changes/add-widget/proposal.md")).unwrap(),
        "proposal\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("docs/specs/widget/spec.md")).unwrap(),
        "specification\n"
    );
    assert!(!root.path().join("openspec").exists());
}

#[test]
fn validate_spec_tree_reports_mdschema_violations_through_the_bridge() {
    let root = TempDir::new().unwrap();
    let malformed = root.path().join("docs/specs/broken/spec.md");
    fs::create_dir_all(malformed.parent().unwrap()).unwrap();
    fs::write(
        &malformed,
        "# Broken

## Requirements
",
    )
    .unwrap();

    let violations = validate_spec_tree(root.path()).unwrap();

    assert!(
        violations.iter().any(|violation| {
            violation
                .message
                .contains("missing required section matching '^# .+ Specification$'")
        }),
        "unexpected violations: {violations:?}"
    );
}
