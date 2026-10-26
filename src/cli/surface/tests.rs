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
fn warnings_show_a_dropped_value_only_for_a_fixed_choice_option() {
    // A free value is hidden whole, on one line: a forged line break
    // cannot add a warning, and a model name may be anything.
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--model", "evil\nwarning: forged"],
    ));
    assert_eq!(filtered.warnings.len(), 1);
    assert!(!filtered.warnings[0].contains('\n'));
    assert!(
        filtered.warnings[0].contains("owns --model; the profile value <redacted> is dropped"),
        "{}",
        filtered.warnings[0]
    );
    assert!(!filtered.warnings[0].contains("evil"));
    // A mode from a fixed list reads as written, and only a known value
    // of that list: an allowed flag can carry a URL with a password too.
    let grok = filter_surface_args(&invocation(
        Surface::Grok,
        &[
            "--sandbox",
            "read-only",
            "--permission-mode",
            "plan",
            "--sandbox=https://user:fake-sandbox-secret@host.test/",
            "--permission-mode",
            "workspace-local",
        ],
    ));
    assert_eq!(grok.warnings.len(), 4, "{:?}", grok.warnings);
    assert!(
        grok.warnings[0].contains("owns --sandbox; the profile value read-only is dropped"),
        "{}",
        grok.warnings[0]
    );
    assert!(
        grok.warnings[1].contains("owns --permission-mode; the profile value plan is dropped"),
        "{}",
        grok.warnings[1]
    );
    assert!(
        grok.warnings[2].contains("owns --sandbox; the profile value <redacted> is dropped"),
        "{}",
        grok.warnings[2]
    );
    assert!(!grok.warnings[2].contains("fake-"), "{}", grok.warnings[2]);
    assert!(
        !grok.warnings[2].contains("host.test"),
        "{}",
        grok.warnings[2]
    );
    assert!(
        grok.warnings[3]
            .contains("owns --permission-mode; the profile value <redacted> is dropped"),
        "{}",
        grok.warnings[3]
    );
    let codex = filter_surface_args(&invocation(
        Surface::Codex,
        &["-s", "read-only", "-s", "fake-unknown-mode"],
    ));
    assert!(
        codex.warnings[0].contains("owns -s; the profile value read-only is dropped"),
        "{:?}",
        codex.warnings
    );
    assert!(
        codex.warnings[1].contains("owns -s; the profile value <redacted> is dropped"),
        "{:?}",
        codex.warnings
    );

    // A credential-named flag, a URL with a password, and a model name
    // are all hidden whole: the warning names the option and nothing else.
    let opencode = filter_surface_args(&invocation(
        Surface::Opencode,
        &[
            "--password",
            "fake-opencode-secret",
            "--attach=http://user:fake-url-secret@127.0.0.1:4096/?token=fake-query-secret",
            "--model",
            "proton-lumo/lumo-max",
            "-s",
            "fake-session-id",
        ],
    ));
    assert_eq!(opencode.warnings.len(), 4, "{:?}", opencode.warnings);
    for (warning, option) in
        opencode
            .warnings
            .iter()
            .zip(["--password", "--attach", "--model", "-s"])
    {
        assert!(!warning.contains("fake-"), "{warning}");
        assert!(
            warning.contains(&format!(
                "owns {option}; the profile value <redacted> is dropped"
            )),
            "{warning}"
        );
    }
    assert!(!opencode.warnings[1].contains("127.0.0.1"));
    assert!(!opencode.warnings[2].contains("lumo"));
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
            "json",
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
fn claude_result_keeps_the_text_the_counts_and_the_models() {
    let stdout = concat!(
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,",
        "\"result\":\"Final text.\",\"session_id\":\"s1\",\"total_cost_usd\":0.01,",
        "\"usage\":{\"input_tokens\":11,\"cache_creation_input_tokens\":3,",
        "\"cache_read_input_tokens\":5,\"output_tokens\":7,\"server_tool_use\":{}},",
        "\"modelUsage\":{\"claude-opus-4-6\":{\"inputTokens\":11,\"outputTokens\":7}}}"
    );
    let result = claude_result(stdout).expect("a result");

    assert_eq!(result.result.as_deref(), Some("Final text."));
    let usage = result.usage.expect("usage");
    assert_eq!(usage.summed_total(), Some(26));
    assert_eq!(usage.completion_tokens(), Some(7.0));
    assert_eq!(usage.total_tokens, None, "claude reports no total itself");
    let models: Vec<&String> = result
        .model_usage
        .as_ref()
        .expect("models")
        .keys()
        .collect();
    assert_eq!(models, ["claude-opus-4-6"]);
}

#[test]
fn claude_result_is_read_after_an_api_error_line() {
    let stdout = concat!(
        "API Error: 429 rate limited\n",
        "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,",
        "\"errors\":[\"Provider quota exhausted\"],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}\n"
    );
    let result = claude_result(stdout).expect("a result");
    assert!(result.is_error);
    assert_eq!(result.errors, ["Provider quota exhausted"]);
    assert_eq!(
        claude_error_messages(stdout),
        ["API Error: 429 rate limited", "Provider quota exhausted"]
    );
    assert_eq!(
        claude_error_messages("API Error: 400 bad\nplain text\n"),
        ["API Error: 400 bad"]
    );
}

#[test]
fn claude_result_rejects_malformed_output() {
    for (stdout, expected) in [
        ("", "claude print returned no JSON result"),
        ("plain text", "claude print returned no JSON result"),
        ("{}", "claude print returned no JSON result"),
        (
            "{\"type\":\"system\",\"subtype\":\"init\"}",
            "claude print returned a system object, not a result",
        ),
        (
            "{\"type\":\"result\",\"subtype\":\"success\",\"usage\":{\"output_tokens\":-1}}",
            "claude print returned no JSON result",
        ),
        (
            "{\"type\":\"result\",\"subtype\":\"success\",\"modelUsage\":{\"model\":3}}",
            "claude print returned no JSON result",
        ),
    ] {
        let failure = claude_result(stdout).expect_err(stdout);
        let SurfaceFailure::Reported(message) = failure else {
            panic!("{failure:?}");
        };
        assert!(message.starts_with(expected), "{stdout}: {message}");
    }
}

#[test]
fn token_usage_keeps_incomplete_and_overflowing_totals_unknown() {
    let complete = TokenUsage {
        input_tokens: Some(1),
        cache_creation_input_tokens: Some(2),
        cache_read_input_tokens: Some(3),
        output_tokens: Some(4),
        ..TokenUsage::default()
    };
    assert_eq!(complete.summed_total(), Some(10));
    let incomplete = TokenUsage {
        cache_read_input_tokens: None,
        ..complete
    };
    assert_eq!(incomplete.summed_total(), None);
    let overflowing = TokenUsage {
        input_tokens: Some(u64::MAX),
        ..complete
    };
    assert_eq!(overflowing.summed_total(), None);
    assert_eq!(sum_counts([None, None]), None);
    assert_eq!(sum_counts([Some(1), None]), Some(1));
    assert_eq!(sum_counts([None, Some(2)]), Some(2));
    assert_eq!(sum_counts([Some(u64::MAX), Some(1)]), None);
}

/// An overflowing sum is unknown even when a later step would fit: the
/// wrapped number never reports as the run's count.
#[test]
fn opencode_overflowing_step_sum_stays_unknown() {
    assert_eq!(sum_counts([Some(u64::MAX), Some(1), Some(7)]), None);
    let events: Vec<OpencodeEvent> = parse_jsonl(&format!(
        concat!(
            "{{\"type\":\"step_finish\",\"part\":{{\"tokens\":{{\"input\":{max},\"output\":1}}}}}}\n",
            "{{\"type\":\"step_finish\",\"part\":{{\"tokens\":{{\"input\":1,\"output\":1}}}}}}\n",
            "{{\"type\":\"step_finish\",\"part\":{{\"tokens\":{{\"input\":7,\"output\":1}}}}}}\n",
        ),
        max = u64::MAX
    ));
    let steps: Vec<&OpencodeTokens> = events
        .iter()
        .filter_map(|event| event.part.as_ref())
        .filter_map(|part| part.tokens.as_ref())
        .collect();
    let usage = OpencodeTokens::sum(&steps);
    assert_eq!(usage.input_tokens, None, "overflow wrapped");
    assert_eq!(usage.output_tokens, Some(3));
}

