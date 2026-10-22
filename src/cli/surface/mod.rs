//! Noninteractive adapters for the coding surfaces `rune run` drives:
//! Claude, Codex, Grok, `agy`, and `OpenCode`. Each adapter owns its argument
//! contract, prompt transport, and output parsing; process lifecycle comes
//! from [`crate::cli::process`]. Serde field names on the event types mirror
//! each tool's real stdout and must not change.
use crate::cli::launch::{
    REDACTED_VALUE, credential_values, inherited_env, redact_credentials, redact_text,
};
use crate::cli::process::{
    ProcessFailure, ProcessOutput, ProcessRequest, ProcessTermination, run_process_request,
};
use serde::Deserialize;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod isolation;
pub(crate) use isolation::{Isolation, parent_session_isolation, tmux_removals};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    Claude,
    Codex,
    Agy,
    Grok,
    Opencode,
}

impl Surface {
    pub(crate) fn from_tool(tool: &str) -> Option<Self> {
        match tool {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "agy" => Some(Self::Agy),
            "grok" => Some(Self::Grok),
            "opencode" => Some(Self::Opencode),
            _ => None,
        }
    }
}

/// What a coding surface is permitted to touch. Enforcement per surface is a
/// mix of native sandbox flags, permission rules, and tool allowlists, so the
/// name describes the access granted rather than any single mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum AccessMode {
    ReadOnly,
    WorkspaceWrite,
    /// No tool at all: the harness answers from its prompt alone. For a seat
    /// the owner does not trust with the repository. Claude and `OpenCode` can
    /// run so; codex, agy and grok cannot switch their tools off and refuse.
    AnswerOnly,
}

impl AccessMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::AnswerOnly => "answer-only",
        }
    }
}

/// Why `surface` cannot take `mode`, else `None`. Codex, grok and agy have
/// no switch for answer-only. agy cannot enforce read-only access: it wrote
/// files in `--mode plan`, and with a settings file that denied every write
/// (experiments of 2026-10-22), so it runs in workspace-write mode only,
/// where its own jj workspace takes the edits.
pub(crate) fn unsupported_mode(surface: Surface, mode: AccessMode) -> Option<String> {
    match (surface, mode) {
        (Surface::Codex | Surface::Grok | Surface::Agy, AccessMode::AnswerOnly) => Some(format!(
            "answer-only mode needs a harness that runs without tools (claude or opencode); {} cannot switch its tools off",
            surface_name(surface)
        )),
        (Surface::Agy, AccessMode::ReadOnly) => Some(
            "agy cannot enforce read-only access, because it writes files in plan mode and ignores deny rules: use --mode workspace-write, which runs agy in its own jj workspace"
                .to_string(),
        ),
        _ => None,
    }
}

/// The refusal of a mode the harness cannot take, at the surface, so no
/// argument builder runs for it.
fn refuse_unsupported_mode(invocation: &SurfaceInvocation) -> Result<(), SurfaceFailure> {
    unsupported_mode(invocation.surface, invocation.mode)
        .map_or(Ok(()), |message| Err(SurfaceFailure::Arguments(message)))
}

#[derive(Debug, Clone)]
pub(crate) struct SurfaceInvocation {
    pub(crate) surface: Surface,
    pub(crate) binary: OsString,
    pub(crate) extra_args: Vec<OsString>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) repository: PathBuf,
    pub(crate) mode: AccessMode,
    pub(crate) system_prompt: String,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    pub(crate) native_timeout: Option<Duration>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) clean_state_root: Option<PathBuf>,
    /// A name for the harness session, sent where the harness takes one (see
    /// [`session_name_flag`]); the dry run and the JSON result say whether it went.
    pub(crate) session_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SurfaceReply {
    pub(crate) text: String,
    pub(crate) stderr: String,
    pub(crate) completion_tokens: Option<f64>,
    /// The id agy gives the conversation, which names the folder its
    /// generated files land in. `None` from every other surface.
    pub(crate) conversation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SurfaceFailure {
    /// The supervisor's failure. A stop it forced (deadline, forwarded
    /// signal, output limit) carries the errors the harness reported on
    /// the stdout it captured before the stop, so a denial reported before
    /// the stop still names the sandbox.
    Process {
        failure: ProcessFailure,
        harness_errors: Vec<String>,
    },
    /// A nonzero exit or a signal, with the last lines of both streams:
    /// `claude --print` and `opencode run` write their errors on stdout.
    /// `harness_errors` are the messages a harness reports in its own
    /// event stream (codex's JSONL), apart from the raw tails.
    Exit {
        termination: ProcessTermination,
        stdout_tail: String,
        stderr_tail: String,
        harness_errors: Vec<String>,
    },
    /// A clean exit without a usable answer, with the last lines of both
    /// streams, which hold the reason.
    NoAnswer {
        message: String,
        stdout_tail: String,
        stderr_tail: String,
        harness_errors: Vec<String>,
    },
    Reported(String),
    Io(String),
    Arguments(String),
    /// A failure with a note from the cleanup after it: the failure keeps
    /// its kind, its tails and its hint, and the note is appended.
    Noted {
        inner: Box<SurfaceFailure>,
        note: String,
    },
}

/// The last lines of one stream for an error: 40 lines or 4000 characters,
/// whichever is shorter.
const TAIL_LINES: usize = 40;
const TAIL_CHARS: usize = 4000;

pub(crate) fn output_tail(text: &str) -> String {
    let text = text.trim_end();
    let lines: Vec<&str> = text.lines().collect();
    let from_line = lines.len().saturating_sub(TAIL_LINES);
    let by_lines = lines[from_line..].join("\n");
    let chars = by_lines.chars().count();
    if chars <= TAIL_CHARS {
        return by_lines;
    }
    by_lines.chars().skip(chars - TAIL_CHARS).collect()
}

/// Where a sandbox signature counts. Free model text on stdout never does:
/// a model that quotes "Operation not permitted" is not a blocked harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HintSource {
    /// The harness's stderr, or the supervisor's own message.
    Stderr,
    /// Stderr, or an error message the harness reported in its own event
    /// stream (codex's JSONL, claude's `API Error:` lines).
    StderrOrHarnessError,
}

/// A known sandbox denial, where it counts, and the sentence that names the
/// sandbox. The signatures come from runs inside an outer macOS Seatbelt
/// sandbox: a blocked syscall, a blocked TLS trust evaluation for the proxy
/// certificate, and a blocked keyring read.
const SANDBOX_HINTS: &[(&str, HintSource, &str)] = &[
    (
        "Operation not permitted",
        HintSource::Stderr,
        "an outer sandbox blocked a file or network operation of the harness; run rune outside the sandbox",
    ),
    (
        "-26276",
        HintSource::StderrOrHarnessError,
        "an outer sandbox blocked the certificate check for the proxy (OSStatus -26276): set NODE_EXTRA_CA_CERTS to the proxy's root certificate file, or run rune outside the sandbox",
    ),
    (
        "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
        HintSource::StderrOrHarnessError,
        "an outer sandbox's filtering proxy answered the endpoint with a certificate the harness does not trust (UNABLE_TO_VERIFY_LEAF_SIGNATURE); run rune outside the sandbox",
    ),
    (
        "failed to load CLI auth from keyring",
        HintSource::Stderr,
        "an outer sandbox blocked the keyring that holds the harness credential; run rune outside the sandbox",
    ),
];

/// What the Claude Code sandbox's filtering proxy answers for a host that
/// is not on its network allowlist: a 403 whose body names the block. The
/// harness reports it as an `API Error:` line or on stderr.
const ALLOWLIST_SIGNATURES: &[&str] = &[
    "blocked-by-allowlist",
    "Connection blocked by network allowlist",
];

/// The hint for a failure: a known sandbox denial, a network allowlist
/// block, or agy's headless soft-deny, read from stderr and the harness's
/// own errors.
fn sandbox_hint(stderr: &[&str], harness_errors: &[String]) -> Option<String> {
    let texts = || {
        stderr
            .iter()
            .copied()
            .chain(harness_errors.iter().map(String::as_str))
    };
    SANDBOX_HINTS
        .iter()
        .find(|(signature, source, _)| {
            stderr.iter().any(|text| text.contains(signature))
                || (*source == HintSource::StderrOrHarnessError
                    && harness_errors.iter().any(|text| text.contains(signature)))
        })
        .map(|(_, _, hint)| (*hint).to_string())
        .or_else(|| allowlist_hint(texts()))
        .or_else(|| soft_deny_hint(texts()))
}

/// The line headless agy writes to its log when it denies a tool call it
/// cannot confirm, after the glog prefix:
/// `Print mode: soft-denying tool confirmation "RunCommand" at step 5`.
const AGY_SOFT_DENY: &str = "soft-denying tool confirmation";

/// The soft-deny lines of agy's log, each without its glog prefix.
fn agy_soft_denials(log: &str) -> Vec<String> {
    log.lines()
        .filter(|line| line.contains(AGY_SOFT_DENY))
        .map(|line| {
            line.find("Print mode:")
                .map_or(line.trim(), |at| line[at..].trim())
                .to_string()
        })
        .collect()
}

/// The sentence for agy's headless soft-deny, naming the denied tool: a
/// shell command for `RunCommand`, the tool's own name otherwise.
fn soft_deny_hint<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let text = texts
        .into_iter()
        .find(|text| text.contains(AGY_SOFT_DENY))?;
    let tool = text
        .split_once(AGY_SOFT_DENY)
        .and_then(|(_, rest)| rest.trim_start().strip_prefix('"'))
        .and_then(|rest| rest.split_once('"'))
        .map(|(tool, _)| tool);
    let call = match tool {
        Some("RunCommand") => "a shell command (RunCommand)".to_string(),
        Some(tool) => format!("its {tool} call"),
        None => "a tool call".to_string(),
    };
    Some(format!(
        "agy denied {call} in headless mode, which cannot ask for a tool confirmation: tell the model to read with its file tools, or run the prompt in an interactive agy session"
    ))
}

/// The sentence for a network allowlist block, naming the blocked host when
/// the text that reports the block holds the URL.
fn allowlist_hint<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let text = texts.into_iter().find(|text| {
        ALLOWLIST_SIGNATURES
            .iter()
            .any(|signature| text.contains(signature))
    })?;
    let host = url_host(text).map_or_else(
        || "the endpoint's host".to_string(),
        |host| format!("the endpoint's host {host}"),
    );
    Some(format!(
        "an outer sandbox's network allowlist blocked {host}: add it to the allowlist, or run rune outside the sandbox"
    ))
}

