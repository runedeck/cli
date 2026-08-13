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
    };

    assert_eq!(
        grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt")),
        [
            "--cwd",
            "/repo",
            "--prompt-file",
            "/scratch/prompt.txt",
            "--output-format",
            "plain",
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
