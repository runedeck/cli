use super::*;

#[test]
fn codex_is_the_one_harness_with_a_queue() {
    assert_eq!(queue_transport(Surface::Codex), Ok(Transport::CodexQueue));
    for (surface, name) in [
        (Surface::Claude, "claude"),
        (Surface::Agy, "agy"),
        (Surface::Grok, "grok"),
        (Surface::Opencode, "opencode"),
    ] {
        let reason = queue_transport(surface).expect_err("no queue");
        assert!(reason.starts_with(name), "{reason}");
    }
}

#[test]
fn codex_arguments_keep_a_dash_message_as_a_value_and_reach_the_local_daemon() {
    let args = codex_queue_args("ouroboros", "-v is not a flag here", None, None);
    assert_eq!(
        args,
        [
            "queue",
            "--thread=ouroboros",
            "--message=-v is not a flag here",
            "--remote=unix://",
        ]
        .map(OsString::from)
        .to_vec()
    );
}

#[test]
fn codex_arguments_carry_the_remote_server_and_the_token_variable_name() {
    let args = codex_queue_args(
        "ouroboros",
        "hi",
        Some("wss://host:4500"),
        Some("CODEX_TOKEN"),
    );
    assert_eq!(
        args[3..],
        [
            "--remote=wss://host:4500",
            "--remote-auth-token-env=CODEX_TOKEN"
        ]
        .map(OsString::from)
    );
}

#[test]
fn receipt_is_read_among_other_lines() {
    let stdout = "WARNING: proceeding\nQueued message msg-7 for thread 019a-thread.\n";
    assert_eq!(
        parse_receipt(stdout),
        Some(Receipt {
            message_id: "msg-7".to_string(),
            thread_id: "019a-thread".to_string(),
        })
    );
}

#[test]
fn output_without_a_receipt_line_has_no_receipt() {
    assert_eq!(parse_receipt(""), None);
    assert_eq!(parse_receipt("Queued message  for thread ."), None);
    assert_eq!(parse_receipt("Queued something else\n"), None);
}

#[test]
fn profile_arguments_are_named_with_credentials_hidden() {
    assert_eq!(dropped_profile_args(&[]), None);
    let warning = dropped_profile_args(&[
        OsString::from("-m"),
        OsString::from("gpt-5.6-sol"),
        OsString::from("--api-key=fake-secret-value"),
    ])
    .expect("a warning");
    assert!(warning.contains("-m gpt-5.6-sol"), "{warning}");
    assert!(!warning.contains("fake-secret-value"), "{warning}");
}