/// A malformed count is an unknown count: the event around it still
/// parses, so an error event with a negative count still reports its error.
#[test]
fn malformed_counts_never_drop_the_event_around_them() {
    let events: Vec<CodexEvent> = parse_jsonl(concat!(
        "{\"type\":\"turn.failed\",\"error\":{\"message\":\"turn aborted\"},\"usage\":{\"input_tokens\":-1,\"output_tokens\":2}}\n",
        "{\"type\":\"turn.completed\",\"usage\":\"none\"}\n",
    ));
    assert_eq!(codex_error_messages(&events), ["turn aborted"]);
    let usage = events[0].usage.as_ref().expect("usage").token_usage();
    assert_eq!(usage.input_tokens, None);
    assert_eq!(usage.output_tokens, Some(2));
    assert!(events[1].usage.is_none(), "a usage that is no object");

    let events: Vec<OpencodeEvent> = parse_jsonl(concat!(
        "{\"type\":\"session.error\",\"properties\":{\"error\":{\"data\":{\"message\":\"route failed\"}}},",
        "\"part\":{\"tokens\":{\"input\":-1,\"output\":\"3\",\"cache\":\"none\"}}}\n",
    ));
    assert_eq!(opencode_session_error(&events), Some("route failed"));
    let tokens = events[0]
        .part
        .as_ref()
        .and_then(|part| part.tokens.as_ref());
    let tokens = tokens.expect("tokens");
    assert_eq!(tokens.input, None);
    assert_eq!(tokens.output, None);
    assert!(tokens.cache.is_none());
}

/// A number out of range (`1e999`) fails serde's own number parse. It is
/// an unknown count like a negative one, and the event around it, which
/// may name a failure, still reads.
#[test]
fn counts_out_of_range_never_drop_the_event_around_them() {
    let lines = JsonLines::<CodexEvent>::read(
        "{\"type\":\"turn.failed\",\"error\":{\"message\":\"turn aborted\"},\"usage\":{\"input_tokens\":1e999,\"output_tokens\":2}}\n",
    );
    assert!(lines.unreadable.is_none(), "{:?}", lines.unreadable);
    assert_eq!(codex_error_messages(&lines.events), ["turn aborted"]);
    let usage = lines.events[0].usage.as_ref().expect("usage").token_usage();
    assert_eq!(usage.input_tokens, None);
    assert_eq!(usage.output_tokens, Some(2));

    let lines = JsonLines::<OpencodeEvent>::read(concat!(
        "{\"type\":\"session.error\",\"properties\":{\"error\":{\"data\":{\"message\":\"route failed\"}}},",
        "\"part\":{\"tokens\":{\"input\":1e999,\"output\":3}}}\n",
    ));
    assert!(lines.unreadable.is_none(), "{:?}", lines.unreadable);
    assert_eq!(opencode_session_error(&lines.events), Some("route failed"));
    let tokens = lines.events[0]
        .part
        .as_ref()
        .and_then(|part| part.tokens.as_ref())
        .expect("tokens");
    assert_eq!(tokens.input, None);
    assert_eq!(tokens.output, Some(3));
}

/// An object line rune cannot read is never dropped in silence: a clean
/// run fails on it, named by its line on stdout, while the lines around
/// it still read for the errors a failed run reports.
#[test]
fn unreadable_event_lines_fail_a_clean_read_and_keep_the_rest() {
    let lines = JsonLines::<CodexEvent>::read(concat!(
        "banner line\n",
        "{\"type\":\"error\",\"message\":\"quota gone\"}\n",
        "{\"type\":\"turn.failed\",\"error\":{\"message\":\"turn aborted\"\n",
        "{\"type\":\"turn.completed\",\"usage\":\"none\"}\n",
    ));
    assert_eq!(codex_error_messages(&lines.events), ["quota gone"]);
    assert_eq!(lines.events.len(), 2, "the readable lines");
    let failure = lines
        .require_all("codex exec")
        .err()
        .expect("an unreadable line");
    assert_eq!(
        failure.to_string(),
        "codex exec printed a JSON line rune cannot read (stdout line 3, column 55: EOF while parsing an object)"
    );

    let lines = JsonLines::<OpencodeEvent>::read(
        "{\"type\":\"text\",\"part\":{\"messageID\":\"m1\",\"text\":5}}\n",
    );
    assert!(lines.events.is_empty());
    let failure = lines
        .require_all("opencode run")
        .err()
        .expect("an unreadable line");
    assert!(
        failure.to_string().starts_with(
            "opencode run printed a JSON line rune cannot read (stdout line 1, column "
        ),
        "{failure}"
    );

    let clean =
        JsonLines::<CodexEvent>::read("plain text\n{\"type\":\"error\",\"message\":\"x\"}\n");
    assert_eq!(clean.require_all("codex exec").expect("readable").len(), 1);
}

/// A `session.error` event fails the run whatever its shape: one without
/// a message names that lack instead of passing as no error.
#[test]
fn opencode_session_error_without_a_message_still_fails() {
    for stdout in [
        "{\"type\":\"session.error\"}",
        "{\"type\":\"session.error\",\"properties\":{}}",
        "{\"type\":\"session.error\",\"properties\":{\"error\":{\"name\":\"UnknownError\"}}}",
    ] {
        let events: Vec<OpencodeEvent> = parse_jsonl(stdout);
        assert_eq!(
            opencode_session_error(&events),
            Some("the event names no message"),
            "{stdout}"
        );
    }
    let events: Vec<OpencodeEvent> = parse_jsonl(
        "{\"type\":\"session.error\",\"properties\":{\"error\":{\"message\":\"beside the data\"}}}",
    );
    assert_eq!(opencode_session_error(&events), Some("beside the data"));
}

/// A codex failure event fails the run whatever its message holds: a null,
/// missing, or blank message is named for the event. Claude has no such
/// gap, as its verdict comes from the result's flags, not its messages.
#[test]
fn codex_failure_events_without_a_message_still_fail() {
    let events: Vec<CodexEvent> = parse_jsonl(concat!(
        "{\"type\":\"turn.failed\",\"error\":{\"message\":null}}\n",
        "{\"type\":\"turn.failed\",\"error\":null}\n",
        "{\"type\":\"turn.failed\"}\n",
        "{\"type\":\"error\",\"message\":null}\n",
        "{\"type\":\"error\",\"message\":\"  \"}\n",
        "{\"type\":\"item.completed\",\"item\":{\"type\":\"error\"}}\n",
        "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"partial\"}}\n",
    ));
    assert_eq!(
        codex_error_messages(&events),
        [
            "turn.failed names no message",
            "turn.failed names no message",
            "turn.failed names no message",
            "error event names no message",
            "error event names no message",
            "error item names no message",
        ]
    );

    let silent = "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"result\":null}";
    let result = claude_result(silent).expect("a result");
    assert!(result.failed());
    assert_eq!(claude_error_messages(silent), [] as [String; 0]);
}

/// Every harness error text is cut to the cap with a mark at the cut, at
/// the reader that collects it and again where a failure takes the list:
/// a failed claude result's text, a codex message, and a list handed to a
/// failure all come out the same size, and a cut text passes the cap
/// again unchanged.
#[test]
fn harness_error_texts_are_cut_at_the_cap() {
    let long = "a".repeat(HARNESS_ERROR_CHARS + 1);
    let cut = harness_error(&long);
    assert_eq!(cut.chars().count(), HARNESS_ERROR_CHARS);
    assert!(cut.ends_with(CUT_MARK), "{cut}");
    assert_eq!(harness_error(&cut), cut, "a cut text is cut no further");
    let short = "a".repeat(HARNESS_ERROR_CHARS);
    assert_eq!(harness_error(&format!("  {short} \n")), short);
    // The cut lands on a character boundary.
    let wide = "é".repeat(HARNESS_ERROR_CHARS + 1);
    assert_eq!(harness_error(&wide).chars().count(), HARNESS_ERROR_CHARS);

    let stdout = format!(
        "{{\"type\":\"result\",\"subtype\":\"error_max_turns\",\"is_error\":true,\"result\":\"{long}\",\"errors\":[\"{long}\"]}}"
    );
    assert_eq!(claude_error_messages(&stdout), std::slice::from_ref(&cut));

    let events: Vec<CodexEvent> =
        parse_jsonl(&format!("{{\"type\":\"error\",\"message\":\"{long}\"}}\n"));
    assert_eq!(codex_error_messages(&events), std::slice::from_ref(&cut));

    let failure = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        harness_errors: Vec::new(),
    }
    .with_harness_errors(vec![long]);
    assert_eq!(failure.harness_errors(), [cut]);
}

/// A field of the result rune does not report is skipped as it is read,
/// never held, and an array is judged by its last element alone: a number
/// out of range in an earlier message costs nothing.
#[test]
fn claude_result_skips_the_fields_it_does_not_report() {
    let padding = "0,".repeat(1024 * 1024);
    let object = format!(
        "{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"reply\",\"padding\":[{padding}0]}}"
    );
    let result = claude_result(&object).expect("a result");
    assert_eq!(result.result.as_deref(), Some("reply"));
    assert!(!result.failed());

    let array = format!(
        "[{{\"type\":\"system\",\"subtype\":\"init\",\"ratio\":1e999,\"padding\":[{padding}0]}},{object}]"
    );
    let result = claude_result(&array).expect("a result");
    assert_eq!(result.result.as_deref(), Some("reply"));
}

