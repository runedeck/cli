use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn context(root: &Path, launch: Launch, extensions: Vec<PathBuf>) -> LaunchContext {
    LaunchContext {
        cwd: root.to_path_buf(),
        root: root.to_path_buf(),
        config: ontology::ResolvedConfig {
            deck: None,
            env: None,
            ontology: ontology::ResolvedOntology::default(),
            extensions,
            setup: None,
            launch,
            hooks: rune::hooks::Hooks::default(),
            bench: Vec::new(),

            theme: None,
        },
    }
}

fn format_dry_run(tool: &ResolvedTool, argv: &[OsString], plan: &LaunchPlan) -> String {
    format_dry_run_parts(
        &DryRunParts {
            tool: &tool.name,
            argv,
            env: &final_env(tool, plan),
            env_removed: &[],
            base_url: plan.base_url.as_deref(),
            model: None,
            model_override: None,
            warnings: &plan.warnings,
        },
        &plan.wrap,
        &plan.pre,
    )
}

fn names(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    values
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

#[cfg(unix)]
fn write_executable(path: &Path, content: &str) {
    std::fs::write(path, content).expect("write script");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("permissions");
}

#[test]
fn shell_join_preserves_empty_args_and_quotes_values() {
    let argv = vec![
        OsString::from("a"),
        OsString::from(""),
        OsString::from("b"),
        OsString::from("two words"),
    ];

    assert_eq!(shell_join(&argv), "a '' b 'two words'");
}

#[test]
fn plan_composes_otel_then_pxpipe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let launch = Launch::default();
    let middleware = vec!["otel".to_string(), "pxpipe".to_string()];

    let plan =
        compose_plan(&middleware, None, &context(dir.path(), launch, Vec::new())).expect("compose");

    assert_eq!(
        plan.env,
        names(&[
            ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4318"),
            ("OTEL_SERVICE_NAME", "rune-launch"),
        ])
    );
    assert_eq!(plan.base_url.as_deref(), Some("http://127.0.0.1:47821"));
    assert_eq!(plan.pre.len(), 1);
    assert_eq!(plan.pre[0].name, "pxpipe");
    assert_eq!(plan.pre[0].command, vec!["pxpipe"]);
}

#[test]
fn base_url_conflict_warns_and_last_wins() {
    let dir = tempfile::tempdir().expect("tempdir");
    let middleware = vec!["presidio".to_string(), "pxpipe".to_string()];

    let plan = compose_plan(
        &middleware,
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");

    assert_eq!(plan.base_url.as_deref(), Some("http://127.0.0.1:47821"));
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("overrides base_url"));
    assert!(plan.warnings[0].contains("pxpipe"));
}

#[test]
fn tmux_flag_adds_outer_wrap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["otel".to_string()],
        Some("work"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(
        &tool,
        &[OsString::from("--model"), OsString::from("opus")],
        &plan,
        &[],
    );

    assert_eq!(plan.wrap.len(), 1);
    assert_eq!(
        plan.wrap[0],
        vec![
            OsString::from("tmux"),
            OsString::from("new-session"),
            OsString::from("-A"),
            OsString::from("-s"),
            OsString::from("work"),
        ]
    );
    assert_eq!(argv[0], OsString::from("tmux"));
    assert!(
        argv.last()
            .expect("tmux shell command")
            .to_string_lossy()
            .contains("claude --model opus")
    );
}

#[test]
fn docker_wrap_forwards_final_env_to_container() {
    let dir = tempfile::tempdir().expect("tempdir");
    let middleware = vec!["docker".to_string(), "pxpipe".to_string()];
    let plan = compose_plan(
        &middleware,
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[], &plan, &[]);

    let rendered = display_argv(&argv);
    assert!(rendered.contains("-e ANTHROPIC_BASE_URL=http://127.0.0.1:47821"));
    assert!(rendered.contains("ghcr.io/runedeck/rune-coding-tool:latest claude"));
}

#[test]
fn tmux_wrap_forwards_final_env_to_inner_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["pxpipe".to_string()],
        Some("work"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan, &[]);
    let inner = argv.last().expect("tmux shell command").to_string_lossy();

    assert!(inner.contains("env ANTHROPIC_BASE_URL=http://127.0.0.1:47821 claude --version"));
}

#[test]
fn unwrapped_launch_keeps_env_out_of_argv() {
    let mut plan = LaunchPlan {
        base_url: Some("http://127.0.0.1:47821".to_string()),
        ..LaunchPlan::default()
    };
    plan.env.push((
        OsString::from("OTEL_SERVICE_NAME"),
        OsString::from("rune-launch"),
    ));
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan, &[]);

    assert_eq!(
        argv,
        vec![OsString::from("claude"), OsString::from("--version")]
    );
    assert_eq!(
        process_env(&tool, &plan),
        names(&[
            ("OTEL_SERVICE_NAME", "rune-launch"),
            ("ANTHROPIC_BASE_URL", "http://127.0.0.1:47821"),
        ])
    );
}

#[test]
fn wrapped_launch_does_not_apply_env_to_wrapper_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["docker".to_string(), "pxpipe".to_string()],
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());

    assert_eq!(
        process_env(&tool, &plan),
        [] as [(std::ffi::OsString, std::ffi::OsString); 0]
    );
}

#[test]
fn dry_run_prints_plan_without_launching() {
    let dir = tempfile::tempdir().expect("tempdir");
    let middleware = vec!["otel".to_string(), "pxpipe".to_string()];
    let plan = compose_plan(
        &middleware,
        Some("x"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(
        &tool,
        &[OsString::from("--model"), OsString::from("opus")],
        &plan,
        &[],
    );
    let output = format_dry_run(&tool, &argv, &plan);

    assert!(output.contains("tool: claude"));
    assert!(output.contains("OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318"));
    assert!(output.contains("ANTHROPIC_BASE_URL=http://127.0.0.1:47821"));
    assert!(output.contains("base_url: http://127.0.0.1:47821"));
    assert!(output.contains("tmux new-session -A -s x"));
    assert!(output.contains("pxpipe 127.0.0.1:47821"));
}

#[test]
fn resolved_launch_preserves_interactive_arguments_and_dry_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved = resolve_with_config(
        "claude",
        &[OsString::from("--resume")],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        ontology::ResolvedConfig {
            deck: None,
            env: None,
            ontology: ontology::ResolvedOntology::default(),
            extensions: Vec::new(),
            setup: None,
            launch: Launch::default(),
            hooks: rune::hooks::Hooks::default(),
            bench: Vec::new(),

            theme: None,
        },
    )
    .expect("resolved launch");

    assert_eq!(
        resolved.argv,
        vec![OsString::from("claude"), OsString::from("--resume")]
    );
    assert_eq!(
        resolved.format_dry_run(&surface::Isolation::default(), &[]),
        "tool: claude\nargv: claude --resume\nenv:\nenv_removed:\nbase_url: <none>\nwrap:\npre:"
    );
}

#[test]
fn unwrapped_launch_dry_run_is_stable() {
    let tool = resolve_tool("claude", &Launch::default());
    let plan = LaunchPlan::default();
    let argv = build_argv(&tool, &[OsString::from("--resume")], &plan, &[]);

    assert_eq!(
        format_dry_run(&tool, &argv, &plan),
        "tool: claude\nargv: claude --resume\nenv:\nenv_removed:\nbase_url: <none>\nwrap:\npre:"
    );
}

#[test]
fn dry_run_redacts_credential_env_values_everywhere() {
    let mut plan = LaunchPlan::default();
    plan.env.push((
        OsString::from("ANTHROPIC_API_KEY"),
        OsString::from("sk-manual-test-credential"),
    ));
    plan.env.push((
        OsString::from("ANTHROPIC_MODEL"),
        OsString::from("gpt-5.6-sol"),
    ));
    plan.env.push((
        OsString::from("CLAUDE_CODE_MAX_CONTEXT_TOKENS"),
        OsString::from("270000"),
    ));
    plan.wrap
        .push(vec![OsString::from("tmux"), OsString::from("new-session")]);
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[], &plan, &[]);
    let output = format_dry_run(&tool, &argv, &plan);

    assert!(!output.contains("sk-manual-test-credential"), "{output}");
    assert!(output.contains("ANTHROPIC_API_KEY=<redacted>"), "{output}");
    assert!(output.contains("ANTHROPIC_MODEL=gpt-5.6-sol"), "{output}");
    assert!(
        output.contains("CLAUDE_CODE_MAX_CONTEXT_TOKENS=270000"),
        "{output}"
    );
}

