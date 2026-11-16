use std::path::Path;

use rune::ontology::Launch;

use super::plan::{self, Plan, instance_id};
use super::t3code::{self, Settings};
use super::{render_plan, render_status};

/// A value that must never reach any output.
const SECRET: &str = "do-not-print-this-value";

fn launch(yaml: &str) -> Launch {
    serde_yaml::from_str(yaml).expect("the launch section parses")
}

fn settings(json: &str) -> Settings {
    Settings::parse(Path::new("/t3/userdata/settings.json"), json).expect("the settings parse")
}

fn no_settings() -> Settings {
    Settings {
        path: "/t3/userdata/settings.json".into(),
        present: false,
        instances: Vec::new(),
    }
}

const NATIVE: &str = r#"{
  "providerInstances": {
    "claudeAgent": {
      "driver": "claudeAgent",
      "environment": [{"name": "OWNER_TOKEN", "value": "do-not-print-this-value", "sensitive": true}],
      "config": {"launchArgs": "--settings x.json", "customModels": ["owner-model", {"slug": "gpt-5.6-sol", "name": "Sol"}]}
    },
    "cursor": {"driver": "cursor", "enabled": false},
    "broken": {"displayName": "no driver"}
  },
  "favorites": ["a"]
}"#;

fn planned(yaml: &str, settings: &Settings) -> Plan {
    plan::plan(&launch(yaml), settings)
}

#[test]
fn a_profile_without_environment_goes_to_the_native_instance() {
    let plan = planned(
        "profiles:\n  claude:\n    grok:\n      model: grok\n",
        &settings(NATIVE),
    );
    assert_eq!(plan.rejected, []);
    assert_eq!(plan.profiles.len(), 1);
    assert_eq!(plan.profiles[0].profile, "grok@claude");
    assert_eq!(plan.profiles[0].instance, "claudeAgent");
    let instance = &plan.instances[0];
    assert!(instance.native && instance.exists);
    assert_eq!(instance.driver, "claudeAgent");
    assert_eq!(instance.display_name, None);
    assert_eq!(instance.environment, []);
    assert_eq!(instance.models.len(), 1);
    assert_eq!(instance.models[0].slug, "grok-4.6");
    assert_eq!(instance.models[0].name, "grok");
    assert!(!instance.models[0].present);
    assert_eq!(
        instance.owned_keys,
        ["providerInstances.claudeAgent.config.customModels[slug=grok-4.6]"]
    );
}

#[test]
fn a_model_already_in_the_list_is_not_owned() {
    let plan = planned(
        "profiles:\n  claude:\n    sol:\n      model: sol\n",
        &settings(NATIVE),
    );
    let instance = &plan.instances[0];
    assert!(instance.models[0].present, "{instance:?}");
    assert_eq!(instance.owned_keys, Vec::<String>::new());
}

#[test]
fn a_proxy_profile_gets_an_instance_beside_the_native_one() {
    let plan = planned(
        "profiles:\n  claude:\n    fable-alt:\n      model: lumo\n      env:\n        ANTHROPIC_BASE_URL: http://127.0.0.1:8317\n        ANTHROPIC_AUTH_TOKEN:\n          from_env: CLIPROXY_API_KEY\n",
        &settings(NATIVE),
    );
    assert_eq!(plan.rejected, []);
    assert_eq!(
        plan.instances.len(),
        1,
        "the native instance is not planned"
    );
    let instance = &plan.instances[0];
    assert_eq!(instance.id, "rune_claude_fable-alt");
    assert!(!instance.native && !instance.exists);
    assert_eq!(
        instance.display_name.as_deref(),
        Some("claude through rune (fable-alt)")
    );
    assert_eq!(
        instance.owned_keys,
        ["providerInstances.rune_claude_fable-alt"]
    );
    let token = &instance.environment[0];
    assert_eq!(token.name, "ANTHROPIC_AUTH_TOKEN");
    assert_eq!(token.source, "from_env");
    assert_eq!(token.from_env.as_deref(), Some("CLIPROXY_API_KEY"));
    assert!(token.sensitive && token.value.is_none());
    let url = &instance.environment[1];
    assert_eq!(url.name, "ANTHROPIC_BASE_URL");
    assert_eq!(url.value.as_deref(), Some("http://127.0.0.1:8317"));
    assert!(!url.sensitive);
}

