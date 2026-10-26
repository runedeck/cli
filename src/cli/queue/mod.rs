//! `rune queue`: messages between harness sessions, read at the receiving
//! session's next turn. `post` hands a message to the harness's own queue
//! where one exists (Codex, `codex queue`, help read on 2026-10-25) and
//! records it in the postbox ledger. Every other harness reads its messages
//! from the postbox, through the `postbox` hook handler or `rune queue read`.
//! Nothing imitates a queue with a second turn or a copy of the session.
use crate::cli::launch;
use crate::cli::process::{ProcessRequest, run_process_request};
use crate::cli::run::{
    EXIT_RUNNER_ERROR, Outcome, Text, configuration_outcome, duration_label, failure_exit_code,
    failure_json, parse_duration, print_outcome, read_text,
};
use crate::cli::surface::{
    Isolation, Surface, SurfaceFailure, parent_session_isolation, require_success,
};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(crate) mod inbox;
pub(crate) mod ledger;

#[cfg(test)]
mod ledger_tests;
#[cfg(test)]
mod tests;

pub(crate) use inbox::{ListOptions, ReadOptions, execute_list, execute_read};

/// The placeholder the dry run prints for the message.
const MESSAGE_PLACEHOLDER: &str = "<message>";

/// The harnesses a message can be posted for, named in every refusal.
const HARNESSES: &[&str] = &["claude", "codex", "agy", "grok", "opencode"];

pub(crate) struct PostOptions {
    pub(crate) tool: String,
    pub(crate) session: String,
    pub(crate) message: Option<String>,
    pub(crate) message_file: Option<PathBuf>,
    pub(crate) from: Option<String>,
    /// Post to the ledger even where the harness has its own queue.
    pub(crate) postbox: bool,
    pub(crate) binary: Option<PathBuf>,
    pub(crate) remote: Option<String>,
    pub(crate) remote_auth_token_env: Option<String>,
    pub(crate) timeout: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) json: bool,
}

/// How a message reaches a running session of one harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    /// `codex queue --thread=<session> --message=<text>`, which prints
    /// `Queued message <id> for thread <id>.`
    CodexQueue,
    /// The postbox ledger, read by the session's hook or by `rune queue read`.
    Postbox,
}

impl Transport {
    fn label(self) -> &'static str {
        match self {
            Self::CodexQueue => "codex queue",
            Self::Postbox => "postbox",
        }
    }
}

/// How a message reaches a running session of `surface`. Codex has its own
/// queue. Claude Code (2.1.310), agy (1.4.7), and grok (1.0.62) can only
/// resume a session for a new turn, and `OpenCode`'s `run --attach`
/// (1.18.29) waits for the turn, so their messages wait in the postbox.
pub(crate) fn queue_transport(surface: Surface) -> Transport {
    match surface {
        Surface::Codex => Transport::CodexQueue,
        Surface::Claude | Surface::Agy | Surface::Grok | Surface::Opencode | Surface::Cursor => {
            Transport::Postbox
        }
    }
}

/// The app server a queue reaches when the caller names none: the socket of
/// the local daemon that holds the running sessions. Without `--remote`,
/// `codex queue` starts an embedded app server, which refuses a config
/// override while the daemon runs ("remove configuration overrides or use
/// --remote", Codex 0.154.0 on 2026-10-25), and a route's wrapper such as
/// `codex-proxy` adds one. With `--remote unix://` the same override passed.
const CODEX_LOCAL_DAEMON: &str = "unix://";

/// The `codex queue` arguments. The equals form keeps a session or a
/// message that starts with `-` from reading as a flag.
pub(crate) fn codex_queue_args(
    session: &str,
    message: &str,
    remote: Option<&str>,
    remote_auth_token_env: Option<&str>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("queue"),
        OsString::from(format!("--thread={session}")),
        OsString::from(format!("--message={message}")),
        OsString::from(format!("--remote={}", remote.unwrap_or(CODEX_LOCAL_DAEMON))),
    ];
    if let Some(variable) = remote_auth_token_env {
        args.push(OsString::from(format!(
            "--remote-auth-token-env={variable}"
        )));
    }
    args
}

/// What the harness printed for a queued message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub(crate) message_id: String,
    pub(crate) thread_id: String,
}

