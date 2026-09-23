use super::*;

fn invocation(surface: Surface, extra_args: &[&str]) -> SurfaceInvocation {
    SurfaceInvocation {
        surface,
        binary: OsString::from(surface_name(surface)),
        extra_args: extra_args.iter().map(OsString::from).collect(),
        env: Vec::new(),
        repository: PathBuf::from("."),
        mode: AccessMode::ReadOnly,
        system_prompt: String::new(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: None,
    }
}

fn strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn owned_profile_flag_is_dropped_with_a_warning_not_a_refusal() {
    let filtered = filter_surface_args(&invocation(
        Surface::Codex,
        &["--sandbox=workspace-write", "--search"],
    ));
    assert_eq!(strings(&filtered.kept), ["--search"]);
    assert_eq!(filtered.warnings.len(), 1);
    assert!(
        filtered.warnings[0].contains("owns --sandbox"),
        "{}",
        filtered.warnings[0]
    );
}

#[test]
fn owned_flag_drops_its_separate_value_too() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--permission-mode", "acceptEdits", "--verbose"],
    ));
    assert_eq!(strings(&filtered.kept), ["--verbose"]);
    assert_eq!(filtered.warnings.len(), 1);
}

#[test]
fn codex_config_keeps_keys_the_run_does_not_set() {
    let filtered = filter_surface_args(&invocation(
        Surface::Codex,
        &[
            "--config",
            "model=\"gpt-6-astra\"",
            "--config",
            "model_reasoning_effort=\"ultra\"",
        ],
    ));
    assert_eq!(
        strings(&filtered.kept),
        ["--config", "model_reasoning_effort=\"ultra\""]
    );
    assert_eq!(filtered.warnings.len(), 1);
    assert!(filtered.warnings[0].contains("config key model"));
}

#[test]
fn claude_settings_keeps_env_and_drops_permissions() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &[
            "--settings",
            r#"{"env":{"CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS":"0"},"permissions":{"allow":["Bash"]}}"#,
        ],
    ));
    let kept = strings(&filtered.kept);
    assert_eq!(kept[0], "--settings");
    let settings: serde_json::Value = serde_json::from_str(&kept[1]).expect("json");
    assert_eq!(settings["env"]["CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS"], "0");
    assert!(settings.get("permissions").is_none());
    assert_eq!(filtered.warnings.len(), 1);
    assert!(filtered.warnings[0].contains("settings key permissions"));
}

#[test]
fn claude_settings_with_only_owned_keys_is_dropped_whole() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--settings", r#"{"permissions":{"allow":["Bash"]}}"#],
    ));
    assert!(filtered.kept.is_empty());
    assert_eq!(filtered.warnings.len(), 2, "{:?}", filtered.warnings);
    assert!(filtered.warnings[1].contains("nothing remains"));
}

#[test]
fn attached_short_and_equals_forms_are_owned_too() {
    let filtered = filter_surface_args(&invocation(
        Surface::Codex,
        &[
            "-csandbox_mode=\"danger-full-access\"",
            "-c=approval_policy=\"never\"",
            "--config=model_reasoning_effort=\"high\"",
        ],
    ));
    assert_eq!(
        strings(&filtered.kept),
        ["--config=model_reasoning_effort=\"high\""]
    );
    assert_eq!(filtered.warnings.len(), 2, "{:?}", filtered.warnings);
}

#[test]
fn equals_form_of_an_owned_flag_is_dropped() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--permission-mode=acceptEdits", "--verbose"],
    ));
    assert_eq!(strings(&filtered.kept), ["--verbose"]);
    assert_eq!(filtered.warnings.len(), 1);
}

#[test]
fn an_owned_flag_with_a_missing_value_does_not_swallow_its_neighbor() {
    let filtered = filter_surface_args(&invocation(Surface::Claude, &["--model", "--verbose"]));
    assert_eq!(strings(&filtered.kept), ["--verbose"]);
    assert!(filtered.warnings[0].contains("the profile flag is dropped"));
}

#[test]
fn arity_is_per_surface() {
    // `-p` is codex's --profile, which takes a value, and claude's print switch, which does not.
    let codex = filter_surface_args(&invocation(Surface::Codex, &["-p", "work", "--search"]));
    assert_eq!(strings(&codex.kept), ["--search"]);
    let claude = filter_surface_args(&invocation(Surface::Claude, &["-p", "--verbose"]));
    assert_eq!(strings(&claude.kept), ["--verbose"]);
}

