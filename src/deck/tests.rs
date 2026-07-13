use super::*;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn deck_yaml(providers: &str) -> String {
    format!("name: fixture-deck\nversion: 0.1.0\ndescription: Fixture deck.\n{providers}")
}

fn module_yaml(name: &str, providers: &str) -> String {
    format!("name: {name}\nversion: 0.1.0\ndescription: Fixture domain.\nevents: []\n{providers}")
}

#[test]
fn discovers_deck_modules() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("runes/science/module.yaml"),
        &module_yaml("science", ""),
    );
    write(
        &root.path().join("runes/writing/module.yaml"),
        &module_yaml("writing", ""),
    );

    let deck = load(root.path()).unwrap();

    assert_eq!(deck.manifest.name, "fixture-deck");
    assert_eq!(
        deck.domains
            .iter()
            .map(|domain| domain.name.as_str())
            .collect::<Vec<_>>(),
        ["science", "writing"]
    );
}

#[test]
fn skips_entry_without_module_manifest() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("runes/notes/README.md"),
        "Descriptive fixture placeholder.\n",
    );

    let deck = load(root.path()).unwrap();

    assert!(deck.domains.is_empty());
    assert_eq!(deck.warnings.len(), 1);
    assert!(deck.warnings[0].contains("notes"));
    assert!(deck.warnings[0].contains("module.yaml"));
}

#[test]
fn rejects_domain_name_that_differs_from_directory() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("runes/science/module.yaml"),
        &module_yaml("other", ""),
    );

    let error = load(root.path()).unwrap_err();

    assert!(error.contains("science"), "{error}");
    assert!(error.contains("other"), "{error}");
}

#[test]
fn discovers_domains_in_lexicographic_order() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    for name in ["zoology", "astronomy", "botany"] {
        write(
            &root.path().join(format!("runes/{name}/module.yaml")),
            &module_yaml(name, ""),
        );
    }

    let deck = load(root.path()).unwrap();

    assert_eq!(
        deck.domains
            .iter()
            .map(|domain| domain.name.as_str())
            .collect::<Vec<_>>(),
        ["astronomy", "botany", "zoology"]
    );
}

#[test]
fn domain_provider_list_overrides_deck_default() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root.path().join("deck.yaml"),
        &deck_yaml("providers: [claude, codex]\n"),
    );
    write(
        &root.path().join("runes/defaulted/module.yaml"),
        &module_yaml("defaulted", ""),
    );
    write(
        &root.path().join("runes/overridden/module.yaml"),
        &module_yaml("overridden", "providers: [gemini]\n"),
    );

    let deck = load(root.path()).unwrap();

    assert_eq!(
        deck.providers_for(&deck.domains[0]).unwrap(),
        ["claude", "codex"]
    );
    assert_eq!(deck.providers_for(&deck.domains[1]).unwrap(), ["gemini"]);
}