/// The host (with its port) of the first `http://` or `https://` URL in
/// `text`, without the user information before an `@`. The text is read
/// redacted, so `<` and `>` end nothing: `u:<redacted>@host` still yields
/// `host`.
fn url_host(text: &str) -> Option<String> {
    let start = ["https://", "http://"]
        .iter()
        .filter_map(|scheme| text.find(scheme).map(|at| at + scheme.len()))
        .min()?;
    let authority = &text[start..];
    let end = authority
        .find(|character: char| character.is_whitespace() || "/?#\"',;)]}\\".contains(character))
        .unwrap_or(authority.len());
    let authority = &authority[..end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    (!host.is_empty()).then(|| host.to_string())
}

/// The keys the route sets for the child: its environment, and for claude
/// the `env` keys of the one settings object the child reads, which claude
/// exports into its own process the same way.
pub(crate) fn route_keys(invocation: &SurfaceInvocation) -> Vec<OsString> {
    effective_env(invocation)
        .into_iter()
        .map(|(key, _)| key)
        .collect()
}

/// The environment as the child sees it: the route's process env, then for
/// claude the `env` of the one settings object, which wins where they
/// overlap because claude applies it inside its own process.
pub(crate) fn effective_env(invocation: &SurfaceInvocation) -> Vec<(OsString, OsString)> {
    let mut env = invocation.env.clone();
    if invocation.surface == Surface::Claude {
        env.extend(settings_env(&invocation.extra_args));
    }
    env
}

/// The `env` pairs of the one settings object the inline `--settings`
/// arguments in `args` merge into. A later object replaces `env` whole, as
/// claude's own merge does, so a key from an earlier object is not set.
pub(crate) fn settings_env(args: &[OsString]) -> Vec<(OsString, OsString)> {
    merged_settings(args)
        .0
        .and_then(|settings| settings.get("env").cloned())
        .and_then(|env| match env {
            serde_json::Value::Object(env) => Some(env),
            _ => None,
        })
        .map(|env| {
            env.into_iter()
                .map(|(key, value)| {
                    let value = match value {
                        serde_json::Value::String(text) => text,
                        other => other.to_string(),
                    };
                    (OsString::from(key), OsString::from(value))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every inline `--settings` object in `args` merged in order, a later
/// object replacing a key whole, and the arguments that are not one. A
/// value that is not a JSON object is skipped; `None` when there is none.
fn merged_settings(
    args: &[OsString],
) -> (
    Option<serde_json::Map<String, serde_json::Value>>,
    Vec<OsString>,
) {
    let (objects, remaining) = settings_objects(args);
    if objects.is_empty() {
        return (None, remaining);
    }
    let mut merged = serde_json::Map::new();
    for settings in objects {
        merged.extend(settings);
    }
    (Some(merged), remaining)
}

/// Every credential-named key's value inside `value`, at any depth, as
/// text: the values a settings object or a config table could leak. A
/// table under a credential-named key (`http_headers`) counts whole and
/// through each string inside it, since a harness echoes one header.
fn structured_credential_values(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Object(object) => object
            .iter()
            .flat_map(|(key, value)| {
                if crate::cli::launch::is_credential_env_key(key) {
                    match value {
                        serde_json::Value::String(text) => vec![text.clone()],
                        serde_json::Value::Null => Vec::new(),
                        other => {
                            let mut values = vec![other.to_string()];
                            values.extend(string_leaves(other));
                            values
                        }
                    }
                } else {
                    structured_credential_values(value)
                }
            })
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(structured_credential_values)
            .collect(),
        _ => Vec::new(),
    }
}

/// Every string inside `value`, at any depth.
fn string_leaves(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Object(object) => object.values().flat_map(string_leaves).collect(),
        serde_json::Value::Array(items) => items.iter().flat_map(string_leaves).collect(),
        serde_json::Value::String(text) => vec![text.clone()],
        _ => Vec::new(),
    }
}

/// The values of codex `-c key=value` assignments in `args` whose key path
/// holds a credential-named segment, as the TOML string when the value is
/// one, else as written; and every credential-named key's value nested
/// anywhere inside a TOML table value (`model_providers.x={ http_headers =
/// { Authorization = "..." } }`).
fn assignment_credential_values(args: &[OsString]) -> Vec<String> {
    let mut values = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let text = args[index].to_string_lossy();
        let assignment = if text == "-c" || text == "--config" {
            index += 1;
            args.get(index)
                .map(|value| value.to_string_lossy().into_owned())
        } else if let Some(rest) = text.strip_prefix("--config=") {
            Some(rest.to_string())
        } else {
            text.strip_prefix("-c")
                .map(|rest| rest.strip_prefix('=').unwrap_or(rest).to_string())
        };
        if let Some(assignment) = assignment
            && let Some((key, value)) = assignment.split_once('=')
        {
            if key
                .split('.')
                .any(|segment| crate::cli::launch::is_credential_env_key(segment.trim()))
            {
                values.push(codex_config_string(value));
            }
            if let Ok(parsed) = toml::from_str::<toml::Value>(&format!("v = {}", value.trim()))
                && let Ok(json) = serde_json::to_value(parsed)
            {
                values.extend(structured_credential_values(&json));
            }
        }
        index += 1;
    }
    values
}

/// The separate or attached value of every credential-named flag in
/// `args`: `--api-key x` and `--api-key=x`.
fn flag_credential_values(args: &[OsString]) -> Vec<String> {
    let mut values = Vec::new();
    let mut previous_is_credential_flag = false;
    for part in args {
        let part = part.to_string_lossy();
        if previous_is_credential_flag && !part.starts_with('-') {
            values.push(part.to_string());
        }
        previous_is_credential_flag = crate::cli::launch::is_credential_flag(&part);
        if let Some((flag, value)) = part.split_once('=')
            && crate::cli::launch::is_credential_flag(flag)
        {
            values.push(value.to_string());
        }
    }
    values
}

/// Each header value inside a `*_HEADERS` value of `env`: claude's
/// `ANTHROPIC_CUSTOM_HEADERS` carries `Name: value` lines, and a harness
/// echoes one header, not the whole variable.
fn header_values(env: &[(OsString, OsString)]) -> Vec<String> {
    env.iter()
        .filter(|(key, _)| {
            key.to_string_lossy()
                .to_ascii_uppercase()
                .contains("HEADERS")
        })
        .flat_map(|(_, value)| {
            value
                .to_string_lossy()
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(_, value)| value.trim().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The token after a `Bearer` or `Basic` scheme word in `value`, which an
/// error message quotes without the scheme.
fn scheme_token(value: &str) -> Option<String> {
    let (scheme, token) = value.trim().split_once(char::is_whitespace)?;
    (scheme.eq_ignore_ascii_case("bearer") || scheme.eq_ignore_ascii_case("basic"))
        .then(|| token.trim().to_string())
}

/// One argument of a claude argument list as the settings merge sees it:
/// an inline `--settings` object, or any other argument. A `--settings`
/// value that is not a JSON object (a file path) is an other argument,
/// kept with its flag.
enum SettingsArg {
    Object(serde_json::Map<String, serde_json::Value>),
    Other(OsString),
}

fn settings_args(args: &[OsString]) -> Vec<SettingsArg> {
    let mut parsed = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let text = args[index].to_string_lossy();
        let (value, width) = if text == "--settings" {
            (
                args.get(index + 1)
                    .map(|value| value.to_string_lossy().into_owned()),
                2,
            )
        } else if let Some(value) = text.strip_prefix("--settings=") {
            (Some(value.to_string()), 1)
        } else {
            parsed.push(SettingsArg::Other(args[index].clone()));
            index += 1;
            continue;
        };
        let end = (index + width).min(args.len());
        match value.and_then(|value| serde_json::from_str(&value).ok()) {
            Some(serde_json::Value::Object(settings)) => parsed.push(SettingsArg::Object(settings)),
            _ => parsed.extend(args[index..end].iter().cloned().map(SettingsArg::Other)),
        }
        index = end;
    }
    parsed
}

/// Every inline `--settings` object in `args`, and the arguments that are
/// not one.
fn settings_objects(
    args: &[OsString],
) -> (
    Vec<serde_json::Map<String, serde_json::Value>>,
    Vec<OsString>,
) {
    let mut objects = Vec::new();
    let mut remaining = Vec::new();
    for arg in settings_args(args) {
        match arg {
            SettingsArg::Object(settings) => objects.push(settings),
            SettingsArg::Other(arg) => remaining.push(arg),
        }
    }
    (objects, remaining)
}

/// `args` with every inline `--settings` object merged into one, in the
/// place of the last, a later object replacing a key whole. Claude reads
/// only its last `--settings`, so a profile's object and the command
/// line's would otherwise lose one of them. Fewer than two objects leave
/// `args` as they are.
pub(crate) fn merge_settings_args(args: &[OsString]) -> Vec<OsString> {
    let parsed = settings_args(args);
    let objects = parsed
        .iter()
        .filter(|arg| matches!(arg, SettingsArg::Object(_)))
        .count();
    if objects < 2 {
        return args.to_vec();
    }
    let mut merged = serde_json::Map::new();
    let mut seen = 0;
    let mut result = Vec::with_capacity(args.len());
    for arg in parsed {
        match arg {
            SettingsArg::Object(settings) => {
                merged.extend(settings);
                seen += 1;
                if seen == objects {
                    result.push(OsString::from("--settings"));
                    result.push(OsString::from(
                        serde_json::Value::Object(std::mem::take(&mut merged)).to_string(),
                    ));
                }
            }
            SettingsArg::Other(arg) => result.push(arg),
        }
    }
    result
}

/// Every credential value the child could echo, longest first so a value
/// that contains another is replaced whole: the effective environment's,
/// the inherited ones the isolation leaves in place, the ones inside the
/// retained structured arguments (the claude settings object, codex `-c`
/// assignments), the value of a credential-named flag, and the parts a
/// harness quotes alone: each header of a `*_HEADERS` value and the token
/// after a `Bearer` or `Basic` scheme word.
pub(crate) fn child_secrets(invocation: &SurfaceInvocation) -> Vec<String> {
    let removals = parent_session_isolation(&route_keys(invocation)).removals();
    let inherited = inherited_env(&removals);
    let effective = effective_env(invocation);
    let mut secrets = credential_values(&effective);
    secrets.extend(credential_values(&inherited));
    secrets.extend(header_values(&effective));
    secrets.extend(header_values(&inherited));
    match invocation.surface {
        Surface::Claude => {
            if let (Some(settings), _) = merged_settings(&invocation.extra_args) {
                secrets.extend(structured_credential_values(&serde_json::Value::Object(
                    settings,
                )));
            }
        }
        Surface::Codex => secrets.extend(assignment_credential_values(&invocation.extra_args)),
        // The owner's key map names the variable that holds each model's
        // key, whatever that variable is called: its value is a credential.
        Surface::Grok => {
            let names = grok_config_source()
                .map(|source| grok_key_variable_names(&source))
                .unwrap_or_default();
            secrets.extend(names.iter().filter_map(|name| {
                crate::cli::launch::env_value(&effective, name)
                    .or_else(|| crate::cli::launch::env_value(&inherited, name))
            }));
        }
        Surface::Agy | Surface::Opencode => {}
    }
    secrets.extend(flag_credential_values(&invocation.extra_args));
    let tokens: Vec<String> = secrets
        .iter()
        .filter_map(|secret| scheme_token(secret))
        .collect();
    secrets.extend(tokens);
    secrets.retain(|secret| !secret.is_empty());
    secrets.sort_unstable_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    secrets.dedup();
    secrets
}

impl SurfaceFailure {
    /// The failure with every credential value hidden in its message and
    /// its tails. The tails are built redacted already; this is the last
    /// word for the process layer's partial output and for messages.
    fn redacted(self, secrets: &[String]) -> Self {
        let hide = |text: String| redact_text(&text, secrets);
        match self {
            Self::Process {
                failure,
                harness_errors,
            } => Self::Process {
                harness_errors: harness_errors.into_iter().map(hide).collect(),
                failure: match failure {
                    ProcessFailure::Timeout {
                        timeout,
                        stdout,
                        stderr,
                    } => ProcessFailure::Timeout {
                        timeout,
                        stdout: hide(stdout),
                        stderr: hide(stderr),
                    },
                    ProcessFailure::OutputLimit {
                        stream,
                        limit,
                        stdout,
                        stderr,
                    } => ProcessFailure::OutputLimit {
                        stream,
                        limit,
                        stdout: hide(stdout),
                        stderr: hide(stderr),
                    },
                    ProcessFailure::ForwardedSignal {
                        signal,
                        stdout,
                        stderr,
                    } => ProcessFailure::ForwardedSignal {
                        signal,
                        stdout: hide(stdout),
                        stderr: hide(stderr),
                    },
                    ProcessFailure::Spawn(message) => ProcessFailure::Spawn(hide(message)),
                    ProcessFailure::Wait {
                        message,
                        stdout,
                        stderr,
                    } => ProcessFailure::Wait {
                        message: hide(message),
                        stdout: hide(stdout),
                        stderr: hide(stderr),
                    },
                    ProcessFailure::Stdin(message) => ProcessFailure::Stdin(hide(message)),
                    ProcessFailure::Stdout(message) => ProcessFailure::Stdout(hide(message)),
                    ProcessFailure::Stderr(message) => ProcessFailure::Stderr(hide(message)),
                },
            },
            Self::Exit {
                termination,
                stdout_tail,
                stderr_tail,
                harness_errors,
            } => Self::Exit {
                termination,
                stdout_tail: hide(stdout_tail),
                stderr_tail: hide(stderr_tail),
                harness_errors: harness_errors.into_iter().map(hide).collect(),
            },
            Self::NoAnswer {
                message,
                stdout_tail,
                stderr_tail,
                harness_errors,
            } => Self::NoAnswer {
                message: hide(message),
                stdout_tail: hide(stdout_tail),
                stderr_tail: hide(stderr_tail),
                harness_errors: harness_errors.into_iter().map(hide).collect(),
            },
            Self::Reported(message) => Self::Reported(hide(message)),
            Self::Io(message) => Self::Io(hide(message)),
            Self::Arguments(message) => Self::Arguments(hide(message)),
            Self::Noted { inner, note } => Self::Noted {
                inner: Box::new(inner.redacted(secrets)),
                note: hide(note),
            },
        }
    }

    /// The failure a cleanup note wraps, else the failure itself.
    pub(crate) fn unwrapped(&self) -> &Self {
        match self {
            Self::Noted { inner, .. } => inner.unwrapped(),
            other => other,
        }
    }

    /// The same failure with a cleanup note appended, its kind and tails
    /// kept.
    fn with_note(self, note: String) -> Self {
        Self::Noted {
            inner: Box::new(self),
            note,
        }
    }

    /// The last lines of stdout and stderr, for failures that carry them:
    /// an exit, a clean exit without an answer, and every stop the
    /// supervisor forced (deadline, forwarded signal, output limit).
    pub(crate) fn tails(&self) -> Option<(&str, &str)> {
        match self {
            Self::Process {
                failure:
                    ProcessFailure::Timeout { stdout, stderr, .. }
                    | ProcessFailure::ForwardedSignal { stdout, stderr, .. }
                    | ProcessFailure::OutputLimit { stdout, stderr, .. }
                    | ProcessFailure::Wait { stdout, stderr, .. },
                ..
            } => Some((stdout, stderr)),
            Self::Noted { inner, .. } => inner.tails(),
            Self::Exit {
                stdout_tail,
                stderr_tail,
                ..
            }
            | Self::NoAnswer {
                stdout_tail,
                stderr_tail,
                ..
            } => Some((stdout_tail, stderr_tail)),
            Self::Process { .. } | Self::Reported(_) | Self::Io(_) | Self::Arguments(_) => None,
        }
    }

    /// The messages the harness reported in its own event stream.
    pub(crate) fn harness_errors(&self) -> &[String] {
        match self {
            Self::Process { harness_errors, .. }
            | Self::Exit { harness_errors, .. }
            | Self::NoAnswer { harness_errors, .. } => harness_errors,
            Self::Noted { inner, .. } => inner.harness_errors(),
            Self::Reported(_) | Self::Io(_) | Self::Arguments(_) => &[],
        }
    }

    /// The same failure with the harness's own error messages attached,
    /// where the failure can carry them.
    fn with_harness_errors(self, errors: Vec<String>) -> Self {
        match self {
            Self::Process { failure, .. } => Self::Process {
                failure,
                harness_errors: errors,
            },
            Self::Exit {
                termination,
                stdout_tail,
                stderr_tail,
                ..
            } => Self::Exit {
                termination,
                stdout_tail,
                stderr_tail,
                harness_errors: errors,
            },
            Self::NoAnswer {
                message,
                stdout_tail,
                stderr_tail,
                ..
            } => Self::NoAnswer {
                message,
                stdout_tail,
                stderr_tail,
                harness_errors: errors,
            },
            other => other,
        }
    }

    /// The sandbox sentence when the harness's stderr, the supervisor's own
    /// message, or a harness-reported error carries a known denial or a
    /// network allowlist block. Stdout is never read: it holds the model's
    /// text.
    pub(crate) fn hint(&self) -> Option<String> {
        let message = match self.unwrapped() {
            Self::Process { failure, .. } => Some(failure.to_string()),
            _ => None,
        };
        let (_, stderr_tail) = self.tails().unwrap_or_default();
        let stderr: Vec<&str> = std::iter::once(stderr_tail)
            .chain(message.as_deref())
            .collect();
        sandbox_hint(&stderr, self.harness_errors())
    }

    /// Turn a parse failure of a clean exit into a `NoAnswer` that carries
    /// the output the parse rejected, credential values hidden before the
    /// tail is cut.
    fn with_output(self, output: &ProcessOutput, secrets: &[String]) -> Self {
        match self {
            Self::Reported(message) => Self::NoAnswer {
                message,
                stdout_tail: redacted_tail(&output.stdout, secrets),
                stderr_tail: redacted_tail(&output.stderr, secrets),
                harness_errors: Vec::new(),
            },
            other => other,
        }
    }
}

/// `harness errors:`, `stdout tail:` and `stderr tail:` sections, each
/// line indented, then the sandbox hint when there is one.
fn write_tails_and_hint(
    formatter: &mut fmt::Formatter<'_>,
    failure: &SurfaceFailure,
) -> fmt::Result {
    let harness_errors = failure.harness_errors();
    if !harness_errors.is_empty() {
        write!(formatter, "\nharness errors:")?;
        for error in harness_errors {
            write!(formatter, "\n  {}", shown(error))?;
        }
    }
    if let Some((stdout_tail, stderr_tail)) = failure.tails() {
        for (name, tail) in [("stdout", stdout_tail), ("stderr", stderr_tail)] {
            let tail = output_tail(tail);
            if tail.is_empty() {
                continue;
            }
            write!(formatter, "\n{name} tail:")?;
            for line in tail.lines() {
                write!(formatter, "\n  {line}")?;
            }
        }
    }
    if let Some(hint) = failure.hint() {
        write!(formatter, "\nhint: {hint}")?;
    }
    Ok(())
}

impl fmt::Display for SurfaceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process { failure, .. } => failure.fmt(formatter)?,
            Self::Exit { termination, .. } => write!(formatter, "process {termination:?}")?,
            Self::NoAnswer { message, .. }
            | Self::Reported(message)
            | Self::Io(message)
            | Self::Arguments(message) => formatter.write_str(message)?,
            Self::Noted { inner, note } => {
                inner.fmt(formatter)?;
                return write!(formatter, "\nnote: {note}");
            }
        }
        write_tails_and_hint(formatter, self)
    }
}

impl From<ProcessFailure> for SurfaceFailure {
    fn from(failure: ProcessFailure) -> Self {
        Self::Process {
            failure,
            harness_errors: Vec::new(),
        }
    }
}

/// A supervisor failure with the harness's own errors parsed from the
/// stdout it captured, where the failure carries that stdout: a stop the
/// supervisor forced still names a denial the harness reported before it.
fn stopped_with_errors(
    failure: ProcessFailure,
    parse: impl Fn(&str) -> Vec<String>,
) -> SurfaceFailure {
    let failure = SurfaceFailure::from(failure);
    let errors = failure
        .tails()
        .map(|(stdout, _)| parse(stdout))
        .unwrap_or_default();
    failure.with_harness_errors(errors)
}

fn is_short_option(option: &str) -> bool {
    option.len() == 2 && option.starts_with('-') && !option.starts_with("--")
}

fn surface_name(surface: Surface) -> &'static str {
    match surface {
        Surface::Claude => "claude",
        Surface::Codex => "codex",
        Surface::Agy => "agy",
        Surface::Grok => "grok",
        Surface::Opencode => "opencode",
    }
}

/// The flag that names a session on `surface`: claude `--name`, opencode
/// `run --title`. Codex, agy and grok take none, and a session name given to
/// them is reported, not sent.
pub(crate) fn session_name_flag(surface: Surface) -> Option<&'static str> {
    match surface {
        Surface::Claude => Some("--name"),
        Surface::Opencode => Some("--title"),
        Surface::Codex | Surface::Agy | Surface::Grok => None,
    }
}