/// A JSON line before claude's result that rune cannot read fails the
/// read, named by its line on stdout. A line that reads, whatever it
/// holds, and a line that is no JSON (an `API Error:` note) stay
/// harmless, and a number out of range in a field rune does not report
/// is skipped with the field.
#[test]
fn claude_unreadable_line_before_the_result_fails_the_read() {
    let result =
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}";
    for (stdout, expected) in [
        (
            format!(
                "API Error: 429 slow down\n{{\"type\":\"system\",\"subtype\":\"init\"\n{result}\n"
            ),
            "claude print printed a JSON line rune cannot read (stdout line 2, column 33: EOF while parsing an object)",
        ),
        (
            format!("[{{\"type\":\"system\"}}\n{result}"),
            "claude print printed a JSON line rune cannot read (stdout line 1, column 18: EOF while parsing a list)",
        ),
    ] {
        let failure = claude_result(&stdout).expect_err(&stdout);
        assert_eq!(failure.to_string(), expected, "{stdout}");
    }

    let readable = format!("{{\"type\":\"system\",\"ratio\":1e999}}\nnot json {{\n{result}");
    let result = claude_result(&readable).expect("a result after readable lines");
    assert_eq!(result.result.as_deref(), Some("ok"));

    let skipped = "{\"type\":\"result\",\"subtype\":\"success\",\"ratio\":1e999,\"result\":\"ok\"}";
    let result = claude_result(skipped).expect("a result with a skipped field");
    assert_eq!(result.result.as_deref(), Some("ok"));
}

/// The model map is the one allocation of a result its text does not
/// bound, so its text is capped, and the reckoning behind the cap holds:
/// a JSON value takes 32 bytes.
#[test]
fn claude_model_usage_over_its_limit_is_rejected() {
    assert_eq!(std::mem::size_of::<serde_json::Value>(), 32);
    assert_eq!(MODEL_USAGE_LIMIT_BYTES, 128 * 1024);
    let note = "x".repeat(MODEL_USAGE_LIMIT_BYTES);
    let over = format!(
        "{{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"ok\",\"modelUsage\":{{\"claude-opus-4-6\":{{\"note\":\"{note}\"}}}}}}"
    );
    let failure = claude_result(&over).expect_err("an oversized map");
    assert!(
        failure.to_string().starts_with(&format!(
            "claude print returned no JSON result: modelUsage is {} bytes, over the {MODEL_USAGE_LIMIT_BYTES} rune reads",
            MODEL_USAGE_LIMIT_BYTES + "{\"claude-opus-4-6\":{\"note\":\"\"}}".len()
        )),
        "{failure}"
    );
    let within = format!(
        "{{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"ok\",\"modelUsage\":{{\"claude-opus-4-6\":{{\"note\":\"{}\"}}}}}}",
        &note[..MODEL_USAGE_LIMIT_BYTES - 64]
    );
    let result = claude_result(&within).expect("a result");
    assert_eq!(
        result.model_usage.map(|usage| usage.len()),
        Some(1),
        "a map within the limit"
    );
}

/// The array `--verbose` prints holds every session message; the result
/// is its last element.
#[test]
fn claude_result_is_read_from_a_verbose_message_array() {
    let stdout = concat!(
        "[{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\"},",
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"PING\"}]}},",
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"PING\",",
        "\"usage\":{\"output_tokens\":2}}]\n"
    );
    let result = claude_result(stdout).expect("a result");
    assert_eq!(result.result.as_deref(), Some("PING"));
    assert!(!result.failed());
    assert_eq!(result.usage.and_then(|usage| usage.output_tokens), Some(2));
    let result = claude_result("API Error: 429 slow down\n[{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"ok\"}]")
        .expect("a result after the error line");
    assert_eq!(result.result.as_deref(), Some("ok"));

    for (stdout, expected) in [
        (
            "[]",
            "claude print returned no JSON result: the message array is empty",
        ),
        (
            "[{\"type\":\"result\",\"subtype\":\"success\"},{\"type\":\"assistant\"}]",
            "claude print returned a assistant object, not a result",
        ),
    ] {
        let failure = claude_result(stdout).expect_err(stdout);
        assert_eq!(failure.to_string(), expected, "{stdout}");
    }
}

/// One predicate decides the verdict and the messages: a success that
/// lists errors fails, and a failed subtype keeps its text as a message
/// whatever its error flag says.
#[test]
fn claude_result_fails_on_errors_whatever_the_flags_say() {
    let with_errors = concat!(
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,",
        "\"result\":\"done\",\"errors\":[\"tool denied\"]}"
    );
    let result = claude_result(with_errors).expect("a result");
    assert!(result.failed());
    assert_eq!(
        result.failure_message(),
        "claude print reported errors beside a success"
    );
    assert_eq!(claude_error_messages(with_errors), ["tool denied", "done"]);

    let flagged =
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":true,\"result\":\"x\"}";
    let result = claude_result(flagged).expect("a result");
    assert!(result.failed());
    assert_eq!(
        result.failure_message(),
        "claude print reported a success flagged as an error"
    );

    let unflagged = concat!(
        "{\"type\":\"result\",\"subtype\":\"error_max_turns\",\"is_error\":false,",
        "\"result\":\"Reached the turn limit\"}"
    );
    let result = claude_result(unflagged).expect("a result");
    assert!(result.failed());
    assert_eq!(
        result.failure_message(),
        "claude print reported error_max_turns"
    );
    assert_eq!(claude_error_messages(unflagged), ["Reached the turn limit"]);

    let clean = "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\",\"errors\":[]}";
    assert!(!claude_result(clean).expect("a result").failed());
    assert_eq!(claude_error_messages(clean), [] as [String; 0]);
}

/// Each message once: a repeated `API Error:` line, an error repeated in
/// the list, and a text the list already carries all collapse.
#[test]
fn claude_error_messages_are_deduplicated_in_order() {
    let stdout = concat!(
        "API Error: 429 rate limited\n",
        "API Error: 429 rate limited\n",
        "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,",
        "\"result\":\"quota gone\",",
        "\"errors\":[\"API Error: 429 rate limited\",\"quota gone\",\"quota gone\",\"  \"]}\n"
    );
    assert_eq!(
        claude_error_messages(stdout),
        ["API Error: 429 rate limited", "quota gone"]
    );
}

/// Credential values leave the model names and the per-model usage the
/// same way they leave the text: known values only, at any depth.
#[test]
fn model_usage_redaction_hides_known_values_in_keys_and_strings() {
    let secrets = vec!["fake-route-secret".to_string()];
    let usage: ModelUsage = serde_json::from_str(concat!(
        "{\"claude-opus-4-6\":{\"inputTokens\":11,\"note\":\"token fake-route-secret here\",",
        "\"nested\":{\"fake-route-secret\":[\"fake-route-secret\",1,null]}},",
        "\"fake-route-secret\":{\"outputTokens\":7}}"
    ))
    .expect("usage");
    let redacted = redact_model_usage(usage, &secrets);
    let text = serde_json::to_string(&redacted).expect("json");
    assert!(!text.contains("fake-route-secret"), "{text}");
    assert_eq!(
        text,
        concat!(
            "{\"<redacted>\":{\"outputTokens\":7},",
            "\"claude-opus-4-6\":{\"inputTokens\":11,\"nested\":{\"<redacted>\":[\"<redacted>\",1,null]},",
            "\"note\":\"token <redacted> here\"}}"
        )
    );
}

/// The claude capture limit leaves room for the JSON escaping of an
/// answer at the ordinary limit, and stays bounded.
#[test]
fn claude_capture_limit_covers_the_escaped_answer_and_stays_bounded() {
    let answer = "\u{1}".repeat(DEFAULT_OUTPUT_LIMIT_BYTES);
    let escaped = serde_json::to_string(&answer).expect("json");
    assert!(
        escaped.len() <= CLAUDE_OUTPUT_LIMIT_BYTES,
        "{}",
        escaped.len()
    );
    // Bounded: one more escaped answer of that size would not fit.
    assert!(
        escaped.len() * 2 > CLAUDE_OUTPUT_LIMIT_BYTES,
        "{}",
        escaped.len()
    );
}

