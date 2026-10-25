//! `rune queue`: one message for a harness session that is already running,
//! read at the session's next turn. The message goes through the harness's
//! own queue, and a harness without one is refused: nothing imitates a queue
//! with a second turn or a copy of the session. Codex is the one harness
//! with a queue command (`codex queue`, help read on 2026-10-25), and
//! [`queue_transport`] says why each other harness is refused.
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
use std::path::PathBuf;

#[cfg(test)]
mod tests;

/// The placeholder the dry run prints for the message.
const MESSAGE_PLACEHOLDER: &str = "<message>";

/// The harnesses with a queue, named in every refusal.
const SUPPORTED: &[&str] = &["codex"];

pub(crate) struct QueueOptions {
    pub(crate) tool: String,
    pub(crate) session: String,
    pub(crate) message: Option<String>,
    pub(crate) message_file: Option<PathBuf>,
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
}

impl Transport {
    fn label(self) -> &'static str {
        match self {
            Self::CodexQueue => "codex queue",
        }
    }
}

/// How a message reaches a running session of `surface`, else why it
/// cannot. Each reason comes from the harness's own help, read on
/// 2026-10-25, and the `OpenCode` server page of the same day.
pub(crate) fn queue_transport(surface: Surface) -> Result<Transport, &'static str> {
    match surface {
        Surface::Codex => Ok(Transport::CodexQueue),
        Surface::Claude => Err(
            "claude has no command that queues a message for a running session: --resume starts a new turn, and with --bg a running session gets a copy",
        ),
        Surface::Agy => Err(
            "agy has no command that queues a message for a running conversation: --conversation resumes one for a new turn",
        ),
        Surface::Grok => Err(
            "grok has no command that queues a message for a running session: --resume starts a new turn",
        ),
        Surface::Opencode => Err(
            "opencode has no command that queues without waiting: run --attach waits for the turn, and its server does not document what a prompt does to a busy session",
        ),
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

pub(crate) fn execute(options: &QueueOptions) -> i32 {
    let outcome =
        execute_inner(options).unwrap_or_else(|error| configuration_outcome(&options.tool, error));
    print_outcome(options.json, outcome)
}

fn execute_inner(options: &QueueOptions) -> Result<Outcome, launch::ResolveError> {
    // A harness without a queue is refused from the route alone: no
    // profile resolves, no middleware runs, and no message is read.
    let (tool, _) = launch::split_invocation(&options.tool);
    let transport = match Surface::from_tool(tool)
        .ok_or("rune drives no such harness")
        .and_then(queue_transport)
    {
        Ok(transport) => transport,
        Err(reason) => return Ok(unsupported_outcome(options, tool, reason)),
    };
    if options.session.trim().is_empty() {
        return Err(
            "the session must not be empty: pass its id or its exact name"
                .to_string()
                .into(),
        );
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
    let message = read_text(
        options.message.as_deref(),
        options.message_file.as_deref(),
        "message",
        "--message-file",
    )?;
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
    let args = |message: &str| match transport {
        Transport::CodexQueue => codex_queue_args(
            &options.session,
            message,
            options.remote.as_deref(),
            options.remote_auth_token_env.as_deref(),
        ),
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
            message.chars().count(),
            &warnings,
        ));
    }

    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    request.args = args(&message);
    Ok(send(options, tool, transport, &request, warnings))
}

/// Start the harness command and read its outcome. The credentials of the
/// child's environment, and the remote token's value, are hidden in the
/// stream tails of a failure.
fn send(
    options: &QueueOptions,
    tool: &str,
    transport: Transport,
    request: &ProcessRequest,
    warnings: Vec<String>,
) -> Outcome {
    let mut secrets = launch::credential_values(&request.env);
    secrets.extend(
        options
            .remote_auth_token_env
            .as_deref()
            .and_then(std::env::var_os)
            .map(|token| token.to_string_lossy().into_owned())
            .filter(|token| !token.is_empty()),
    );
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
            queued_outcome(options, tool, transport, &output.stdout, warnings)
        }
        Err(failure) => Outcome {
            exit_code: failure_exit_code(&failure),
            json: failure_json(tool, &failure),
            text: Text::Stderr(format!("error: {failure}")),
        },
    }
}

/// The refusal of a harness without a queue: exit 125, the reason, and the
/// harnesses that have one.
fn unsupported_outcome(options: &QueueOptions, tool: &str, reason: &str) -> Outcome {
    let message = format!(
        "cannot queue a message for a {tool} session: {reason}. Harnesses with a queue: {}",
        SUPPORTED.join(", ")
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
                "supported": SUPPORTED,
            },
        }),
        text: Text::Fatal(message),
    }
}

/// A message the harness took: the ids from its receipt line, or null ids
/// and a warning when it printed none.
fn queued_outcome(
    options: &QueueOptions,
    tool: &str,
    transport: Transport,
    stdout: &str,
    mut warnings: Vec<String>,
) -> Outcome {
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
            "session": options.session,
            "message_id": receipt.as_ref().map(|receipt| receipt.message_id.as_str()),
            "thread_id": receipt.as_ref().map(|receipt| receipt.thread_id.as_str()),
            "warnings": warnings,
        }),
        text: Text::Stdout(text),
    }
}

/// The command the queue would start, with the message as a placeholder
/// and every credential hidden. Nothing starts.
fn dry_run_outcome(
    options: &QueueOptions,
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
    let env = launch::redacted_env(&request.env);
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
