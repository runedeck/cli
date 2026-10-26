//! The Cursor adapter: `cursor-agent -p --output-format json`, the prompt
//! on standard input behind the system prompt, and one JSON result object
//! on standard output. Cursor's terms allow the service only through this
//! CLI, so nothing here talks to Cursor's backend itself.
use super::{
    AccessMode, FilteredArgs, SurfaceFailure, SurfaceInvocation, SurfaceReply, TokenUsage,
    clean_system_prompt, combined_prompt, lenient_count, nonempty_text, process_request,
    require_success, shown,
};
use crate::cli::launch::env_value;
use crate::cli::process::run_process_request;
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The key a clean run authenticates with: the browser login lives in the
/// macOS Keychain, which a clean home cannot reach.
pub(crate) const API_KEY_ENV: &str = "CURSOR_API_KEY";
/// Where `cursor-agent` keeps `cli-config.json`, in the order the tool
/// resolves it: this variable, then `$XDG_CONFIG_HOME/cursor`, then
/// `$HOME/.cursor`.
const CONFIG_DIR_ENV: &str = "CURSOR_CONFIG_DIR";
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";
const CONFIG_FILE: &str = "cli-config.json";

const CLEAN_KEY_ERROR: &str = "a clean cursor run needs CURSOR_API_KEY: the cursor-agent browser login lives in the macOS Keychain, which a clean home cannot reach, and CURSOR_CONFIG_DIR alone still loads ~/.cursor/rules; set CURSOR_API_KEY or run without --clean";

/// The signature of a process that cannot read the Keychain: macOS reports
/// the blocked trust-settings read, and `cursor-agent` then finds no login.
pub(super) const TRUST_SIGNATURE: &str = "failed to copy trust settings";
pub(super) const LOGIN_SIGNATURE: &str = "Authentication required";
pub(super) const KEYCHAIN_HINT: &str = "an outer sandbox blocked the macOS Keychain that holds the harness login; run rune outside the sandbox";

/// The `cursor-agent` flags that stand alone.
pub(super) const BOOLEAN_OPTIONS: &[&str] = &[
    "-p",
    "--print",
    "--stream-partial-output",
    "--plan",
    "-f",
    "--force",
    "--yolo",
    "--auto-review",
    "--trust",
    "--continue",
    "--approve-mcps",
];

/// The flags the run sets itself or that change access, dropped from a
/// profile with a value-aware warning. `--plugin-dir` is here because a
/// plugin can add tools.
pub(super) const OWNED_OPTIONS: &[&str] = &[
    "-p",
    "--print",
    "--output-format",
    "--stream-partial-output",
    "--model",
    "--mode",
    "--plan",
    "-f",
    "--force",
    "--yolo",
    "--auto-review",
    "--sandbox",
    "--workspace",
    "--trust",
    "-w",
    "--worktree",
    "--worktree-base",
    "--resume",
    "--continue",
    "--add-dir",
    "--plugin-dir",
    "--approve-mcps",
];

/// The only profile flags that reach the tool. `cursor-agent` accepts
/// options its help does not list (`--allowed-tools`, `--data-dir`), reads
/// a positional as a subcommand or the prompt, and `--api-key` would put
/// the key in `ps`: the key belongs in the profile `env`.
const KEPT_OPTIONS: &[&str] = &["-e", "--endpoint"];