#[test]
fn profiles_with_the_same_environment_share_one_instance() {
    let plan = planned(
        "profiles:\n  claude:\n    b:\n      model: lumo\n      env:\n        ANTHROPIC_BASE_URL: http://hub\n    a:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://hub\n    c:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://other\n",
        &no_settings(),
    );
    let ids: Vec<&str> = plan.instances.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["rune_claude_a", "rune_claude_c"]);
    let shared = &plan.instances[0];
    let slugs: Vec<&str> = shared.models.iter().map(|m| m.slug.as_str()).collect();
    assert_eq!(slugs, ["gpt-5.6-sol", "lumo-max"]);
    assert_eq!(
        shared.display_name.as_deref(),
        Some("claude through rune (a, b)")
    );
    let of = |profile: &str| {
        plan.profiles
            .iter()
            .find(|p| p.profile == profile)
            .map(|p| p.instance.as_str())
    };
    assert_eq!(of("a@claude"), Some("rune_claude_a"));
    assert_eq!(of("b@claude"), Some("rune_claude_a"));
    assert_eq!(of("c@claude"), Some("rune_claude_c"));
}

#[test]
fn a_profile_the_interface_cannot_carry_is_rejected_whole() {
    let plan = planned(
        "profiles:\n  codex:\n    wrapped:\n      model: sol\n      with: [otel, pxpipe]\n      args: [--search]\n  grok:\n    fast:\n      model: grok\n  claude:\n    unknown:\n      model: no-such-route\n    keyed:\n      env:\n        ANTHROPIC_BASE_URL: https://user:hunter2@hub.example\n",
        &no_settings(),
    );
    assert_eq!(plan.profiles, []);
    assert_eq!(plan.instances, []);
    let reasons = |profile: &str| {
        plan.rejected
            .iter()
            .find(|r| r.profile == profile)
            .unwrap_or_else(|| panic!("{profile} in {:?}", plan.rejected))
            .reasons
            .join("\n")
    };
    let wrapped = reasons("wrapped@codex");
    assert!(
        wrapped.contains("with: the middleware (otel, pxpipe)"),
        "{wrapped}"
    );
    assert!(
        wrapped.contains("args: this release maps no profile arguments"),
        "{wrapped}"
    );
    assert!(reasons("fast@grok").contains("tool: this release maps no T3 Code driver to grok"));
    assert!(
        reasons("unknown@claude").contains("model: unknown launch model route 'no-such-route'")
    );
    let keyed = reasons("keyed@claude");
    assert!(keyed.contains("env.ANTHROPIC_BASE_URL"), "{keyed}");
    assert!(
        !keyed.contains("hunter2"),
        "a credential reached the output: {keyed}"
    );
}

#[test]
fn a_base_url_with_a_query_or_a_fragment_is_rejected_and_never_shown() {
    for url in [
        format!("https://gateway.example/v1?api_key={SECRET}"),
        format!("https://gateway.example/v1#{SECRET}"),
        format!("gateway.example/v1?token={SECRET}&x=1"),
    ] {
        let plan = planned(
            &format!(
                "profiles:\n  codex:\n    hub:\n      model: sol\n      env:\n        OPENAI_BASE_URL: \"{url}\"\n"
            ),
            &no_settings(),
        );
        assert_eq!(plan.profiles, [], "{url}");
        assert_eq!(plan.instances, [], "{url}");
        assert_eq!(plan.rejected.len(), 1, "{url}");
        assert_eq!(plan.rejected[0].profile, "hub@codex");
        assert!(
            plan.rejected[0].reasons[0]
                .starts_with("env.OPENAI_BASE_URL: the base URL has a query string or a fragment"),
            "{:?}",
            plan.rejected
        );
        let json = serde_json::to_string(&plan).expect("the plan serializes");
        assert!(!json.contains(SECRET), "{json}");
        let text = render_plan(&no_settings(), &plan);
        assert!(!text.contains(SECRET), "{text}");
        let status = render_status(&no_settings(), &plan);
        assert!(!status.contains(SECRET), "{status}");
    }
    // The rule is for the value the plan prints. Another literal is never
    // shown, so a `?` in it rejects nothing.
    let plan = planned(
        "profiles:\n  codex:\n    hub:\n      env:\n        OPENAI_BASE_URL: http://hub/v1\n        EXTRA: \"a?b#c\"\n",
        &no_settings(),
    );
    assert_eq!(plan.rejected, []);
}

