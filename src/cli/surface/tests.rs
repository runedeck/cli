use super::*;
use crate::cli::launch;

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
        session_name: None,
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
    assert_eq!(filtered.kept, [] as [std::ffi::OsString; 0]);
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
    assert_eq!(filtered.kept, [] as [std::ffi::OsString; 0]);
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
    assert_eq!(filtered.warnings, [] as [String; 0]);
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
        session_name: None,
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
        session_name: None,
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
    assert!(clean.contains("[model_providers.proxy]"));
    assert!(clean.contains("env_key = \"PROXY_KEY\""));
    assert!(clean.contains("http_headers"));
    assert!(clean.contains("env_http_headers"));
    assert!(clean.contains("query_params"));
    assert!(!clean.contains("hooks"));
    assert!(!clean.contains("plugins"));
}

#[test]
fn clean_codex_config_names_the_single_custom_provider_and_never_the_source_default() {
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

    assert_eq!(
        clean["model_provider"].as_str(),
        Some("cliproxyapi"),
        "one custom table names the route in the file, because codex drops a global --config once the subcommand has its own"
    );
    assert_eq!(
        clean["model_providers"]["cliproxyapi"]["base_url"].as_str(),
        Some("http://127.0.0.1:8317/v1")
    );
    assert_eq!(
        clean["model_providers"]["cliproxyapi"]["auth"]["command"].as_str(),
        Some("/usr/bin/sed")
    );
    assert_eq!(
        clean["model_providers"]["cliproxyapi"]["auth"]["args"]
            .as_array()
            .map(Vec::len),
        Some(3),
        "the auth command's arguments are the credential source"
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
        session_name: None,
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
        session_name: None,
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
        session_name: None,
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
            session_name: None,
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
        session_name: None,
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
        session_name: None,
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

#[test]
fn clean_codex_config_with_two_custom_providers_names_none() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.toml");
    let target = directory.path().join("clean.toml");
    std::fs::write(
        &source,
        "model_provider = \"openai\"\n[model_providers.a]\nbase_url = \"http://a\"\n[model_providers.b]\nbase_url = \"http://b\"\n",
    )
    .expect("write config");

    copy_codex_route_config(&source, &target).expect("copy clean config");
    let clean: toml::Value = toml::from_str(&std::fs::read_to_string(target).expect("read"))
        .expect("clean config parses");

    assert!(clean.get("model_provider").is_none(), "{clean}");
    assert!(clean["model_providers"].get("a").is_some());
    assert!(clean["model_providers"].get("b").is_some());
}

#[test]
fn claude_session_name_goes_as_the_name_flag() {
    let mut invocation = invocation(Surface::Claude, &[]);
    invocation.session_name = Some("claude/runedeck/mods/council".to_string());
    let args = strings(&claude_args(&invocation));
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--name", "claude/runedeck/mods/council"]),
        "{args:?}"
    );
}

#[test]
fn opencode_session_name_goes_as_the_title_flag() {
    let mut invocation = invocation(Surface::Opencode, &[]);
    invocation.session_name = Some("opencode/runedeck/mods/council".to_string());
    let args = strings(&opencode_args(&invocation));
    assert_eq!(args[0], "run");
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--title", "opencode/runedeck/mods/council"]),
        "{args:?}"
    );
}

#[test]
fn no_session_name_sends_no_name_flag() {
    let claude = strings(&claude_args(&invocation(Surface::Claude, &[])));
    assert!(!claude.iter().any(|arg| arg == "--name"), "{claude:?}");
    let opencode = strings(&opencode_args(&invocation(Surface::Opencode, &[])));
    assert!(!opencode.iter().any(|arg| arg == "--title"), "{opencode:?}");
}

#[test]
fn profile_name_and_title_are_owned_by_the_run() {
    let claude = filter_surface_args(&invocation(Surface::Claude, &["--name", "mine"]));
    assert_eq!(claude.kept, [] as [std::ffi::OsString; 0]);
    assert_eq!(claude.warnings.len(), 1, "{:?}", claude.warnings);
    let opencode = filter_surface_args(&invocation(Surface::Opencode, &["--title", "mine"]));
    assert_eq!(opencode.kept, [] as [std::ffi::OsString; 0]);
    assert_eq!(opencode.warnings.len(), 1, "{:?}", opencode.warnings);
}