#[test]
fn codex_usage_maps_the_cache_counts_and_adds_the_total() {
    let events: Vec<CodexEvent> = parse_jsonl(concat!(
        "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":100,\"cached_input_tokens\":60,",
        "\"cache_write_input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":5}}\n"
    ));
    let usage = events[0].usage.as_ref().expect("usage").token_usage();

    assert_eq!(
        usage,
        TokenUsage {
            input_tokens: Some(100),
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(60),
            output_tokens: Some(20),
            reasoning_output_tokens: Some(5),
            total_tokens: Some(120),
        }
    );
    let events: Vec<CodexEvent> =
        parse_jsonl("{\"type\":\"turn.completed\",\"usage\":{\"output_tokens\":20}}\n");
    let partial = events[0].usage.as_ref().expect("usage").token_usage();
    assert_eq!(partial.input_tokens, None);
    assert_eq!(partial.total_tokens, None);
    assert_eq!(partial.output_tokens, Some(20));
}

#[test]
fn opencode_usage_sums_the_steps() {
    let events: Vec<OpencodeEvent> = parse_jsonl(concat!(
        "{\"type\":\"step_finish\",\"part\":{\"messageID\":\"m\",\"tokens\":{\"total\":30,\"input\":20,\"output\":5,\"reasoning\":2,\"cache\":{\"read\":3,\"write\":0}}}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"messageID\":\"m\",\"tokens\":{\"input\":10,\"output\":7}}}\n",
    ));
    let steps: Vec<&OpencodeTokens> = events
        .iter()
        .filter_map(|event| event.part.as_ref())
        .filter_map(|part| part.tokens.as_ref())
        .collect();
    let usage = OpencodeTokens::sum(&steps);

    assert_eq!(
        usage,
        TokenUsage {
            input_tokens: Some(30),
            cache_creation_input_tokens: Some(0),
            cache_read_input_tokens: Some(3),
            output_tokens: Some(12),
            reasoning_output_tokens: Some(2),
            total_tokens: Some(30),
        }
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
    let failure = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: "{\"type\":\"error\"}".to_string(),
        stderr_tail: "cache warning".to_string(),
        harness_errors: Vec::new(),
    }
    .with_harness_errors(codex_error_messages(&events));
    let text = failure.to_string();
    assert!(
        text.contains(
            "harness errors:\n  hooks loaded twice\n  You've hit your usage limit.\n  turn aborted"
        ),
        "{text}"
    );
    assert!(text.contains("stderr tail:\n  cache warning"), "{text}");
    assert_eq!(failure.harness_errors().len(), 3);
}

#[test]
fn signal_and_output_limit_failures_carry_both_tails() {
    let signal = SurfaceFailure::from(ProcessFailure::ForwardedSignal {
        signal: 15,
        stdout: "half an answer".to_string(),
        stderr: "still working".to_string(),
    });
    assert_eq!(signal.tails(), Some(("half an answer", "still working")));
    let text = signal.to_string();
    assert!(
        text.starts_with("process stopped after forwarding signal 15"),
        "{text}"
    );
    assert!(text.contains("stdout tail:\n  half an answer"), "{text}");
    assert!(text.contains("stderr tail:\n  still working"), "{text}");

    let limit = SurfaceFailure::from(ProcessFailure::OutputLimit {
        stream: "stdout",
        limit: 64,
        stdout: "x".repeat(64),
        stderr: "noisy".to_string(),
    });
    assert_eq!(limit.tails(), Some((&"x".repeat(64)[..], "noisy")));
    assert!(
        limit.to_string().contains("stderr tail:\n  noisy"),
        "{limit}"
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
        Some(17)
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
        let refused = refuse_unsupported_mode(&invocation).expect_err("must refuse");
        assert!(
            refused
                .to_string()
                .contains("answer-only mode needs a harness"),
            "{surface:?}: {refused}"
        );
    }
    let mut claude = invocation(Surface::Claude, &[]);
    claude.mode = AccessMode::AnswerOnly;
    assert!(refuse_unsupported_mode(&claude).is_ok());
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
        "CODEX_SESSION_ID",
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
            "CODEX_SESSION_ID",
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
fn a_later_settings_object_replaces_env_whole_in_the_child_view() {
    let claude = invocation(
        Surface::Claude,
        &[
            "--settings",
            r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"fake-early-secret"}}"#,
            "--settings",
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9/late"},"apiKeyHelper":"echo fake-helper-secret"}"#,
        ],
    );
    let keys = route_keys(&claude);
    assert!(
        keys.contains(&OsString::from("ANTHROPIC_BASE_URL")),
        "{keys:?}"
    );
    assert!(
        !keys.contains(&OsString::from("ANTHROPIC_AUTH_TOKEN")),
        "{keys:?}"
    );
    assert_eq!(
        settings_env(&claude.extra_args),
        env(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/late")])
    );
    assert_eq!(
        launch::env_value(&effective_env(&claude), "ANTHROPIC_BASE_URL").as_deref(),
        Some("http://127.0.0.1:9/late")
    );
}