#[test]
fn merge_patch_rejects_sensitive_env_and_keeps_normal_env() {
    let mut plan = LaunchPlan::default();
    merge_patch(
        &mut plan,
        "external",
        PlanPatch {
            env: vec![
                ("LD_PRELOAD".to_string(), "/tmp/hijack.dylib".to_string()),
                (
                    "OTEL_RESOURCE_ATTRIBUTES".to_string(),
                    "service.name=rune".to_string(),
                ),
            ],
            ..PlanPatch::default()
        },
    );

    assert_eq!(
        plan.env,
        names(&[("OTEL_RESOURCE_ATTRIBUTES", "service.name=rune")])
    );
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("sensitive env LD_PRELOAD"));
    assert!(plan.warnings[0].contains("dropping"));
}

#[test]
fn external_base_url_from_none_warns() {
    let mut plan = LaunchPlan::default();
    merge_patch(
        &mut plan,
        "external",
        PlanPatch {
            base_url: Some("https://api.example.invalid".to_string()),
            ..PlanPatch::default()
        },
    );

    assert_eq!(
        plan.base_url.as_deref(),
        Some("https://api.example.invalid")
    );
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("external middleware 'external'"));
    assert!(plan.warnings[0].contains("https://api.example.invalid"));
}

#[test]
fn unknown_middleware_without_script_errors_with_known_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = compose_plan(
        &["missing".to_string()],
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect_err("unknown middleware");

    assert!(error.contains("unknown middleware 'missing'"));
    assert!(error.contains("pxpipe, otel, presidio, squid, cliproxy, docker, tmux"));
}

#[test]
fn cliproxy_adds_a_health_check_pre_step_without_touching_env_or_base_url() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["cliproxy".to_string()],
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");

    assert_eq!(plan.pre.len(), 1);
    assert_eq!(plan.pre[0].name, "cliproxy");
    assert_eq!(plan.pre[0].host, "127.0.0.1");
    assert_eq!(plan.pre[0].port, 8317);
    assert!(
        plan.pre[0].command.is_empty(),
        "default is check-only; auto-start is opt-in config"
    );
    assert!(plan.pre[0].optional);
    assert!(plan.pre[0].log_path.is_none());
    assert!(plan.env.is_empty(), "profile owns env, not the middleware");
    assert!(plan.base_url.is_none(), "profile owns the base url");
}

#[test]
fn cliproxy_start_command_is_split_into_argv_when_configured() {
    let mut launch = Launch::default();
    launch.middleware.cliproxy.command = "brew services run cliproxyapi".to_string();
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["cliproxy".to_string()],
        None,
        &context(dir.path(), launch, Vec::new()),
    )
    .expect("compose");

    assert_eq!(
        plan.pre[0].command,
        vec!["brew", "services", "run", "cliproxyapi"]
    );
}

#[cfg(unix)]
#[test]
fn external_middleware_patch_is_merged_from_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let extension = tempfile::tempdir().expect("extension");
    let script = extension.path().join("rune-launch-mw-test");
    write_executable(
        &script,
        "#!/usr/bin/env bash\ncat >/dev/null\nprintf '%s\\n' '{\"env\":[[\"TEST_MW\",\"enabled\"]]}'\n",
    );

    let plan = compose_plan(
        &["test".to_string()],
        None,
        &context(
            dir.path(),
            Launch::default(),
            vec![extension.path().to_path_buf()],
        ),
    )
    .expect("compose external");

    assert_eq!(plan.env, names(&[("TEST_MW", "enabled")]));
}

#[cfg(unix)]
#[test]
fn external_middleware_oversized_stdout_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("rune-launch-mw-large");
    write_executable(
        &script,
        "#!/usr/bin/env bash\ncat >/dev/null\nprintf '%1048577s' ''\n",
    );

    let error =
        run_external_middleware(&script, &LaunchPlan::default()).expect_err("oversized stdout");

    assert!(error.contains("stdout exceeded 1048576 byte limit"));
}

#[test]
fn pxpipe_pre_step_command_with_args_builds_argv() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut launch = Launch::default();
    launch.middleware.pxpipe.command = "pxpipe --port 47821".to_string();
    launch.middleware.pxpipe.log_path = dir.path().join("pxpipe.log").to_string_lossy().to_string();
    let plan = compose_plan(
        &["pxpipe".to_string()],
        None,
        &context(dir.path(), launch, Vec::new()),
    )
    .expect("compose");

    assert_eq!(plan.pre[0].command, vec!["pxpipe", "--port", "47821"]);
    let command = build_pre_step_command(&plan.pre[0])
        .expect("pre-step command")
        .expect("configured command");
    let args = command.get_args().map(OsString::from).collect::<Vec<_>>();

    assert_eq!(command.get_program(), OsStr::new("pxpipe"));
    assert_eq!(
        args,
        vec![OsString::from("--port"), OsString::from("47821")]
    );
}

#[test]
fn profile_resolution_errors_with_known_names() {
    let mut launch = Launch::default();
    launch.profiles.insert(
        "claude".to_string(),
        [("sol".to_string(), ontology::LaunchProfile::default())]
            .into_iter()
            .collect(),
    );

    let error = resolve_profile("claude", Some("missing"), &launch).unwrap_err();
    assert!(
        error.message.contains("no launch profile 'missing'"),
        "{error}"
    );
    assert!(
        error.message.contains("sol"),
        "error must list known names: {error}"
    );

    let found = resolve_profile("claude", Some("sol"), &launch).unwrap();
    assert!(found.is_some());
}

#[test]
fn cursor_has_a_built_in_fable_profile_that_names_the_model_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved = resolve_with_config_for_tests(
        "fable@cursor",
        &[
            OsString::from("--endpoint"),
            OsString::from("https://api.example.test"),
        ],
        dir.path(),
    );
    let argv: Vec<String> = resolved
        .argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        argv,
        [
            "cursor-agent",
            "--model",
            "claude-fable-5-1-high",
            "--endpoint",
            "https://api.example.test",
        ]
    );
    let model = resolved.model.as_ref().expect("resolved model");
    assert_eq!(model.alias, "fable-cursor");
    assert_eq!(model.id, "claude-fable-5-1-high");
    assert_eq!(model.context, 1_000_000);
    assert_eq!(model.source, ModelSource::BuiltIn);
    assert_eq!(
        resolved.model_args,
        [
            OsString::from("--model"),
            OsString::from("claude-fable-5-1-high")
        ]
    );
    // The model travels on the command line, not in Claude's variables.
    assert!(
        !resolved
            .effective_env()
            .iter()
            .any(|(key, _)| key == "ANTHROPIC_MODEL"),
        "{:?}",
        resolved.effective_env()
    );

    let profile = resolve_profile("cursor", Some("fable"), &Launch::default())
        .expect("built-in profile")
        .expect("found");
    assert_eq!(profile.model.as_deref(), Some("fable-cursor"));
    assert_eq!(profile_names("cursor", &Launch::default()), ["fable"]);
    assert_eq!(
        model_args("claude", resolved.model.as_ref()),
        [] as [OsString; 0]
    );
    assert_eq!(model_args("cursor", None), [] as [OsString; 0]);
}

#[test]
fn a_configured_cursor_profile_or_route_replaces_the_built_in_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut launch = Launch::default();
    launch.profiles.insert(
        "cursor".to_string(),
        [
            (
                "fable".to_string(),
                ontology::LaunchProfile {
                    args: vec![
                        "--endpoint".to_string(),
                        "https://own.example.test".to_string(),
                    ],
                    ..ontology::LaunchProfile::default()
                },
            ),
            ("mine".to_string(), ontology::LaunchProfile::default()),
        ]
        .into_iter()
        .collect(),
    );
    let resolved = resolve_with_config(
        "fable@cursor",
        &[],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        ontology::ResolvedConfig {
            launch: launch.clone(),
            ..ontology::ResolvedConfig::default()
        },
    )
    .expect("configured profile");
    assert!(resolved.model.is_none(), "{:?}", resolved.model);
    assert_eq!(resolved.model_args, [] as [OsString; 0]);
    assert_eq!(
        resolved.argv,
        ["cursor-agent", "--endpoint", "https://own.example.test"].map(OsString::from)
    );
    assert_eq!(profile_names("cursor", &launch), ["fable", "mine"]);

    // A configured route of the built-in name wins over the built-in route.
    let mut launch = Launch::default();
    launch.models.insert(
        "fable-cursor".to_string(),
        ontology::LaunchModel {
            id: "claude-fable-5-1-medium".to_string(),
            context: 200_000,
            compact: None,
        },
    );
    let resolved = resolve_with_config(
        "fable@cursor",
        &[],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        ontology::ResolvedConfig {
            launch,
            ..ontology::ResolvedConfig::default()
        },
    )
    .expect("configured route");
    let model = resolved.model.as_ref().expect("resolved model");
    assert_eq!(model.id, "claude-fable-5-1-medium");
    assert_eq!(model.source, ModelSource::Config);
    assert_eq!(
        resolved.argv,
        ["cursor-agent", "--model", "claude-fable-5-1-medium"].map(OsString::from)
    );
}