/// The profile arguments after the owned filter, cut down to the kept
/// flags, each with a plain value. A warning names a flag or a position,
/// never a value.
pub(super) fn keep_allowed(filtered: FilteredArgs) -> FilteredArgs {
    let FilteredArgs { kept, mut warnings } = filtered;
    let mut allowed = Vec::new();
    let mut index = 0;
    while index < kept.len() {
        let position = index + 1;
        let text = kept[index].to_string_lossy().into_owned();
        if text == "--" {
            warnings.push(format!(
                "automated cursor execution drops the profile arguments from position {position} on; cursor-agent reads them as a prompt"
            ));
            break;
        }
        if !text.starts_with('-') {
            warnings.push(format!(
                "automated cursor execution drops the positional profile argument at position {position}; cursor-agent reads it as a subcommand or a prompt"
            ));
            index += 1;
            continue;
        }
        let (name, inline) = split_option(&text);
        if !KEPT_OPTIONS.contains(&name) {
            warnings.push(format!(
                "automated cursor execution keeps only --endpoint from a profile; the profile flag {} is dropped",
                shown(name)
            ));
            index += 1;
            continue;
        }
        // The value: attached to the flag, else the next token. A token
        // that is not a plain value drops the flag, because commander takes
        // the next token as the value whatever it is.
        let next = kept
            .get(index + 1)
            .map(|value| value.to_string_lossy().into_owned());
        let (value, consumed) = match inline {
            Some(value) => (Some(value), 1),
            None => (next.filter(|value| is_plain_value(value)), 2),
        };
        if let Some(value) = value.filter(|value| is_plain_value(value)) {
            allowed.push(OsString::from(name));
            allowed.push(OsString::from(value));
            index += consumed;
        } else {
            warnings.push(format!(
                "automated cursor execution drops the profile flag {} because it has no plain value",
                shown(name)
            ));
            index += 1;
        }
    }
    FilteredArgs {
        kept: allowed,
        warnings,
    }
}

fn is_plain_value(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('-')
}

/// One profile token that starts with `-`, split into the option name and
/// the value attached to it: `--long=VALUE` names `--long`, and a short
/// token names its first two characters, so `-xVALUE` and `-x=VALUE` both
/// name `-x`, as commander reads them. The name is all a warning may show,
/// because the rest of the token is a value.
fn split_option(text: &str) -> (&str, Option<String>) {
    if text.starts_with("--") {
        return match text.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (text, None),
        };
    }
    let name_end = text
        .char_indices()
        .nth(2)
        .map_or(text.len(), |(offset, _)| offset);
    let (name, rest) = text.split_at(name_end);
    let value = rest.strip_prefix('=').unwrap_or(rest);
    (name, (!value.is_empty()).then(|| value.to_string()))
}

/// The argument list: print mode with JSON output in the repository, trust
/// granted so the tool does not stop on a new workspace, then the access
/// mode, the model, and the kept profile arguments.
pub(super) fn args(invocation: &SurfaceInvocation) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-p"),
        OsString::from("--output-format"),
        OsString::from("json"),
        OsString::from("--workspace"),
        invocation.repository.as_os_str().to_os_string(),
        OsString::from("--trust"),
    ];
    match invocation.mode {
        // Answer-only is refused before this runs: Cursor has no switch
        // that takes its tools away.
        AccessMode::ReadOnly | AccessMode::AnswerOnly => {
            args.extend([OsString::from("--mode"), OsString::from("ask")]);
        }
        AccessMode::WorkspaceWrite => args.extend([
            OsString::from("--force"),
            OsString::from("--sandbox"),
            OsString::from("enabled"),
        ]),
    }
    if let Some(model) = &invocation.model {
        args.extend([OsString::from("--model"), OsString::from(model)]);
    }
    args.extend(invocation.extra_args.iter().cloned());
    args
}

/// The standard input: the system prompt, then the prompt. Cursor has no
/// system prompt flag, so the clean preamble travels the same way.
pub(super) fn prompt_input(invocation: &SurfaceInvocation) -> String {
    let text = if invocation.clean_state_root.is_some() {
        format!(
            "{}\n\n{}",
            clean_system_prompt(&invocation.system_prompt),
            invocation.prompt
        )
    } else {
        combined_prompt(invocation)
    };
    format!("{text}\n")
}