#[test]
fn nested_codex_sandbox_keys_are_owned_by_prefix() {
    let filtered = filter_surface_args(&invocation(
        Surface::Codex,
        &["-c", "sandbox_workspace_write.network_access=true"],
    ));
    assert!(filtered.kept.is_empty(), "{:?}", filtered.kept);
    assert!(filtered.warnings[0].contains("config key sandbox_workspace_write.network_access"));
}

#[test]
fn claude_settings_path_is_dropped_not_loaded() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--settings", "/home/me/wide-open.json"],
    ));
    assert!(filtered.kept.is_empty());
    assert!(filtered.warnings[0].contains("only an inline JSON object is filtered"));
}

#[test]
fn warnings_escape_control_characters_and_name_the_value() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--model", "evil\nwarning: forged"],
    ));
    assert_eq!(filtered.warnings.len(), 1);
    assert!(!filtered.warnings[0].contains('\n'));
    assert!(filtered.warnings[0].contains("evil"));
}

#[test]
fn unowned_profile_args_pass_through_untouched() {
    let filtered = filter_surface_args(&invocation(Surface::Grok, &["--effort", "high"]));
    assert_eq!(strings(&filtered.kept), ["--effort", "high"]);
    assert!(filtered.warnings.is_empty());
}

#[test]
fn claude_read_only_limits_available_tools() {
    let invocation = SurfaceInvocation {
        surface: Surface::Claude,
        binary: OsString::from("claude"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("."),
        mode: AccessMode::ReadOnly,
        system_prompt: String::new(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: None,
    };

    assert_eq!(
        claude_args(&invocation),
        [
            "--print",
            "--output-format",
            "text",
            "--permission-mode",
            "plan",
            "--tools",
            "Read,Glob,Grep",
            "--allowedTools",
            "Read,Glob,Grep",
        ]
        .map(OsString::from)
    );
}

#[test]
fn grok_read_only_limits_tools_and_denies_writes() {
    let invocation = SurfaceInvocation {
        surface: Surface::Grok,
        binary: OsString::from("grok"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("/repo"),
        mode: AccessMode::ReadOnly,
        system_prompt: String::new(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: None,
    };

    assert_eq!(
        grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt")),
        [
            "--cwd",
            "/repo",
            "--prompt-file",
            "/scratch/prompt.txt",
            "--output-format",
            "json",
            "--no-memory",
            "--sandbox",
            "read-only",
            "--permission-mode",
            "dontAsk",
            "--tools",
            "Read,Glob,Grep",
            "--deny",
            "Write(**)",
            "--deny",
            "Edit(**)",
            "--deny",
            "Bash(**)",
        ]
        .map(OsString::from)
    );
}

#[test]
fn automated_providers_drop_read_only_bypass_arguments() {
    for (surface, argument) in [
        (Surface::Claude, "--dangerously-skip-permissions"),
        (
            Surface::Codex,
            "--config=sandbox_mode=\"danger-full-access\"",
        ),
        (Surface::Codex, "-sdanger-full-access"),
        (Surface::Codex, "-csandbox_mode=\"danger-full-access\""),
        (Surface::Codex, "-C/tmp"),
        (Surface::Codex, "-mgpt-5.6-sol"),
        (Surface::Grok, "--always-approve"),
        (Surface::Grok, "-mgrok-4"),
        (Surface::Agy, "--dangerously-skip-permissions"),
        (Surface::Opencode, "--attach=http://127.0.0.1:4096"),
        (Surface::Opencode, "-mproton-lumo/lumo-max"),
    ] {
        let filtered = filter_surface_args(&invocation(surface, &[argument]));
        assert!(
            filtered.kept.is_empty(),
            "{surface:?} kept {argument}: {:?}",
            filtered.kept
        );
        assert_eq!(
            filtered.warnings.len(),
            1,
            "{surface:?} {argument}: {:?}",
            filtered.warnings
        );
    }
}

#[test]
fn codex_failure_reports_its_own_error_events() {
    let events: Vec<CodexEvent> = parse_jsonl(concat!(
        "{\"type\":\"item.completed\",\"item\":{\"type\":\"error\",\"message\":\"hooks loaded twice\"}}\n",
        "{\"type\":\"error\",\"message\":\"You've hit your usage limit.\"}\n",
        "{\"type\":\"turn.failed\",\"error\":{\"message\":\"turn aborted\"}}",
    ));

    assert_eq!(
        codex_error_messages(&events),
        vec![
            "hooks loaded twice".to_string(),
            "You've hit your usage limit.".to_string(),
            "turn aborted".to_string(),
        ]
    );
    assert_eq!(
        with_surface_diagnostics(&codex_error_messages(&events), "cache warning\n"),
        "hooks loaded twice\nYou've hit your usage limit.\nturn aborted\ncache warning"
    );
}

#[test]
fn opencode_session_error_precedes_text_output() {
    let events: Vec<OpencodeEvent> = parse_jsonl(
        "{\"type\":\"session.error\",\"properties\":{\"error\":{\"data\":{\"message\":\"route failed\"}}}}",
    );

    assert_eq!(opencode_session_error(&events), Some("route failed"));
}

#[test]
fn clean_codex_config_keeps_only_route_fields() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.toml");
    let target = directory.path().join("clean.toml");
    std::fs::write(
        &source,
        "model_provider = \"proxy\"\nhooks = [\"secret\"]\n[model_providers.proxy]\nname = \"Proxy\"\nbase_url = \"http://localhost\"\nwire_api = \"responses\"\nenv_key = \"PROXY_KEY\"\nhttp_headers = { version = \"1\" }\nenv_http_headers = { authorization = \"PROXY_AUTH\" }\nquery_params = { beta = \"true\" }\n[plugins.test]\nenabled = true\n",
    )
    .expect("write config");

    copy_codex_route_config(&source, &target).expect("copy clean config");
    let clean = std::fs::read_to_string(target).expect("read clean config");

    assert!(clean.contains("model_provider = \"proxy\""));
    assert!(clean.contains("env_key = \"PROXY_KEY\""));
    assert!(clean.contains("http_headers"));
    assert!(clean.contains("env_http_headers"));
    assert!(clean.contains("query_params"));
    assert!(!clean.contains("hooks"));
    assert!(!clean.contains("plugins"));
}

#[test]
fn clean_codex_config_keeps_every_provider_table_and_a_builtin_provider_name() {
    // The live shape: a built-in provider at the top, and the proxy provider
    // that `codex-proxy` selects at launch, with its auth command.
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.toml");
    let target = directory.path().join("clean.toml");
    std::fs::write(
        &source,
        "model = \"gpt-6-astra\"\nmodel_provider = \"openai\"\n[memories]\ngenerate_memories = true\n[model_providers.cliproxyapi]\nbase_url = \"http://127.0.0.1:8317/v1\"\nname = \"cliproxyapi\"\nwire_api = \"responses\"\n[model_providers.cliproxyapi.auth]\ncommand = \"/usr/bin/sed\"\nargs = [\"-n\", \"s/^KEY=//p\", \"/home/me/.env\"]\n",
    )
    .expect("write config");

    copy_codex_route_config(&source, &target).expect("copy clean config");
    let clean: toml::Value = toml::from_str(&std::fs::read_to_string(target).expect("read"))
        .expect("clean config parses");

    assert_eq!(clean["model_provider"].as_str(), Some("openai"));
    assert_eq!(
        clean["model_providers"]["cliproxyapi"]["base_url"].as_str(),
        Some("http://127.0.0.1:8317/v1")
    );
    assert_eq!(
        clean["model_providers"]["cliproxyapi"]["auth"]["command"].as_str(),
        Some("/usr/bin/sed")
    );
    assert!(clean.get("model").is_none());
    assert!(clean.get("memories").is_none());
}

#[test]
fn clean_claude_overrides_system_prompt() {
    let invocation = SurfaceInvocation {
        surface: Surface::Claude,
        binary: OsString::from("claude"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("."),
        mode: AccessMode::ReadOnly,
        system_prompt: "Use this rule.".to_string(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: Some(PathBuf::from("/clean")),
    };
    let args = claude_args(&invocation);

    assert!(args.windows(2).any(|pair| pair
        == [
            OsString::from("--system-prompt"),
            OsString::from(format!("{CLEAN_SYSTEM_PROMPT}\n\nUse this rule.")),
        ]));
    assert!(!args.contains(&OsString::from("--append-system-prompt")));
}

#[test]
fn clean_claude_state_keeps_only_credentials() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let target = temporary.path().join("target");
    std::fs::create_dir_all(&source).expect("source directory");
    std::fs::create_dir_all(&target).expect("target directory");
    std::fs::write(source.join(".credentials.json"), "credentials").expect("credentials");
    std::fs::write(source.join("settings.json"), "settings").expect("settings");

    copy_optional_auth_file(
        &source.join(".credentials.json"),
        &target.join(".credentials.json"),
        "Claude",
    )
    .expect("clean Claude state");

    assert_eq!(
        std::fs::read_to_string(target.join(".credentials.json")).expect("copied credentials"),
        "credentials"
    );
    assert!(!target.join("settings.json").exists());
}

#[test]
fn clean_grok_uses_neutral_system_prompt() {
    let invocation = SurfaceInvocation {
        surface: Surface::Grok,
        binary: OsString::from("grok"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("/repo"),
        mode: AccessMode::ReadOnly,
        system_prompt: String::new(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: Some(PathBuf::from("/clean")),
    };
    let args = grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt"));

    assert!(args.contains(&OsString::from(format!(
        "--system-prompt-override={CLEAN_SYSTEM_PROMPT}"
    ))));
}

#[test]
fn grok_system_prompt_accepts_frontmatter() {
    let invocation = SurfaceInvocation {
        surface: Surface::Grok,
        binary: OsString::from("grok"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("/repo"),
        mode: AccessMode::ReadOnly,
        system_prompt: "---\nname: Example".to_string(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: None,
    };
    let args = grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt"));

    assert!(args.contains(&OsString::from(
        "--system-prompt-override=---\nname: Example"
    )));
}

#[test]
fn grok_json_returns_only_the_final_message_from_a_multi_turn_run() {
    let stdout = concat!(
        "{\"text\":\"Final rewritten text.\",",
        "\"thought\":\"I will read the source first.\",",
        "\"num_turns\":2,",
        "\"messages\":[\"I will read the source first.\",\"Final rewritten text.\"]}"
    );

    assert_eq!(
        grok_final_text(stdout).expect("final Grok response"),
        "Final rewritten text."
    );
}

#[test]
fn agy_json_returns_only_the_final_message_from_a_multi_turn_run() {
    let stdout = concat!(
        "{\"status\":\"SUCCESS\",",
        "\"response\":\"Final rewritten text.\",",
        "\"num_turns\":3,",
        "\"messages\":[\"I will read the source.\",\"Final rewritten text.\"],",
        "\"usage\":{\"output_tokens\":17}}"
    );
    let response: AgyResponse = serde_json::from_str(stdout).expect("valid Agy response");

    assert_eq!(response.response, "Final rewritten text.");
    assert_eq!(
        response.usage.and_then(|usage| usage.output_tokens),
        Some(17.0)
    );
}

#[test]
fn clean_state_redirects_codex_and_agy_data() {
    for (surface, key) in [
        (Surface::Codex, "CODEX_HOME"),
        (Surface::Agy, "ANTIGRAVITY_EXECUTABLE_DATA_DIR"),
        (Surface::Grok, "HOME"),
    ] {
        let invocation = SurfaceInvocation {
            surface,
            binary: OsString::from("surface"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("."),
            mode: AccessMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: Some(PathBuf::from("/clean")),
        };
        let request = process_request(&invocation, Vec::new(), None);

        assert!(
            request
                .env
                .contains(&(OsString::from(key), OsString::from("/clean")))
        );
    }
}

#[test]
fn clean_opencode_config_keeps_only_selected_provider() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("target.json");
    std::fs::write(
        &source,
        r#"{"$schema":"schema","instructions":["rules/*.md"],"plugin":["plugin"],"provider":{"proton-lumo":{"npm":"provider"},"other":{"npm":"other"}}}"#,
    )
    .expect("source configuration");

    copy_opencode_route_config(&source, &target, Some("proton-lumo/lumo-max"))
        .expect("clean configuration");
    let clean: serde_json::Value =
        serde_json::from_slice(&std::fs::read(target).expect("target configuration"))
            .expect("valid target JSON");

    assert_eq!(clean["$schema"], "schema");
    assert_eq!(clean["provider"]["proton-lumo"]["npm"], "provider");
    assert!(clean.get("instructions").is_none());
    assert!(clean.get("plugin").is_none());
    assert!(clean["provider"].get("other").is_none());
}

#[test]
fn clean_opencode_config_keeps_all_providers_without_selection() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source.json");
    std::fs::write(
        &source,
        r#"{"instructions":["rules/*.md"],"provider":{"proton-lumo":{"npm":"provider"},"other":{"npm":"other"}}}"#,
    )
    .expect("source configuration");

    for model in [None, Some("lumo-max")] {
        let target = temporary.path().join("target.json");
        copy_opencode_route_config(&source, &target, model).expect("clean configuration");
        let clean: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target).expect("target configuration"))
                .expect("valid target JSON");

        assert_eq!(clean["provider"]["proton-lumo"]["npm"], "provider");
        assert_eq!(clean["provider"]["other"]["npm"], "other");
        assert!(clean.get("instructions").is_none());
    }
}

#[test]
fn clean_opencode_redirects_xdg_state() {
    let invocation = SurfaceInvocation {
        surface: Surface::Opencode,
        binary: OsString::from("opencode"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("."),
        mode: AccessMode::ReadOnly,
        system_prompt: String::new(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: Some(PathBuf::from("/clean")),
    };
    let request = process_request(&invocation, Vec::new(), None);

    for (key, value) in [
        ("XDG_CONFIG_HOME", "/clean/config"),
        ("XDG_DATA_HOME", "/clean/data"),
        ("XDG_STATE_HOME", "/clean/state"),
    ] {
        assert!(
            request
                .env
                .contains(&(OsString::from(key), OsString::from(value)))
        );
    }
}

#[test]
fn clean_grok_names_the_real_install_directory_for_the_shim() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let home = temporary.path();
    assert!(grok_install_dir(home).is_none());

    std::fs::create_dir_all(home.join(".grok/bin")).expect("install directory");
    std::fs::write(home.join(".grok/bin/grok"), "#!/bin/sh\n").expect("grok binary");

    assert_eq!(
        grok_install_dir(home),
        Some(home.join(".grok/bin").into_os_string())
    );
}

#[test]
fn clean_grok_moves_home_after_the_shim_target() {
    let invocation = SurfaceInvocation {
        surface: Surface::Grok,
        binary: OsString::from("grok"),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: PathBuf::from("."),
        mode: AccessMode::ReadOnly,
        system_prompt: String::new(),
        prompt: "Inspect only".to_string(),
        model: None,
        native_timeout: None,
        timeout: None,
        clean_state_root: Some(PathBuf::from("/clean")),
    };
    let request = process_request(&invocation, Vec::new(), None);

    let home = request
        .env
        .iter()
        .position(|(key, _)| key == "HOME")
        .expect("HOME is set for the clean Grok run");
    assert_eq!(request.env[home].1, OsString::from("/clean"));
    if let Some(real_bin) = request
        .env
        .iter()
        .position(|(key, _)| key == "HARNESS_REAL_BIN_DIR")
    {
        assert!(
            real_bin < home,
            "the shim target is named before HOME moves"
        );
        assert!(
            request.env[real_bin]
                .1
                .to_string_lossy()
                .ends_with(".grok/bin")
        );
    }
}

#[test]
fn opencode_returns_only_the_last_assistant_message() {
    let events: Vec<OpencodeEvent> = parse_jsonl(concat!(
        "{\"type\":\"text\",\"part\":{\"messageID\":\"msg_1\",\"text\":\"I will read the source first.\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"messageID\":\"msg_1\",\"tokens\":{\"output\":12}}}\n",
        "{\"type\":\"text\",\"part\":{\"messageID\":\"msg_2\",\"text\":\"Final rewritten \"}}\n",
        "{\"type\":\"text\",\"part\":{\"messageID\":\"msg_2\",\"text\":\"text.\"}}",
    ));

    assert_eq!(
        opencode_final_text(&events).expect("final OpenCode response"),
        "Final rewritten text."
    );
}

#[test]
fn opencode_rejects_mixed_identified_and_unidentified_text() {
    let events: Vec<OpencodeEvent> = parse_jsonl(concat!(
        "{\"type\":\"text\",\"part\":{\"messageID\":\"msg_1\",\"text\":\"Earlier text.\"}}\n",
        "{\"type\":\"text\",\"part\":{\"text\":\"Unidentified final text.\"}}",
    ));

    assert_eq!(
        opencode_final_text(&events),
        Err(SurfaceFailure::Reported(
            "opencode text event has no messageID".to_string()
        ))
    );
}