#[test]
fn session_name_flag_names_the_two_harnesses_that_take_one() {
    assert_eq!(session_name_flag(Surface::Claude), Some("--name"));
    assert_eq!(session_name_flag(Surface::Opencode), Some("--title"));
    assert_eq!(session_name_flag(Surface::Codex), None);
    assert_eq!(session_name_flag(Surface::Agy), None);
    assert_eq!(session_name_flag(Surface::Grok), None);
}

#[test]
fn claude_answer_only_switches_every_tool_off() {
    let mut invocation = invocation(Surface::Claude, &[]);
    invocation.mode = AccessMode::AnswerOnly;
    let args = strings(&claude_args(&invocation));
    assert!(
        args.windows(2).any(|pair| pair == ["--tools", ""]),
        "{args:?}"
    );
    assert!(!args.iter().any(|arg| arg == "Read,Glob,Grep"), "{args:?}");
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--permission-mode", "plan"]),
        "{args:?}"
    );
}

#[test]
fn harnesses_that_cannot_drop_their_tools_refuse_answer_only() {
    for surface in [Surface::Codex, Surface::Grok, Surface::Agy] {
        let mut invocation = invocation(surface, &[]);
        invocation.mode = AccessMode::AnswerOnly;
        let refused = refuse_answer_only(&invocation).expect_err("must refuse");
        assert!(
            refused
                .to_string()
                .contains("answer-only mode needs a harness"),
            "{surface:?}: {refused}"
        );
    }
    let mut claude = invocation(Surface::Claude, &[]);
    claude.mode = AccessMode::AnswerOnly;
    assert!(refuse_answer_only(&claude).is_ok());
}

fn keys(names: &[&str]) -> Vec<OsString> {
    names.iter().map(OsString::from).collect()
}

fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

#[test]
fn desktop_session_variables_leave_the_child_and_the_owner_keys_stay() {
    let parent = keys(&[
        "PATH",
        "HOME",
        "CLAUDE_CONFIG_DIR",
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
        "CLAUDE_AGENT_SDK_VERSION",
        "CLAUDE_PID",
        "CLAUDE_PREVIEW_PORT",
    ]);
    let route = env(&[
        ("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317"),
        ("ANTHROPIC_AUTH_TOKEN", "route-token"),
        ("ANTHROPIC_MODEL", "gpt-5.6-sol"),
    ]);

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&route));

    assert_eq!(
        isolation.session,
        [
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
            "CLAUDE_AGENT_SDK_VERSION",
            "CLAUDE_PID",
            "CLAUDE_PREVIEW_PORT",
        ]
    );
    assert!(isolation.withheld.is_empty(), "{:?}", isolation.withheld);
}

#[test]
fn codex_session_variables_leave_the_child() {
    let parent = keys(&[
        "CODEX_SANDBOX",
        "CODEX_SANDBOX_NETWORK_DISABLED",
        "CODEX_THREAD_ID",
        "HARNESS_AUTOMATED",
        "CODEX_HOME",
    ]);

    let isolation = super::isolation::isolate(&parent, &[]);

    assert_eq!(
        strings(&isolation.removals()),
        [
            "CODEX_SANDBOX",
            "CODEX_SANDBOX_NETWORK_DISABLED",
            "CODEX_THREAD_ID",
            "HARNESS_AUTOMATED",
        ]
    );
}

#[test]
fn route_endpoint_without_a_credential_withholds_the_inherited_one() {
    let parent = keys(&[
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
    ]);
    let anthropic = env(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317")]);
    let openai = env(&[("OPENAI_BASE_URL", "http://127.0.0.1:8317/v1")]);

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&anthropic));
    assert_eq!(
        isolation.withheld,
        [
            (
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                "ANTHROPIC_BASE_URL".to_string()
            ),
            (
                "ANTHROPIC_API_KEY".to_string(),
                "ANTHROPIC_BASE_URL".to_string()
            ),
        ]
    );
    assert_eq!(
        strings(&isolation.removals()),
        ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"]
    );

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&openai));
    assert_eq!(
        isolation.withheld,
        [("OPENAI_API_KEY".to_string(), "OPENAI_BASE_URL".to_string())]
    );
}