pub(super) fn invoke(
    invocation: &SurfaceInvocation,
    secrets: &[String],
) -> Result<SurfaceReply, SurfaceFailure> {
    super::refuse_unsupported_mode(invocation)?;
    let output = run_process_request(&process_request(
        invocation,
        args(invocation),
        Some(prompt_input(invocation).into_bytes()),
    ))?;
    let output = require_success(output, secrets)?;
    parse(&output.stdout, output.stderr.clone())
        .map_err(|failure| failure.with_output(&output, secrets))
}

/// The one object `cursor-agent -p --output-format json` prints. Cursor
/// documents `type`, `subtype`, `is_error` and `result`; `usage` is read
/// when present and its absence changes nothing.
#[derive(Deserialize)]
struct CursorResponse {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    usage: Option<CursorUsage>,
}

/// The one count Cursor is known to print. A count that is not a
/// non-negative integer is unknown, and the result object stays.
#[derive(Deserialize)]
struct CursorUsage {
    #[serde(default, rename = "outputTokens", deserialize_with = "lenient_count")]
    output_tokens: Option<u64>,
}

const NO_RESULT: &str = "cursor-agent printed no JSON result object";

/// The top-level JSON records on stdout: the whole output as one value
/// when the tool pretty-prints its object, else one value per line. Only
/// a top-level record counts, so an object nested inside an error cannot
/// stand in for the result. A line that is not JSON fails the whole
/// output, because a truncated error would otherwise leave a stale
/// success in place.
fn top_level_records(stdout: &str) -> Result<Vec<serde_json::Value>, SurfaceFailure> {
    let text = stdout.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(whole) = serde_json::from_str::<serde_json::Value>(text) {
        return Ok(vec![whole]);
    }
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).map_err(|error| {
                SurfaceFailure::Reported(format!(
                    "cursor-agent printed output that is not JSON: {error}"
                ))
            })
        })
        .collect()
}

/// The terminal record: the last top-level record, which must be the
/// result object. A record before it is a streamed event and never the
/// answer, whatever its `type` says.
fn result_object(stdout: &str) -> Result<CursorResponse, SurfaceFailure> {
    let records = top_level_records(stdout)?;
    let no_result = || SurfaceFailure::Reported(NO_RESULT.to_string());
    let last = records.into_iter().last().ok_or_else(no_result)?;
    let response: CursorResponse = serde_json::from_value(last).map_err(|_| no_result())?;
    if response.kind.as_deref() != Some("result") {
        return Err(no_result());
    }
    Ok(response)
}

pub(super) fn parse(stdout: &str, stderr: String) -> Result<SurfaceReply, SurfaceFailure> {
    let response = result_object(stdout)?;
    if response.is_error {
        let message = response
            .result
            .as_deref()
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .or_else(|| {
                response
                    .subtype
                    .as_deref()
                    .map(str::trim)
                    .filter(|subtype| !subtype.is_empty())
            })
            .unwrap_or("no message");
        return Err(SurfaceFailure::Reported(format!(
            "cursor-agent reported an error: {message}"
        )));
    }
    let text = nonempty_text(
        response.result.as_deref().unwrap_or_default(),
        "cursor-agent print",
    )?;
    // Cursor names its output count and nothing else: no input count, no
    // total, no model. A zero is no report.
    let usage = response
        .usage
        .and_then(|usage| usage.output_tokens)
        .filter(|tokens| *tokens > 0)
        .map(|output_tokens| TokenUsage {
            output_tokens: Some(output_tokens),
            ..TokenUsage::default()
        });
    Ok(SurfaceReply {
        text,
        stderr,
        completion_tokens: usage.as_ref().and_then(TokenUsage::completion_tokens),
        conversation_id: None,
        usage,
        ..SurfaceReply::default()
    })
}