#[test]
fn child_secrets_include_credentials_inside_retained_structured_arguments() {
    let mut claude = invocation(
        Surface::Claude,
        &[
            "--settings",
            r#"{"apiKeyHelper":"echo fake-helper-secret","env":{"X_TOKEN":"fake-settings-secret","X":"1"},"hooks":{"PreToolUse":[{"command":"fake-hook-secret"}]}}"#,
        ],
    );
    claude.env = env(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8317")]);
    let secrets = child_secrets(&claude);
    assert!(
        secrets.contains(&"echo fake-helper-secret".to_string()),
        "{secrets:?}"
    );
    assert!(
        secrets.contains(&"fake-settings-secret".to_string()),
        "{secrets:?}"
    );
    assert!(!secrets.contains(&"1".to_string()), "{secrets:?}");

    let codex = invocation(
        Surface::Codex,
        &[
            "-c",
            "http_headers.Authorization=\"Bearer fake-header-secret\"",
            "--config=experimental_bearer_token=\"fake-bearer-secret\"",
            "-c",
            "model_reasoning_effort=\"high\"",
        ],
    );
    let secrets = child_secrets(&codex);
    assert!(
        secrets.contains(&"Bearer fake-header-secret".to_string()),
        "{secrets:?}"
    );
    assert!(
        secrets.contains(&"fake-bearer-secret".to_string()),
        "{secrets:?}"
    );
    assert!(!secrets.contains(&"high".to_string()), "{secrets:?}");
}

#[test]
fn child_secrets_hold_the_parts_a_harness_quotes_alone() {
    // Each header value of a `*_HEADERS` variable, and the token after the
    // scheme word, from the process env and from the settings env alike.
    let mut claude = invocation(
        Surface::Claude,
        &[
            "--settings",
            r#"{"env":{"ANTHROPIC_CUSTOM_HEADERS":"X-Proxy-Key: fake-settings-header\nX-Plain: fake-plain-header"}}"#,
            "--api-key",
            "fake-flag-secret",
            "--token=fake-attached-secret",
        ],
    );
    claude.env = env(&[(
        "ANTHROPIC_CUSTOM_HEADERS",
        "Authorization: Bearer fake-env-bearer\nX-Other: fake-env-other",
    )]);
    let secrets = child_secrets(&claude);
    for expected in [
        "Authorization: Bearer fake-env-bearer\nX-Other: fake-env-other",
        "Bearer fake-env-bearer",
        "fake-env-bearer",
        "fake-env-other",
        "fake-settings-header",
        "fake-plain-header",
        "fake-flag-secret",
        "fake-attached-secret",
    ] {
        assert!(
            secrets.contains(&expected.to_string()),
            "{expected}: {secrets:?}"
        );
    }
    assert!(!secrets.contains(&"--api-key".to_string()), "{secrets:?}");

    // A credential nested anywhere inside a retained codex table value.
    let codex = invocation(
        Surface::Codex,
        &[
            "-c",
            r#"model_providers.proxy={ base_url = "http://127.0.0.1:9", http_headers = { Authorization = "Basic fake-nested-basic" } }"#,
            "-c",
            "model_providers.proxy.wire_api=\"chat\"",
        ],
    );
    let secrets = child_secrets(&codex);
    assert!(
        secrets.contains(&"Basic fake-nested-basic".to_string()),
        "{secrets:?}"
    );
    assert!(
        secrets.contains(&"fake-nested-basic".to_string()),
        "{secrets:?}"
    );
    assert!(!secrets.contains(&"chat".to_string()), "{secrets:?}");
    assert!(
        !secrets.contains(&"http://127.0.0.1:9".to_string()),
        "{secrets:?}"
    );
}

#[test]
fn a_settings_file_path_warning_says_the_run_uses_the_inherited_endpoint() {
    let filtered = filter_surface_args(&invocation(
        Surface::Claude,
        &["--settings", "/home/me/gateway-settings.json"],
    ));
    assert_eq!(filtered.kept, [] as [std::ffi::OsString; 0]);
    assert!(
        filtered.warnings[0].contains("dropped --settings <redacted>"),
        "{}",
        filtered.warnings[0]
    );
    assert!(
        filtered.warnings[0].contains("the run uses the route's or the inherited endpoint"),
        "{}",
        filtered.warnings[0]
    );
    assert!(
        !filtered.warnings[0].contains("gateway-settings"),
        "{}",
        filtered.warnings[0]
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
    // The attached object comes later and replaces `env` whole, as claude's
    // own merge does: only its key is route-set.
    assert!(
        !keys.contains(&OsString::from("ANTHROPIC_BASE_URL")),
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
    // A profile that sets the scrub to 0 on purpose keeps its value.
    let parent = keys(&["CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "CLAUDE_CODE_ENTRYPOINT"]);
    let route = env(&[("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "0")]);

    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&route));

    assert_eq!(isolation.session, ["CLAUDE_CODE_ENTRYPOINT"]);
}

#[test]
fn owner_controls_with_the_claude_code_prefix_survive() {
    let parent = keys(&[
        "CLAUDE_CODE_CLIENT_CERT",
        "CLAUDE_CODE_CLIENT_KEY",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_MAX_OUTPUT_TOKENS",
        "CLAUDE_CODE_MAX_CONTEXT_TOKENS",
        "CLAUDE_CODE_AUTO_COMPACT_WINDOW",
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDE_CODE_MESSAGING_SOCKET",
        "CLAUDE_CODE_SIMPLE",
        "CLAUDE_AGENT_SDK_VERSION",
        "CLAUDE_PREVIEW_PORT",
        "CODEX_SESSION_ID",
    ]);

    let isolation = super::isolation::isolate(&parent, &[]);

    assert_eq!(
        isolation.session,
        [
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_MESSAGING_SOCKET",
            "CLAUDE_CODE_SIMPLE",
            "CLAUDE_AGENT_SDK_VERSION",
            "CLAUDE_PREVIEW_PORT",
            "CODEX_SESSION_ID",
        ]
    );
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
        harness_errors: Vec::new(),
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
    let failure = SurfaceFailure::from(ProcessFailure::Timeout {
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
            harness_errors: Vec::new(),
        }
    );
    assert!(
        failure
            .to_string()
            .contains("stderr tail:\n  model refused"),
        "{failure}"
    );
    let short_beside_its_name = SurfaceFailure::Reported("no answer".to_string()).with_output(
        &ProcessOutput {
            termination: ProcessTermination::Exited(0),
            stdout: String::new(),
            stderr: "auth failed for ANTHROPIC_AUTH_TOKEN=ab12".to_string(),
        },
        &[],
    );
    assert!(
        short_beside_its_name
            .to_string()
            .contains("ANTHROPIC_AUTH_TOKEN=<redacted>"),
        "{short_beside_its_name}"
    );
    let untouched = SurfaceFailure::Io("disk".to_string()).with_output(&output, &[]);
    assert_eq!(untouched, SurfaceFailure::Io("disk".to_string()));
}

#[test]
fn sandbox_signature_on_stderr_or_in_a_harness_error_adds_a_hint() {
    let certificate = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: "TLS handshake failed: OSStatus -26276".to_string(),
        harness_errors: Vec::new(),
    };
    let hint = certificate.hint().expect("certificate hint");
    assert!(hint.contains("certificate"), "{hint}");
    assert!(
        certificate.to_string().contains(&format!("\nhint: {hint}")),
        "{certificate}"
    );

    let reported = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail:
            "API Error: SSL certificate verification failed (UNABLE_TO_VERIFY_LEAF_SIGNATURE)"
                .to_string(),
        stderr_tail: String::new(),
        harness_errors: vec![
            "API Error: SSL certificate verification failed (UNABLE_TO_VERIFY_LEAF_SIGNATURE)"
                .to_string(),
        ],
    };
    assert!(
        reported
            .hint()
            .is_some_and(|hint| hint.contains("filtering proxy"))
    );

    let keyring = SurfaceFailure::NoAnswer {
        message: "codex exec produced no final agent message".to_string(),
        stdout_tail: String::new(),
        stderr_tail: "failed to load CLI auth from keyring".to_string(),
        harness_errors: Vec::new(),
    };
    assert!(keyring.hint().is_some_and(|hint| hint.contains("keyring")));

    let spawn = SurfaceFailure::from(ProcessFailure::Spawn(
        "claude not runnable: Operation not permitted (os error 1)".to_string(),
    ));
    assert!(spawn.hint().is_some_and(|hint| hint.contains("sandbox")));
}

#[test]
fn model_text_on_stdout_never_earns_a_sandbox_hint() {
    let quoted = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: "The shell said: Operation not permitted. Try again outside.".to_string(),
        stderr_tail: String::new(),
        harness_errors: Vec::new(),
    };
    assert_eq!(quoted.hint(), None);

    let certificate_in_text = SurfaceFailure::NoAnswer {
        message: "claude print produced no final response".to_string(),
        stdout_tail: "node prints UNABLE_TO_VERIFY_LEAF_SIGNATURE when a CA is missing".to_string(),
        stderr_tail: String::new(),
        harness_errors: Vec::new(),
    };
    assert_eq!(certificate_in_text.hint(), None);

    // A syscall denial counts on stderr alone, not as a harness error.
    let harness_only = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        harness_errors: vec!["Operation not permitted".to_string()],
    };
    assert_eq!(harness_only.hint(), None);

    let plain = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(2),
        stdout_tail: "usage: claude".to_string(),
        stderr_tail: String::new(),
        harness_errors: Vec::new(),
    };
    assert_eq!(plain.hint(), None);
}

#[test]
fn a_forced_stop_keeps_the_harness_errors_reported_before_it() {
    let timeout = stopped_with_errors(
        ProcessFailure::Timeout {
            timeout: Duration::from_secs(30),
            stdout: "Thinking.\nAPI Error: 403 OSStatus -26276\n".to_string(),
            stderr: String::new(),
        },
        claude_error_messages,
    );
    assert_eq!(timeout.harness_errors(), ["API Error: 403 OSStatus -26276"]);
    assert!(
        timeout
            .hint()
            .is_some_and(|hint| hint.contains("certificate")),
        "{timeout}"
    );
    assert!(
        timeout
            .to_string()
            .contains("harness errors:\n  API Error: 403 OSStatus -26276"),
        "{timeout}"
    );

    let signal = stopped_with_errors(
        ProcessFailure::ForwardedSignal {
            signal: 15,
            stdout: "{\"type\":\"error\",\"message\":\"TLS: UNABLE_TO_VERIFY_LEAF_SIGNATURE\"}\n"
                .to_string(),
            stderr: String::new(),
        },
        |stdout| codex_error_messages(&parse_jsonl(stdout)),
    );
    assert_eq!(
        signal.harness_errors(),
        ["TLS: UNABLE_TO_VERIFY_LEAF_SIGNATURE"]
    );
    assert!(
        signal
            .hint()
            .is_some_and(|hint| hint.contains("filtering proxy")),
        "{signal}"
    );

    // A failure without captured output has no errors to parse.
    let spawn = stopped_with_errors(
        ProcessFailure::Spawn("claude not runnable".to_string()),
        claude_error_messages,
    );
    assert_eq!(spawn.harness_errors(), [] as [String; 0]);

    // The redaction reads the kept errors too.
    let secrets = vec!["sk-fake-stop-secret".to_string()];
    let hidden = stopped_with_errors(
        ProcessFailure::Timeout {
            timeout: Duration::from_secs(1),
            stdout: "API Error: 401 bad key sk-fake-stop-secret\n".to_string(),
            stderr: String::new(),
        },
        claude_error_messages,
    )
    .redacted(&secrets);
    assert_eq!(
        hidden.harness_errors(),
        ["API Error: 401 bad key <redacted>"]
    );
    assert!(
        !hidden.to_string().contains("sk-fake-stop-secret"),
        "{hidden}"
    );
}

#[test]
fn a_network_allowlist_block_names_the_blocked_host() {
    let reported = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        harness_errors: vec![
            "API Error: 403 {\"error\":\"Connection blocked by network allowlist\",\"url\":\"https://api.example.test/v1/messages\"}"
                .to_string(),
        ],
    };
    let hint = reported.hint().expect("allowlist hint");
    assert!(hint.contains("api.example.test"), "{hint}");
    assert!(hint.contains("allowlist"), "{hint}");
    assert!(!hint.contains("/v1/messages"), "{hint}");

    let on_stderr = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: "proxy: blocked-by-allowlist".to_string(),
        harness_errors: Vec::new(),
    };
    let hint = on_stderr.hint().expect("allowlist hint");
    assert!(hint.contains("blocked the endpoint's host:"), "{hint}");

    // Model text on stdout never counts.
    let quoted = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: "The proxy said: Connection blocked by network allowlist".to_string(),
        stderr_tail: String::new(),
        harness_errors: Vec::new(),
    };
    assert_eq!(quoted.hint(), None);

    assert_eq!(
        url_host("see https://user:sk-fake@api.example.test:8443/v1?x=1 now"),
        Some("api.example.test:8443".to_string())
    );
    assert_eq!(
        url_host("{\"url\":\"http://127.0.0.1:8080\"}"),
        Some("127.0.0.1:8080".to_string())
    );
    assert_eq!(url_host("no url here"), None);
    assert_eq!(url_host("https://"), None);
}