#[test]
fn a_literal_that_is_not_the_base_url_is_never_shown() {
    let plan = planned(
        &format!(
            "profiles:\n  codex:\n    hub:\n      model: sol\n      env:\n        OPENAI_BASE_URL: http://hub/v1\n        EXTRA_HEADER: {SECRET}\n"
        ),
        &no_settings(),
    );
    let extra = plan.instances[0]
        .environment
        .iter()
        .find(|v| v.name == "EXTRA_HEADER")
        .expect("the variable is planned");
    assert_eq!(extra.source, "literal");
    assert_eq!(extra.value, None);
    let json = serde_json::to_string(&plan).expect("the plan serializes");
    assert!(!json.contains(SECRET), "{json}");
    assert!(json.contains("http://hub/v1"), "{json}");
    let text = render_plan(&no_settings(), &plan);
    assert!(!text.contains(SECRET), "{text}");
    assert!(
        text.contains("env    EXTRA_HEADER  (a literal, not shown)"),
        "{text}"
    );
    assert!(
        text.contains("env    OPENAI_BASE_URL = http://hub/v1"),
        "{text}"
    );
}

#[test]
fn a_configured_base_url_key_decides_which_value_is_shown() {
    let plan = planned(
        "tools:\n  codex:\n    base_url_env: MY_HUB\nprofiles:\n  codex:\n    hub:\n      env:\n        MY_HUB: http://hub/v1\n        OPENAI_BASE_URL: http://not-the-key\n",
        &no_settings(),
    );
    let shown: Vec<(&str, Option<&str>)> = plan.instances[0]
        .environment
        .iter()
        .map(|v| (v.name.as_str(), v.value.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [("MY_HUB", Some("http://hub/v1")), ("OPENAI_BASE_URL", None)]
    );
}

#[test]
fn an_instance_of_the_same_id_with_another_driver_is_a_conflict() {
    let existing = settings(r#"{"providerInstances": {"rune_claude_hub": {"driver": "codex"}}}"#);
    let plan = planned(
        "profiles:\n  claude:\n    hub:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://hub\n",
        &existing,
    );
    let instance = &plan.instances[0];
    assert!(instance.exists);
    assert!(
        instance
            .conflict
            .as_deref()
            .is_some_and(|why| why.contains("with the driver codex")),
        "{instance:?}"
    );
    assert_eq!(instance.owned_keys, Vec::<String>::new());
    let text = render_status(&existing, &plan);
    assert!(
        text.contains("hub@claude -> rune_claude_hub  gpt-5.6-sol  conflict"),
        "{text}"
    );
}

/// A profile behind a hub, with a literal base URL and a secret reference.
const HUB_PROFILE: &str = "profiles:\n  codex:\n    hub:\n      model: sol\n      env:\n        OPENAI_BASE_URL: https://new.example/v1\n        OPENAI_API_KEY:\n          from_env: HUB_KEY\n";

/// Settings with the instance of [`HUB_PROFILE`], its model, and the
/// given environment.
fn hub_settings(environment: &str) -> Settings {
    settings(&format!(
        r#"{{"providerInstances": {{"rune_codex_hub": {{"driver": "codex", "environment": {environment},
            "config": {{"customModels": [{{"slug": "gpt-5.6-sol", "name": "hub"}}]}}}}}}}}"#
    ))
}