/// The refusal of a read-only run under Cursor's saved `unrestricted`
/// approval. `cursor-agent` enforces ask mode itself, and the model keeps
/// its Shell and Write tools under it; `unrestricted` is the one approval
/// mode that would run such a call without asking if that enforcement
/// failed, so it is refused as defense in depth.
pub(super) fn check_access(invocation: &SurfaceInvocation) -> Result<(), SurfaceFailure> {
    if invocation.mode != AccessMode::ReadOnly {
        return Ok(());
    }
    match config_dir(invocation) {
        Some(config_dir) => check_approval_mode(&config_dir),
        None => Ok(()),
    }
}

/// The directory `cursor-agent` reads `cli-config.json` from, resolved as
/// the tool does, each variable read from the launch environment first
/// because the child gets that environment last. A relative directory is
/// resolved against the child's working directory, the repository, as the
/// tool resolves it from its own: a check against rune's directory would
/// read a different file than the child.
fn config_dir(invocation: &SurfaceInvocation) -> Option<PathBuf> {
    if let Some(root) = &invocation.clean_state_root {
        return Some(clean_config_dir(root));
    }
    let lookup = |key: &str| {
        env_value(&invocation.env, key)
            .or_else(|| std::env::var(key).ok())
            .filter(|value| !value.trim().is_empty())
    };
    let directory = if let Some(directory) = lookup(CONFIG_DIR_ENV) {
        PathBuf::from(directory)
    } else if let Some(xdg) = lookup(XDG_CONFIG_HOME_ENV) {
        PathBuf::from(xdg).join("cursor")
    } else {
        lookup("HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)?
            .join(".cursor")
    };
    Some(invocation.repository.join(directory))
}

/// A missing or unreadable file refuses nothing: the tool then runs with
/// its own default, which asks.
fn check_approval_mode(config_dir: &Path) -> Result<(), SurfaceFailure> {
    let path = config_dir.join(CONFIG_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    if config
        .get("approvalMode")
        .and_then(serde_json::Value::as_str)
        == Some("unrestricted")
    {
        return Err(SurfaceFailure::Arguments(format!(
            "a read-only cursor run refuses approvalMode unrestricted, set in {}: ask mode is enforced by cursor-agent, and unrestricted is the one approval mode that would run a write without asking if that enforcement failed; set approvalMode to allowlist or auto-review, or run with --mode workspace-write",
            path.display()
        )));
    }
    Ok(())
}

/// Whether the child gets a nonempty `CURSOR_API_KEY`: the launch
/// environment's value wins, then rune's own.
pub(super) fn has_api_key(env: &[(OsString, OsString)]) -> bool {
    env_value(env, API_KEY_ENV)
        .or_else(|| std::env::var(API_KEY_ENV).ok())
        .is_some_and(|value| !value.trim().is_empty())
}

/// The clean config directory inside the clean root, which the process
/// request names in `CURSOR_CONFIG_DIR`.
pub(super) fn clean_config_dir(root: &Path) -> PathBuf {
    root.join(".cursor")
}

/// A clean run needs the API key: without it `cursor-agent` under a clean
/// home finds no login and fails after the run started. The refusal is a
/// configuration error before anything starts.
pub(super) fn prepare_clean_state(root: &Path, has_key: bool) -> Result<(), SurfaceFailure> {
    if !has_key {
        return Err(SurfaceFailure::Arguments(CLEAN_KEY_ERROR.to_string()));
    }
    let config_dir = clean_config_dir(root);
    std::fs::create_dir_all(&config_dir).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot create Cursor clean state {}: {error}",
            config_dir.display()
        ))
    })
}

/// The Keychain hint: the trust-settings signature always counts, and the
/// missing-login signature counts only while `sandboxed`, because outside
/// a sandbox a missing login is real and gets no hint.
pub(super) fn keychain_hint(stderr: &[&str], sandboxed: bool) -> Option<String> {
    let blocked = stderr.iter().any(|text| {
        text.contains(TRUST_SIGNATURE) || (sandboxed && text.contains(LOGIN_SIGNATURE))
    });
    blocked.then(|| KEYCHAIN_HINT.to_string())
}