#[test]
fn a_failure_hides_the_endpoint_password_and_still_names_its_host() {
    // The harness echoes the endpoint it could not reach, password and
    // all. The redaction reads the kept errors, and the hint reads them
    // redacted: `u:<redacted>@host` still yields the host.
    let reported = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: "proxy: https://u:sk-fake-url-password@api.example.test/v1 refused".to_string(),
        harness_errors: vec![
            "API Error: 403 {\"error\":\"Connection blocked by network allowlist\",\"url\":\"https://u:sk-fake-url-password@api.example.test/v1/messages?api_key=sk-fake-query-secret\"}"
                .to_string(),
        ],
    }
    .redacted(&[]);
    assert_eq!(
        reported.harness_errors(),
        [
            "API Error: 403 {\"error\":\"Connection blocked by network allowlist\",\"url\":\"https://u:<redacted>@api.example.test/v1/messages?api_key=<redacted>\"}"
        ]
    );
    assert!(!reported.to_string().contains("sk-fake-"), "{reported}");
    let hint = reported.hint().expect("allowlist hint");
    assert!(hint.contains("host api.example.test:"), "{hint}");
    assert_eq!(
        url_host("https://u:<redacted>@host:8443/v1?api_key=<redacted>"),
        Some("host:8443".to_string())
    );
}

#[test]
fn launch_merges_several_settings_objects_into_the_last_one() {
    let args: Vec<OsString> = [
        "--settings",
        r#"{"env":{"A":"1"},"model":"x"}"#,
        "--verbose",
        r#"--settings={"env":{"B":"2"}}"#,
        "--debug",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let merged = strings(&merge_settings_args(&args));
    assert_eq!(merged.len(), 4, "{merged:?}");
    assert_eq!(merged[0], "--verbose");
    assert_eq!(merged[1], "--settings");
    let settings: serde_json::Value = serde_json::from_str(&merged[2]).expect("settings json");
    assert_eq!(settings, serde_json::json!({"env":{"B":"2"},"model":"x"}));
    assert_eq!(merged[3], "--debug");

    // One object, or a file path beside it, stays as it is.
    let single: Vec<OsString> = ["--settings", r#"{"env":{"A":"1"}}"#, "--verbose"]
        .into_iter()
        .map(OsString::from)
        .collect();
    assert_eq!(merge_settings_args(&single), single);
    let with_path: Vec<OsString> = [
        "--settings",
        "/home/me/settings.json",
        "--settings",
        r#"{"env":{"A":"1"}}"#,
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    assert_eq!(merge_settings_args(&with_path), with_path);
}

#[test]
fn claude_api_error_lines_are_harness_errors() {
    let stdout =
        "Thinking.\nAPI Error: 400 {\"error\":\"unknown model\"}\n  API Error: retry failed\nok\n";
    assert_eq!(
        claude_error_messages(stdout),
        [
            "API Error: 400 {\"error\":\"unknown model\"}",
            "API Error: retry failed"
        ]
    );
    assert_eq!(claude_error_messages("plain answer"), [] as [String; 0]);
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

    let mut agy_invocation = invocation(Surface::Agy, &[]);
    agy_invocation.mode = AccessMode::WorkspaceWrite;
    let agy = planned_request(&agy_invocation).expect("agy plan");
    let agy_args = strings(&agy.args);
    let printed = agy_args
        .windows(2)
        .find(|pair| pair[0] == "--print")
        .map(|pair| pair[1].as_str())
        .expect("a --print value");
    assert!(printed.starts_with(AGY_READ_INSTRUCTION), "{agy_args:?}");
    assert!(printed.ends_with("\n\n<prompt>"), "{agy_args:?}");
    assert!(
        agy_args
            .windows(2)
            .any(|pair| pair == ["--log-file", "<scratch>/agy.log"]),
        "{agy_args:?}"
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
    let read_only_agy = invocation(Surface::Agy, &[]);
    let refused = planned_request(&read_only_agy).expect_err("read-only agy");
    assert!(
        refused
            .to_string()
            .contains("agy cannot enforce read-only access"),
        "{refused}"
    );
    assert!(
        refused.to_string().contains("--mode workspace-write"),
        "{refused}"
    );
}

#[test]
fn agy_takes_workspace_write_alone_and_is_told_to_read_with_its_file_tools() {
    assert!(unsupported_mode(Surface::Agy, AccessMode::WorkspaceWrite).is_none());
    let refusal = unsupported_mode(Surface::Agy, AccessMode::ReadOnly).expect("refused");
    assert!(
        refusal.contains("cannot enforce read-only access"),
        "{refusal}"
    );
    assert!(refusal.contains("workspace-write"), "{refusal}");
    assert!(refusal.contains("jj workspace"), "{refusal}");
    for surface in [
        Surface::Claude,
        Surface::Codex,
        Surface::Grok,
        Surface::Opencode,
    ] {
        assert_eq!(unsupported_mode(surface, AccessMode::ReadOnly), None);
    }

    let mut invocation = invocation(Surface::Agy, &[]);
    invocation.mode = AccessMode::WorkspaceWrite;
    invocation.system_prompt = "Be brief.".to_string();
    let args = strings(&agy_args(&invocation, &PathBuf::from("/scratch/agy.log")));
    assert_eq!(
        &args[..5],
        [
            "--sandbox",
            "--mode",
            "accept-edits",
            "--log-file",
            "/scratch/agy.log"
        ]
    );
    assert_eq!(args[5], "--print");
    assert_eq!(
        args[6],
        format!("{AGY_READ_INSTRUCTION}\n\nBe brief.\n\nInspect only")
    );
    assert_eq!(&args[7..9], ["--output-format", "json"]);
    // A profile cannot point the log elsewhere.
    let filtered = filter_surface_args(&super::tests::invocation(
        Surface::Agy,
        &["--log-file", "/elsewhere.log", "--verbose"],
    ));
    assert_eq!(strings(&filtered.kept), ["--verbose"]);
}

#[test]
fn agy_soft_deny_in_its_log_names_the_denied_tool_in_the_hint() {
    let log = concat!(
        "I1022 12:32:09.920100    1293 main.go:40] starting\n",
        "I1022 12:32:09.920149    1293 tool_confirmation_manager.go:212] ",
        "Print mode: soft-denying tool confirmation \"RunCommand\" at step 5\n",
        "I1022 12:32:10.000000    1293 main.go:80] done\n",
    );
    assert_eq!(
        agy_soft_denials(log),
        ["Print mode: soft-denying tool confirmation \"RunCommand\" at step 5"]
    );
    assert_eq!(agy_soft_denials("nothing denied\n"), Vec::<String>::new());

    let silent = SurfaceFailure::NoAnswer {
        message: "agy print produced no final response".to_string(),
        stdout_tail: "{\"response\":\"\"}".to_string(),
        stderr_tail: String::new(),
        harness_errors: agy_soft_denials(log),
    };
    let hint = silent.hint().expect("a hint");
    assert!(hint.contains("shell command"), "{hint}");
    assert!(hint.contains("RunCommand"), "{hint}");
    assert!(hint.contains("headless"), "{hint}");

    let other = SurfaceFailure::NoAnswer {
        message: "agy print produced no final response".to_string(),
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        harness_errors: vec![
            "Print mode: soft-denying tool confirmation \"WriteFile\" at step 2".to_string(),
        ],
    };
    let hint = other.hint().expect("a hint");
    assert!(hint.contains("its WriteFile call"), "{hint}");

    // The line counts on stderr too, and a run with no denial earns none.
    let on_stderr = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: String::new(),
        stderr_tail: "Print mode: soft-denying tool confirmation \"RunCommand\" at step 1"
            .to_string(),
        harness_errors: Vec::new(),
    };
    assert!(on_stderr.hint().is_some());
    let plain = SurfaceFailure::NoAnswer {
        message: "agy print produced no final response".to_string(),
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        harness_errors: Vec::new(),
    };
    assert_eq!(plain.hint(), None);
}

#[test]
fn clean_grok_configuration_keeps_only_the_model_key_map() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("config.toml");
    let target = temporary.path().join("clean.toml");
    std::fs::write(
        &source,
        concat!(
            "[cli]\nauto_update = true\n\n",
            "[marketplace]\nurl = \"https://example.test\"\n\n",
            "[model.\"grok-4.5\"]\nenv_key = \"FAKE_GROK_KEY\"\nbase_url = \"https://api.example.test\"\n\n",
            "[model.\"grok-4.6\"]\nenv_key = \"FAKE_GROK_KEY_TWO\"\n\n",
            "[model.local]\nbase_url = \"http://127.0.0.1:1\"\n\n",
            "[permission]\nallow = [\"Bash\"]\n\n",
            "[sandbox]\nmode = \"workspace\"\n\n",
            "[session]\npersist = true\n\n",
            "[shell_environment_policy]\ninherit = \"all\"\n\n",
            "[telemetry]\nenabled = true\n\n",
            "[tools]\nweb = true\n\n",
            "[ui]\ntheme = \"dark\"\n",
        ),
    )
    .expect("source configuration");

    let kept = copy_grok_model_keys(&source, &target).expect("clean configuration");
    assert_eq!(kept, ["model.grok-4.5.env_key", "model.grok-4.6.env_key"]);
    let clean: toml::Value =
        toml::from_str(&std::fs::read_to_string(&target).expect("target")).expect("valid TOML");
    let table = clean.as_table().expect("a table");
    assert_eq!(table.keys().collect::<Vec<_>>(), ["model"]);
    assert_eq!(
        clean["model"]["grok-4.5"]["env_key"].as_str(),
        Some("FAKE_GROK_KEY")
    );
    assert_eq!(
        clean["model"]["grok-4.6"]["env_key"].as_str(),
        Some("FAKE_GROK_KEY_TWO")
    );
    assert!(clean["model"]["grok-4.5"].get("base_url").is_none());
    assert!(clean["model"].get("local").is_none());
    for dropped in [
        "cli",
        "marketplace",
        "permission",
        "sandbox",
        "session",
        "shell_environment_policy",
        "telemetry",
        "tools",
        "ui",
    ] {
        assert!(clean.get(dropped).is_none(), "{dropped} was kept");
    }

    // No configuration, no file, no keys.
    let missing = temporary.path().join("absent.toml");
    let untouched = temporary.path().join("untouched.toml");
    assert_eq!(
        copy_grok_model_keys(&missing, &untouched).expect("no source"),
        Vec::<String>::new()
    );
    assert!(!untouched.exists());
}

#[test]
fn the_grok_key_map_names_the_variables_whose_values_are_credentials() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("config.toml");
    std::fs::write(
        &source,
        concat!(
            "[model.\"grok-4.5\"]\nenv_key = \"GROK_SEAT_HANDLE\"\n\n",
            "[model.\"grok-4.6\"]\nenv_key = \"XAI_SECOND_HANDLE\"\nbase_url = \"https://api.example.test\"\n\n",
            "[model.local]\nbase_url = \"http://127.0.0.1:1\"\n",
        ),
    )
    .expect("source configuration");
    let mut names = grok_key_variable_names(&source);
    names.sort();
    assert_eq!(names, ["GROK_SEAT_HANDLE", "XAI_SECOND_HANDLE"]);
    assert_eq!(
        grok_key_variable_names(&temporary.path().join("absent.toml")),
        Vec::<String>::new()
    );
    // One model's entry names its variable. The two entries name different
    // variables and sit at the same distance from `grok-4`, so a model
    // without an entry gets nothing, and so does a model of another family
    // or a missing key map.
    assert_eq!(
        grok_model_env_key(&source, "grok-4.5").variable(),
        Some("GROK_SEAT_HANDLE")
    );
    assert_eq!(
        grok_model_env_key(&source, "grok-4.6").variable(),
        Some("XAI_SECOND_HANDLE")
    );
    assert_eq!(grok_model_env_key(&source, "local").variable(), None);
    let tie = grok_model_env_key(&source, "grok-4");
    assert_eq!(tie.variable(), None);
    assert!(
        tie.note()
            .is_some_and(|note| note.contains("grok-4.5 and grok-4.6")),
        "{tie:?}"
    );
    assert_eq!(
        grok_model_env_key(&temporary.path().join("absent.toml"), "grok-4.5").variable(),
        None
    );
}

#[test]
fn the_grok_key_map_reads_an_unquoted_dotted_id_as_one_model() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("config.toml");
    // `[model.grok-4.5]` nests as model, grok-4, 5: the same entry as the
    // quoted form, and the same kept key name in the dry run.
    std::fs::write(
        &source,
        "[model.grok-4.5]\nenv_key = \"XAI_NESTED_HANDLE\"\n\n[model.grok-3]\nenv_key = \"XAI_NESTED_HANDLE\"\n",
    )
    .expect("source configuration");
    assert_eq!(
        grok_model_env_key(&source, "grok-4.5").variable(),
        Some("XAI_NESTED_HANDLE")
    );
    let target = temporary.path().join("clean.toml");
    let mut names = copy_grok_model_keys(&source, &target).expect("clean copy");
    names.sort();
    assert_eq!(names, ["model.grok-3.env_key", "model.grok-4.5.env_key"]);
    // Every grok entry names one variable: a grok model without an entry
    // gets it, and the choice says so.
    let guessed = grok_model_env_key(&source, "grok-4.7");
    assert_eq!(guessed.variable(), Some("XAI_NESTED_HANDLE"));
    assert_eq!(
        guessed.note(),
        Some("no entry for grok-4.7: every grok entry names XAI_NESTED_HANDLE")
    );
}