/// The flags of one surface that stand alone, so a following token is never
/// their value. Everything else the surface owns takes a value. Spellings
/// are not portable: `-p` is claude's print switch and codex's profile.
fn boolean_options(surface: Surface) -> &'static [&'static str] {
    match surface {
        Surface::Claude => &[
            "-p",
            "--print",
            "--dangerously-skip-permissions",
            "--allow-dangerously-skip-permissions",
        ],
        Surface::Codex => &[
            "--json",
            "--oss",
            "--ignore-rules",
            "--dangerously-bypass-approvals-and-sandbox",
        ],
        Surface::Grok => &["--always-approve", "--no-plan"],
        Surface::Agy => &["--dangerously-skip-permissions"],
        Surface::Opencode => &["--print", "--disable-slash-commands"],
    }
}

/// One profile argument after parsing: the option it names, the value it
/// carries inline (`--flag=value`, `-fvalue`), and whether the value came
/// attached. A token that names no owned option is `None`.
struct OwnedArg {
    option: &'static str,
    inline: Option<String>,
}

fn owned_arg(token: &OsString, owned: &[&'static str]) -> Option<OwnedArg> {
    let text = token.to_string_lossy();
    for option in owned {
        if text == *option {
            return Some(OwnedArg {
                option,
                inline: None,
            });
        }
        if option.starts_with("--") {
            if let Some(rest) = text.strip_prefix(option)
                && let Some(value) = rest.strip_prefix('=')
            {
                return Some(OwnedArg {
                    option,
                    inline: Some(value.to_string()),
                });
            }
        } else if is_short_option(option)
            && let Some(rest) = text.strip_prefix(option)
        {
            // `-cvalue` and `-c=value` both attach the value to the short flag.
            let value = rest.strip_prefix('=').unwrap_or(rest);
            return Some(OwnedArg {
                option,
                inline: Some(value.to_string()),
            });
        }
    }
    None
}

/// A value shown in a warning: one line, control characters escaped, long
/// values cut, so a profile cannot forge or hide a warning.
fn shown(value: &str) -> String {
    let mut out: String = value
        .chars()
        .take(120)
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect();
    if value.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// The profile arguments the automated run keeps, and one warning line per
/// argument it drops. The run owns the flags in `owned_options`, so a
/// profile value for one of them is dropped with a warning instead of
/// refusing the run. Codex `-c key=value` and claude `--settings` are
/// filtered at the key level by the surface's `keyed` hook.
#[derive(Debug, Default)]
pub(crate) struct FilteredArgs {
    pub(crate) kept: Vec<OsString>,
    pub(crate) warnings: Vec<String>,
}

/// A key-level decision for one owned option and its value: `None` means
/// no key-level opinion (drop whole with the generic warning),
/// `Some((replacement, warnings))` keeps `replacement` under the same option
/// when it is `Some` and always emits `warnings`.
type Keyed = Option<(Option<String>, Vec<String>)>;

fn filter_profile_args(
    invocation: &SurfaceInvocation,
    owned: &[&'static str],
    booleans: &[&'static str],
    keyed: &dyn Fn(&str, &str) -> Keyed,
) -> FilteredArgs {
    let name = surface_name(invocation.surface);
    let mut filtered = FilteredArgs::default();
    let args = &invocation.extra_args;
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = owned_arg(&args[index], owned) else {
            filtered.kept.push(args[index].clone());
            index += 1;
            continue;
        };
        let takes_value = !booleans.contains(&arg.option);
        // A value in the next token, unless that token is itself a flag: an
        // owned option with a missing value must not swallow its neighbor.
        let next_is_value = takes_value
            && arg.inline.is_none()
            && args
                .get(index + 1)
                .is_some_and(|next| !next.to_string_lossy().starts_with('-'));
        let value = arg
            .inline
            .clone()
            .or_else(|| next_is_value.then(|| args[index + 1].to_string_lossy().into_owned()));
        let attached = arg.inline.is_some();
        match value.as_deref().and_then(|v| keyed(arg.option, v)) {
            Some((replacement, mut notes)) => {
                if let Some(replacement) = replacement {
                    if attached && arg.option.starts_with("--") {
                        filtered
                            .kept
                            .push(OsString::from(format!("{}={replacement}", arg.option)));
                    } else {
                        filtered.kept.push(OsString::from(arg.option));
                        filtered.kept.push(OsString::from(replacement));
                    }
                }
                filtered.warnings.append(&mut notes);
            }
            // The value is shown only for an option whose values come from
            // a fixed list, and only when it is one of them (`--sandbox
            // read-only`); every other value is hidden whole.
            None => filtered.warnings.push(match value.as_deref() {
                Some(v) if !v.is_empty() => format!(
                    "automated {name} execution owns {}; the profile value {} is dropped",
                    arg.option,
                    dropped_value(invocation.surface, arg.option, v)
                ),
                _ => format!(
                    "automated {name} execution owns {}; the profile flag is dropped",
                    arg.option
                ),
            }),
        }
        index += 1 + usize::from(next_is_value);
    }
    filtered
}

/// The owned options and config keys of one surface whose values come from
/// a fixed list the harness documents (`--sandbox read-only`,
/// `--permission-mode plan`), with that list: the only dropped values a
/// warning shows, and only when the value is one of them. The lists come
/// from each harness's own `--help` (grok names no sandbox profiles there,
/// so its list holds the two profiles this adapter passes). Spellings are
/// not portable: `-s` is codex's sandbox and opencode's session.
fn fixed_choice_values(surface: Surface, name: &str) -> Option<&'static [&'static str]> {
    let values: &'static [&'static str] = match (surface, name) {
        (Surface::Claude | Surface::Agy, "--output-format") => &["text", "json", "stream-json"],
        (Surface::Claude, "--permission-mode") => &[
            "acceptEdits",
            "auto",
            "bypassPermissions",
            "manual",
            "dontAsk",
            "plan",
        ],
        (Surface::Codex, "-s" | "--sandbox" | "sandbox_mode") => {
            &["read-only", "workspace-write", "danger-full-access"]
        }
        (Surface::Codex, "approval_policy") => &["on-request", "never"],
        (Surface::Grok, "--output-format") => {
            &["plain", "json", "streaming-json", "streaming-messages-json"]
        }
        (Surface::Grok, "--permission-mode") => &[
            "default",
            "acceptEdits",
            "auto",
            "dontAsk",
            "bypassPermissions",
            "plan",
        ],
        (Surface::Grok, "--sandbox") => &["read-only", "workspace"],
        (Surface::Agy, "--mode") => &["accept-edits", "auto", "plan"],
        (Surface::Opencode, "--format") => &["default", "json"],
        _ => return None,
    };
    Some(values)
}

/// A dropped profile value as a warning shows it: as written when the
/// option's values come from a fixed list and the value is one of them,
/// so the owner reads which mode was dropped; hidden whole otherwise. A
/// free value can carry a credential under any name (an MCP server's
/// `args` array holds `--api-key` and the key side by side, a URL holds a
/// password), an allowed flag can carry one too (`--sandbox=https://
/// user:secret@host`), and a by-name redaction cannot know every shape,
/// so only a known value reaches a warning.
fn dropped_value(surface: Surface, name: &str, value: &str) -> String {
    if fixed_choice_values(surface, name).is_some_and(|values| values.contains(&value)) {
        shown(value)
    } else {
        REDACTED_VALUE.to_string()
    }
}

/// No key-level handling: every owned option is dropped whole.
fn drop_whole(_: &str, _: &str) -> Keyed {
    None
}

/// Codex config keys the run owns, matched on the first path segment so
/// `sandbox_workspace_write.network_access` falls under `sandbox_workspace_write`.
const CODEX_OWNED_KEYS: &[&str] = &[
    "model",
    "model_provider",
    "sandbox_mode",
    "sandbox_workspace_write",
    "approval_policy",
    "cwd",
    "shell_environment_policy",
    "mcp_servers",
];

/// The string a codex `-c model=<value>` names: the TOML string when the
/// value is one, else the raw value, which codex takes as a bare string.
fn codex_config_string(value: &str) -> String {
    toml::from_str::<toml::Value>(&format!("v = {value}"))
        .ok()
        .and_then(|parsed| {
            parsed
                .get("v")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| value.trim().to_string())
}

/// Codex `-c key=value`: keep keys the run does not set, drop the rest. A
/// `model` the run passes anyway is dropped without a warning.
fn codex_config_key(option: &str, assignment: &str, model: Option<&str>) -> Keyed {
    if option != "-c" && option != "--config" {
        return None;
    }
    let Some((key, value)) = assignment.split_once('=') else {
        return Some((
            None,
            vec![format!(
                "automated codex execution dropped {option} {REDACTED_VALUE}: expected key=value"
            )],
        ));
    };
    let key = key.trim();
    let head = key.split('.').next().unwrap_or(key);
    if !CODEX_OWNED_KEYS.contains(&head) {
        return Some((Some(assignment.to_string()), Vec::new()));
    }
    if key == "model" && model.is_some_and(|model| codex_config_string(value) == model) {
        return Some((None, Vec::new()));
    }
    // `sandbox_mode` and `approval_policy` read as written; every other
    // value (`mcp_servers.*.env.TOKEN`, an `args` array with a key in it)
    // is hidden.
    Some((
        None,
        vec![format!(
            "automated codex execution owns config key {}; the profile value {} is dropped",
            shown(key),
            dropped_value(Surface::Codex, key, &codex_config_string(value))
        )],
    ))
}

/// Claude settings keys that change permissions, hooks, tools, plugins, or
/// the model. Everything else, `env` included, stays.
const CLAUDE_OWNED_SETTINGS: &[&str] = &[
    "permissions",
    "hooks",
    "allowedTools",
    "disallowedTools",
    "enabledPlugins",
    "model",
    "sandbox",
];

/// Claude `--settings <json>`: keep `env` and unknown keys, drop the keys
/// the run owns, and drop a value that is not an inline JSON object (a
/// file path would load unfiltered settings). A `model` the run passes
/// anyway is dropped without a warning.
fn claude_settings_key(option: &str, json: &str, model: Option<&str>) -> Keyed {
    if option != "--settings" {
        return None;
    }
    let Ok(serde_json::Value::Object(mut settings)) =
        serde_json::from_str::<serde_json::Value>(json)
    else {
        // A settings file is not filtered, so it is not loaded: an endpoint or
        // credential inside it never reaches the child, and the seat runs on
        // the route's or the inherited endpoint instead. Said out loud.
        return Some((
            None,
            vec![format!(
                "automated claude execution dropped --settings {REDACTED_VALUE}: only an inline JSON object is filtered; an endpoint or credential in that file does not apply, so the run uses the route's or the inherited endpoint"
            )],
        ));
    };
    let mut warnings = Vec::new();
    for key in CLAUDE_OWNED_SETTINGS {
        let Some(value) = settings.remove(*key) else {
            continue;
        };
        if *key == "model" && value.as_str().is_some_and(|value| Some(value) == model) {
            continue;
        }
        // The key alone: `hooks` and `env` values may hold a credential.
        warnings.push(format!(
            "automated claude execution owns settings key {key}; the profile value {REDACTED_VALUE} is dropped"
        ));
    }
    if settings.is_empty() {
        // Nothing to say when every dropped key went silently: the run
        // sends the same model, so the profile lost nothing.
        if !warnings.is_empty() {
            warnings.push(
                "automated claude execution dropped --settings: nothing remains after the owned keys"
                    .to_string(),
            );
        }
        return Some((None, warnings));
    }
    Some((
        Some(serde_json::Value::Object(settings).to_string()),
        warnings,
    ))
}

fn combined_prompt(invocation: &SurfaceInvocation) -> String {
    if invocation.system_prompt.trim().is_empty() {
        invocation.prompt.clone()
    } else {
        format!(
            "{}\n\n{}",
            invocation.system_prompt.trim(),
            invocation.prompt
        )
    }
}

fn copy_optional_auth_file(
    source: &Path,
    target: &Path,
    surface: &str,
) -> Result<(), SurfaceFailure> {
    match std::fs::copy(source, target) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SurfaceFailure::Io(format!(
            "cannot copy {surface} authentication file {}: {error}",
            source.display()
        ))),
    }
}