#[test]
fn an_existing_instance_on_the_same_route_is_installed() {
    let existing = hub_settings(
        r#"[{"name": "OPENAI_BASE_URL", "value": "https://new.example/v1", "sensitive": false},
            {"name": "OPENAI_API_KEY", "value": "", "sensitive": true}]"#,
    );
    let plan = planned(HUB_PROFILE, &existing);
    let instance = &plan.instances[0];
    assert!(instance.exists, "{instance:?}");
    assert_eq!(instance.conflict, None);
    assert_eq!(instance.route_unknown, None);
    let text = render_status(&existing, &plan);
    assert!(
        text.contains("hub@codex -> rune_codex_hub  gpt-5.6-sol  installed"),
        "{text}"
    );
}

#[test]
fn an_existing_instance_on_another_route_is_a_conflict() {
    let old = "https://old.example/v1";
    for (environment, why) in [
        (
            format!(
                r#"[{{"name": "OPENAI_BASE_URL", "value": "{old}", "sensitive": false}},
                    {{"name": "OPENAI_API_KEY", "value": "", "sensitive": true}}]"#
            ),
            "its OPENAI_BASE_URL has another value",
        ),
        (
            r#"[{"name": "OPENAI_API_KEY", "value": "", "sensitive": true}]"#.to_string(),
            "it sets no OPENAI_BASE_URL",
        ),
        (
            format!(
                r#"[{{"name": "OPENAI_BASE_URL", "value": "https://new.example/v1"}},
                    {{"name": "OPENAI_API_KEY", "value": "", "sensitive": true}},
                    {{"name": "HTTPS_PROXY", "value": "{old}"}}]"#
            ),
            "it sets HTTPS_PROXY, which the profile does not",
        ),
    ] {
        let existing = hub_settings(&environment);
        let plan = planned(HUB_PROFILE, &existing);
        let instance = &plan.instances[0];
        assert!(instance.exists);
        let conflict = instance.conflict.as_deref().unwrap_or_default();
        assert!(
            conflict
                .starts_with("the settings file has an instance rune_codex_hub on another route: ")
                && conflict.contains(why),
            "{conflict}"
        );
        assert_eq!(instance.route_unknown, None);
        assert_eq!(instance.owned_keys, Vec::<String>::new());
        let json = serde_json::to_string(&plan).expect("the plan serializes");
        let status = render_status(&existing, &plan);
        let text = render_plan(&existing, &plan);
        for output in [&json, &status, &text] {
            assert!(!output.contains(old), "the file's value is shown: {output}");
        }
        assert!(
            status.contains("hub@codex -> rune_codex_hub  gpt-5.6-sol  conflict"),
            "{status}"
        );
        assert!(text.contains("    conflict  the settings file"), "{text}");
    }
}

#[test]
fn a_route_that_needs_a_secret_to_compare_is_unknown() {
    for (environment, why) in [
        (
            r#"[{"name": "OPENAI_BASE_URL", "value": "", "sensitive": true},
                {"name": "OPENAI_API_KEY", "value": "", "sensitive": true}]"#,
            "its OPENAI_BASE_URL is a secret",
        ),
        (
            r#"[{"name": "OPENAI_BASE_URL", "value": "https://new.example/v1"},
                {"name": "OPENAI_API_KEY", "value": "do-not-print-this-value"}]"#,
            "its OPENAI_API_KEY is a value in the file, and the profile reads HUB_KEY",
        ),
    ] {
        let existing = hub_settings(environment);
        let plan = planned(HUB_PROFILE, &existing);
        let instance = &plan.instances[0];
        assert_eq!(instance.conflict, None, "{instance:?}");
        let unknown = instance.route_unknown.as_deref().unwrap_or_default();
        assert!(unknown.contains(why), "{unknown}");
        assert_eq!(instance.owned_keys, Vec::<String>::new());
        let status = render_status(&existing, &plan);
        assert!(
            status.contains("hub@codex -> rune_codex_hub  gpt-5.6-sol  route unknown"),
            "{status}"
        );
        let text = render_plan(&existing, &plan);
        assert!(
            text.contains("    route unknown  rune cannot tell"),
            "{text}"
        );
        let json = serde_json::to_string(&plan).expect("the plan serializes");
        for output in [&json, &status, &text, &format!("{existing:?}")] {
            assert!(!output.contains(SECRET), "{output}");
        }
    }
}