#[test]
fn a_malformed_grok_configuration_names_the_place_and_quotes_nothing() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("config.toml");
    std::fs::write(
        &source,
        "[model.\"grok-4.5\"]\nenv_key = \"FAKE_GROK_KEY\"\nsecret = \"fake-grok-secret-value\nbroken\n",
    )
    .expect("source configuration");
    let error = read_grok_config(&source).expect_err("malformed");
    let message = error.to_string();
    assert!(message.contains("at line 3, column"), "{message}");
    assert!(!message.contains("fake-grok-secret-value"), "{message}");
    assert!(!message.contains("secret"), "{message}");
    assert!(!message.contains("broken"), "{message}");
    assert_eq!(line_and_column("ab\ncd\nef", 4), (2, 2));
    assert_eq!(line_and_column("ab", 99), (1, 3));
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
                "Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit"
            ]),
        "{args:?}"
    );
    let settings = claude_settings_in(&args);
    assert_eq!(settings.len(), 1, "{args:?}");
    assert_eq!(settings[0]["sandbox"]["enabled"], true);
    assert_eq!(settings[0]["sandbox"]["failIfUnavailable"], true);
    assert_eq!(settings[0]["sandbox"]["autoAllowBashIfSandboxed"], true);
    assert_eq!(settings[0]["sandbox"]["allowUnsandboxedCommands"], false);
    assert!(
        settings[0]["sandbox"].get("excludedCommands").is_none(),
        "the run names no excluded command: {:?}",
        settings[0]
    );
}