/// The clean Codex configuration keeps the route: every `[model_providers.*]`
/// table reduced to its transport fields and `auth`, and a top-level
/// `model_provider` that names the one custom table when the file defines
/// exactly one. The file-level name is never copied from the source. The
/// runner places every `--config` before `exec` (see [`codex_args`]), so
/// the wrapper's global provider flag survives; the file names the one
/// custom table as well, so a clean run keeps its route whatever the
/// flag order. With several tables, or none, the launch-time choice
/// stands and the built-in default applies.
fn copy_codex_route_config(source: &Path, target: &Path) -> Result<(), SurfaceFailure> {
    const ROUTE_KEYS: [&str; 8] = [
        "name",
        "base_url",
        "wire_api",
        "env_key",
        "http_headers",
        "env_http_headers",
        "query_params",
        "auth",
    ];
    let text = std::fs::read_to_string(source).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot read Codex route configuration {}: {error}",
            source.display()
        ))
    })?;
    let config: toml::Value = toml::from_str(&text).map_err(|error| {
        SurfaceFailure::Arguments(format!(
            "cannot parse Codex route configuration {}: {error}",
            source.display()
        ))
    })?;
    let mut clean = toml::map::Map::new();
    let mut providers = toml::map::Map::new();
    let mut custom_provider: Option<String> = None;
    for (name, provider) in config
        .get("model_providers")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
    {
        let Some(provider) = provider.as_table() else {
            continue;
        };
        let clean_provider: toml::map::Map<String, toml::Value> = ROUTE_KEYS
            .iter()
            .filter_map(|key| {
                provider
                    .get(*key)
                    .map(|value| ((*key).to_string(), value.clone()))
            })
            .collect();
        custom_provider = match (custom_provider, providers.is_empty()) {
            (None, true) => Some(name.clone()),
            _ => None,
        };
        providers.insert(name.clone(), toml::Value::Table(clean_provider));
    }
    if providers.len() == 1
        && let Some(name) = custom_provider
    {
        clean.insert("model_provider".to_string(), toml::Value::String(name));
    }
    if !providers.is_empty() {
        clean.insert("model_providers".to_string(), toml::Value::Table(providers));
    }
    let rendered = toml::to_string(&toml::Value::Table(clean)).map_err(|error| {
        SurfaceFailure::Arguments(format!("cannot render Codex clean configuration: {error}"))
    })?;
    std::fs::write(target, rendered).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot write Codex clean configuration {}: {error}",
            target.display()
        ))
    })
}

