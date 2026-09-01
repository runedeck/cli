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
            bench: Vec::new(),

            theme: None,
        },
    }
}

fn format_dry_run(tool: &ResolvedTool, argv: &[OsString], plan: &LaunchPlan) -> String {
    format_dry_run_parts(
        &tool.name,
        argv,
        &final_env(tool, plan),
        plan.base_url.as_deref(),
        &plan.wrap,
        &plan.pre,
        &plan.warnings,
        None,
        None,
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
    let argv = build_argv(&tool, &[], &plan);

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
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan);
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
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan);

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

    assert!(process_env(&tool, &plan).is_empty());
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
        resolved.format_dry_run(),
        "tool: claude\nargv: claude --resume\nenv:\nbase_url: <none>\nwrap:\npre:"
    );
}

#[test]
fn unwrapped_launch_dry_run_is_stable() {
    let tool = resolve_tool("claude", &Launch::default());
    let plan = LaunchPlan::default();
    let argv = build_argv(&tool, &[OsString::from("--resume")], &plan);

    assert_eq!(
        format_dry_run(&tool, &argv, &plan),
        "tool: claude\nargv: claude --resume\nenv:\nbase_url: <none>\nwrap:\npre:"
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
    let argv = build_argv(&tool, &[], &plan);
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
    assert!(error.contains("no launch profile 'missing'"), "{error}");
    assert!(
        error.contains("sol"),
        "error must list known names: {error}"
    );

    let found = resolve_profile("claude", Some("sol"), &launch).unwrap();
    assert!(found.is_some());
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

    assert!(resolved.pre.is_empty());
    assert!(resolved.wrap.is_empty());
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
    let argv = build_argv(&tool, &[], &plan);
    let output = format_dry_run_parts(
        "claude",
        &argv,
        &final_env(&tool, &plan),
        plan.base_url.as_deref(),
        &plan.wrap,
        &plan.pre,
        &plan.warnings,
        Some(&model),
        None,
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

    assert!(resolved.wrap.is_empty());
    assert!(resolved.pre.is_empty());
}
