use super::*;

#[test]
fn codex_has_its_own_queue_and_the_rest_read_the_postbox() {
    assert_eq!(queue_transport(Surface::Codex), Transport::CodexQueue);
    for surface in [
        Surface::Claude,
        Surface::Agy,
        Surface::Grok,
        Surface::Opencode,
    ] {
        assert_eq!(queue_transport(surface), Transport::Postbox, "{surface:?}");
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

fn hook_payload(
    harness: &str,
    event: &str,
    session: &str,
) -> crate::cli::hook::payload::HandlerPayload {
    serde_json::from_value(serde_json::json!({
        "v": 1,
        "harness": harness,
        "event": event,
        "native_event": event,
        "session_id": session,
        "raw": {},
    }))
    .expect("a handler payload")
}

fn posted(path: &Path, id: &str, session: &str) {
    ledger::append(
        path,
        &[ledger::Event {
            timestamp: ledger::now(),
            id: id.to_string(),
            state: ledger::State::Posted,
            sender: "codex:01a0abff".to_string(),
            harness: "claude".to_string(),
            session: session.to_string(),
            message: "Is the deck ready?".to_string(),
        }],
    )
    .expect("post");
}

#[test]
fn hook_adds_a_posted_message_to_the_next_prompt_and_marks_it_read() {
    use crate::cli::hook::adapters::postbox_at;
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().join(ledger::FILE_NAME);
    posted(&path, "m-1", "42e43c0b");

    let result =
        postbox_at(&hook_payload("claude", "prompt.before", "42e43c0b"), &path).expect("hook");
    assert_eq!(result.context.len(), 1);
    assert!(
        result.context[0].contains("Is the deck ready?"),
        "{result:?}"
    );
    assert!(
        result.context[0].contains("from codex:01a0abff"),
        "{result:?}"
    );

    let again =
        postbox_at(&hook_payload("claude", "prompt.before", "42e43c0b"), &path).expect("hook");
    assert!(again.context.is_empty(), "a read message arrives once");
}

#[test]
fn hook_keeps_a_codex_turn_going_with_the_message_at_its_end() {
    use crate::cli::hook::adapters::postbox_at;
    use crate::cli::hook::encode::Effect;
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().join(ledger::FILE_NAME);
    posted(&path, "m-1", "01a0abff");

    let result =
        postbox_at(&hook_payload("codex", "turn.finish", "01a0abff"), &path).expect("hook");
    assert_eq!(result.effect, Effect::ContinueTurn);
    assert!(
        result
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("Is the deck ready?")),
        "{result:?}"
    );
}

#[test]
fn hook_reads_nothing_where_the_harness_cannot_show_it() {
    use crate::cli::hook::adapters::postbox_at;
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().join(ledger::FILE_NAME);
    posted(&path, "m-1", "01a0abff");

    // Codex takes no context on a prompt: the message must stay posted.
    let result =
        postbox_at(&hook_payload("codex", "prompt.before", "01a0abff"), &path).expect("hook");
    assert!(result.context.is_empty() && result.reason.is_none());
    let (events, _) = ledger::load(&path).expect("load");
    assert_eq!(ledger::fold(&events)[0].state, ledger::State::Posted);
}