#[test]
fn cursor_starts_the_agent_binary_unless_the_configuration_names_another() {
    let launch = Launch::default();
    assert_eq!(
        resolve_tool("cursor", &launch).binary,
        OsString::from("cursor-agent")
    );
    for tool in ["claude", "codex", "agy", "grok", "opencode", "ollama"] {
        assert_eq!(resolve_tool(tool, &launch).binary, OsString::from(tool));
    }

    let mut launch = Launch::default();
    launch.tools.insert(
        "cursor".to_string(),
        ontology::LaunchTool {
            binary: Some("/opt/cursor/agent".to_string()),
            base_url_env: None,
        },
    );
    assert_eq!(
        resolve_tool("cursor", &launch).binary,
        OsString::from("/opt/cursor/agent")
    );
}

#[test]
fn an_unknown_cursor_profile_lists_the_built_in_one() {
    let error = resolve_profile("cursor", Some("nope"), &Launch::default()).expect_err("unknown");
    assert!(
        error
            .message
            .contains("no launch profile 'nope' for cursor (profiles: fable)"),
        "{}",
        error.message
    );
    assert_eq!(error.profiles, Some(vec!["fable".to_string()]));
    // Claude has no built-in profile: its list is the configured one alone.
    assert_eq!(
        profile_names("claude", &Launch::default()),
        [] as [String; 0]
    );
}

#[test]
fn invocation_splits_as_profile_at_tool() {
    assert_eq!(split_invocation("sol@claude"), ("claude", Some("sol")));
    assert_eq!(
        split_invocation("llama3@ollama"),
        ("ollama", Some("llama3"))
    );
    assert_eq!(split_invocation("claude"), ("claude", None));
}

#[test]
fn ollama_profile_name_falls_back_to_a_model() {
    let launch = Launch::default();
    let resolved = resolve_profile("ollama", Some("llama3"), &launch).unwrap();
    assert!(resolved.is_none(), "unknown ollama profile is a model name");
}

#[test]
fn direct_flag_disables_profile_middleware() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut launch = Launch::default();
    launch.profiles.insert(
        "claude".to_string(),
        [(
            "sol".to_string(),
            ontology::LaunchProfile {
                with: vec!["cliproxy".to_string()],
                ..ontology::LaunchProfile::default()
            },
        )]
        .into_iter()
        .collect(),
    );

    let resolved = resolve_with_config(
        "sol@claude",
        &[OsString::from("--direct")],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        ontology::ResolvedConfig {
            launch,
            ..ontology::ResolvedConfig::default()
        },
    )
    .expect("direct profile launch");

    assert_eq!(resolved.pre, [] as [crate::cli::launch::PreStep; 0]);
    assert_eq!(resolved.wrap, [] as [Vec<std::ffi::OsString>; 0]);
}

#[test]
fn grok_route_resolves_from_the_built_in_catalog() {
    let launch = Launch::default();
    let profile = ontology::LaunchProfile {
        model: Some("grok".to_string()),
        ..ontology::LaunchProfile::default()
    };
    let mut plan = LaunchPlan::default();

    let model = apply_profile_model("claude", &profile, &launch, &mut plan)
        .expect("model route")
        .expect("resolved model");

    assert_eq!(model.alias, "grok");
    assert_eq!(model.id, "grok-4.6");
    assert_eq!(model.context, 500_000);
    assert_eq!(model.source, ModelSource::BuiltIn);
    assert_eq!(
        plan.env,
        names(&[
            ("ANTHROPIC_MODEL", "grok-4.6"),
            ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", "500000"),
            ("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "500000"),
        ])
    );
}

#[test]
fn claude_model_route_generates_atomic_context_settings() {
    let mut launch = Launch::default();
    let profile = ontology::LaunchProfile {
        model: Some("sol".to_string()),
        ..ontology::LaunchProfile::default()
    };
    let mut plan = LaunchPlan::default();

    let model = apply_profile_model("claude", &profile, &launch, &mut plan)
        .expect("model route")
        .expect("resolved model");

    assert_eq!(model.alias, "sol");
    assert_eq!(model.id, "gpt-5.6-sol");
    assert_eq!(model.context, 272_000);
    assert_eq!(model.source, ModelSource::BuiltIn);
    assert_eq!(
        plan.env,
        names(&[
            ("ANTHROPIC_MODEL", "gpt-5.6-sol"),
            ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", "272000"),
            ("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "272000"),
        ])
    );

    launch.models.insert(
        "sol".to_string(),
        LaunchModel {
            id: "configured-sol".to_string(),
            context: 300_000,
            compact: Some(85),
        },
    );
    let mut configured_plan = LaunchPlan::default();
    let configured = apply_profile_model("claude", &profile, &launch, &mut configured_plan)
        .expect("configured route")
        .expect("resolved configured model");
    assert_eq!(configured.source, ModelSource::Config);
    assert_eq!(configured.id, "configured-sol");
    assert_eq!(configured.context, 300_000);
    assert_eq!(
        configured_plan.env,
        names(&[
            ("ANTHROPIC_MODEL", "configured-sol"),
            ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", "300000"),
            ("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "300000"),
            ("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", "85"),
        ])
    );
}

#[test]
fn kimi_is_a_built_in_route_that_config_overrides() {
    let profile = ontology::LaunchProfile {
        model: Some("kimi".to_string()),
        ..ontology::LaunchProfile::default()
    };
    let mut plan = LaunchPlan::default();
    let model = apply_profile_model("claude", &profile, &Launch::default(), &mut plan)
        .expect("model route")
        .expect("resolved model");
    assert_eq!(model.id, "kimi-k3");
    assert_eq!(model.context, 262_144);
    assert_eq!(model.source, ModelSource::BuiltIn);
    assert!(
        plan.env
            .contains(&(OsString::from("ANTHROPIC_MODEL"), OsString::from("kimi-k3")))
    );

    let mut launch = Launch::default();
    launch.models.insert(
        "kimi".to_string(),
        LaunchModel {
            id: "kimi-k3-256k".to_string(),
            context: 262_144,
            compact: None,
        },
    );
    let configured = apply_profile_model("claude", &profile, &launch, &mut LaunchPlan::default())
        .expect("configured route")
        .expect("resolved configured model");
    assert_eq!(configured.id, "kimi-k3-256k");
    assert_eq!(configured.source, ModelSource::Config);

    let error = resolve_model("nope", &Launch::default()).expect_err("unknown route");
    assert!(error.contains("kimi"), "{error}");
}

#[test]
fn check_flag_parses_like_dry_run() {
    let parsed = parse_cli_tail(
        &[OsString::from("--check"), OsString::from("--dry-run")],
        &Launch::default(),
    )
    .expect("parse");
    assert!(parsed.check);
    assert!(parsed.dry_run);
    assert_eq!(parsed.args, [] as [std::ffi::OsString; 0]);
}

#[test]
fn model_route_rejects_profile_owned_generated_environment() {
    let mut profile = ontology::LaunchProfile {
        model: Some("sol".to_string()),
        ..ontology::LaunchProfile::default()
    };
    profile.env.insert(
        "ANTHROPIC_MODEL".to_string(),
        ontology::ProfileEnvValue::Literal("gpt-5.6-sol".to_string()),
    );

    let error = apply_profile_model(
        "claude",
        &profile,
        &Launch::default(),
        &mut LaunchPlan::default(),
    )
    .expect_err("generated key conflict");

    assert!(error.contains("ANTHROPIC_MODEL"), "{error}");
    assert!(
        error.contains("remove them from the profile env"),
        "{error}"
    );
}

#[test]
fn model_route_dry_run_reports_provenance_and_generated_settings() {
    let profile = ontology::LaunchProfile {
        model: Some("sol".to_string()),
        ..ontology::LaunchProfile::default()
    };
    let mut plan = LaunchPlan::default();
    let model = apply_profile_model("claude", &profile, &Launch::default(), &mut plan)
        .expect("model route")
        .expect("resolved model");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[], &plan, &[]);
    let output = format_dry_run_parts(
        &DryRunParts {
            tool: "claude",
            argv: &argv,
            env: &final_env(&tool, &plan),
            env_removed: &[],
            base_url: plan.base_url.as_deref(),
            model: Some(&model),
            model_override: None,
            warnings: &plan.warnings,
        },
        &plan.wrap,
        &plan.pre,
    );

    assert!(output.contains("model: sol"), "{output}");
    assert!(output.contains("model_id: gpt-5.6-sol"), "{output}");
    assert!(output.contains("model_context: 272000"), "{output}");
    assert!(output.contains("model_source: built-in"), "{output}");
    assert!(
        output.contains("CLAUDE_CODE_AUTO_COMPACT_WINDOW=272000"),
        "{output}"
    );
}

#[test]
fn profile_env_supports_literals_and_from_env_references() {
    let mut profile = ontology::LaunchProfile::default();
    profile.env.insert(
        "ANTHROPIC_MODEL".to_string(),
        ontology::ProfileEnvValue::Literal("claude-fable-5".to_string()),
    );
    let mut plan = LaunchPlan::default();
    apply_profile_env(&profile, &mut plan, None).unwrap();
    assert_eq!(
        plan.env,
        vec![(
            OsString::from("ANTHROPIC_MODEL"),
            OsString::from("claude-fable-5")
        )]
    );

    let mut broken = ontology::LaunchProfile::default();
    broken.env.insert(
        "KEY".to_string(),
        ontology::ProfileEnvValue::FromEnv {
            from_env: "RUNE_TEST_UNSET_VARIABLE".to_string(),
        },
    );
    let mut plan = LaunchPlan::default();
    let error = apply_profile_env(&broken, &mut plan, None).unwrap_err();
    assert!(error.contains("RUNE_TEST_UNSET_VARIABLE"), "{error}");
}

#[test]
fn from_env_reference_falls_back_to_the_env_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env_path = dir.path().join("env");
    std::fs::write(
        &env_path,
        "# comment line\nexport RUNE_TEST_FILE_ONLY=\"file-value\"\nOTHER='quoted'\n",
    )
    .expect("write env file");

    let mut profile = ontology::LaunchProfile::default();
    profile.env.insert(
        "TARGET_KEY".to_string(),
        ontology::ProfileEnvValue::FromEnv {
            from_env: "RUNE_TEST_FILE_ONLY".to_string(),
        },
    );
    let mut plan = LaunchPlan::default();
    apply_profile_env(&profile, &mut plan, Some(&env_path)).unwrap();
    assert_eq!(
        plan.env,
        vec![(OsString::from("TARGET_KEY"), OsString::from("file-value"))]
    );

    let mut missing = ontology::LaunchProfile::default();
    missing.env.insert(
        "TARGET_KEY".to_string(),
        ontology::ProfileEnvValue::FromEnv {
            from_env: "RUNE_TEST_ABSENT_EVERYWHERE".to_string(),
        },
    );
    let mut plan = LaunchPlan::default();
    let error = apply_profile_env(&missing, &mut plan, Some(&env_path)).unwrap_err();
    assert!(error.contains("absent from"), "{error}");
    assert!(error.contains("RUNE_TEST_ABSENT_EVERYWHERE"), "{error}");
}

#[test]
fn env_file_parser_handles_export_quotes_and_comments() {
    let parsed = parse_env_file(
        "# header\nexport A=1\nB=\"two words\"\nC='single'\nD=plain\n\nnot-a-pair\n",
    );
    assert_eq!(parsed.get("A").map(String::as_str), Some("1"));
    assert_eq!(parsed.get("B").map(String::as_str), Some("two words"));
    assert_eq!(parsed.get("C").map(String::as_str), Some("single"));
    assert_eq!(parsed.get("D").map(String::as_str), Some("plain"));
    assert!(!parsed.contains_key("not-a-pair"));
}

#[test]
fn direct_mode_omits_profile_middleware() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut launch = Launch::default();
    launch.profiles.insert(
        "claude".to_string(),
        [(
            "sol".to_string(),
            ontology::LaunchProfile {
                with: vec!["cliproxy".to_string()],
                ..ontology::LaunchProfile::default()
            },
        )]
        .into_iter()
        .collect(),
    );

    let resolved = resolve_with_config(
        "sol@claude",
        &[OsString::from("--direct"), OsString::from("--dry-run")],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        ontology::ResolvedConfig {
            launch,
            ..ontology::ResolvedConfig::default()
        },
    )
    .expect("direct launch");

    assert_eq!(resolved.wrap, [] as [Vec<std::ffi::OsString>; 0]);
    assert_eq!(resolved.pre, [] as [crate::cli::launch::PreStep; 0]);
}