#[test]
fn a_profile_sandbox_setting_is_dropped_by_key_and_the_runs_sandbox_wins() {
    let mut invocation = invocation(
        Surface::Claude,
        &[
            "--settings",
            r#"{"sandbox":{"enabled":false,"failIfUnavailable":false},"env":{"X":"1"}}"#,
        ],
    );
    invocation.mode = AccessMode::WorkspaceWrite;
    let filtered = filter_surface_args(&invocation);
    assert_eq!(filtered.warnings.len(), 1, "{:?}", filtered.warnings);
    assert!(filtered.warnings[0].contains("owns settings key sandbox"));
    assert!(
        !filtered.warnings[0].contains("false"),
        "{}",
        filtered.warnings[0]
    );

    let invocation = SurfaceInvocation {
        extra_args: filtered.kept,
        ..invocation
    };
    let settings = claude_settings_in(&strings(&claude_args(&invocation)));
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0]["sandbox"]["enabled"], true);
    assert_eq!(settings[0]["sandbox"]["failIfUnavailable"], true);
    assert_eq!(settings[0]["sandbox"]["allowUnsandboxedCommands"], false);
    assert_eq!(settings[0]["env"]["X"], "1");
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
    // A key under no credential name still carries one: an MCP server's
    // `args` array holds the flag and the key side by side. Hidden whole.
    let args = filter_surface_args(&invocation(
        Surface::Codex,
        &[
            "-c",
            "mcp_servers.demo.args=[\"--api-key\",\"fake-mcp-arg-secret\"]",
        ],
    ));
    assert_eq!(args.warnings.len(), 1, "{:?}", args.warnings);
    assert!(
        args.warnings[0]
            .contains("config key mcp_servers.demo.args; the profile value <redacted> is dropped"),
        "{}",
        args.warnings[0]
    );
    assert!(!args.warnings[0].contains("fake-"), "{}", args.warnings[0]);
    // A key whose values come from a fixed list shows its value.
    let plain = filter_surface_args(&invocation(
        Surface::Codex,
        &[
            "-c",
            "sandbox_mode=\"danger-full-access\"",
            "-c",
            "approval_policy=\"never\"",
        ],
    ));
    assert_eq!(plain.warnings.len(), 2, "{:?}", plain.warnings);
    assert!(
        plain.warnings[0]
            .contains("config key sandbox_mode; the profile value danger-full-access is dropped"),
        "{}",
        plain.warnings[0]
    );
    assert!(
        plain.warnings[1]
            .contains("config key approval_policy; the profile value never is dropped"),
        "{}",
        plain.warnings[1]
    );

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
    // A short value stays in the set for the structural pass and is never
    // substituted by text: only values of eight characters or more are.
    assert!(secrets.contains(&"ab".to_string()), "{secrets:?}");
    assert_eq!(
        launch::redact_credentials("ab fake-r fake-route-secret-long", &secrets),
        "ab fake-r <redacted>"
    );
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
            harness_errors: Vec::new(),
        }
    );
    let silent = SurfaceFailure::Reported("no answer".to_string()).with_output(&output, &secrets);
    assert!(
        !silent.to_string().contains("fake-route-secret"),
        "{silent}"
    );

    let timeout = SurfaceFailure::from(ProcessFailure::Timeout {
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

#[test]
fn host_bound_family_members_travel_with_the_family() {
    let parent = keys(&[
        "ANTHROPIC_CUSTOM_HEADERS",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
        "CLAUDE_CODE_CLIENT_CERT",
        "CLAUDE_CODE_CLIENT_KEY",
        "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE",
        "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST",
        "ANTHROPIC_BASE_URL",
    ]);

    // The route sets one Anthropic key: every other member is withheld.
    let route = env(&[("ANTHROPIC_AUTH_TOKEN", "fake-route-secret")]);
    let isolation = super::isolation::isolate(&parent, &launch::env_keys(&route));
    let withheld: Vec<&str> = isolation
        .withheld
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    for member in [
        "ANTHROPIC_CUSTOM_HEADERS",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
        "CLAUDE_CODE_CLIENT_CERT",
        "CLAUDE_CODE_CLIENT_KEY",
        "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE",
        "ANTHROPIC_BASE_URL",
    ] {
        assert!(withheld.contains(&member), "{member} kept: {withheld:?}");
    }
    assert_eq!(
        isolation.session,
        ["CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST"],
        "a host-session name, not a family member"
    );

    // The route sets none: the family passes as a set.
    let isolation = super::isolation::isolate(&parent, &[]);
    assert!(isolation.withheld.is_empty(), "{:?}", isolation.withheld);
}

#[test]
fn tmux_removals_cover_the_whole_tables_whatever_rune_inherited() {
    let route = env(&[("ANTHROPIC_AUTH_TOKEN", "fake-route-secret")]);
    let removals = strings(&tmux_removals(&launch::env_keys(&route)));

    for name in [
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
        "CODEX_SESSION_ID",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE",
    ] {
        assert!(removals.contains(&name.to_string()), "{name}: {removals:?}");
    }
    assert!(
        !removals.contains(&"ANTHROPIC_AUTH_TOKEN".to_string()),
        "{removals:?}"
    );
    assert!(
        !removals.contains(&"OPENAI_API_KEY".to_string()),
        "no OpenAI key was set: {removals:?}"
    );
    assert!(
        !removals.contains(&"CLAUDE_CONFIG_DIR".to_string()),
        "{removals:?}"
    );
}

#[test]
fn a_scratch_cleanup_failure_keeps_the_run_failure_and_its_tails() {
    let missing = PathBuf::from("/nonexistent/rune-scratch-that-never-was");
    let failure = SurfaceFailure::Exit {
        termination: ProcessTermination::Exited(1),
        stdout_tail: "API Error: 400".to_string(),
        stderr_tail: "warning".to_string(),
        harness_errors: vec!["quota".to_string()],
    };

    let noted = finish_scratch::<()>(Err(failure), &missing).expect_err("still a failure");

    assert_eq!(noted.tails(), Some(("API Error: 400", "warning")));
    assert_eq!(noted.harness_errors(), ["quota"]);
    let text = noted.to_string();
    assert!(text.starts_with("process Exited(1)"), "{text}");
    assert!(text.contains("stdout tail:\n  API Error: 400"), "{text}");
    assert!(
        text.contains(
            "note: cannot remove scratch directory /nonexistent/rune-scratch-that-never-was"
        ),
        "{text}"
    );
    let redacted = noted.redacted(&["API Error: 400".to_string()]);
    assert_eq!(redacted.tails(), Some(("<redacted>", "warning")));

    let after_success = finish_scratch(Ok(()), &missing).expect_err("the cleanup failed");
    assert!(
        matches!(after_success, SurfaceFailure::Io(_)),
        "{after_success}"
    );
}

/// A stop's redaction keeps the window around the tail and drops the rest
/// of the capture, with a value near the end still hidden: the window is
/// cut before the redaction, and the tail is read from the window.
#[test]
fn a_redacted_stop_keeps_the_window_around_its_tail() {
    let early = "early line\n".repeat(20_000);
    let stdout = format!("{early}API_KEY=hunter2secret\nlast line");
    assert!(stdout.len() > TAIL_WINDOW_BYTES, "{}", stdout.len());
    let failure = SurfaceFailure::from(ProcessFailure::OutputLimit {
        stream: "stdout",
        limit: 64,
        stdout,
        stderr: "x".repeat(TAIL_WINDOW_BYTES),
    });

    let redacted = failure.redacted(&["hunter2secret".to_string()]);

    let (stdout, stderr) = redacted.tails().expect("tails");
    assert_eq!(
        stdout.len(),
        TAIL_WINDOW_BYTES + "<redacted>".len() - "hunter2secret".len()
    );
    assert!(stdout.starts_with("early line\n"), "{stdout:.30}");
    assert!(
        stdout.ends_with("API_KEY=<redacted>\nlast line"),
        "{stdout:.30}"
    );
    assert_eq!(stderr.len(), TAIL_WINDOW_BYTES);
    assert_eq!(tail_window("short".to_string()), "short");
    let multibyte = "é".repeat(TAIL_WINDOW_BYTES);
    assert_eq!(tail_window(multibyte).len(), TAIL_WINDOW_BYTES);
}

#[test]
fn a_wait_failure_carries_the_streams_read_before_it() {
    let failure = SurfaceFailure::from(ProcessFailure::Wait {
        message: "cannot signal process group 7: gone".to_string(),
        stdout: "read so far".to_string(),
        stderr: "still going".to_string(),
    });
    assert_eq!(failure.tails(), Some(("read so far", "still going")));
    let text = failure.to_string();
    assert!(
        text.starts_with("cannot signal process group 7: gone"),
        "{text}"
    );
    assert!(text.contains("stdout tail:\n  read so far"), "{text}");
}

#[test]
fn codex_config_flags_go_before_the_exec_subcommand_in_their_order() {
    let mut invocation = invocation(
        Surface::Codex,
        &[
            "--search",
            "-c",
            "model_reasoning_effort=\"low\"",
            "--config=sandbox_workspace_write.network_access=true",
            "-cfoo=\"bar\"",
            "--full-auto",
        ],
    );
    invocation.model = Some("gpt-6.1-sol".to_string());
    let args = strings(&codex_args(
        &invocation,
        &PathBuf::from("<scratch>/last-message.txt"),
    ));

    let exec = args.iter().position(|arg| arg == "exec").expect("exec");
    assert_eq!(
        &args[..exec],
        [
            "-c",
            "model_reasoning_effort=\"low\"",
            "--config=sandbox_workspace_write.network_access=true",
            "-cfoo=\"bar\"",
        ],
        "{args:?}"
    );
    let after: Vec<&str> = args[exec..].iter().map(String::as_str).collect();
    assert!(
        after.windows(2).any(|pair| pair == ["-m", "gpt-6.1-sol"]),
        "{args:?}"
    );
    assert!(
        after.contains(&"--search") && after.contains(&"--full-auto"),
        "{args:?}"
    );
    assert!(
        !after
            .iter()
            .any(|arg| arg.starts_with("-c") || arg.starts_with("--config")),
        "{args:?}"
    );
    assert_eq!(after.last().copied(), Some("-"));

    // The dry run renders from the same builder.
    let planned = planned_request(&invocation).expect("codex plan");
    let planned = strings(&planned.args);
    assert_eq!(planned[0], "-c");
    assert_eq!(planned.iter().position(|arg| arg == "exec"), Some(4));
}
