use super::*;

#[test]
fn automated_provider_rejects_profile_owned_flags() {
    let invocation = SurfaceInvocation {
        surface: Surface::Codex,
        binary: OsString::from("codex"),
        extra_args: vec![OsString::from("--sandbox=workspace-write")],
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
            reject_owned_args(&invocation, &["-s", "--sandbox"]),
            Err(SurfaceFailure::Arguments(
                "automated codex execution owns these profile arguments: --sandbox=workspace-write; remove them from the launch profile".to_string()
            ))
        );
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
fn automated_providers_reject_read_only_bypass_arguments() {
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
        let invocation = SurfaceInvocation {
            surface,
            binary: OsString::from("missing-surface-binary"),
            extra_args: vec![OsString::from(argument)],
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

        assert!(
            matches!(
                invoke_surface(&invocation),
                Err(SurfaceFailure::Arguments(_))
            ),
            "{surface:?} accepted {argument}"
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

    assert!(args.contains(&OsString::from(format!(
        "--system-prompt-override={CLEAN_SYSTEM_PROMPT}\n\n---\nname: Example"
    ))));
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