#[test]
fn route_key_of_a_family_withholds_every_inherited_sibling() {
    // The route names its own endpoint and token; the inherited key of the
    // same family that the route does not set is withheld too.
    let parent = keys(&["ANTHROPIC_API_KEY", "OPENAI_API_KEY"]);
    let route = env(&[
        ("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317"),
        ("ANTHROPIC_AUTH_TOKEN", "route-token"),
    ]);

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&route));

    assert_eq!(
        isolation.withheld,
        [(
            "ANTHROPIC_API_KEY".to_string(),
            "ANTHROPIC_BASE_URL".to_string()
        )]
    );
}

#[test]
fn route_credential_without_an_endpoint_withholds_the_inherited_endpoint() {
    let parent = keys(&["ANTHROPIC_BASE_URL", "ANTHROPIC_API_KEY", "PATH"]);
    let route = env(&[("ANTHROPIC_AUTH_TOKEN", "route-token")]);

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&route));

    assert_eq!(
        isolation.withheld,
        [
            (
                "ANTHROPIC_BASE_URL".to_string(),
                "ANTHROPIC_AUTH_TOKEN".to_string()
            ),
            (
                "ANTHROPIC_API_KEY".to_string(),
                "ANTHROPIC_AUTH_TOKEN".to_string()
            ),
        ]
    );
}

