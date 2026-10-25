//! The postbox from the reader's side: `rune queue read` takes the messages
//! posted for a session and marks them read, and `rune queue list` shows the
//! ledger without changing it. The hook handler reads through the same
//! functions, so a message reads the same wherever it arrives.

use super::ledger::{self, Message, State};
use crate::cli::run::{EXIT_RUNNER_ERROR, Outcome, Text, print_outcome};
use serde_json::{Value, json};

/// The variables a harness sets for its children that name its session:
/// Claude Code's was read in a live desktop session on 2026-10-25, and
/// Codex sets `CODEX_THREAD_ID` to the thread id for every shell command
/// since openai/codex#10096 (merged 2026-02-03).
const SESSION_VARIABLES: &[(&str, &str)] = &[
    ("claude", "CLAUDE_CODE_SESSION_ID"),
    ("codex", "CODEX_THREAD_ID"),
];

pub(crate) struct ReadOptions {
    pub(crate) sessions: Vec<String>,
    pub(crate) json: bool,
}

pub(crate) struct ListOptions {
    pub(crate) session: Option<String>,
    pub(crate) state: Option<String>,
    pub(crate) json: bool,
}

/// The harness session this process runs in, as `(harness, id)`.
pub(crate) fn own_session() -> Option<(&'static str, String)> {
    SESSION_VARIABLES.iter().find_map(|(harness, variable)| {
        std::env::var(variable)
            .ok()
            .filter(|id| !id.trim().is_empty())
            .map(|id| (*harness, id))
    })
}

/// Who posts: the caller's `--from`, else this process's harness session as
/// `harness:id`, else `user:<login>`.
pub(crate) fn sender(explicit: Option<&str>) -> String {
    if let Some(sender) = explicit.filter(|sender| !sender.trim().is_empty()) {
        return sender.to_string();
    }
    if let Some((harness, id)) = own_session() {
        return format!("{harness}:{id}");
    }
    std::env::var("USER")
        .ok()
        .filter(|user| !user.is_empty())
        .map_or_else(|| "user".to_string(), |user| format!("user:{user}"))
}

/// The messages as an agent reads them: who sent each, when, and its text.
pub(crate) fn format_messages(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| {
            format!(
                "Message {} from {} at {}:\n{}",
                message.id, message.sender, message.timestamp, message.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn message_json(message: &Message) -> Value {
    json!({
        "id": message.id,
        "timestamp": message.timestamp,
        "state": message.state.as_str(),
        "sender": message.sender,
        "harness": message.harness,
        "session": message.session,
        "message": message.text,
    })
}

fn failure(json_kind: &str, message: String) -> Outcome {
    Outcome {
        exit_code: EXIT_RUNNER_ERROR,
        json: json!({
            "ok": false,
            "kind": json_kind,
            "details": { "message": message },
        }),
        text: Text::Fatal(message),
    }
}

pub(crate) fn execute_read(options: &ReadOptions) -> i32 {
    print_outcome(options.json, read(options))
}

fn read(options: &ReadOptions) -> Outcome {
    let sessions = if options.sessions.is_empty() {
        match own_session() {
            Some((_, id)) => vec![id],
            None => {
                return failure(
                    "configuration_error",
                    "name the session with --session: no harness session variable is set here"
                        .to_string(),
                );
            }
        }
    } else {
        options.sessions.clone()
    };
    let path = match ledger::path() {
        Ok(path) => path,
        Err(message) => return failure("postbox_error", message),
    };
    let messages = match ledger::take_posted(&path, &sessions, &ledger::now()) {
        Ok(messages) => messages,
        Err(message) => return failure("postbox_error", message),
    };
    let text = if messages.is_empty() {
        Text::Stderr(format!("no messages for {}", sessions.join(", ")))
    } else {
        Text::Stdout(format_messages(&messages))
    };
    Outcome {
        exit_code: 0,
        json: json!({
            "ok": true,
            "kind": "read",
            "sessions": sessions,
            "postbox": path.display().to_string(),
            "messages": messages.iter().map(message_json).collect::<Vec<_>>(),
        }),
        text,
    }
}

pub(crate) fn execute_list(options: &ListOptions) -> i32 {
    print_outcome(options.json, list(options))
}

fn list(options: &ListOptions) -> Outcome {
    let state = match options.state.as_deref().map(State::parse) {
        Some(None) => {
            return failure(
                "configuration_error",
                "--state takes posted, queued, or read".to_string(),
            );
        }
        Some(Some(state)) => Some(state),
        None => None,
    };
    let path = match ledger::path() {
        Ok(path) => path,
        Err(message) => return failure("postbox_error", message),
    };
    let (events, skipped) = match ledger::load(&path) {
        Ok(loaded) => loaded,
        Err(message) => return failure("postbox_error", message),
    };
    let messages: Vec<Message> = ledger::fold(&events)
        .into_iter()
        .filter(|message| {
            options
                .session
                .as_ref()
                .is_none_or(|session| *session == message.session)
                && state.is_none_or(|state| state == message.state)
        })
        .collect();
    if skipped > 0 {
        eprintln!(
            "warning: {skipped} line(s) of {} are not ledger events and were skipped",
            path.display()
        );
    }
    let text = if messages.is_empty() {
        Text::Stderr("no messages".to_string())
    } else {
        Text::Stdout(
            messages
                .iter()
                .map(|message| {
                    format!(
                        "{}  {}  {:<6}  {} -> {}:{}  {}",
                        message.timestamp,
                        message.id,
                        message.state.as_str(),
                        message.sender,
                        message.harness,
                        message.session,
                        first_line(&message.text, 72)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };
    Outcome {
        exit_code: 0,
        json: json!({
            "ok": true,
            "kind": "list",
            "postbox": path.display().to_string(),
            "messages": messages.iter().map(message_json).collect::<Vec<_>>(),
            "skipped_lines": skipped,
        }),
        text,
    }
}

/// The first line of `text`, cut to `limit` characters with an ellipsis.
fn first_line(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or_default();
    if line.chars().count() <= limit && !text.contains('\n') {
        return line.to_string();
    }
    let cut: String = line.chars().take(limit).collect();
    format!("{cut}…")
}