#[test]
fn what_the_interface_loses_is_named() {
    let plan = planned(
        "default_with: [otel]\nprofiles:\n  claude:\n    sol:\n      model: sol\n  codex:\n    sol:\n      model: sol\n",
        &no_settings(),
    );
    let fields: Vec<(&str, &str)> = plan
        .unrepresented
        .iter()
        .map(|u| (u.profile.as_str(), u.field.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            ("*", "launch.default_with"),
            ("sol@claude", "launch.models.sol")
        ]
    );
    assert!(plan.unrepresented[0].reason.contains("(otel)"));
}

#[test]
fn settings_give_instances_models_and_variable_names_and_no_value() {
    let read = settings(NATIVE);
    assert!(read.present);
    let ids: Vec<&str> = read.instances.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(
        ids,
        ["claudeAgent", "cursor"],
        "an entry without a driver is no instance"
    );
    let claude = read.instance("claudeAgent").expect("the instance");
    assert!(claude.enabled);
    assert!(claude.has_model("owner-model") && claude.has_model("gpt-5.6-sol"));
    assert_eq!(claude.models[1].name.as_deref(), Some("Sol"));
    assert_eq!(claude.environment[0].name, "OWNER_TOKEN");
    assert!(claude.environment[0].sensitive);
    assert!(!read.instance("cursor").expect("the instance").enabled);
    let json = serde_json::to_string(&read).expect("the settings serialize");
    assert!(!json.contains(SECRET), "{json}");
    assert!(!json.contains("launchArgs"), "{json}");
    let text = render_status(&read, &planned("{}", &read));
    assert!(!text.contains(SECRET), "{text}");
    assert!(text.contains("  env    OWNER_TOKEN  (sensitive)"), "{text}");
    assert!(
        text.contains("  none: the configuration has no launch profile"),
        "{text}"
    );
}

#[test]
fn settings_that_are_not_json_are_an_error_and_an_absent_file_is_not() {
    let error = Settings::parse(Path::new("/x/settings.json"), "{not json")
        .expect_err("broken settings are refused");
    assert!(
        error.to_string().contains("/x/settings.json is not JSON"),
        "{error}"
    );
    let directory = tempfile::tempdir().expect("scratch");
    let absent = Settings::read(&directory.path().join("settings.json")).expect("absent is fine");
    assert!(!absent.present);
    assert_eq!(absent.instances, []);
}

#[test]
fn the_settings_path_follows_t3code_home() {
    let home = Path::new("/home/owner");
    assert_eq!(
        t3code::settings_path(home, None),
        Path::new("/home/owner/.t3/userdata/settings.json")
    );
    assert_eq!(
        t3code::settings_path(home, Some(std::ffi::OsStr::new(""))),
        Path::new("/home/owner/.t3/userdata/settings.json")
    );
    assert_eq!(
        t3code::settings_path(home, Some(std::ffi::OsStr::new("/elsewhere"))),
        Path::new("/elsewhere/userdata/settings.json")
    );
}

#[test]
fn an_instance_id_is_one_t3_code_accepts() {
    assert_eq!(instance_id("claude", "fable-alt"), "rune_claude_fable-alt");
    assert_eq!(instance_id("codex", "a.b c/d"), "rune_codex_a-b-c-d");
    let long = instance_id("claude", &"x".repeat(100));
    assert_eq!(long.len(), 64);
    assert!(long.starts_with("rune_claude_x"));
}