fn copy_opencode_route_config(
    source: &Path,
    target: &Path,
    model: Option<&str>,
) -> Result<(), SurfaceFailure> {
    let text = match std::fs::read_to_string(source) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(error) => {
            return Err(SurfaceFailure::Io(format!(
                "cannot read OpenCode route configuration {}: {error}",
                source.display()
            )));
        }
    };
    let source_config: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        SurfaceFailure::Arguments(format!(
            "cannot parse OpenCode route configuration {}: {error}",
            source.display()
        ))
    })?;
    let mut clean = serde_json::Map::new();
    if let Some(schema) = source_config.get("$schema") {
        clean.insert("$schema".to_string(), schema.clone());
    }
    if let Some(source_providers) = source_config
        .get("provider")
        .and_then(serde_json::Value::as_object)
    {
        let selected_provider = model
            .and_then(|value| value.split_once('/').map(|pair| pair.0))
            .and_then(|provider_name| {
                source_providers
                    .get(provider_name)
                    .map(|provider| (provider_name, provider))
            });
        let providers = if let Some((provider_name, provider)) = selected_provider {
            serde_json::Map::from_iter([(provider_name.to_string(), provider.clone())])
        } else {
            source_providers.clone()
        };
        clean.insert("provider".to_string(), serde_json::Value::Object(providers));
    }
    let rendered = serde_json::to_vec(&serde_json::Value::Object(clean)).map_err(|error| {
        SurfaceFailure::Arguments(format!(
            "cannot render OpenCode clean configuration: {error}"
        ))
    })?;
    std::fs::write(target, rendered).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot write OpenCode clean configuration {}: {error}",
            target.display()
        ))
    })
}

/// The owner's grok configuration, when there is one.
fn read_grok_config(source: &Path) -> Result<Option<toml::Value>, SurfaceFailure> {
    let text = match std::fs::read_to_string(source) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(SurfaceFailure::Io(format!(
                "cannot read Grok configuration {}: {error}",
                source.display()
            )));
        }
    };
    // The toml crate's message quotes the source line, which may hold a
    // credential: the error names the place and nothing from the file.
    toml::from_str(&text).map(Some).map_err(|error| {
        let (line, column) = line_and_column(&text, error.span().map_or(0, |span| span.start));
        SurfaceFailure::Arguments(format!(
            "cannot parse Grok configuration {} at line {line}, column {column}",
            source.display()
        ))
    })
}

/// The one-based line and column of byte `offset` in `text`.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, rest)| rest)
        .chars()
        .count()
        + 1;
    (line, column)
}

/// The `env_key` entries of a grok `model` table, and nothing else: the
/// name of the environment variable that holds each model's API key. A
/// model id with a dot that was written unquoted nests one table deeper,
/// so the walk recurses. Every other table of the owner's file (`cli`,
/// `marketplace`, `ui`, `sandbox`, `shell_environment_policy`, `tools`,
/// `session`, `telemetry`, `permission`) stays with the owner.
fn grok_env_keys_only(table: &toml::map::Map<String, toml::Value>) -> Option<toml::Value> {
    let mut kept = toml::map::Map::new();
    if let Some(env_key) = table.get("env_key").filter(|value| value.is_str()) {
        kept.insert("env_key".to_string(), env_key.clone());
    }
    for (key, value) in table {
        if let Some(inner) = value.as_table()
            && let Some(inner_kept) = grok_env_keys_only(inner)
        {
            kept.insert(key.clone(), inner_kept);
        }
    }
    (!kept.is_empty()).then_some(toml::Value::Table(kept))
}

/// The dotted names of the keys a clean grok run keeps, `model.<id>.env_key`.
fn grok_kept_key_names(prefix: &str, value: &toml::Value) -> Vec<String> {
    let Some(table) = value.as_table() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for (key, inner) in table {
        let path = format!("{prefix}.{key}");
        if key == "env_key" && inner.is_str() {
            names.push(path);
        } else {
            names.extend(grok_kept_key_names(&path, inner));
        }
    }
    names
}

/// The clean grok `model` table from the owner's configuration, with the
/// names of the kept keys. Empty without a configuration or a key map.
fn grok_clean_model_table(
    source: &Path,
) -> Result<Option<(toml::Value, Vec<String>)>, SurfaceFailure> {
    let Some(config) = read_grok_config(source)? else {
        return Ok(None);
    };
    let kept = config
        .get("model")
        .and_then(toml::Value::as_table)
        .and_then(grok_env_keys_only);
    Ok(kept.map(|models| {
        let names = grok_kept_key_names("model", &models);
        (models, names)
    }))
}

/// Write the clean grok configuration: the `model.<id>.env_key` entries
/// alone. Nothing is written without a key map, and the kept key names
/// come back for the result.
fn copy_grok_model_keys(source: &Path, target: &Path) -> Result<Vec<String>, SurfaceFailure> {
    let Some((models, names)) = grok_clean_model_table(source)? else {
        return Ok(Vec::new());
    };
    let mut clean = toml::map::Map::new();
    clean.insert("model".to_string(), models);
    let rendered = toml::to_string(&toml::Value::Table(clean)).map_err(|error| {
        SurfaceFailure::Arguments(format!("cannot render Grok clean configuration: {error}"))
    })?;
    std::fs::write(target, rendered).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot write Grok clean configuration {}: {error}",
            target.display()
        ))
    })?;
    Ok(names)
}

/// The names of the environment variables the owner's grok key map points
/// at: every `env_key` value under `model`. Empty without a readable key
/// map, since this feeds redaction and never fails a run.
pub(crate) fn grok_key_variable_names(source: &Path) -> Vec<String> {
    fn collect(value: &toml::Value, names: &mut Vec<String>) {
        if let Some(table) = value.as_table() {
            for (key, inner) in table {
                match inner.as_str() {
                    Some(name) if key == "env_key" => names.push(name.to_string()),
                    _ => collect(inner, names),
                }
            }
        }
    }
    let mut names = Vec::new();
    if let Ok(Some((models, _))) = grok_clean_model_table(source) {
        collect(&models, &mut names);
    }
    names
}

/// The environment variable the owner's grok key map names for `model`
/// alone: `model.<model>.env_key`. `None` without a readable key map or
/// an entry for that model, so a check of one model never borrows the key
/// of another.
pub(crate) fn grok_model_env_key(source: &Path, model: &str) -> Option<String> {
    let (models, _) = grok_clean_model_table(source).ok()??;
    models
        .get(model)?
        .get("env_key")?
        .as_str()
        .map(str::to_string)
}

/// The owner's grok configuration file.
pub(crate) fn grok_config_source() -> Result<PathBuf, SurfaceFailure> {
    dirs::home_dir()
        .map(|home| home.join(".grok/config.toml"))
        .ok_or_else(|| SurfaceFailure::Io("cannot resolve home directory".to_string()))
}

/// The names of the configuration keys a clean run of `surface` keeps
/// from the owner's files, for the dry run: grok's `model.<id>.env_key`
/// entries. The other surfaces keep no configuration keys this way.
pub(crate) fn clean_config_keys(surface: Surface) -> Result<Vec<String>, SurfaceFailure> {
    if surface != Surface::Grok {
        return Ok(Vec::new());
    }
    Ok(grok_clean_model_table(&grok_config_source()?)?
        .map(|(_, names)| names)
        .unwrap_or_default())
}

pub(crate) fn prepare_clean_state(
    surface: Surface,
    root: &Path,
    model: Option<&str>,
) -> Result<(), SurfaceFailure> {
    let home = dirs::home_dir()
        .ok_or_else(|| SurfaceFailure::Io("cannot resolve home directory".to_string()))?;
    match surface {
        Surface::Codex => {
            let source_root =
                std::env::var_os("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from);
            copy_codex_route_config(&source_root.join("config.toml"), &root.join("config.toml"))
        }
        Surface::Grok => {
            // A grok route may authenticate through its environment alone
            // (`GROK_MODELS_BASE_URL` and an API key variable), so the file
            // is bridged when it exists and its absence stops nothing.
            let source = home.join(".grok/auth.json");
            let target_root = root.join(".grok");
            std::fs::create_dir_all(&target_root).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot create Grok clean state {}: {error}",
                    target_root.display()
                ))
            })?;
            copy_optional_auth_file(&source, &target_root.join("auth.json"), "Grok")?;
            // The model key map alone: without it a clean run is "Not
            // signed in", and the rest of the owner's file stays home.
            copy_grok_model_keys(&grok_config_source()?, &target_root.join("config.toml"))?;
            Ok(())
        }
        Surface::Opencode => {
            let source_config_root = std::env::var_os("XDG_CONFIG_HOME")
                .map_or_else(|| home.join(".config"), PathBuf::from);
            let target_config_root = root.join("config/opencode");
            std::fs::create_dir_all(&target_config_root).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot create OpenCode clean configuration {}: {error}",
                    target_config_root.display()
                ))
            })?;
            copy_opencode_route_config(
                &source_config_root.join("opencode/opencode.json"),
                &target_config_root.join("opencode.json"),
                model,
            )?;

            let source_data_root = std::env::var_os("XDG_DATA_HOME")
                .map_or_else(|| home.join(".local/share"), PathBuf::from);
            let source = source_data_root.join("opencode/auth.json");
            let target_data_root = root.join("data/opencode");
            std::fs::create_dir_all(&target_data_root).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot create OpenCode clean state {}: {error}",
                    target_data_root.display()
                ))
            })?;
            copy_optional_auth_file(&source, &target_data_root.join("auth.json"), "OpenCode")
        }
        Surface::Agy => {
            let source_root = std::env::var_os("ANTIGRAVITY_EXECUTABLE_DATA_DIR")
                .map_or_else(|| home.join(".gemini/antigravity-cli"), PathBuf::from);
            std::fs::create_dir_all(root).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot create Agy clean state {}: {error}",
                    root.display()
                ))
            })?;
            let source = source_root.join("antigravity-oauth-token");
            std::fs::copy(&source, root.join("antigravity-oauth-token")).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot bridge Agy authentication {}: {error}",
                    source.display()
                ))
            })?;
            Ok(())
        }
        Surface::Claude => {
            let source_root = std::env::var_os("CLAUDE_CONFIG_DIR")
                .map_or_else(|| home.join(".claude"), PathBuf::from);
            copy_optional_auth_file(
                &source_root.join(".credentials.json"),
                &root.join(".credentials.json"),
                "Claude",
            )
        }
    }
}

/// The Grok install directory under the real home, when it holds a `grok`
/// executable; `None` when Grok is installed elsewhere or not at all.
fn grok_install_dir(home: &Path) -> Option<OsString> {
    let directory = home.join(".grok/bin");
    let binary = directory.join("grok");
    let executable = std::fs::metadata(&binary).is_ok_and(|metadata| metadata.is_file());
    executable.then(|| directory.into_os_string())
}

