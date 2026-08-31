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
            targets: Some("/from-config".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let resolved = resolve_config(&config, &env_from(&[("RUNE_TARGETS", "/from-env")]));
    let targets = resolved.ontology.targets.expect("targets resolved");
    assert_eq!(targets.value, "/from-env");
    assert_eq!(targets.source, Source::Env);
}

#[test]
fn legacy_quests_config_and_env_still_resolve_targets() {
    let config = Config {
        ontology: Ontology {
            quests: Some("/legacy-config".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let from_config = resolve_config(&config, &no_env);
    assert_eq!(
        from_config
            .ontology
            .targets
            .expect("legacy key resolves")
            .value,
        "/legacy-config"
    );

    let from_env = resolve_config(
        &Config::default(),
        &env_from(&[("RUNE_QUESTS", "/legacy-env")]),
    );
    assert_eq!(
        from_env
            .ontology
            .targets
            .expect("legacy env resolves")
            .value,
        "/legacy-env"
    );
}

#[test]
fn skeleton_env_beats_config_and_has_owner_default() {
    let config = Config {
        ontology: Ontology {
            skeleton: Some("/from-config".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let from_env = resolve_config(&config, &env_from(&[("RUNE_SKELETON", "/from-env")]));
    assert_eq!(
        from_env.ontology.skeleton.expect("skeleton").value,
        "/from-env"
    );

    // No built-in skeleton default: init falls back to the embedded skeleton
    // when neither config nor environment name a checkout.
    assert!(
        resolve_config(&Config::default(), &no_env)
            .ontology
            .skeleton
            .is_none()
    );
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
fn setup_record_parses_and_resolves() {
    let directory = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        directory.path().join("config.yaml"),
        "setup:\n    version: 1\n    completed:\n        - deck\n        - providers\n",
    )
    .expect("write config");

    let resolved = load_from_dir_with_env(directory.path(), &no_env).expect("load config");
    let record = resolved.setup.expect("setup record");
    assert_eq!(record.version, 1);
    assert_eq!(record.completed, ["deck", "providers"]);
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
    let targets = resolved.ontology.targets.expect("targets default");
    assert!(targets.value.ends_with("Agents"));
    assert_eq!(targets.source, Source::Default);
    // Machine-specific keys carry no built-in default.
    assert!(resolved.ontology.domain.is_none());
    assert!(resolved.ontology.vault.is_none());
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
    assert!(resolved.ontology.targets.is_some_and(|targets| {
        targets.source == Source::Default && targets.value.ends_with("Agents")
    }));
}

#[test]
fn malformed_config_has_a_stable_repair() {
    let path = Path::new("/tmp/config.yaml");
    let error = parse_config("ontology: [", path).expect_err("config must be malformed");

    assert_eq!(error.kind(), ErrorKind::Config);
    assert_eq!(error.code(), "config.invalid");
    assert_eq!(error.fix_command(), Some("rune config path"));
    assert!(
        error
            .message()
            .starts_with("/tmp/config.yaml is malformed:")
    );
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
fn missing_config_includes_default_proxy_profiles() {
    let resolved = resolve_config(&Config::default(), &no_env);
    let claude_profiles = resolved
        .launch
        .profiles
        .get("claude")
        .expect("default Claude profiles");

    assert_eq!(claude_profiles["sol"].model.as_deref(), Some("sol"));
    assert_eq!(claude_profiles["grok"].model.as_deref(), Some("grok"));
    assert!(resolved.launch.models.is_empty());
    assert_eq!(
        claude_profiles["grok"].env["ANTHROPIC_BASE_URL"],
        ProfileEnvValue::Literal("http://127.0.0.1:8317".to_string())
    );
}

#[test]
fn configured_proxy_endpoint_updates_default_profiles() {
    let config = Config {
        launch: Launch {
            middleware: LaunchMiddleware {
                cliproxy: CliproxyConfig {
                    host: "cliproxy.internal".to_string(),
                    port: 9443,
                    command: String::new(),
                },
                ..LaunchMiddleware::default()
            },
            ..Launch::default()
        },
        ..Config::default()
    };

    let resolved = resolve_config(&config, &no_env);
    assert_eq!(
        resolved.launch.profiles["claude"]["sol"].env["ANTHROPIC_BASE_URL"],
        ProfileEnvValue::Literal("http://cliproxy.internal:9443".to_string())
    );
}

#[test]
fn configured_launch_profiles_replace_matching_defaults() {
    let configured_profile = LaunchProfile {
        model: Some("grok".to_string()),
        env: [(
            "ANTHROPIC_BASE_URL".to_string(),
            ProfileEnvValue::Literal("https://proxy.example.com".to_string()),
        )]
        .into_iter()
        .collect(),
        ..LaunchProfile::default()
    };
    let config = Config {
        launch: Launch {
            models: [(
                "grok".to_string(),
                LaunchModel {
                    id: "configured-grok".to_string(),
                    context: 300_000,
                    compact: None,
                },
            )]
            .into_iter()
            .collect(),
            profiles: [(
                "claude".to_string(),
                [("grok".to_string(), configured_profile)]
                    .into_iter()
                    .collect(),
            )]
            .into_iter()
            .collect(),
            ..Launch::default()
        },
        ..Config::default()
    };

    let resolved = resolve_config(&config, &no_env);
    assert_eq!(resolved.launch.models["grok"].id, "configured-grok");
    assert_eq!(resolved.launch.models["grok"].context, 300_000);
    assert_eq!(
        resolved.launch.profiles["claude"]["grok"].env["ANTHROPIC_BASE_URL"],
        ProfileEnvValue::Literal("https://proxy.example.com".to_string())
    );
    assert!(resolved.launch.profiles["claude"].contains_key("sol"));
}

#[test]
fn launch_model_routes_deserialize_with_profile_references() {
    let config: Config = serde_yaml::from_str(
        "launch:\n    models:\n        sol:\n            id: gpt-5.6-sol\n            context: 272000\n            compact: 85\n    profiles:\n        claude:\n            sol:\n                model: sol\n",
    )
    .expect("launch model config");

    let model = config.launch.models.get("sol").expect("sol route");
    assert_eq!(model.id, "gpt-5.6-sol");
    assert_eq!(model.context, 272_000);
    assert_eq!(model.compact, Some(85));
    assert_eq!(
        config.launch.profiles["claude"]["sol"].model.as_deref(),
        Some("sol")
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