#[test]
fn profiles_that_would_share_an_instance_id_are_rejected() {
    // `a.b` and `a-b` after the character replacement.
    let plan = planned(
        "profiles:\n  claude:\n    a.b:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://one.example\n    a-b:\n      model: lumo\n      env:\n        ANTHROPIC_BASE_URL: http://two.example\n    friend:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://two.example\n    other:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://three.example\n",
        &no_settings(),
    );
    let ids: Vec<&str> = plan.instances.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(
        ids,
        ["rune_claude_other"],
        "no instance of the shared id is planned"
    );
    let carried: Vec<&str> = plan.profiles.iter().map(|p| p.profile.as_str()).collect();
    assert_eq!(carried, ["other@claude"]);
    let rejected: Vec<&str> = plan.rejected.iter().map(|r| r.profile.as_str()).collect();
    // `friend` shares the environment of `a-b`, so it has no instance.
    assert_eq!(rejected, ["a-b@claude", "a.b@claude", "friend@claude"]);
    for gone in &plan.rejected {
        assert_eq!(
            gone.reasons,
            [
                "instance: the profiles a-b@claude and a.b@claude would get the same instance id rune_claude_a-b with different environments. Rename one of them"
            ]
        );
    }
    let lost: Vec<&str> = plan
        .unrepresented
        .iter()
        .map(|u| u.profile.as_str())
        .collect();
    assert_eq!(lost, ["other@claude"], "a rejected profile loses nothing");

    // Two long names after the cut at 64 characters.
    let long = "x".repeat(70);
    let plan = planned(
        &format!(
            "profiles:\n  codex:\n    {long}a:\n      env:\n        OPENAI_BASE_URL: http://one.example\n    {long}b:\n      env:\n        OPENAI_BASE_URL: http://two.example\n"
        ),
        &no_settings(),
    );
    assert_eq!(plan.instances, []);
    assert_eq!(plan.rejected.len(), 2);

    // The same environment is one group, and one id is no collision.
    let plan = planned(
        "profiles:\n  claude:\n    a.b:\n      model: sol\n      env:\n        ANTHROPIC_BASE_URL: http://one.example\n    a-b:\n      model: lumo\n      env:\n        ANTHROPIC_BASE_URL: http://one.example\n",
        &no_settings(),
    );
    assert_eq!(plan.rejected, []);
    assert_eq!(plan.instances.len(), 1);
}

#[test]
fn the_plan_text_says_that_nothing_is_written() {
    let existing = settings(NATIVE);
    let plan = planned(
        "profiles:\n  claude:\n    grok:\n      model: grok\n    hub:\n      model: lumo\n      env:\n        ANTHROPIC_AUTH_TOKEN:\n          from_env: HUB_KEY\n",
        &existing,
    );
    let text = render_plan(&existing, &plan);
    assert!(text.starts_with("dry run: rune writes nothing\n"), "{text}");
    assert!(
        text.contains("settings  /t3/userdata/settings.json (present)"),
        "{text}"
    );
    assert!(
        text.contains("  grok@claude -> claudeAgent  grok-4.6"),
        "{text}"
    );
    assert!(
        text.contains("  claudeAgent  driver claudeAgent  native, in the settings file"),
        "{text}"
    );
    assert!(
        text.contains("    model  grok-4.6  \"grok\"  add"),
        "{text}"
    );
    assert!(
        text.contains(
            "    owns   providerInstances.claudeAgent.config.customModels[slug=grok-4.6]"
        ),
        "{text}"
    );
    assert!(
        text.contains("  rune_claude_hub  driver claudeAgent  new, beside the native instance"),
        "{text}"
    );
    assert!(
        text.contains("    env    ANTHROPIC_AUTH_TOKEN  from_env HUB_KEY  (a secret: enter its value in T3 Code)"),
        "{text}"
    );
    assert!(
        text.contains("    owns   providerInstances.rune_claude_hub"),
        "{text}"
    );
}