fn process_request(
    invocation: &SurfaceInvocation,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
) -> ProcessRequest {
    let mut request = ProcessRequest::new(invocation.binary.clone());
    request.args = args;
    request.current_dir = Some(invocation.repository.clone());
    request.env.clone_from(&invocation.env);
    // The parent harness's session leaves first; the route's own keys are
    // not on that list, so the route's values win over anything inherited.
    request.env_remove = parent_session_isolation(&route_keys(invocation)).removals();
    if let Some(root) = &invocation.clean_state_root {
        let root = root.as_os_str().to_os_string();
        match invocation.surface {
            Surface::Claude => {
                request
                    .env
                    .push((OsString::from("CLAUDE_CONFIG_DIR"), root));
            }
            Surface::Codex => {
                request.env.push((OsString::from("CODEX_HOME"), root));
            }
            Surface::Agy => {
                request
                    .env
                    .push((OsString::from("ANTIGRAVITY_EXECUTABLE_DATA_DIR"), root));
            }
            Surface::Grok => {
                // `~/.local/bin/grok` is the harness-run shim, and its last
                // fallback looks for `${HOME}/.grok/bin/grok`. With HOME moved
                // to the clean root that search is empty, so the real install
                // directory is named explicitly through the variable the shim
                // reads first. The shim, and its sandbox, stay in the loop.
                if std::env::var_os("HARNESS_REAL_BIN_DIR").is_none()
                    && let Some(real_bin) =
                        dirs::home_dir().and_then(|home| grok_install_dir(&home))
                {
                    request
                        .env
                        .push((OsString::from("HARNESS_REAL_BIN_DIR"), real_bin));
                }
                request.env.push((OsString::from("HOME"), root));
            }
            Surface::Opencode => {
                let root = PathBuf::from(root);
                for (key, directory) in [
                    ("XDG_CONFIG_HOME", "config"),
                    ("XDG_DATA_HOME", "data"),
                    ("XDG_STATE_HOME", "state"),
                ] {
                    request
                        .env
                        .push((OsString::from(key), root.join(directory).into_os_string()));
                }
                for key in [
                    "OPENCODE_DISABLE_PROJECT_CONFIG",
                    "OPENCODE_DISABLE_EXTERNAL_SKILLS",
                    "OPENCODE_DISABLE_CLAUDE_CODE_PROMPT",
                    "OPENCODE_DISABLE_CLAUDE_CODE_SKILLS",
                ] {
                    request.env.push((OsString::from(key), OsString::from("1")));
                }
            }
        }
    }
    if invocation.surface == Surface::Opencode
        && let Some(permission) = opencode_permission(invocation)
    {
        request.env.push((
            OsString::from("OPENCODE_PERMISSION"),
            OsString::from(permission),
        ));
    }
    request.stdin = stdin;
    request.timeout = invocation.timeout;
    request
}

/// The placeholder the dry run prints for the prompt.
pub(crate) const PROMPT_PLACEHOLDER: &str = "<prompt>";
/// The placeholder for a system prompt the user supplied; the clean
/// preamble, a fixed sentence, prints as is.
const SYSTEM_PROMPT_PLACEHOLDER: &str = "<system-prompt>";
/// The placeholder for the scratch directory a surface writes beside its
/// run: codex's final message, grok's prompt file.
const SCRATCH_PLACEHOLDER: &str = "<scratch>";

/// The process the surface would start, for the dry run: the same argument
/// builders the spawn uses, so the printed argv cannot drift from the
/// started one, with the prompt, the system prompt and the scratch paths
/// as placeholders. `stdin` is set where the prompt travels that way.
pub(crate) fn planned_request(
    invocation: &SurfaceInvocation,
) -> Result<ProcessRequest, SurfaceFailure> {
    refuse_unsupported_mode(invocation)?;
    let planned = SurfaceInvocation {
        prompt: PROMPT_PLACEHOLDER.to_string(),
        system_prompt: if invocation.system_prompt.trim().is_empty() {
            String::new()
        } else {
            SYSTEM_PROMPT_PLACEHOLDER.to_string()
        },
        ..invocation.clone()
    };
    let scratch = Path::new(SCRATCH_PLACEHOLDER);
    let (args, stdin) = match planned.surface {
        Surface::Claude => (
            claude_args(&planned),
            Some(format!("{PROMPT_PLACEHOLDER}\n").into_bytes()),
        ),
        Surface::Codex => (
            codex_args(&planned, &scratch.join("last-message.txt")),
            Some(format!("{}\n", combined_prompt(&planned)).into_bytes()),
        ),
        Surface::Grok => (grok_args(&planned, &scratch.join("prompt.txt")), None),
        Surface::Agy => (agy_args(&planned, &scratch.join("agy.log")), None),
        Surface::Opencode => (opencode_args(&planned), None),
    };
    Ok(process_request(&planned, args, stdin))
}

/// The tail of one stream with the credentials hidden first (beside their
/// names, then the known values), so a value cut by the tail boundary
/// cannot leak in part.
fn redacted_tail(text: &str, secrets: &[String]) -> String {
    output_tail(&redact_text(text, secrets))
}

fn require_success(
    output: ProcessOutput,
    secrets: &[String],
) -> Result<ProcessOutput, SurfaceFailure> {
    if output.termination == ProcessTermination::Exited(0) {
        return Ok(output);
    }
    Err(SurfaceFailure::Exit {
        termination: output.termination,
        stdout_tail: redacted_tail(&output.stdout, secrets),
        stderr_tail: redacted_tail(&output.stderr, secrets),
        harness_errors: Vec::new(),
    })
}

fn nonempty_text(text: &str, surface: &str) -> Result<String, SurfaceFailure> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(SurfaceFailure::Reported(format!(
            "{surface} produced no final response"
        )));
    }
    Ok(text)
}

fn create_scratch_directory(kind: &str) -> Result<PathBuf, SurfaceFailure> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| {
            SurfaceFailure::Io(format!("cannot timestamp scratch directory: {error}"))
        })?;
    let scratch = std::env::temp_dir().join(format!(
        "rune-{kind}-{}-{}",
        std::process::id(),
        timestamp.as_nanos()
    ));
    std::fs::create_dir_all(&scratch).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot create scratch directory {}: {error}",
            scratch.display()
        ))
    })?;
    Ok(scratch)
}

/// Remove the scratch directory. A cleanup failure after a run failure
/// keeps that failure, its kind and its tails, with the cleanup error as a
/// note; after a success it is the failure.
fn finish_scratch<T>(
    result: Result<T, SurfaceFailure>,
    scratch: &PathBuf,
) -> Result<T, SurfaceFailure> {
    match std::fs::remove_dir_all(scratch) {
        Ok(()) => result,
        Err(cleanup_error) => {
            let note = format!(
                "cannot remove scratch directory {}: {cleanup_error}",
                scratch.display()
            );
            match result {
                Ok(_) => Err(SurfaceFailure::Io(note)),
                Err(failure) => Err(failure.with_note(note)),
            }
        }
    }
}

const READ_ONLY_TOOLS: &str = "Read,Glob,Grep";
const CLEAN_SYSTEM_PROMPT: &str = "Follow only the current user prompt. Do not use user rules, project rules, skills, memory, plugins, or prior sessions.";

fn clean_system_prompt(system_prompt: &str) -> String {
    if system_prompt.trim().is_empty() {
        CLEAN_SYSTEM_PROMPT.to_string()
    } else {
        format!("{CLEAN_SYSTEM_PROMPT}\n\n{}", system_prompt.trim())
    }
}

fn claude_args(invocation: &SurfaceInvocation) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--print"),
        OsString::from("--output-format"),
        OsString::from("text"),
        OsString::from("--permission-mode"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly | AccessMode::AnswerOnly => "plan",
            AccessMode::WorkspaceWrite => "acceptEdits",
        }),
    ];
    if invocation.clean_state_root.is_some() {
        args.extend([OsString::from("--setting-sources"), OsString::from("")]);
    }
    // The allowed tools are always named: a user setting that resets the
    // permission mode (the subprocess scrub does) cannot take them away.
    match invocation.mode {
        AccessMode::ReadOnly => args.extend([
            OsString::from("--tools"),
            OsString::from(READ_ONLY_TOOLS),
            OsString::from("--allowedTools"),
            OsString::from(READ_ONLY_TOOLS),
        ]),
        // `--tools ""` is claude's own switch for no built-in tool at all.
        AccessMode::AnswerOnly => args.extend([OsString::from("--tools"), OsString::from("")]),
        AccessMode::WorkspaceWrite => args.extend([
            OsString::from("--allowedTools"),
            OsString::from(WORKSPACE_WRITE_TOOLS),
        ]),
    }
    if let Some(model) = &invocation.model {
        args.push(OsString::from("--model"));
        args.push(OsString::from(model));
    }
    if let Some(name) = &invocation.session_name {
        args.push(OsString::from("--name"));
        args.push(OsString::from(name));
    }
    if invocation.clean_state_root.is_some() {
        args.push(OsString::from("--system-prompt"));
        args.push(OsString::from(clean_system_prompt(
            &invocation.system_prompt,
        )));
    } else if !invocation.system_prompt.trim().is_empty() {
        args.push(OsString::from("--append-system-prompt"));
        args.push(OsString::from(&invocation.system_prompt));
    }
    let (settings, extra_args) = claude_settings(invocation);
    args.extend(extra_args);
    if let Some(settings) = settings {
        args.push(OsString::from("--settings"));
        args.push(OsString::from(settings));
    }
    args
}

/// The tools a workspace-write claude run may use without a prompt: the
/// readers and the editors. Bash is not on the list on purpose: an allow
/// rule that matches a command also approves claude's unsandboxed retry of
/// it, so the shell is allowed through the sandbox settings alone.
const WORKSPACE_WRITE_TOOLS: &str = "Read,Glob,Grep,Edit,Write,MultiEdit,NotebookEdit";
/// The settings a workspace-write run adds: Claude Code's own sandbox
/// around the shell, a sandboxed Bash command allowed without a prompt,
/// no silent fallback to an unsandboxed shell when the sandbox cannot
/// start, and no unsandboxed retry of a command the sandbox blocked. With
/// the retry forbidden through `--settings` the sandbox is admin-required,
/// and claude ignores a repository's `excludedCommands`. The keys are the
/// ones Claude Code's settings file takes.
const WORKSPACE_WRITE_SETTINGS: &str = r#"{"sandbox":{"enabled":true,"failIfUnavailable":true,"autoAllowBashIfSandboxed":true,"allowUnsandboxedCommands":false}}"#;

/// The one `--settings` object a claude run sends, and the profile
/// arguments without theirs. Claude reads one `--settings` value, so the
/// profile's kept keys (filtered by [`claude_settings_key`]) and the run's
/// own keys merge into one object; the run's keys win.
fn claude_settings(invocation: &SurfaceInvocation) -> (Option<String>, Vec<OsString>) {
    let (merged, remaining) = merged_settings(&invocation.extra_args);
    let mut merged = merged.unwrap_or_default();
    if invocation.mode == AccessMode::WorkspaceWrite
        && let Ok(serde_json::Value::Object(own)) = serde_json::from_str(WORKSPACE_WRITE_SETTINGS)
    {
        merged.extend(own);
    }
    let settings = (!merged.is_empty()).then(|| serde_json::Value::Object(merged).to_string());
    (settings, remaining)
}