#[test]
fn settings_env_keys_count_as_route_keys_for_claude() {
    let mut claude = invocation(
        Surface::Claude,
        &[
            "--verbose",
            "--settings",
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/settings"}}"#,
            r#"--settings={"env":{"OPENAI_API_KEY":"fake-settings-secret"}}"#,
        ],
    );
    claude.env = env(&[("ANTHROPIC_MODEL", "gpt-5.6-sol")]);
    let keys = route_keys(&claude);
    assert!(
        keys.contains(&OsString::from("ANTHROPIC_MODEL")),
        "{keys:?}"
    );
    assert!(
        keys.contains(&OsString::from("ANTHROPIC_BASE_URL")),
        "{keys:?}"
    );
    assert!(keys.contains(&OsString::from("OPENAI_API_KEY")), "{keys:?}");

    let mut codex = invocation(Surface::Codex, &["--settings", r#"{"env":{"X":"1"}}"#]);
    codex.env = Vec::new();
    assert_eq!(route_keys(&codex), [] as [std::ffi::OsString; 0]);
}

#[test]
fn native_claude_keeps_the_inherited_endpoint_and_credential_as_one_pair() {
    let parent = keys(&["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "CLAUDECODE"]);

    let isolation = super::isolation::isolate(&parent, &[]);

    assert_eq!(isolation.session, ["CLAUDECODE"]);
    assert!(isolation.withheld.is_empty(), "{:?}", isolation.withheld);
}

#[test]
fn route_keys_win_over_the_parent_session() {
    let parent = keys(&["CLAUDE_CODE_MAX_CONTEXT_TOKENS", "CLAUDE_CODE_ENTRYPOINT"]);
    let route = env(&[("CLAUDE_CODE_MAX_CONTEXT_TOKENS", "272000")]);

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&route));

    assert_eq!(isolation.session, ["CLAUDE_CODE_ENTRYPOINT"]);
}

#[test]
fn process_request_never_removes_a_key_the_route_sets() {
    let mut invocation = invocation(Surface::Claude, &[]);
    invocation.env = env(&[
        ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", "272000"),
        ("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317"),
    ]);
    let request = process_request(&invocation, Vec::new(), None);

    for (key, _) in &invocation.env {
        assert!(
            !request.env_remove.contains(key),
            "{:?}",
            request.env_remove
        );
    }
}

#[test]
fn exit_failure_names_the_exit_code_and_shows_both_streams() {
    let failure = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: "API Error: 400 {\"error\":\"unknown model\"}".to_string(),
        stderr_tail: "[claude-code:unrecognized_model] {\"model\":\"gpt-5.6-sol\"}".to_string(),
    };
    let text = failure.to_string();

    assert!(text.starts_with("process Exited(1)"), "{text}");
    assert!(text.contains("stdout tail:\n  API Error: 400"), "{text}");
    assert!(
        text.contains("stderr tail:\n  [claude-code:unrecognized_model]"),
        "{text}"
    );
    assert!(!text.contains("hint:"), "{text}");
}

#[test]
fn output_tail_keeps_the_last_lines_or_characters_whichever_is_shorter() {
    let fifty_lines = (1..=50)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tail = output_tail(&fifty_lines);
    assert!(tail.starts_with("line 11\n"), "{tail}");
    assert!(tail.ends_with("line 50"), "{tail}");
    assert_eq!(tail.lines().count(), 40);

    let long_line = "x".repeat(4100);
    assert_eq!(output_tail(&long_line).chars().count(), 4000);
    assert_eq!(output_tail("short\n\n"), "short");
}

#[test]
fn timeout_failure_carries_the_partial_output() {
    let failure = SurfaceFailure::Process(ProcessFailure::Timeout {
        timeout: Duration::from_secs(30),
        stdout: "thinking...\n".to_string(),
        stderr: String::new(),
    });
    let text = failure.to_string();

    assert!(
        text.starts_with("process timed out after 30 seconds"),
        "{text}"
    );
    assert!(text.contains("stdout tail:\n  thinking..."), "{text}");
    assert!(!text.contains("stderr tail:"), "{text}");
    assert_eq!(failure.tails(), Some(("thinking...\n", "")));
}

#[test]
fn clean_exit_without_an_answer_carries_the_output() {
    let output = ProcessOutput {
        termination: ProcessTermination::Exited(0),
        stdout: "   \n".to_string(),
        stderr: "model refused".to_string(),
    };
    let failure = nonempty_text(&output.stdout, "claude print")
        .expect_err("empty answer")
        .with_output(&output, &[]);

    assert_eq!(
        failure,
        SurfaceFailure::NoAnswer {
            message: "claude print produced no final response".to_string(),
            stdout_tail: String::new(),
            stderr_tail: "model refused".to_string(),
        }
    );
    assert!(
        failure
            .to_string()
            .contains("stderr tail:\n  model refused"),
        "{failure}"
    );
    let untouched = SurfaceFailure::Io("disk".to_string()).with_output(&output, &[]);
    assert_eq!(untouched, SurfaceFailure::Io("disk".to_string()));
}

#[test]
fn sandbox_signature_in_the_output_adds_a_hint() {
    let certificate = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: "TLS handshake failed: OSStatus -26276".to_string(),
    };
    let hint = certificate.hint().expect("certificate hint");
    assert!(hint.contains("certificate"), "{hint}");
    assert!(
        certificate.to_string().contains(&format!("\nhint: {hint}")),
        "{certificate}"
    );

    let keyring = SurfaceFailure::NoAnswer {
        message: "codex exec produced no final agent message".to_string(),
        stdout_tail: "failed to load CLI auth from keyring".to_string(),
        stderr_tail: String::new(),
    };
    assert!(keyring.hint().is_some_and(|hint| hint.contains("keyring")));

    let node_tls = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: "API Error: Unable to connect to API: SSL certificate verification failed (UNABLE_TO_VERIFY_LEAF_SIGNATURE)".to_string(),
        stderr_tail: String::new(),
    };
    assert!(
        node_tls
            .hint()
            .is_some_and(|hint| hint.contains("filtering proxy"))
    );

    let spawn = SurfaceFailure::Process(ProcessFailure::Spawn(
        "claude not runnable: Operation not permitted (os error 1)".to_string(),
    ));
    assert!(spawn.hint().is_some_and(|hint| hint.contains("sandbox")));

    let plain = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(2),
        stdout_tail: "usage: claude".to_string(),
        stderr_tail: String::new(),
    };
    assert_eq!(plain.hint(), None);
}

