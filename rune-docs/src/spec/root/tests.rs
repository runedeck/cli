use super::*;
use std::fs;
use tempfile::TempDir;

mod scenarios {
    include!("../root_scenarios.rs");
}
use scenarios::{RootExpectation, root_scenarios};

#[test]
fn shared_scenarios_resolve_identically() {
    for scenario in root_scenarios() {
        let root = TempDir::new().unwrap();
        for directory in scenario.directories {
            fs::create_dir_all(root.path().join(directory)).unwrap();
        }
        for (path, content) in scenario.files {
            let destination = root.path().join(path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(destination, content).unwrap();
        }

        let resolved = resolve_with_config(root.path(), scenario.configured_root);

        match &scenario.expectation {
            RootExpectation::Root(expected) => {
                let resolved = resolved.unwrap_or_else(|error| {
                    panic!("{}: expected {expected}, got error {error}", scenario.name)
                });
                assert_eq!(
                    resolved.relative(),
                    Path::new(expected),
                    "{}",
                    scenario.name
                );
            }
            RootExpectation::ErrorContains(fragment) => {
                let error = resolved.err().unwrap_or_else(|| {
                    panic!("{}: expected an error containing {fragment}", scenario.name)
                });
                assert!(
                    error.message().contains(fragment),
                    "{}: {} does not contain {fragment}",
                    scenario.name,
                    error.message()
                );
            }
        }
    }
}

#[test]
fn layouts_follow_the_resolved_relative_root() {
    let native = TempDir::new().unwrap();
    fs::create_dir_all(native.path().join("docs/specs")).unwrap();
    assert_eq!(
        resolve_with_config(native.path(), None).unwrap().layout(),
        SpecLayout::Native
    );

    let openspec = TempDir::new().unwrap();
    fs::create_dir_all(openspec.path().join("openspec/changes")).unwrap();
    assert_eq!(
        resolve_with_config(openspec.path(), None).unwrap().layout(),
        SpecLayout::OpenSpec
    );

    let custom = TempDir::new().unwrap();
    assert_eq!(
        resolve_with_config(custom.path(), Some("artifacts/specifications"))
            .unwrap()
            .layout(),
        SpecLayout::Custom
    );
}

#[test]
fn configured_custom_root_resolves_missing_destinations() {
    let root = TempDir::new().unwrap();

    let resolved = resolve_with_config(root.path(), Some("artifacts/specifications")).unwrap();

    assert!(
        resolved
            .changes()
            .ends_with("artifacts/specifications/changes")
    );
    assert!(
        resolved
            .specifications()
            .ends_with("artifacts/specifications/specs")
    );
}

#[cfg(unix)]
#[test]
fn configured_root_rejects_a_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    symlink(outside.path(), root.path().join("linked")).unwrap();

    let error = resolve_with_config(root.path(), Some("linked/specifications")).unwrap_err();

    assert!(error.message().contains("escapes repository"));
}

#[test]
fn live_trees_count_journal_only_roots() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("docs/.rune-transaction")).unwrap();
    fs::write(
        root.path().join("docs/.rune-transaction/journal.yaml"),
        "version: 1\n",
    )
    .unwrap();
    fs::create_dir_all(root.path().join("openspec/changes")).unwrap();

    let trees = live_trees(root.path());

    assert_eq!(
        trees,
        LiveTrees {
            native: true,
            openspec: true
        }
    );
}