fn invoke_claude(
    invocation: &SurfaceInvocation,
    secrets: &[String],
) -> Result<SurfaceReply, SurfaceFailure> {
    // `claude --print` reports its own failures as `API Error:` lines on
    // stdout, beside the model's text; they are harness errors, not text,
    // and a forced stop keeps the ones reported before it.
    let output = run_process_request(&process_request(
        invocation,
        claude_args(invocation),
        Some(format!("{}\n", invocation.prompt).into_bytes()),
    ))
    .map_err(|failure| stopped_with_errors(failure, claude_error_messages))?;
    let reported_errors = claude_error_messages(&output.stdout);
    let output = require_success(output, secrets)
        .map_err(|failure| failure.with_harness_errors(reported_errors.clone()))?;
    Ok(SurfaceReply {
        text: nonempty_text(&output.stdout, "claude print").map_err(|failure| {
            failure
                .with_output(&output, secrets)
                .with_harness_errors(reported_errors)
        })?,
        stderr: output.stderr,
        completion_tokens: None,
        conversation_id: None,
    })
}

/// The `API Error:` lines `claude --print` writes on stdout when the request
/// itself fails: the harness's report, apart from any model text.
fn claude_error_messages(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("API Error:"))
        .map(str::to_string)
        .collect()
}

// Codex reports usage limits, auth failures, and turn errors as JSONL events on
// stdout, so a nonzero exit alone leaves the actionable reason invisible.
fn codex_error_messages(events: &[CodexEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event.kind.as_deref() {
            Some("error") => event.message.clone(),
            Some("turn.failed") => event.error.as_ref().and_then(|error| error.message.clone()),
            Some("item.completed") => event
                .item
                .as_ref()
                .filter(|item| item.kind.as_deref() == Some("error"))
                .and_then(|item| item.message.clone()),
            _ => None,
        })
        .filter(|message| !message.trim().is_empty())
        .collect()
}

fn read_codex_final_message(
    path: &PathBuf,
    events: &[CodexEvent],
) -> Result<String, SurfaceFailure> {
    let file_message = match std::fs::read_to_string(path) {
        Ok(content) if !content.trim().is_empty() => Some(content),
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(SurfaceFailure::Io(format!(
                "cannot read codex final message {}: {error}",
                path.display()
            )));
        }
    };
    file_message
        .or_else(|| {
            events
                .iter()
                .filter(|event| event.kind.as_deref() == Some("item.completed"))
                .filter_map(|event| event.item.as_ref())
                .filter(|item| item.kind.as_deref() == Some("agent_message"))
                .filter_map(|item| item.text.clone())
                .next_back()
        })
        .ok_or_else(|| {
            SurfaceFailure::Reported("codex exec produced no final agent message".to_string())
        })
}

/// Split the profile's kept codex arguments into the `-c`/`--config`
/// flags (with their values, in their order) and the rest.
fn codex_config_flags(extra_args: &[OsString]) -> (Vec<OsString>, Vec<OsString>) {
    let mut config = Vec::new();
    let mut rest = Vec::new();
    let mut index = 0;
    while index < extra_args.len() {
        let text = extra_args[index].to_string_lossy();
        if text == "-c" || text == "--config" {
            config.push(extra_args[index].clone());
            if let Some(value) = extra_args.get(index + 1) {
                config.push(value.clone());
                index += 1;
            }
        } else if text.starts_with("--config=") || (text.starts_with("-c") && text.len() > 2) {
            config.push(extra_args[index].clone());
        } else {
            rest.push(extra_args[index].clone());
        }
        index += 1;
    }
    (config, rest)
}

/// Codex drops a global `--config` (the form `codex-proxy` uses to name
/// its provider, before the subcommand) as soon as the subcommand carries
/// its own `--config`, so a profile's `model_reasoning_effort` after
/// `exec` sent the run to the built-in route. Every `--config` flag goes
/// before `exec`, in its order, where it joins the wrapper's.
fn codex_args(invocation: &SurfaceInvocation, final_message_path: &Path) -> Vec<OsString> {
    let (config, extra_args) = codex_config_flags(&invocation.extra_args);
    let mut args = config;
    args.extend([
        OsString::from("exec"),
        OsString::from("--json"),
        OsString::from("--ephemeral"),
        OsString::from("--skip-git-repo-check"),
        OsString::from("--color"),
        OsString::from("never"),
        OsString::from("-C"),
        invocation.repository.as_os_str().to_os_string(),
        OsString::from("-s"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly | AccessMode::AnswerOnly => "read-only",
            AccessMode::WorkspaceWrite => "workspace-write",
        }),
    ]);
    if invocation.clean_state_root.is_some() {
        args.push(OsString::from("--ignore-rules"));
    }
    if let Some(model) = &invocation.model {
        args.push(OsString::from("-m"));
        args.push(OsString::from(model));
    }
    args.extend(extra_args);
    args.push(OsString::from("-o"));
    args.push(final_message_path.as_os_str().to_os_string());
    args.push(OsString::from("-"));
    args
}

fn invoke_codex(
    invocation: &SurfaceInvocation,
    secrets: &[String],
) -> Result<SurfaceReply, SurfaceFailure> {
    refuse_unsupported_mode(invocation)?;
    let scratch = create_scratch_directory("codex")?;
    let result = (|| {
        let final_message_path = scratch.join("last-message.txt");
        let output = run_process_request(&process_request(
            invocation,
            codex_args(invocation, &final_message_path),
            Some(format!("{}\n", combined_prompt(invocation)).into_bytes()),
        ))
        .map_err(|failure| {
            stopped_with_errors(failure, |stdout| codex_error_messages(&parse_jsonl(stdout)))
        })?;
        let events: Vec<CodexEvent> = parse_jsonl(&output.stdout);
        let reported_errors = codex_error_messages(&events);
        // The harness's own messages stay apart from the raw stream tails.
        let with_output = |failure: SurfaceFailure| {
            failure
                .with_output(&output, secrets)
                .with_harness_errors(reported_errors.clone())
        };
        if output.termination != ProcessTermination::Exited(0) {
            return Err(require_success(output.clone(), secrets)
                .expect_err("a nonzero exit")
                .with_harness_errors(reported_errors));
        }
        if !reported_errors.is_empty() && !final_message_path.exists() {
            return Err(with_output(SurfaceFailure::Reported(
                "codex exec reported an error and no final agent message".to_string(),
            )));
        }
        let final_message =
            read_codex_final_message(&final_message_path, &events).map_err(with_output)?;
        let text = nonempty_text(&final_message, "codex exec").map_err(with_output)?;
        let completion_tokens = events
            .iter()
            .filter(|event| event.kind.as_deref() == Some("turn.completed"))
            .filter_map(|event| event.usage.as_ref())
            .filter_map(|usage| usage.output_tokens)
            .next_back()
            .map(|tokens| tokens.max(0.0));
        Ok(SurfaceReply {
            text,
            stderr: output.stderr,
            completion_tokens,
            conversation_id: None,
        })
    })();
    finish_scratch(result, &scratch)
}

fn grok_args(invocation: &SurfaceInvocation, prompt_path: &Path) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--cwd"),
        invocation.repository.as_os_str().to_os_string(),
        OsString::from("--prompt-file"),
        prompt_path.as_os_str().to_os_string(),
        OsString::from("--output-format"),
        OsString::from("json"),
        OsString::from("--no-memory"),
        OsString::from("--sandbox"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly | AccessMode::AnswerOnly => "read-only",
            AccessMode::WorkspaceWrite => "workspace",
        }),
        OsString::from("--permission-mode"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly | AccessMode::AnswerOnly => "dontAsk",
            AccessMode::WorkspaceWrite => "acceptEdits",
        }),
    ];
    // Grok's read-only sandbox profile still permits writes through allowed
    // tools, so the read-only contract comes from the tool set and deny rules.
    if invocation.mode == AccessMode::ReadOnly {
        args.extend([
            OsString::from("--tools"),
            OsString::from(READ_ONLY_TOOLS),
            OsString::from("--deny"),
            OsString::from("Write(**)"),
            OsString::from("--deny"),
            OsString::from("Edit(**)"),
            OsString::from("--deny"),
            OsString::from("Bash(**)"),
        ]);
    }
    if let Some(model) = &invocation.model {
        args.push(OsString::from("-m"));
        args.push(OsString::from(model));
    }
    if invocation.clean_state_root.is_some() {
        args.push(OsString::from(format!(
            "--system-prompt-override={}",
            clean_system_prompt(&invocation.system_prompt)
        )));
    } else if !invocation.system_prompt.trim().is_empty() {
        args.push(OsString::from(format!(
            "--system-prompt-override={}",
            invocation.system_prompt.trim()
        )));
    }
    args.extend(invocation.extra_args.clone());
    args
}

#[derive(Deserialize)]
struct GrokResponse {
    text: String,
}

fn grok_final_text(stdout: &str) -> Result<String, SurfaceFailure> {
    let response: GrokResponse = serde_json::from_str(stdout).map_err(|error| {
        SurfaceFailure::Reported(format!("grok returned invalid JSON output: {error}"))
    })?;
    nonempty_text(&response.text, "grok single-prompt")
}

fn invoke_grok(
    invocation: &SurfaceInvocation,
    secrets: &[String],
) -> Result<SurfaceReply, SurfaceFailure> {
    refuse_unsupported_mode(invocation)?;
    let scratch = create_scratch_directory("grok")?;
    let result = (|| {
        let prompt_path = scratch.join("prompt.txt");
        std::fs::write(&prompt_path, &invocation.prompt).map_err(|error| {
            SurfaceFailure::Io(format!(
                "cannot write grok prompt {}: {error}",
                prompt_path.display()
            ))
        })?;
        let output = require_success(
            run_process_request(&process_request(
                invocation,
                grok_args(invocation, &prompt_path),
                None,
            ))?,
            secrets,
        )?;
        Ok(SurfaceReply {
            text: grok_final_text(&output.stdout)
                .map_err(|failure| failure.with_output(&output, secrets))?,
            stderr: output.stderr,
            completion_tokens: None,
            conversation_id: None,
        })
    })();
    finish_scratch(result, &scratch)
}

/// What every agy run is told before the prompt. Headless agy denies a
/// shell command it cannot confirm, and the file tools need no
/// confirmation. Advice to a model, not a guard: the jj workspace is the
/// guard.
const AGY_READ_INSTRUCTION: &str = "Read files with your file tools (read, list, search), never with a shell command: this run is headless, and a shell command that needs a confirmation is denied.";

/// The agy argument list. `log_path` receives agy's own log, where a
/// headless soft-deny is written and nowhere else.
fn agy_args(invocation: &SurfaceInvocation, log_path: &Path) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--sandbox"),
        OsString::from("--mode"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly | AccessMode::AnswerOnly => "plan",
            AccessMode::WorkspaceWrite => "accept-edits",
        }),
        OsString::from("--log-file"),
        log_path.as_os_str().to_os_string(),
        OsString::from("--print"),
        OsString::from(format!(
            "{AGY_READ_INSTRUCTION}\n\n{}",
            combined_prompt(invocation)
        )),
        OsString::from("--output-format"),
        OsString::from("json"),
    ];
    if invocation.clean_state_root.is_some() {
        args.push(OsString::from("--disable-slash-commands"));
    }
    if let Some(model) = &invocation.model {
        args.push(OsString::from("--model"));
        args.push(OsString::from(model));
    }
    if let Some(timeout) = invocation.native_timeout {
        args.push(OsString::from("--print-timeout"));
        args.push(OsString::from(format!("{}ms", timeout.as_millis())));
    }
    args.extend(invocation.extra_args.clone());
    args
}