#[test]
fn planned_request_uses_the_spawn_builders_with_placeholders() {
    let mut claude = invocation(Surface::Claude, &["--verbose"]);
    claude.system_prompt = "Use the rule.".to_string();
    let planned = planned_request(&claude).expect("claude plan");
    let mut expected = invocation(Surface::Claude, &["--verbose"]);
    expected.prompt = "<prompt>".to_string();
    expected.system_prompt = "<system-prompt>".to_string();
    assert_eq!(planned.args, claude_args(&expected));
    assert_eq!(planned.stdin, Some(b"<prompt>\n".to_vec()));
    assert!(
        !planned
            .args
            .iter()
            .any(|arg| arg.to_string_lossy().contains("Use the rule")),
        "{:?}",
        planned.args
    );

    let grok = planned_request(&invocation(Surface::Grok, &[])).expect("grok plan");
    assert!(
        strings(&grok.args)
            .windows(2)
            .any(|pair| pair == ["--prompt-file", "<scratch>/prompt.txt"]),
        "{:?}",
        grok.args
    );
    assert_eq!(grok.stdin, None);

    let agy = planned_request(&invocation(Surface::Agy, &[])).expect("agy plan");
    assert!(
        strings(&agy.args)
            .windows(2)
            .any(|pair| pair == ["--print", "<prompt>"]),
        "{:?}",
        agy.args
    );

    let codex = planned_request(&invocation(Surface::Codex, &[])).expect("codex plan");
    assert!(
        strings(&codex.args)
            .windows(2)
            .any(|pair| pair == ["-o", "<scratch>/last-message.txt"]),
        "{:?}",
        codex.args
    );
    assert_eq!(codex.stdin, Some(b"<prompt>\n".to_vec()));

    let opencode = planned_request(&invocation(Surface::Opencode, &[])).expect("opencode plan");
    assert!(
        opencode
            .env
            .iter()
            .any(|(key, _)| key == "OPENCODE_PERMISSION"),
        "{:?}",
        opencode.env
    );
}

#[test]
fn planned_request_refuses_what_the_run_refuses() {
    let mut codex = invocation(Surface::Codex, &[]);
    codex.mode = AccessMode::AnswerOnly;
    let refused = planned_request(&codex).expect_err("answer-only codex");
    assert!(
        refused
            .to_string()
            .contains("answer-only mode needs a harness"),
        "{refused}"
    );
}