/// The receipt line of `codex queue` on stdout, `Queued message <id> for
/// thread <id>.`, wherever it stands among other lines.
pub(crate) fn parse_receipt(stdout: &str) -> Option<Receipt> {
    stdout.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Queued message ")?;
        let (message_id, thread) = rest.split_once(" for thread ")?;
        let thread_id = thread.strip_suffix('.').unwrap_or(thread);
        (!message_id.is_empty() && !thread_id.is_empty()).then(|| Receipt {
            message_id: message_id.to_string(),
            thread_id: thread_id.to_string(),
        })
    })
}

/// The warning for a profile's arguments: they start a session, so the
/// queue never takes them. A credential among them is hidden.
pub(crate) fn dropped_profile_args(args: &[OsString]) -> Option<String> {
    (!args.is_empty()).then(|| {
        format!(
            "the profile's arguments start a session and are not passed to the queue: {}",
            launch::redact_argv(args).join(" ")
        )
    })
}

pub(crate) fn execute_post(options: &PostOptions) -> i32 {
    let outcome =
        execute_inner(options).unwrap_or_else(|error| configuration_outcome(&options.tool, error));
    print_outcome(options.json, outcome)
}

fn execute_inner(options: &PostOptions) -> Result<Outcome, launch::ResolveError> {
    // A route that names no harness is refused from the route alone: no
    // profile resolves, no middleware runs, and no message is read.
    let (tool, _) = launch::split_invocation(&options.tool);
    let Some(surface) = Surface::from_tool(tool) else {
        return Ok(unsupported_outcome(options, tool));
    };
    if options.session.trim().is_empty() {
        return Err(
            "the session must not be empty: pass its id or its exact name"
                .to_string()
                .into(),
        );
    }
    let transport = if options.postbox {
        Transport::Postbox
    } else {
        queue_transport(surface)
    };
    let message = read_text(
        options.message.as_deref(),
        options.message_file.as_deref(),
        "message",
        "--message-file",
    )?;
    let posted = ledger::Event {
        timestamp: ledger::now(),
        id: ledger::new_id(),
        state: ledger::State::Posted,
        sender: inbox::sender(options.from.as_deref()),
        harness: tool.to_string(),
        session: options.session.clone(),
        message,
    };
    let postbox = ledger::path()?;
    if transport == Transport::Postbox {
        return Ok(post_to_postbox(options, &posted, &postbox));
    }
    let resolved = launch::resolve(&options.tool, &[])?;
    let wrappers = resolved.unsupported_wrappers();
    if !wrappers.is_empty() {
        return Err(format!(
            "rune queue cannot pass a message through {} wrappers; remove them from the profile",
            wrappers.join(", ")
        )
        .into());
    }
    let timeout = options.timeout.as_deref().map(parse_duration).transpose()?;
    let (resolved_binary, profile_args) = resolved
        .argv
        .split_first()
        .ok_or_else(|| "resolved launch command is empty".to_string())?;
    let binary = options.binary.as_ref().map_or_else(
        || resolved_binary.clone(),
        |path| path.as_os_str().to_os_string(),
    );
    let warnings: Vec<String> = resolved
        .warnings()
        .iter()
        .cloned()
        .chain(dropped_profile_args(profile_args))
        .collect();
    // The caller's environment, minus the parent session's variables, plus
    // the route's: the isolation `rune run` gives its harness.
    let isolation = parent_session_isolation(&resolved.route_keys());
    let args = |message: &str| {
        codex_queue_args(
            &options.session,
            message,
            options.remote.as_deref(),
            options.remote_auth_token_env.as_deref(),
        )
    };
    let mut request = ProcessRequest::new(binary);
    request.env.clone_from(&resolved.env);
    request.env_remove = isolation.removals();
    request.timeout = timeout;

    if options.dry_run {
        request.args = args(MESSAGE_PLACEHOLDER);
        return Ok(dry_run_outcome(
            options,
            tool,
            transport,
            &request,
            &isolation,
            posted.message.chars().count(),
            &warnings,
        ));
    }

    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    request.args = args(&posted.message);
    Ok(send(options, &posted, &postbox, &request, warnings))
}