fn invoke_agy(
    invocation: &SurfaceInvocation,
    secrets: &[String],
) -> Result<SurfaceReply, SurfaceFailure> {
    refuse_unsupported_mode(invocation)?;
    let scratch = create_scratch_directory("agy")?;
    let result = (|| {
        let log_path = scratch.join("agy.log");
        let run = run_process_request(&process_request(
            invocation,
            agy_args(invocation, &log_path),
            None,
        ));
        // The log is read before anything is removed: a headless soft-deny
        // is agy's own report of why there is no answer, and it goes with
        // every failure as a harness error.
        let denials = agy_soft_denials(&std::fs::read_to_string(&log_path).unwrap_or_default());
        let output = run.map_err(|failure| {
            SurfaceFailure::from(failure).with_harness_errors(denials.clone())
        })?;
        let output = require_success(output, secrets)
            .map_err(|failure| failure.with_harness_errors(denials.clone()))?;
        let with_output = |failure: SurfaceFailure| {
            failure
                .with_output(&output, secrets)
                .with_harness_errors(denials.clone())
        };
        let response: AgyResponse = serde_json::from_str(&output.stdout).map_err(|error| {
            with_output(SurfaceFailure::Reported(format!(
                "agy returned invalid JSON output: {error}"
            )))
        })?;
        Ok(SurfaceReply {
            text: nonempty_text(&response.response, "agy print").map_err(with_output)?,
            stderr: output.stderr,
            completion_tokens: response
                .usage
                .and_then(|usage| usage.output_tokens)
                .filter(|tokens| *tokens > 0.0),
            conversation_id: response.conversation_id.filter(|id| !id.trim().is_empty()),
        })
    })();
    finish_scratch(result, &scratch)
}

#[derive(Deserialize)]
struct AgyResponse {
    response: String,
    #[serde(default)]
    usage: Option<AgyUsage>,
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Deserialize)]
struct AgyUsage {
    #[serde(default)]
    output_tokens: Option<f64>,
}

fn opencode_args(invocation: &SurfaceInvocation) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("run"),
        OsString::from("--dir"),
        invocation.repository.as_os_str().to_os_string(),
        OsString::from("--format"),
        OsString::from("json"),
    ];
    if let Some(model) = &invocation.model {
        args.push(OsString::from("-m"));
        args.push(OsString::from(model));
    }
    if invocation.mode == AccessMode::WorkspaceWrite {
        args.push(OsString::from("--auto"));
    }
    if let Some(name) = &invocation.session_name {
        args.push(OsString::from("--title"));
        args.push(OsString::from(name));
    }
    args.extend(invocation.extra_args.clone());
    args.push(OsString::from(combined_prompt(invocation)));
    args
}

/// The `OPENCODE_PERMISSION` value for the mode: every tool denied for an
/// answer-only run, the readers alone for a read-only one (plus `skill`
/// outside a clean run), and nothing for workspace-write.
fn opencode_permission(invocation: &SurfaceInvocation) -> Option<&'static str> {
    match invocation.mode {
        AccessMode::AnswerOnly => Some(r#"{"*":"deny"}"#),
        AccessMode::ReadOnly if invocation.clean_state_root.is_some() => Some(
            r#"{"*":"deny","read":"allow","glob":"allow","grep":"allow","list":"allow","lsp":"allow"}"#,
        ),
        AccessMode::ReadOnly => Some(
            r#"{"*":"deny","read":"allow","glob":"allow","grep":"allow","list":"allow","lsp":"allow","skill":"allow"}"#,
        ),
        AccessMode::WorkspaceWrite => None,
    }
}

fn invoke_opencode(
    invocation: &SurfaceInvocation,
    secrets: &[String],
) -> Result<SurfaceReply, SurfaceFailure> {
    let request = process_request(invocation, opencode_args(invocation), None);
    let output = require_success(run_process_request(&request)?, secrets)?;
    let events: Vec<OpencodeEvent> = parse_jsonl(&output.stdout);
    if let Some(message) = opencode_session_error(&events) {
        return Err(
            SurfaceFailure::Reported(format!("opencode session error: {message}"))
                .with_output(&output, secrets),
        );
    }
    let text =
        opencode_final_text(&events).map_err(|failure| failure.with_output(&output, secrets))?;
    let completion_tokens = events
        .iter()
        .filter(|event| event.kind.as_deref() == Some("step_finish"))
        .filter_map(|event| event.part.as_ref())
        .filter_map(|part| part.tokens.as_ref())
        .filter_map(|tokens| tokens.output)
        .map(|tokens| tokens.max(0.0))
        .sum::<f64>();
    Ok(SurfaceReply {
        text,
        stderr: output.stderr,
        completion_tokens: (completion_tokens > 0.0).then_some(completion_tokens),
        conversation_id: None,
    })
}

/// The flags the automated run sets itself for each surface. A profile
/// value for one of them is dropped with a warning, never passed through.
// A table per surface, read as data; its length is the flag count, not logic.
#[allow(clippy::too_many_lines)]
fn owned_options(surface: Surface) -> &'static [&'static str] {
    match surface {
        Surface::Claude => &[
            "-p",
            "--print",
            "--output-format",
            "--permission-mode",
            "--model",
            "--append-system-prompt",
            "--system-prompt",
            "--dangerously-skip-permissions",
            "--allow-dangerously-skip-permissions",
            "--allowedTools",
            "--allowed-tools",
            "--disallowedTools",
            "--disallowed-tools",
            "--tools",
            "--add-dir",
            "--settings",
            "--setting-sources",
            "-n",
            "--name",
        ],
        Surface::Codex => &[
            "-s",
            "--sandbox",
            "-C",
            "--cd",
            "-m",
            "--model",
            "--json",
            "--ignore-rules",
            "-o",
            "--output-last-message",
            "-c",
            "--config",
            "-p",
            "--profile",
            "--dangerously-bypass-approvals-and-sandbox",
            "--add-dir",
            "--oss",
            "--local-provider",
        ],
        Surface::Grok => &[
            "--cwd",
            "--prompt-file",
            "--output-format",
            "--sandbox",
            "--permission-mode",
            "-m",
            "--model",
            "--always-approve",
            "--worktree",
            "--system-prompt-override",
            "--system-prompt",
            "--allow",
            "--allowedTools",
            "--allowed-tools",
            "--deny",
            "--disallowedTools",
            "--disallowed-tools",
            "--tools",
            "--no-plan",
        ],
        Surface::Agy => &[
            "-p",
            "--print",
            "--prompt",
            "--print-timeout",
            "--output-format",
            "--sandbox",
            "--mode",
            "--model",
            "--dangerously-skip-permissions",
            "--disable-slash-commands",
            "--add-dir",
            "--log-file",
        ],
        Surface::Opencode => &[
            "--dir",
            "--format",
            "-m",
            "--model",
            "--auto",
            "--attach",
            "--command",
            "-c",
            "--continue",
            "-s",
            "--session",
            "--fork",
            "--share",
            "-i",
            "--interactive",
            "-f",
            "--file",
            "--port",
            "-p",
            "--password",
            "-u",
            "--username",
            "--title",
        ],
    }
}

/// Run the surface. The caller filters the profile arguments first with
/// [`filter_surface_args`]; this function passes `extra_args` through as
/// given.
pub(crate) fn invoke_surface(
    invocation: &SurfaceInvocation,
) -> Result<SurfaceReply, SurfaceFailure> {
    // Every failure and the harness's own stderr pass here last, so no
    // credential value the child could echo reaches the terminal or JSON.
    let secrets = child_secrets(invocation);
    let result = match invocation.surface {
        Surface::Claude => invoke_claude(invocation, &secrets),
        Surface::Codex => invoke_codex(invocation, &secrets),
        Surface::Agy => invoke_agy(invocation, &secrets),
        Surface::Grok => invoke_grok(invocation, &secrets),
        Surface::Opencode => invoke_opencode(invocation, &secrets),
    };
    // The answer passes too: a model that echoes a key is still a leak. It
    // loses only the known values, though: the structural and URL passes
    // that fit a failure tail turned `Primary key: user_id` into `Primary
    // key: <redacted>` in an ordinary coding answer.
    match result {
        Ok(reply) => Ok(SurfaceReply {
            text: redact_credentials(&reply.text, &secrets),
            stderr: redact_text(&reply.stderr, &secrets),
            ..reply
        }),
        Err(failure) => Err(failure.redacted(&secrets)),
    }
}

/// The filtered profile arguments for a surface, with the key-level hook
/// that surface understands.
pub(crate) fn filter_surface_args(invocation: &SurfaceInvocation) -> FilteredArgs {
    let owned = owned_options(invocation.surface);
    let booleans = boolean_options(invocation.surface);
    let model = invocation.model.as_deref();
    match invocation.surface {
        Surface::Claude => filter_profile_args(invocation, owned, booleans, &|option, value| {
            claude_settings_key(option, value, model)
        }),
        Surface::Codex => filter_profile_args(invocation, owned, booleans, &|option, value| {
            codex_config_key(option, value, model)
        }),
        _ => filter_profile_args(invocation, owned, booleans, &drop_whole),
    }
}

#[derive(Deserialize)]
struct CodexEvent {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    item: Option<CodexItem>,
    #[serde(default)]
    usage: Option<CodexUsage>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<CodexError>,
}

#[derive(Deserialize)]
struct CodexError {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct CodexItem {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct CodexUsage {
    #[serde(default)]
    output_tokens: Option<f64>,
}

fn parse_jsonl<T: serde::de::DeserializeOwned>(stdout: &str) -> Vec<T> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('{') {
                serde_json::from_str(trimmed).ok()
            } else {
                None
            }
        })
        .collect()
}

#[derive(Deserialize)]
struct OpencodeEvent {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    part: Option<OpencodePart>,
    #[serde(default)]
    properties: Option<OpencodeProperties>,
}

#[derive(Deserialize)]
struct OpencodeProperties {
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OpencodePart {
    #[serde(default, rename = "messageID")]
    message_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    tokens: Option<OpencodeTokens>,
}

fn opencode_final_text(events: &[OpencodeEvent]) -> Result<String, SurfaceFailure> {
    let text_parts = events
        .iter()
        .filter(|event| event.kind.as_deref() == Some("text"))
        .filter_map(|event| event.part.as_ref())
        .filter(|part| part.text.is_some())
        .collect::<Vec<_>>();
    if text_parts.iter().any(|part| part.message_id.is_none()) {
        return Err(SurfaceFailure::Reported(
            "opencode text event has no messageID".to_string(),
        ));
    }
    let final_message_id = text_parts
        .last()
        .and_then(|part| part.message_id.as_deref())
        .ok_or_else(|| {
            SurfaceFailure::Reported("opencode run produced no assistant text message".to_string())
        })?;
    let text = text_parts
        .iter()
        .filter(|part| part.message_id.as_deref() == Some(final_message_id))
        .filter_map(|part| part.text.as_deref())
        .collect::<Vec<_>>()
        .join("");
    nonempty_text(&text, "opencode run")
}

#[derive(Deserialize)]
struct OpencodeTokens {
    #[serde(default)]
    output: Option<f64>,
}

fn opencode_session_error(events: &[OpencodeEvent]) -> Option<&str> {
    events
        .iter()
        .find(|event| event.kind.as_deref() == Some("session.error"))
        .and_then(|event| event.properties.as_ref())
        .and_then(|properties| properties.error.as_ref())
        .and_then(|error| {
            error
                .pointer("/data/message")
                .or_else(|| error.pointer("/message"))
                .and_then(serde_json::Value::as_str)
        })
}

#[cfg(test)]
mod tests;
