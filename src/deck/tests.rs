use super::*;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn deck_yaml(providers: &str) -> String {
    format!(
        "schema: 1\nname: fixture-deck\nversion: 0.1.0\ndescription: Fixture deck.\n{providers}"
    )
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
fn skips_dotfiles_under_runes_without_warning() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(&root.path().join("runes/.DS_Store"), "metadata");
    write(
        &root.path().join("runes/.notes/README.md"),
        "hidden entry\n",
    );

    let deck = load(root.path()).unwrap();

    assert!(deck.domains.is_empty());
    assert!(deck.warnings.is_empty());
}

#[test]
fn skips_root_readme_under_runes_without_warning() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("runes/README.md"),
        "Deck domain documentation.\n",
    );

    let deck = load(root.path()).unwrap();

    assert!(deck.domains.is_empty());
    assert!(deck.warnings.is_empty());
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

#[test]
fn domain_defaults_provider_keys_override_deck_default() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root.path().join("deck.yaml"),
        &deck_yaml("providers: [claude, codex]\n"),
    );
    write(
        &root.path().join("runes/science/module.yaml"),
        &module_yaml("science", ""),
    );
    write(
        &root.path().join("runes/science/defaults.yaml"),
        "providers:\n    gemini:\n        target: .gemini\n",
    );

    let deck = load(root.path()).unwrap();

    assert_eq!(deck.providers_for(&deck.domains[0]).unwrap(), ["gemini"]);
}

#[test]
fn rejects_missing_deck_schema_with_found_value() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root.path().join("deck.yaml"),
        "name: fixture-deck\nversion: 0.1.0\ndescription: Fixture deck.\n",
    );

    let error = load(root.path()).unwrap_err();

    assert!(error.contains("missing"), "{error}");
    assert!(error.contains("supported schema is 1"), "{error}");
}

#[test]
fn rejects_wrong_deck_schema_with_found_and_supported_values() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root.path().join("deck.yaml"),
        "schema: 7\nname: fixture-deck\nversion: 0.1.0\ndescription: Fixture deck.\n",
    );

    let error = load(root.path()).unwrap_err();

    assert!(error.contains("found 7"), "{error}");
    assert!(error.contains("supported schema is 1"), "{error}");
}

#[test]
fn accepts_supported_deck_schema() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));

    let deck = load(root.path()).unwrap();

    assert_eq!(deck.manifest.schema, 1);
    assert_eq!(deck.manifest.name, "fixture-deck");
}

fn cast_fixture() -> (tempfile::TempDir, Deck) {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("casts/foundations.yaml"),
        "name: foundations\ndescription: Base skills.\nrunes: ['science/skills/**']\nexclude: ['science/skills/Removed']\n",
    );
    write(
        &root.path().join("casts/writers.yaml"),
        "name: writers\ndescription: Writing rules.\nrunes: ['writing/rules/*']\n",
    );
    write(
        &root.path().join("casts/full.yaml"),
        "name: full\ndescription: Ordered union.\nextends: [foundations, writers]\nrunes: ['science/agents/A*']\nexclude: ['writing/**/Draft', 'science/skills/Inherited']\n",
    );
    let deck = load(root.path()).unwrap();
    (root, deck)
}

#[test]
fn resolves_cast_extends_globs_exclude_last_and_deterministic_order() {
    let (_root, deck) = cast_fixture();
    let artifacts = [
        "writing/rules/Published",
        "science/skills/OnlyScience",
        "science/agents/Archivist",
        "science/skills/Inherited",
        "writing/rules/Draft",
        "science/rules/Unselected",
    ];

    let resolved = deck.resolve_cast("full", artifacts).unwrap();

    assert_eq!(
        resolved,
        [
            "science/skills/OnlyScience",
            "science/agents/Archivist",
            "writing/rules/Published",
        ]
    );
}

#[test]
fn double_star_crosses_canonical_id_segments_but_star_does_not() {
    let (_root, deck) = cast_fixture();
    let artifacts = [
        "science/skills/OnlyScience",
        "science/skills/Nested/Companion",
        "writing/rules/Published",
        "writing/rules/Nested/Published",
    ];

    let foundations = deck.resolve_cast("foundations", artifacts).unwrap();
    let writers = deck.resolve_cast("writers", artifacts).unwrap();

    assert_eq!(
        foundations,
        [
            "science/skills/Nested/Companion",
            "science/skills/OnlyScience"
        ]
    );
    assert_eq!(writers, ["writing/rules/Published"]);
}

#[test]
fn rejects_cast_extension_cycle_with_complete_path() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("casts/alpha.yaml"),
        "name: alpha\ndescription: Alpha.\nextends: [beta]\nrunes: []\n",
    );
    write(
        &root.path().join("casts/beta.yaml"),
        "name: beta\ndescription: Beta.\nextends: [alpha]\nrunes: []\n",
    );
    let deck = load(root.path()).unwrap();

    let error = deck
        .resolve_cast("alpha", std::iter::empty::<&str>())
        .unwrap_err();

    assert!(error.contains("alpha -> beta -> alpha"), "{error}");
}

#[test]
fn resolves_cast_parents_in_listed_order() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("casts/first.yaml"),
        "name: first\ndescription: First.\nrunes: ['science/skills/FirstMissing']\n",
    );
    write(
        &root.path().join("casts/second.yaml"),
        "name: second\ndescription: Second.\nrunes: ['science/skills/SecondMissing']\n",
    );
    write(
        &root.path().join("casts/ordered.yaml"),
        "name: ordered\ndescription: Ordered.\nextends: [first, second]\nrunes: []\n",
    );
    let deck = load(root.path()).unwrap();

    let error = deck
        .resolve_cast("ordered", ["science/skills/Present"])
        .unwrap_err();

    assert!(error.contains("FirstMissing"), "{error}");
    assert!(!error.contains("SecondMissing"), "{error}");
}

#[test]
fn rejects_cast_pattern_that_matches_no_artifact() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("deck.yaml"), &deck_yaml(""));
    write(
        &root.path().join("casts/stale.yaml"),
        "name: stale\ndescription: Removed artifact.\nrunes: ['science/skills/Removed']\n",
    );
    let deck = load(root.path()).unwrap();

    let error = deck
        .resolve_cast("stale", ["science/skills/Present"])
        .unwrap_err();

    assert!(error.contains("science/skills/Removed"), "{error}");
    assert!(error.contains("matches no artifact"), "{error}");
}