/// Write the message to the postbox as `posted`, where its session reads
/// it. A dry run prints the line it would write, with the text as a
/// placeholder, and writes nothing.
fn post_to_postbox(options: &PostOptions, posted: &ledger::Event, postbox: &Path) -> Outcome {
    let shown = ledger::Event {
        message: MESSAGE_PLACEHOLDER.to_string(),
        ..posted.clone()
    };
    let json = json!({
        "ok": true,
        "kind": if options.dry_run { "dry_run" } else { "posted" },
        "tool": posted.harness,
        "requested_route": options.tool,
        "transport": Transport::Postbox.label(),
        "id": posted.id,
        "sender": posted.sender,
        "session": posted.session,
        "postbox": postbox.display().to_string(),
        "message_chars": posted.message.chars().count(),
    });
    if options.dry_run {
        return Outcome {
            exit_code: 0,
            json,
            text: Text::Stdout(format!(
                "tool: {}\nroute: {}\ntransport: postbox\npostbox: {}\nline: {}\nmessage_chars: {}",
                posted.harness,
                options.tool,
                postbox.display(),
                shown.line(),
                posted.message.chars().count()
            )),
        };
    }
    if let Err(message) = ledger::append(postbox, std::slice::from_ref(posted)) {
        return postbox_failure(options, &posted.harness, message);
    }
    Outcome {
        exit_code: 0,
        json,
        text: Text::Stdout(format!(
            "posted message {} for {} session {} in {}",
            posted.id,
            posted.harness,
            posted.session,
            postbox.display()
        )),
    }
}

/// The postbox could not be written: exit 125 and the reason.
fn postbox_failure(options: &PostOptions, tool: &str, message: String) -> Outcome {
    Outcome {
        exit_code: EXIT_RUNNER_ERROR,
        json: json!({
            "ok": false,
            "kind": "postbox_error",
            "tool": tool,
            "requested_route": options.tool,
            "details": { "message": message },
        }),
        text: Text::Fatal(message),
    }
}

/// Start the harness command and read its outcome. The credentials of the
/// child's environment, and the remote token's value, are hidden in the
/// stream tails of a failure.
fn send(
    options: &PostOptions,
    posted: &ledger::Event,
    postbox: &Path,
    request: &ProcessRequest,
    warnings: Vec<String>,
) -> Outcome {
    let mut secrets = launch::credential_values(&request.env);
    secrets.extend(remote_token(
        options.remote_auth_token_env.as_deref(),
        &request.env,
    ));
    let result = run_process_request(request)
        .map_err(SurfaceFailure::from)
        .and_then(|output| require_success(output, &secrets));
    match result {
        Ok(output) => {
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
                if !output.stderr.ends_with('\n') {
                    eprintln!();
                }
            }
            queued_outcome(options, posted, postbox, &output.stdout, warnings)
        }
        Err(failure) => Outcome {
            exit_code: failure_exit_code(&failure),
            json: failure_json(&posted.harness, &failure),
            text: Text::Stderr(format!("error: {failure}")),
        },
    }
}

/// The refusal of a route that names no harness: exit 125, and the
/// harnesses a message can be posted for.
fn unsupported_outcome(options: &PostOptions, tool: &str) -> Outcome {
    let message = format!(
        "cannot post a message for a session on {tool}: rune drives no such harness. Harnesses: {}",
        HARNESSES.join(", ")
    );
    Outcome {
        exit_code: EXIT_RUNNER_ERROR,
        json: json!({
            "ok": false,
            "kind": "unsupported_harness",
            "tool": tool,
            "requested_route": options.tool,
            "details": {
                "message": message,
                "supported": HARNESSES,
            },
        }),
        text: Text::Fatal(message),
    }
}

/// A message the harness took: the ids from its receipt line, or null ids
/// and a warning when it printed none. The ledger records it as posted and
/// queued, so the hook never delivers it a second time. A ledger that
/// cannot be written is a warning: the message is already queued.
fn queued_outcome(
    options: &PostOptions,
    posted: &ledger::Event,
    postbox: &Path,
    stdout: &str,
    mut warnings: Vec<String>,
) -> Outcome {
    let tool = posted.harness.as_str();
    let transport = Transport::CodexQueue;
    let queued = ledger::Event {
        timestamp: ledger::now(),
        state: ledger::State::Queued,
        message: String::new(),
        ..posted.clone()
    };
    if let Err(message) = ledger::append(postbox, &[posted.clone(), queued]) {
        let warning =
            format!("the message is queued, and the postbox did not record it: {message}");
        eprintln!("warning: {warning}");
        warnings.push(warning);
    }
    let receipt = parse_receipt(stdout);
    let text = if let Some(receipt) = &receipt {
        format!(
            "queued message {} for {tool} thread {}",
            receipt.message_id, receipt.thread_id
        )
    } else {
        let warning = format!(
            "{} exited 0 without its receipt line, so the message id is unknown",
            transport.label()
        );
        eprintln!("warning: {warning}");
        warnings.push(warning);
        format!(
            "queued for {tool} session {}, message id unknown",
            options.session
        )
    };
    Outcome {
        exit_code: 0,
        json: json!({
            "ok": true,
            "kind": "queued",
            "tool": tool,
            "requested_route": options.tool,
            "transport": transport.label(),
            "id": posted.id,
            "sender": posted.sender,
            "session": options.session,
            "message_id": receipt.as_ref().map(|receipt| receipt.message_id.as_str()),
            "thread_id": receipt.as_ref().map(|receipt| receipt.thread_id.as_str()),
            "warnings": warnings,
        }),
        text: Text::Stdout(text),
    }
}