#[test]
fn opencode_permission_follows_the_mode() {
    let mut opencode = invocation(Surface::Opencode, &[]);
    assert!(
        process_request(&opencode, Vec::new(), None)
            .env
            .contains(&(
                OsString::from("OPENCODE_PERMISSION"),
                OsString::from(
                    r#"{"*":"deny","read":"allow","glob":"allow","grep":"allow","list":"allow","lsp":"allow","skill":"allow"}"#
                )
            ))
    );
    opencode.mode = AccessMode::AnswerOnly;
    assert!(process_request(&opencode, Vec::new(), None).env.contains(&(
        OsString::from("OPENCODE_PERMISSION"),
        OsString::from(r#"{"*":"deny"}"#)
    )));
    opencode.mode = AccessMode::WorkspaceWrite;
    assert!(
        !process_request(&opencode, Vec::new(), None)
            .env
            .iter()
            .any(|(key, _)| key == "OPENCODE_PERMISSION")
    );
}

#[test]
fn isolation_lines_name_the_withheld_credential_and_its_endpoint() {
    let parent = keys(&["CLAUDECODE", "ANTHROPIC_API_KEY"]);
    let route = env(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317")]);

    let lines = super::isolation::isolate(&parent, &launch::env_keys(&route)).lines();

    assert_eq!(
        lines,
        [
            "CLAUDECODE",
            "ANTHROPIC_API_KEY (inherited, withheld: the route sets ANTHROPIC_BASE_URL of the same family)",
        ]
    );
}

#[test]
fn clean_state_bridges_an_absent_authentication_file_as_nothing() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let target = temporary.path().join("auth.json");

    copy_optional_auth_file(&temporary.path().join("missing/auth.json"), &target, "Grok")
        .expect("an absent file stops nothing");

    assert!(!target.exists());
}

fn claude_settings_in(args: &[String]) -> Vec<serde_json::Value> {
    args.windows(2)
        .filter(|pair| pair[0] == "--settings")
        .map(|pair| serde_json::from_str(&pair[1]).expect("inline settings JSON"))
        .collect()
}

#[test]
fn claude_workspace_write_names_its_tools_and_sandboxes_the_shell() {
    let mut invocation = invocation(Surface::Claude, &[]);
    invocation.mode = AccessMode::WorkspaceWrite;
    let args = strings(&claude_args(&invocation));

    assert!(
        args.windows(2)
            .any(|pair| pair == ["--permission-mode", "acceptEdits"]),
        "{args:?}"
    );
    assert!(
        args.windows(2).any(|pair| pair
            == [
                "--allowedTools",
                "Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit,Bash"
            ]),
        "{args:?}"
    );
    let settings = claude_settings_in(&args);
    assert_eq!(settings.len(), 1, "{args:?}");
    assert_eq!(settings[0]["sandbox"]["enabled"], true);
    assert_eq!(settings[0]["sandbox"]["autoAllowBashIfSandboxed"], true);
}

#[test]
fn claude_merges_the_profile_settings_into_the_one_settings_object() {
    let mut invocation = invocation(
        Surface::Claude,
        &["--verbose", "--settings", r#"{"env":{"X":"1"}}"#],
    );
    invocation.mode = AccessMode::WorkspaceWrite;
    let args = strings(&claude_args(&invocation));

    assert!(args.contains(&"--verbose".to_string()), "{args:?}");
    let settings = claude_settings_in(&args);
    assert_eq!(settings.len(), 1, "{args:?}");
    assert_eq!(settings[0]["env"]["X"], "1");
    assert_eq!(settings[0]["sandbox"]["enabled"], true);

    // The attached form merges too, and read-only adds no sandbox.
    let read_only = invocation_with_attached_settings();
    let args = strings(&claude_args(&read_only));
    let settings = claude_settings_in(&args);
    assert_eq!(settings.len(), 1, "{args:?}");
    assert_eq!(settings[0]["env"]["X"], "1");
    assert!(settings[0].get("sandbox").is_none(), "{args:?}");
    assert!(
        !args.iter().any(|arg| arg.starts_with("--settings=")),
        "{args:?}"
    );
}

fn invocation_with_attached_settings() -> SurfaceInvocation {
    invocation(Surface::Claude, &[r#"--settings={"env":{"X":"1"}}"#])
}

#[test]
fn claude_without_settings_sends_none_outside_workspace_write() {
    let args = strings(&claude_args(&invocation(Surface::Claude, &[])));
    assert!(!args.iter().any(|arg| arg == "--settings"), "{args:?}");
}

#[test]
fn codex_profile_model_equal_to_the_run_model_is_dropped_without_a_warning() {
    let mut same = invocation(Surface::Codex, &["-c", "model=\"gpt-5.6-sol\"", "--search"]);
    same.model = Some("gpt-5.6-sol".to_string());
    let filtered = filter_surface_args(&same);
    assert_eq!(strings(&filtered.kept), ["--search"]);
    assert_eq!(filtered.warnings, [] as [String; 0]);

    let mut different = invocation(Surface::Codex, &["-c", "model=\"gpt-6-astra\""]);
    different.model = Some("gpt-5.6-sol".to_string());
    let filtered = filter_surface_args(&different);
    assert_eq!(filtered.kept, [] as [std::ffi::OsString; 0]);
    assert_eq!(filtered.warnings.len(), 1, "{:?}", filtered.warnings);
    assert!(filtered.warnings[0].contains("config key model"));

    let none = invocation(Surface::Codex, &["-c", "model=\"gpt-5.6-sol\""]);
    assert_eq!(filter_surface_args(&none).warnings.len(), 1);
}

#[test]
fn codex_config_string_reads_a_toml_string_or_a_bare_value() {
    assert_eq!(codex_config_string("\"gpt-5.6-sol\""), "gpt-5.6-sol");
    assert_eq!(codex_config_string("'gpt-5.6-sol'"), "gpt-5.6-sol");
    assert_eq!(codex_config_string(" gpt-5.6-sol "), "gpt-5.6-sol");
}

#[test]
fn claude_settings_model_equal_to_the_run_model_is_dropped_without_a_warning() {
    let mut same = invocation(
        Surface::Claude,
        &["--settings", r#"{"model":"gpt-5.6-sol","env":{"X":"1"}}"#],
    );
    same.model = Some("gpt-5.6-sol".to_string());
    let filtered = filter_surface_args(&same);
    assert_eq!(filtered.warnings, [] as [String; 0]);
    let kept = strings(&filtered.kept);
    let settings: serde_json::Value = serde_json::from_str(&kept[1]).expect("json");
    assert!(settings.get("model").is_none());
    assert_eq!(settings["env"]["X"], "1");

    let mut only_model = invocation(
        Surface::Claude,
        &["--settings", r#"{"model":"gpt-5.6-sol"}"#],
    );
    only_model.model = Some("gpt-5.6-sol".to_string());
    let filtered = filter_surface_args(&only_model);
    assert_eq!(filtered.kept, [] as [std::ffi::OsString; 0]);
    assert_eq!(filtered.warnings, [] as [String; 0]);

    let mut different = invocation(Surface::Claude, &["--settings", r#"{"model":"opus"}"#]);
    different.model = Some("gpt-5.6-sol".to_string());
    let filtered = filter_surface_args(&different);
    assert_eq!(filtered.warnings.len(), 2, "{:?}", filtered.warnings);
}

#[test]
fn drop_warnings_name_the_key_and_never_the_value() {
    let codex = filter_surface_args(&invocation(
        Surface::Codex,
        &[
            "-c",
            "mcp_servers.x.env.TOKEN=\"fake-mcp-secret\"",
            "-c",
            "fake-bare-secret",
        ],
    ));
    assert_eq!(codex.warnings.len(), 2, "{:?}", codex.warnings);
    assert!(codex.warnings[0].contains("config key mcp_servers.x.env.TOKEN"));
    assert!(codex.warnings[0].contains("<redacted>"));
    for warning in &codex.warnings {
        assert!(!warning.contains("fake-"), "{warning}");
    }

    let claude = filter_surface_args(&invocation(
        Surface::Claude,
        &[
            "--settings",
            r#"{"hooks":{"PreToolUse":[{"command":"fake-hook-secret"}]},"env":{"X":"1"}}"#,
        ],
    ));
    assert_eq!(claude.warnings.len(), 1, "{:?}", claude.warnings);
    assert!(claude.warnings[0].contains("settings key hooks"));
    assert!(!claude.warnings[0].contains("fake-hook-secret"));
}

#[test]
fn child_secrets_hold_the_route_credentials_longest_first() {
    let mut invocation = invocation(Surface::Claude, &[]);
    invocation.env = env(&[
        ("ANTHROPIC_AUTH_TOKEN", "fake-r"),
        ("ANTHROPIC_API_KEY", "fake-route-secret-long"),
        ("ANTHROPIC_MODEL", "gpt-5.6-sol"),
        ("X_SECRET", "ab"),
    ]);
    let secrets = child_secrets(&invocation);

    // The test process may carry longer inherited credentials of its own,
    // so the order is checked between the route's two values only.
    let long = secrets
        .iter()
        .position(|secret| secret == "fake-route-secret-long")
        .expect("route credential");
    let short = secrets
        .iter()
        .position(|secret| secret == "fake-r")
        .expect("short route credential");
    assert!(long < short, "{secrets:?}");
    assert!(!secrets.contains(&"gpt-5.6-sol".to_string()));
    assert!(!secrets.contains(&"ab".to_string()), "too short to be one");
}

#[test]
fn failure_tails_hide_the_credential_before_the_tail_is_cut() {
    let secrets = vec!["fake-route-secret".to_string()];
    let output = ProcessOutput {
        termination: ProcessTermination::Exited(1),
        stdout: "auth fake-route-secret rejected".to_string(),
        stderr: "token fake-route-secret".to_string(),
    };
    let exit = require_success(output.clone(), &secrets).expect_err("exit 1");
    assert_eq!(
        exit,
        SurfaceFailure::Exit {
            termination: ProcessTermination::Exited(1),
            stdout_tail: "auth <redacted> rejected".to_string(),
            stderr_tail: "token <redacted>".to_string(),
        }
    );
    let silent = SurfaceFailure::Reported("no answer".to_string()).with_output(&output, &secrets);
    assert!(
        !silent.to_string().contains("fake-route-secret"),
        "{silent}"
    );

    let timeout = SurfaceFailure::Process(ProcessFailure::Timeout {
        timeout: Duration::from_secs(1),
        stdout: "partial fake-route-secret".to_string(),
        stderr: String::new(),
    })
    .redacted(&secrets);
    assert!(
        !timeout.to_string().contains("fake-route-secret"),
        "{timeout}"
    );
    assert!(
        timeout.to_string().contains("partial <redacted>"),
        "{timeout}"
    );
}