#[cfg(unix)]
#[test]
fn run_child_removes_the_parent_session_keys_before_the_plan_env_applies() {
    // HOME stands in for a parent-session key: the test process has it, and
    // the child must not. The plan env still reaches the child afterwards.
    let argv = vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("test -z \"${HOME+set}\" && test \"$RUNE_ROUTE\" = yes"),
    ];
    let env = names(&[("RUNE_ROUTE", "yes")]);

    assert_eq!(
        run_child(&argv, &env, &[OsString::from("HOME")]),
        Ok(0),
        "HOME removed and the plan env applied"
    );
    assert_eq!(run_child(&argv, &env, &[]), Ok(1), "HOME inherited");
}

#[test]
fn endpoint_comes_from_the_plan_or_the_tools_base_url_key_in_the_env() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = || ontology::ResolvedConfig {
        deck: None,
        env: None,
        ontology: ontology::ResolvedOntology::default(),
        extensions: Vec::new(),
        setup: None,
        launch: Launch::default(),
        hooks: rune::hooks::Hooks::default(),
        bench: Vec::new(),

        theme: None,
    };
    let resolved = resolve_with_config(
        "claude",
        &[],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        config(),
    )
    .expect("resolved launch");

    assert_eq!(resolved.endpoint(&[], &[]), None);
    assert_eq!(
        resolved.endpoint(
            &names(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317")]),
            &[]
        ),
        Some("http://127.0.0.1:8317".to_string())
    );
    assert_eq!(
        resolved.endpoint(
            &[],
            &names(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")])
        ),
        Some("http://127.0.0.1:9/gateway".to_string()),
        "a retained inherited endpoint is the one the child reaches"
    );
    assert_eq!(
        resolved.endpoint(
            &names(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317")]),
            &names(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/gateway")])
        ),
        Some("http://127.0.0.1:8317".to_string()),
        "the route env wins over the inherited value"
    );

    let proxied = resolve_with_config(
        "claude",
        &[OsString::from("--pxpipe")],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        config(),
    )
    .expect("resolved launch");
    assert_eq!(
        proxied.endpoint(&proxied.effective_env(), &[]),
        Some("http://127.0.0.1:47821".to_string()),
        "the plan's base URL sits in the effective env"
    );
    assert_eq!(
        proxied.endpoint(&[], &names(&[("ANTHROPIC_BASE_URL", "http://elsewhere")])),
        Some("http://127.0.0.1:47821".to_string()),
        "the plan's base URL wins over the inherited one"
    );
}

#[test]
fn a_settings_endpoint_wins_over_the_middleware_endpoint_as_in_the_child() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved = resolve_with_config_for_tests(
        "claude",
        &[
            OsString::from("--pxpipe"),
            OsString::from("--settings"),
            OsString::from(r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/settings"}}"#),
        ],
        dir.path(),
    );
    let isolation = surface::parent_session_isolation(&resolved.route_keys());

    assert_eq!(
        resolved.endpoint(&resolved.effective_env(), &[]),
        Some("http://127.0.0.1:9/settings".to_string()),
        "claude applies the settings env over the process env"
    );
    assert!(
        resolved
            .format_dry_run(&isolation, &[])
            .contains("base_url: http://127.0.0.1:9/settings")
    );
    assert_eq!(
        resolved.plan_json_for_launch(&isolation, &[])["base_url"],
        "http://127.0.0.1:9/settings"
    );
}

#[test]
fn every_shown_endpoint_loses_its_credential_query() {
    let dir = tempfile::tempdir().expect("tempdir");
    let gateway = "https://127.0.0.1:9/v1?api_key=fake-query-secret";
    let shown = "https://127.0.0.1:9/v1?api_key=<redacted>";
    let resolved = resolve_with_config_for_tests(
        "claude",
        &[
            OsString::from("--settings"),
            OsString::from(format!(r#"{{"env":{{"ANTHROPIC_BASE_URL":"{gateway}"}}}}"#)),
        ],
        dir.path(),
    );
    let isolation = surface::parent_session_isolation(&resolved.route_keys());

    // The dry run's endpoint line and JSON field, and the URL inside the
    // settings object in the argv.
    let text = resolved.format_dry_run(&isolation, &[]);
    assert!(text.contains(&format!("base_url: {shown}")), "{text}");
    assert!(!text.contains("fake-"), "{text}");
    let json = resolved.plan_json_for_launch(&isolation, &[]);
    assert_eq!(json["base_url"], shown);
    assert!(!json.to_string().contains("fake-"), "{json}");
    assert_eq!(
        redact_argument(&format!(
            r#"{{"env":{{"ANTHROPIC_BASE_URL":"{gateway}"}}}}"#
        )),
        format!(r#"{{"env":{{"ANTHROPIC_BASE_URL":"{shown}"}}}}"#)
    );

    // The base URL override warnings.
    let mut plan = LaunchPlan {
        base_url: Some(gateway.to_string()),
        ..LaunchPlan::default()
    };
    set_base_url(&mut plan, "pxpipe", "http://127.0.0.1:47821".to_string());
    set_external_base_url(&mut plan, "external", gateway.to_string());
    assert_eq!(plan.warnings.len(), 2, "{:?}", plan.warnings);
    assert!(
        plan.warnings[0].contains(&format!("from '{shown}'")),
        "{}",
        plan.warnings[0]
    );
    assert!(
        plan.warnings[1].contains(&format!("sets base_url to '{shown}'")),
        "{}",
        plan.warnings[1]
    );
    assert!(
        plan.warnings
            .iter()
            .all(|warning| !warning.contains("fake-")),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_tmux_command_line_hides_a_credential_value_with_spaces_whole() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &[],
        Some("work"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let mut plan = LaunchPlan {
        env: names(&[
            ("ANTHROPIC_AUTH_TOKEN", "fake token with spaces"),
            ("ANTHROPIC_MODEL", "sol model"),
        ]),
        ..plan
    };
    plan.base_url = Some("https://127.0.0.1:9/v1".to_string());
    let tool = resolve_tool("claude", &Launch::default());
    let args = [
        OsString::from("--api-key"),
        OsString::from("fake flag secret"),
    ];

    let real = build_argv(&tool, &args, &plan, &[]);
    let inner = real.last().expect("tmux shell command").to_string_lossy();
    assert!(
        inner.contains("'ANTHROPIC_AUTH_TOKEN=fake token with spaces'"),
        "{inner}"
    );

    let shown = display_argv_of(&tool, &args, &plan, &[]);
    let inner = shown.last().expect("tmux shell command").to_string_lossy();
    assert!(
        inner.contains("'ANTHROPIC_AUTH_TOKEN=<redacted>'"),
        "{inner}"
    );
    assert!(inner.contains("'ANTHROPIC_MODEL=sol model'"), "{inner}");
    assert!(
        inner.contains("ANTHROPIC_BASE_URL=https://127.0.0.1:9/v1"),
        "{inner}"
    );
    assert!(inner.ends_with("claude --api-key '<redacted>'"), "{inner}");
    assert!(!inner.contains("fake"), "{inner}");

    // The dry run hides the shown argv by name once more without harm.
    let printed = redact_argv(&shown).join(" ");
    assert!(
        printed.contains("'ANTHROPIC_AUTH_TOKEN=<redacted>' "),
        "{printed}"
    );
    assert!(
        printed.ends_with("claude --api-key '<redacted>'"),
        "{printed}"
    );
}

#[test]
fn the_structural_pass_reads_escaped_quotes_a_scheme_word_and_shell_quotes() {
    assert_eq!(
        redact_structural(r#"{"message":"{\"api_key\":\"fake-nested-secret\"}"}"#),
        r#"{"message":"{\"api_key\":\"<redacted>\"}"}"#
    );
    assert_eq!(
        redact_structural("Authorization: Bearer fake-bearer-secret rejected"),
        "Authorization: Bearer <redacted> rejected"
    );
    assert_eq!(
        redact_structural(r#""Authorization":"Basic fake-basic-secret""#),
        r#""Authorization":"<redacted>""#
    );
    assert_eq!(
        redact_structural("MY_TOKEN='fake token with spaces' claude"),
        "MY_TOKEN='<redacted>' claude"
    );
    assert_eq!(
        redact_structural("'MY_TOKEN=<redacted>' claude"),
        "'MY_TOKEN=<redacted>' claude",
        "a shell quote never joins an unquoted value"
    );
}

#[test]
fn the_structural_pass_hides_a_value_whose_quote_never_closes() {
    // A tail cut before the closing quote still hides the value.
    assert_eq!(
        redact_structural("api_key: \"sk-fake-unclosed"),
        "api_key: \"<redacted>\""
    );
    assert_eq!(
        redact_structural("MY_TOKEN='sk-fake-unclosed then more"),
        "MY_TOKEN='<redacted>' then more"
    );
    assert_eq!(
        redact_structural("model: \"sk-fake-unclosed"),
        "model: \"sk-fake-unclosed",
        "a key that names no credential is left alone"
    );
}

#[test]
fn tails_hide_a_url_password_and_its_credential_query_values() {
    // The endpoint is no credential-named key, so its value is not among
    // the known secrets: only the URL pass hides what it carries.
    assert_eq!(
        redact_text(
            "cannot reach https://u:sk-fake-url-password@gw/v1?api_key=sk-fake-query-secret&x=1 now",
            &[]
        ),
        "cannot reach https://u:<redacted>@gw/v1?api_key=<redacted>&x=1 now"
    );
}

#[test]
fn tails_hide_the_known_values_first_then_beside_a_name() {
    // A known value with spaces is hidden whole: the structural pass alone
    // would hide its first word and leak the rest.
    let secrets = vec!["fake token with spaces".to_string()];
    assert_eq!(
        redact_text("MY_TOKEN=fake token with spaces rejected", &secrets),
        "MY_TOKEN=<redacted> rejected"
    );
    assert_eq!(
        redact_text("unknown_token=fake-unknown-value ok", &[]),
        "unknown_token=<redacted> ok"
    );
}

#[test]
fn each_tool_reads_its_base_url_from_its_own_key_by_default() {
    for (tool, key) in [
        ("claude", Some("ANTHROPIC_BASE_URL")),
        ("codex", Some("OPENAI_BASE_URL")),
        ("grok", Some("GROK_MODELS_BASE_URL")),
        ("opencode", None),
        ("agy", None),
    ] {
        assert_eq!(
            resolve_tool(tool, &Launch::default()).base_url_env,
            key.map(OsString::from),
            "{tool}"
        );
    }
}

#[test]
fn credential_key_classifier_reads_env_settings_config_and_header_names() {
    for key in [
        "ANTHROPIC_AUTH_TOKEN",
        "OPENAI_API_KEY",
        "GROK_AUTH",
        "Authorization",
        "apiKeyHelper",
        "experimental_bearer_token",
        "PROXY_PASSWORD",
        "ANTHROPIC_CUSTOM_HEADERS",
        "http_headers",
    ] {
        assert!(is_credential_env_key(key), "{key}");
    }
    for key in [
        "ANTHROPIC_MODEL",
        "CLAUDE_CODE_MAX_CONTEXT_TOKENS",
        "CLAUDE_CODE_MAX_OUTPUT_TOKENS",
        "model_reasoning_effort",
        "SSH_AUTH_SOCK",
    ] {
        assert!(!is_credential_env_key(key), "{key}");
    }
}

#[test]
fn short_credential_values_are_kept_for_the_structural_pass_and_never_substituted() {
    let values = credential_values(&names(&[
        ("A_TOKEN", "fake-long-secret"),
        ("B_KEY", "ok"),
        ("C_MODEL", "fake-model-name"),
    ]));
    assert_eq!(values, ["fake-long-secret", "ok"]);
    assert_eq!(
        redact_credentials("ok fake-long-secret ok", &values),
        "ok <redacted> ok"
    );
}

#[test]
fn redact_argument_hides_a_credential_inside_a_structured_value() {
    let settings = r#"{"env":{"MY_TOKEN":"fake-settings-secret","X":"1"},"apiKeyHelper":"echo fake-helper-secret","permissions":{"allow":["Bash"]}}"#;
    let redacted = redact_argument(settings);
    assert!(!redacted.contains("fake-"), "{redacted}");
    let parsed: serde_json::Value = serde_json::from_str(&redacted).expect("still JSON");
    assert_eq!(parsed["env"]["MY_TOKEN"], "<redacted>");
    assert_eq!(parsed["env"]["X"], "1");
    assert_eq!(parsed["apiKeyHelper"], "<redacted>");
    assert_eq!(parsed["permissions"]["allow"][0], "Bash");

    let attached = format!("--settings={settings}");
    let redacted = redact_argument(&attached);
    assert!(redacted.starts_with("--settings={"), "{redacted}");
    assert!(!redacted.contains("fake-settings-secret"), "{redacted}");

    assert_eq!(
        redact_argument("http_headers.Authorization=\"Bearer fake-header-secret\""),
        "http_headers.Authorization=<redacted>"
    );
    assert_eq!(
        redact_argument("experimental_bearer_token=\"fake-bearer-secret\""),
        "experimental_bearer_token=<redacted>"
    );
    assert_eq!(
        redact_argument("http_headers={Authorization=\"Bearer fake-header-secret\"}"),
        "http_headers=<redacted>"
    );
    assert_eq!(
        redact_argument("model_reasoning_effort=\"high\""),
        "model_reasoning_effort=\"high\""
    );
    assert_eq!(redact_argument("--verbose"), "--verbose");
}

#[test]
fn plans_redact_by_name_and_never_by_value() {
    // A token's value that is also a model name: the model prints intact.
    let env = names(&[
        ("ANTHROPIC_AUTH_TOKEN", "ollama"),
        ("ANTHROPIC_MODEL", "ollama"),
    ]);
    assert_eq!(
        redacted_env(&env),
        [
            ("ANTHROPIC_AUTH_TOKEN".to_string(), "<redacted>".to_string()),
            ("ANTHROPIC_MODEL".to_string(), "ollama".to_string()),
        ]
    );
    let argv = [
        "claude",
        "--model",
        "qwen3-ollama:30b",
        "--name",
        "ollama-review",
        "--api-key",
        "fake-flag-secret",
        "--api-key=fake-attached-secret",
        "--password",
        "fake-password-secret",
        "--token",
        "fake-token-secret",
        "--verbose",
    ]
    .map(OsString::from);
    assert_eq!(
        redact_argv(&argv),
        [
            "claude",
            "--model",
            "qwen3-ollama:30b",
            "--name",
            "ollama-review",
            "--api-key",
            "<redacted>",
            "--api-key=<redacted>",
            "--password",
            "<redacted>",
            "--token",
            "<redacted>",
            "--verbose",
        ]
    );
}

#[test]
fn assignments_with_whitespace_and_exempt_keys_redact_by_key() {
    assert_eq!(
        redact_argument("http_headers.Authorization = \"Bearer fake-header-secret\""),
        "http_headers.Authorization=<redacted>"
    );
    assert_eq!(
        redact_argument("env_key = \"PROXY_KEY\""),
        "env_key = \"PROXY_KEY\"",
        "env_key names a variable, it holds no credential"
    );
    assert_eq!(
        redact_argument("model_token_limit = 4096"),
        "model_token_limit = 4096"
    );
    assert!(!is_credential_env_key("RATE_LIMIT"));
    assert!(!is_credential_env_key("env_key"));
    assert!(is_credential_env_key("TOKEN_LIMIT_KEY"));
}

#[test]
fn urls_lose_their_password_and_credential_query_parameters() {
    assert_eq!(
        redact_urls("https://user:fake-url-secret@127.0.0.1:9/v1?token=fake-query-secret&x=1#f"),
        "https://user:<redacted>@127.0.0.1:9/v1?token=<redacted>&x=1#f"
    );
    assert_eq!(
        redact_urls(
            "see http://127.0.0.1:9/?api-key=fake-query-secret&key=fake-key-secret and http://h/?a=b"
        ),
        "see http://127.0.0.1:9/?api-key=<redacted>&key=<redacted> and http://h/?a=b"
    );
    assert_eq!(
        redact_argument(
            "--attach=http://u:fake-url-secret@127.0.0.1:4096/?access_token=fake-query-secret"
        ),
        "--attach=http://u:<redacted>@127.0.0.1:4096/?access_token=<redacted>"
    );
    let env = names(&[(
        "OPENAI_BASE_URL",
        "https://u:fake-url-secret@proxy/v1?password=fake-query-secret",
    )]);
    assert_eq!(
        redacted_env(&env)[0].1,
        "https://u:<redacted>@proxy/v1?password=<redacted>"
    );
}

#[test]
fn url_query_secrets_hold_each_credential_named_query_value_as_written_and_decoded() {
    // Two URLs in one text; a plain URL, a host alone, and a plain query
    // value add nothing. A URL's user and password are never read here:
    // the launch refuses them.
    let values = url_query_secrets(
        "-ehttps://example.test/v1?token=fake-query-secret&x=1&sig=fake%3Dsig%40value and http://h/?a=b and example.test",
    );
    for expected in ["fake-query-secret", "fake%3Dsig%40value", "fake=sig@value"] {
        assert!(
            values.contains(&expected.to_string()),
            "{expected}: {values:?}"
        );
    }
    for plain in ["1", "b", "example.test", "h"] {
        assert!(!values.contains(&plain.to_string()), "{plain}: {values:?}");
    }
    assert_eq!(url_query_secrets("no url here"), [] as [String; 0]);
}

#[test]
fn a_url_credential_is_read_by_a_whatwg_parser_in_every_form_the_harness_reads() {
    // Node's `new URL()` reads a user and a password from each of these.
    // A `://` rule saw none of the first six: a backslash, one slash,
    // three slashes, no slash, a tab before the `@`, and an upper-case
    // scheme.
    for text in [
        r"https:\\u:p@h",
        "https:/u:p@h",
        "https:///u:p@h",
        "https:u:p@h",
        "https://u:p\t@h",
        "HTTPS://u:p@h",
        "https://user:pa'ss@example.test",
        "https://u:p@host/x?y=1 and more",
        "https://host/x?redirect=https://u:p@other",
        r#"-c model_providers.p.base_url="https:user:p@h/v1""#,
        "postgres://postgres:p@localhost/app",
    ] {
        assert_eq!(url_userinfo(text), Some(UrlUserinfo::Password), "{text:?}");
    }
    for text in [
        "-ehttps://FAKE-URL-TOKEN@example.test",
        "see postgres://postgres@localhost/app",
        r"ftp:\anonymous@files.example.test/pub",
    ] {
        assert_eq!(url_userinfo(text), Some(UrlUserinfo::User), "{text:?}");
    }
    for text in [
        "https://example.test/?api_key=fake-query-secret",
        "https://example.test/path@with/at?mail=a@b.test",
        "user@host without a scheme",
        "ssh://host/x",
        "mailto:a@b.test",
        "file:///Users/me/x",
        "",
    ] {
        assert_eq!(url_userinfo(text), None, "{text:?}");
    }
}

/// A launch of `tool` with a profile `p` that sets `env` and `args`, with
/// the refusal kept.
fn try_resolve_profile(
    tool: &str,
    env: &[(&str, &str)],
    args: &[&str],
    root: &Path,
) -> Result<ResolvedLaunch, ResolveError> {
    let mut launch = Launch::default();
    launch.profiles.insert(
        tool.to_string(),
        [(
            "p".to_string(),
            ontology::LaunchProfile {
                env: env
                    .iter()
                    .map(|(key, value)| {
                        (
                            (*key).to_string(),
                            ontology::ProfileEnvValue::Literal((*value).to_string()),
                        )
                    })
                    .collect(),
                args: args.iter().map(|arg| (*arg).to_string()).collect(),
                ..ontology::LaunchProfile::default()
            },
        )]
        .into_iter()
        .collect(),
    );
    resolve_with_config(
        &format!("p@{tool}"),
        &[],
        root.to_path_buf(),
        root.to_path_buf(),
        ontology::ResolvedConfig {
            launch,
            ..ontology::ResolvedConfig::default()
        },
    )
}

#[test]
fn a_user_only_url_is_refused_where_the_tool_reads_an_endpoint_and_passes_elsewhere() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A database URL names its user and no password. The route sets it,
    // the tool reads no endpoint from it: the launch resolves.
    let resolved = try_resolve_profile(
        "cursor",
        &[("DATABASE_URL", "postgres://postgres@localhost/app")],
        &["-ehttps://api.example.test"],
        dir.path(),
    );
    assert!(resolved.is_ok(), "{resolved:?}");
    // A password refuses in the same variable.
    let message = try_resolve_profile(
        "cursor",
        &[(
            "DATABASE_URL",
            "postgres://postgres:FAKE-DB-SECRET@localhost/app",
        )],
        &[],
        dir.path(),
    )
    .expect_err("refused")
    .message;
    assert!(message.contains("DATABASE_URL"), "{message}");
    assert!(!message.contains("FAKE-DB-SECRET"), "{message}");
    // A user alone refuses where the tool reads its endpoint: the base URL
    // variable the route sets, and the endpoint argument.
    for (tool, env, args) in [
        (
            "claude",
            &[("ANTHROPIC_BASE_URL", "https://FAKE-URL-TOKEN@proxy.test/v1")][..],
            &[][..],
        ),
        ("cursor", &[], &["-ehttps://FAKE-URL-TOKEN@example.test"]),
    ] {
        let message = try_resolve_profile(tool, env, args, dir.path())
            .err()
            .unwrap_or_else(|| panic!("{tool} resolved"))
            .message;
        assert!(!message.contains("FAKE-URL-TOKEN"), "{tool}: {message}");
    }
}

#[test]
fn a_proxy_variable_keeps_its_credential_and_the_plan_hides_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The child needs the proxy's user and password: the launch resolves,
    // in either case of the name, and the plan hides the password.
    for key in ["HTTPS_PROXY", "http_proxy", "ALL_PROXY"] {
        let resolved = try_resolve_profile(
            "cursor",
            &[(key, "http://user:pa'ss-FAKE-PROXY-SECRET@proxy.test:3128")],
            &[],
            dir.path(),
        );
        let resolved = resolved.unwrap_or_else(|error| panic!("{key}: {error}"));
        let secrets = resolved.known_secrets(&[]);
        assert!(
            secrets.contains(&"pa'ss-FAKE-PROXY-SECRET".to_string()),
            "{key}: {secrets:?}"
        );
        let isolation = surface::parent_session_isolation(&resolved.route_keys());
        let text = resolved.format_dry_run(&isolation, &[]);
        assert!(!text.contains("FAKE-PROXY-SECRET"), "{key}: {text}");
        assert!(text.contains("user:<redacted>@proxy.test"), "{key}: {text}");
    }
    // An inherited proxy with a percent-encoded password, and one with a
    // token for a user and no password: both as written and decoded.
    let resolved = try_resolve_profile("cursor", &[], &[], dir.path()).expect("resolved");
    let inherited = names(&[
        (
            "https_proxy",
            "http://user:pa%27ss-FAKE-PROXY-SECRET@proxy.test:3128",
        ),
        ("HTTP_PROXY", "http://FAKE-PROXY-TOKEN%3D@proxy.test:3128"),
    ]);
    let secrets = resolved.known_secrets(&inherited);
    for expected in [
        "pa%27ss-FAKE-PROXY-SECRET",
        "pa'ss-FAKE-PROXY-SECRET",
        "FAKE-PROXY-TOKEN%3D",
        "FAKE-PROXY-TOKEN=",
    ] {
        assert!(
            secrets.contains(&expected.to_string()),
            "{expected}: {secrets:?}"
        );
    }
    assert!(!secrets.contains(&"user".to_string()), "{secrets:?}");
    // A quote does not end the URL: the password is hidden whole.
    assert_eq!(
        redact_urls("HTTPS_PROXY=http://user:pa'ss-FAKE-PROXY-SECRET@proxy.test:3128 next"),
        "HTTPS_PROXY=http://user:<redacted>@proxy.test:3128 next"
    );
    assert_eq!(
        redact_urls(r#"base_url="https://h/v1?token=fake-query-secret" next"#),
        r#"base_url="https://h/v1?token=<redacted>" next"#
    );
}

#[test]
fn a_codex_config_url_is_hidden_as_codex_decodes_it() {
    // The query name and the value are TOML escapes: the raw text shows
    // neither `token` nor the secret, and codex reads both.
    assert_eq!(
        redact_argument(
            r#"model_providers.p.base_url="https://h/v1?\u0074oken=\u0046AKE-ESCAPED-SECRET&x=1""#
        ),
        r#"model_providers.p.base_url="https://h/v1?token=<redacted>&x=1""#
    );
    assert_eq!(
        redact_argument(
            r#"--config=model_providers.p.base_url="https://h/v1?\u0074oken=\u0046AKE-ESCAPED-SECRET""#
        ),
        r#"--config=model_providers.p.base_url="https://h/v1?token=<redacted>""#
    );
    // A table value that holds such a URL is hidden whole.
    assert_eq!(
        redact_argument(
            r#"model_providers.p={ base_url = "https://h/v1?\u0074oken=\u0046AKE-ESCAPED-SECRET" }"#
        ),
        "model_providers.p=<redacted>"
    );
    // A plain value keeps its text as written.
    assert_eq!(
        redact_argument(r#"model_providers.p.base_url="https://h/v1""#),
        r#"model_providers.p.base_url="https://h/v1""#
    );
}

#[test]
fn query_parameters_are_credentials_by_whole_word_only() {
    // `keyword` holds `key` but is no credential word; `api_key` and
    // `X-Amz-Signature` split into one.
    let values = url_query_secrets(
        "https://example.test/search?keyword=documentation&api_key=fake-query-secret&X-Amz-Signature=fake-sig-value&max_tokens=4096",
    );
    for expected in ["fake-query-secret", "fake-sig-value"] {
        assert!(
            values.contains(&expected.to_string()),
            "{expected}: {values:?}"
        );
    }
    for plain in ["documentation", "4096"] {
        assert!(!values.contains(&plain.to_string()), "{plain}: {values:?}");
    }
    assert_eq!(
        redact_urls("https://example.test/search?keyword=documentation&sig=fake-sig-value"),
        "https://example.test/search?keyword=documentation&sig=<redacted>"
    );
    for name in [
        "key",
        "api_key",
        "access-token",
        "X-Amz-Signature",
        "AUTH",
        "apikey",
    ] {
        assert_eq!(
            redact_urls(&format!("http://h/?{name}=fake-value")),
            format!("http://h/?{name}=<redacted>"),
            "{name}"
        );
    }
    for name in ["keyword", "tokens", "max_tokens", "q", "passage"] {
        let url = format!("http://h/?{name}=plain-value");
        assert_eq!(redact_urls(&url), url, "{name}");
    }
}

#[test]
fn a_percent_encoded_query_name_is_a_credential_once_decoded() {
    // `api%5Fkey` reaches the server as `api_key`: the name stays as
    // written, the value is hidden and collected.
    let url =
        "https://example.test/v1?api%5Fkey=fake-query-secret&access%2Dtoken=fake-token-value&x=1";
    assert_eq!(
        redact_urls(url),
        "https://example.test/v1?api%5Fkey=<redacted>&access%2Dtoken=<redacted>&x=1"
    );
    let values = url_query_secrets(url);
    for expected in ["fake-query-secret", "fake-token-value"] {
        assert!(
            values.contains(&expected.to_string()),
            "{expected}: {values:?}"
        );
    }
    assert!(!values.contains(&"1".to_string()), "{values:?}");
}

#[test]
fn tails_redact_known_values_of_eight_or_more_and_any_value_beside_a_name() {
    let secrets = vec!["fake-long-secret-value".to_string(), "ab12".to_string()];
    let text = concat!(
        "ANTHROPIC_AUTH_TOKEN=ab12 \"api_key\":\"zz\" token: short",
        " model=ollama rest fake-long-secret-value ab12 done",
    );
    assert_eq!(
        redact_text(text, &secrets),
        concat!(
            "ANTHROPIC_AUTH_TOKEN=<redacted> \"api_key\":\"<redacted>\" token: <redacted>",
            " model=ollama rest <redacted> ab12 done",
        ),
        "the short value is hidden beside its name only"
    );
    assert_eq!(
        credential_values(&names(&[
            ("A_KEY", "fake-short"),
            ("B_KEY", "fake-longer-secret")
        ])),
        ["fake-longer-secret", "fake-short"],
        "longest first"
    );
}

#[test]
fn tmux_wrap_unsets_the_parent_session_keys_inside_the_wrapped_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["pxpipe".to_string()],
        Some("work"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let removals = [
        OsString::from("CLAUDECODE"),
        OsString::from("CODEX_SANDBOX"),
    ];
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan, &removals);
    let inner = argv.last().expect("tmux shell command").to_string_lossy();

    assert!(
        inner.contains(
            "env -u CLAUDECODE -u CODEX_SANDBOX ANTHROPIC_BASE_URL=http://127.0.0.1:47821 claude --version"
        ),
        "{inner}"
    );
    assert_eq!(
        env_command(&[OsString::from("claude")], &[], &removals),
        ["env", "-u", "CLAUDECODE", "-u", "CODEX_SANDBOX", "claude"].map(OsString::from)
    );
}

#[test]
fn launch_route_keys_include_the_settings_env_of_a_claude_profile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved = resolve_with_config(
        "claude",
        &[
            OsString::from("--settings"),
            OsString::from(r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/settings"}}"#),
        ],
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
        ontology::ResolvedConfig {
            deck: None,
            env: None,
            ontology: ontology::ResolvedOntology::default(),
            extensions: Vec::new(),
            setup: None,
            launch: Launch::default(),
            hooks: rune::hooks::Hooks::default(),
            bench: Vec::new(),

            theme: None,
        },
    )
    .expect("resolved launch");

    assert!(
        resolved
            .route_keys()
            .contains(&OsString::from("ANTHROPIC_BASE_URL"))
    );
}

#[test]
fn an_unknown_profile_names_the_known_ones_in_the_message_and_as_a_list() {
    let launch = ontology::resolve_launch(&Launch::default());
    let error = resolve_profile("claude", Some("nope"), &launch).expect_err("unknown profile");

    assert!(
        error.message.contains("profiles: grok, sol"),
        "{}",
        error.message
    );
    assert_eq!(
        error.profiles,
        Some(vec!["grok".to_string(), "sol".to_string()])
    );
    let plain: ResolveError = "other".to_string().into();
    assert_eq!(plain.profiles, None);
    assert_eq!(plain.to_string(), "other");
}

/// A resolved launch for a bare tool with `rest`, from the default config.
pub(super) fn resolve_with_config_for_tests(
    tool: &str,
    rest: &[OsString],
    root: &Path,
) -> ResolvedLaunch {
    try_resolve_with_config_for_tests(tool, rest, root).expect("resolved launch")
}

/// [`resolve_with_config_for_tests`] with the refusal kept.
fn try_resolve_with_config_for_tests(
    tool: &str,
    rest: &[OsString],
    root: &Path,
) -> Result<ResolvedLaunch, ResolveError> {
    resolve_with_config(
        tool,
        rest,
        root.to_path_buf(),
        root.to_path_buf(),
        ontology::ResolvedConfig {
            deck: None,
            env: None,
            ontology: ontology::ResolvedOntology::default(),
            extensions: Vec::new(),
            setup: None,
            launch: Launch::default(),
            hooks: rune::hooks::Hooks::default(),
            bench: Vec::new(),

            theme: None,
        },
    )
}

#[test]
fn a_url_that_carries_a_user_or_a_password_is_refused_by_name_of_the_credential_variable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let refusal = |tool: &str, rest: &[&str]| {
        let rest: Vec<OsString> = rest.iter().map(OsString::from).collect();
        try_resolve_with_config_for_tests(tool, &rest, dir.path())
            .err()
            .unwrap_or_else(|| panic!("{tool} {rest:?} resolved"))
            .message
    };
    // A `'` in the password: the URL pass ended the URL at the quote and
    // the password printed. Decoded the way each tool decodes it: a TOML
    // escape for codex, a JSON escape for claude.
    for (tool, rest, source) in [
        (
            "cursor",
            &["-ehttps://user:pa'ss-FAKE-URL-SECRET@example.test"][..],
            "CURSOR_API_KEY",
        ),
        (
            "cursor",
            &["-e", "https://FAKE-URL-TOKEN@example.test"],
            "CURSOR_API_KEY",
        ),
        (
            "codex",
            &[
                "-c",
                r#"model_providers.p.base_url="https://FAKE-URL-TOKEN@example.test/v1""#,
            ],
            "OPENAI_API_KEY",
        ),
        (
            "claude",
            &[
                "--settings",
                r#"{"env":{"ANTHROPIC_BASE_URL":"https://u:FAKE-URL-SECRET@proxy.test/v1"}}"#,
            ],
            "ANTHROPIC_AUTH_TOKEN",
        ),
    ] {
        let message = refusal(tool, rest);
        assert!(message.contains(source), "{tool}: {message}");
        for hidden in ["FAKE-URL", "pa'ss", "example.test", "proxy.test"] {
            assert!(!message.contains(hidden), "{tool}: {message}");
        }
    }
    // A query credential is no userinfo: the route resolves and the plan
    // hides the value by name.
    let resolved = resolve_with_config_for_tests(
        "cursor",
        &[OsString::from(
            "-ehttps://example.test/?api_key=fake-query-secret",
        )],
        dir.path(),
    );
    let isolation = surface::parent_session_isolation(&resolved.route_keys());
    let text = resolved.format_dry_run(&isolation, &[]);
    assert!(text.contains("api_key=<redacted>"), "{text}");
    assert!(!text.contains("fake-query-secret"), "{text}");
}

#[test]
fn query_parameters_split_on_camel_case_and_every_parameter_is_matched() {
    assert_eq!(
        redact_urls(
            "https://h/?accessToken=fake-a&apiToken=fake-b&page=2&clientSecret=fake-c&maxTokens=4096"
        ),
        "https://h/?accessToken=<redacted>&apiToken=<redacted>&page=2&clientSecret=<redacted>&maxTokens=4096"
    );
    for name in ["APIKey", "XAmzSignature", "AuthToken", "passWord"] {
        assert_eq!(
            redact_urls(&format!("http://h/?{name}=fake-value")),
            format!("http://h/?{name}=<redacted>"),
            "{name}"
        );
    }
    for name in ["keyword", "passage", "Tokens", "maxTokens"] {
        let url = format!("http://h/?{name}=plain-value");
        assert_eq!(redact_urls(&url), url, "{name}");
    }
}

#[test]
fn the_output_redaction_hides_known_values_and_url_queries_in_text_and_json() {
    let secrets = vec!["fake-known-secret".to_string()];
    assert_eq!(
        redact_output(
            "Primary key: user_id, SECRET_KEY = os.environ[\"X\"], saw fake-known-secret at https://h/?token=fake-q&page=2",
            &secrets
        ),
        "Primary key: user_id, SECRET_KEY = os.environ[\"X\"], saw <redacted> at https://h/?token=<redacted>&page=2"
    );
    let json = redact_output_json(
        serde_json::json!({
            "warnings": ["uses https://h/?api_key=fake-q", "fake-known-secret"],
            "nested": {"value": "plain", "count": 2}
        }),
        &secrets,
    );
    assert_eq!(
        json,
        serde_json::json!({
            "warnings": ["uses https://h/?api_key=<redacted>", "<redacted>"],
            "nested": {"value": "plain", "count": 2}
        })
    );
}

#[test]
fn a_claude_launch_sends_one_settings_object_merged_from_all_of_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rest: Vec<OsString> = [
        "--settings",
        r#"{"env":{"A":"1"},"model":"x"}"#,
        "--verbose",
        "--settings",
        r#"{"env":{"B":"2"}}"#,
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let resolved = resolve_with_config_for_tests("claude", &rest, dir.path());
    let argv: Vec<String> = resolved
        .argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let settings: Vec<&String> = argv.iter().filter(|arg| *arg == "--settings").collect();
    assert_eq!(settings.len(), 1, "{argv:?}");
    let at = argv
        .iter()
        .position(|arg| arg == "--settings")
        .expect("flag");
    let parsed: serde_json::Value = serde_json::from_str(&argv[at + 1]).expect("settings json");
    assert_eq!(parsed, serde_json::json!({"env":{"B":"2"},"model":"x"}));
    let verbose = argv
        .iter()
        .position(|arg| arg == "--verbose")
        .expect("verbose");
    assert!(verbose < at, "{argv:?}");

    let single: Vec<OsString> = ["--settings", r#"{"env":{"A":"1"}}"#]
        .into_iter()
        .map(OsString::from)
        .collect();
    let resolved = resolve_with_config_for_tests("claude", &single, dir.path());
    assert!(resolved.argv.ends_with(&single), "{:?}", resolved.argv);
}

#[test]
fn a_tmux_launch_unsets_the_host_session_names_and_warns_about_attaching() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolved =
        resolve_with_config_for_tests("claude", &[OsString::from("--tmux=work")], dir.path());
    let inner = resolved
        .argv
        .last()
        .expect("tmux shell command")
        .to_string_lossy()
        .into_owned();

    assert!(inner.contains("env -u CLAUDECODE"), "{inner}");
    assert!(inner.contains("-u CLAUDE_CODE_ENTRYPOINT"), "{inner}");
    assert!(inner.contains("-u CODEX_SESSION_ID"), "{inner}");
    assert!(inner.ends_with(" claude"), "{inner}");
    assert!(
        resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("attaches to an existing session 'work'")),
        "{:?}",
        resolved.warnings()
    );
}