/// The value of the variable `--remote-auth-token-env` names: the route's
/// environment first, as the child sees it, else this process's. Its name
/// says nothing about it being a credential, so it is hidden by the flag
/// that named it, never by its name.
fn remote_token(variable: Option<&str>, env: &[(OsString, OsString)]) -> Option<String> {
    let variable = variable?;
    env.iter()
        .rev()
        .find(|(key, _)| key == variable)
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os(variable))
        .map(|token| token.to_string_lossy().into_owned())
        .filter(|token| !token.is_empty())
}

/// The route's environment as the dry run shows it: every credential hidden
/// by its name, and the variable `--remote-auth-token-env` names hidden
/// whatever its name.
fn shown_env(
    env: &[(OsString, OsString)],
    remote_auth_token_env: Option<&str>,
) -> Vec<(String, String)> {
    launch::redacted_env(env)
        .into_iter()
        .map(|(key, value)| {
            if remote_auth_token_env == Some(key.as_str()) {
                (key, launch::REDACTED_VALUE.to_string())
            } else {
                (key, value)
            }
        })
        .collect()
}

/// The command the queue would start, with the message as a placeholder
/// and every credential hidden. Nothing starts.
fn dry_run_outcome(
    options: &PostOptions,
    tool: &str,
    transport: Transport,
    request: &ProcessRequest,
    isolation: &Isolation,
    message_chars: usize,
    warnings: &[String],
) -> Outcome {
    let argv: Vec<OsString> = std::iter::once(request.binary.clone())
        .chain(request.args.iter().cloned())
        .collect();
    let argv = launch::redact_argv(&argv);
    let env = shown_env(&request.env, options.remote_auth_token_env.as_deref());
    let mut lines = vec![
        format!("tool: {tool}"),
        format!("route: {}", options.tool),
        format!("transport: {}", transport.label()),
        format!("argv: {}", argv.join(" ")),
        "env:".to_string(),
    ];
    lines.extend(env.iter().map(|(key, value)| format!("  {key}={value}")));
    lines.push("env_removed:".to_string());
    lines.extend(isolation.lines().iter().map(|key| format!("  {key}")));
    lines.push(format!("session: {}", options.session));
    // The variable's name, which the argv hides beside a credential-named
    // flag; its value never shows.
    lines.push(format!(
        "remote_auth_token_env: {}",
        options.remote_auth_token_env.as_deref().unwrap_or("none")
    ));
    lines.push(format!("message_chars: {message_chars}"));
    lines.push(format!("timeout: {}", duration_label(request.timeout)));
    if !warnings.is_empty() {
        lines.push("warnings:".to_string());
        lines.extend(warnings.iter().map(|warning| format!("  {warning}")));
    }
    let env: serde_json::Map<String, Value> = env
        .into_iter()
        .map(|(key, value)| (key, Value::String(value)))
        .collect();
    Outcome {
        exit_code: 0,
        json: json!({
            "ok": true,
            "kind": "dry_run",
            "tool": tool,
            "requested_route": options.tool,
            "transport": transport.label(),
            "argv": argv,
            "env": env,
            "env_removed": isolation
                .removals()
                .iter()
                .map(|key| key.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            "session": options.session,
            "remote_auth_token_env": options.remote_auth_token_env,
            "message_chars": message_chars,
            "timeout": request.timeout.map(|timeout| timeout.as_millis()),
            "warnings": warnings,
        }),
        text: Text::Stdout(lines.join("\n")),
    }
}
