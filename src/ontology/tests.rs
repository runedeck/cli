use super::*;

fn no_env(_: &str) -> Option<String> {
    None
}

fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    |name| {
        pairs
            .iter()
            .find_map(|(key, value)| (*key == name).then(|| (*value).to_string()))
    }
}

#[test]
fn env_beats_config_beats_default() {
    let config = Config {
        ontology: Ontology {
            quests: Some("/from-config".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let resolved = resolve_config(&config, &env_from(&[("RUNE_QUESTS", "/from-env")]));
    let quests = resolved.ontology.quests.expect("quests resolved");
    assert_eq!(quests.value, "/from-env");
    assert_eq!(quests.source, Source::Env);
}

#[test]
fn rune_deck_env_beats_config() {
    let config = Config {
        deck: Some("/from-config".to_string()),
        ..Config::default()
    };
    let resolved = resolve_config(&config, &env_from(&[("RUNE_DECK", "/from-env")]));
    let deck = resolved.deck.expect("deck resolved");
    assert_eq!(deck.value, "/from-env");
    assert_eq!(deck.source, Source::Env);
}

#[test]
fn deck_config_is_used_without_env() {
    let config = Config {
        deck: Some("/from-config".to_string()),
        ..Config::default()
    };
    let deck = resolve_config(&config, &no_env)
        .deck
        .expect("deck resolved");
    assert_eq!(deck.value, "/from-config");
    assert_eq!(deck.source, Source::Config);
}

#[test]
fn deck_is_unset_without_env_or_config() {
    assert!(resolve_config(&Config::default(), &no_env).deck.is_none());
}

#[test]
fn config_beats_default() {
    let config = Config {
        ontology: Ontology {
            domain: Some("Research".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let resolved = resolve_config(&config, &no_env);
    let domain = resolved.ontology.domain.expect("domain resolved");
    assert_eq!(domain.value, "Research");
    assert_eq!(domain.source, Source::Config);
}

#[test]
fn missing_config_uses_default() {
    let resolved = resolve_config(&Config::default(), &no_env);
    let domain = resolved.ontology.domain.expect("domain default");
    assert_eq!(domain.value, "Technology");
    assert_eq!(domain.source, Source::Default);
}

#[test]
fn env_override_is_reflected_in_env_vars() {
    let resolved = resolve_config(
        &Config::default(),
        &env_from(&[("RUNE_OWNER", "N4M3Z"), ("RUNE_DOMAIN", "Systems")]),
    );
    let vars = env_vars(&resolved);
    assert!(vars.contains(&("RUNE_OWNER".to_string(), "N4M3Z".to_string())));
    assert!(vars.contains(&("RUNE_DOMAIN".to_string(), "Systems".to_string())));
}

#[test]
fn tilde_expansion_resolves_under_home() {
    let home = dirs::home_dir().expect("home for test");
    assert_eq!(expand_tilde("~/Agents"), home.join("Agents"));
}

#[test]
fn project_yaml_is_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("project.yaml");
    std::fs::write(
        &project,
        "workshop: /workshop\ndefaults:\n    domain: Security\n",
    )
    .expect("write project");

    let resolved = load_from_dir_with_env(dir.path(), &no_env).expect("load defaults");
    assert!(resolved.ontology.quests.is_some_and(|quests| {
        quests.source == Source::Default && quests.value.ends_with("Agents")
    }));
}

#[test]
fn lore_and_artifacts_resolve_from_env() {
    let resolved = resolve_config(
        &Config::default(),
        &env_from(&[("RUNE_LORE", "/lore"), ("RUNE_ARTIFACTS", "/artifacts")]),
    );
    assert_eq!(resolved.ontology.lore.expect("lore").value, "/lore");
    assert_eq!(
        resolved.ontology.artifacts.expect("artifacts").value,
        "/artifacts"
    );
}

#[test]
fn unknown_top_level_config_key_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("config.yaml");
    std::fs::write(&config, "surprise: true\n").expect("write config");

    let error = load_from_dir_with_env(dir.path(), &no_env).expect_err("unknown key");
    assert_eq!(error.kind(), ErrorKind::Config);
    assert!(error.message().contains("unknown field"), "got: {error}");
}
