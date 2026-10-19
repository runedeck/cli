//! The Cursor adapter: the official `cursor-agent` CLI in print mode, which
//! prints one JSON result object on stdout. Cursor has no system prompt flag,
//! so the system prompt travels in front of the prompt on stdin. Serde field
//! names mirror the tool's real stdout and must not change.
use super::{
    AccessMode, FilteredArgs, SurfaceFailure, SurfaceInvocation, SurfaceReply, clean_system_prompt,
    combined_prompt, nonempty_text, parse_jsonl, process_request, require_success, shown,
};
use crate::cli::process::run_process_request;
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The variable that carries a Cursor API key. A clean run needs it,
/// because the browser login lives in the macOS Keychain.
pub(super) const API_KEY_ENV: &str = "CURSOR_API_KEY";

const CLEAN_KEY_ERROR: &str = "a clean cursor run needs CURSOR_API_KEY: the cursor-agent browser login lives in the macOS Keychain, which a clean home cannot reach, and CURSOR_CONFIG_DIR alone still loads ~/.cursor/rules; set CURSOR_API_KEY or run without --clean";

const UNRESTRICTED_ERROR: &str = "a read-only cursor run refuses approvalMode unrestricted: ask mode is enforced by cursor-agent, and unrestricted is the one approval mode that would run a write without asking if that enforcement failed; set approvalMode to allowlist or auto-review in cli-config.json, or run with --mode workspace-write";

pub(super) const CONFIG_DIR_ENV: &str = "CURSOR_CONFIG_DIR";
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";

const SANDBOX_HINT: &str = "hint: cursor-agent cannot read its Keychain login inside the harness sandbox; run `rune run` as a bare command outside the sandbox";

/// The owned flags that take no value. `--resume` and `--worktree` take an
/// optional value, and the filter never takes a following flag as that value.
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

/// The flags the run sets itself, and the flags that widen what the agent
/// can reach: a plugin directory can add tools and servers.
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

/// The only flags a profile keeps, each with its value: they pick the
/// endpoint. `cursor-agent` accepts options that its help does not list,
/// such as `--data-dir` and `--allowed-tools`, so a table of owned flags
/// cannot be complete. `--api-key` is not kept: the key belongs in the
/// profile `env` as `CURSOR_API_KEY`, never in the argument list.
const KEPT_OPTIONS: &[&str] = &["-e", "--endpoint"];

/// Keep the flags in `KEPT_OPTIONS` and drop every other profile argument
/// that the owned table left. A positional goes too: `cursor-agent` reads
/// it as a subcommand, such as `install-shell-integration` or `worker`, or
/// as the prompt. Warnings name a flag, never a dropped value, because the
/// value of an unknown flag can be a secret.
pub(super) fn keep_allowed(filtered: FilteredArgs) -> FilteredArgs {
    let FilteredArgs {
        kept: args,
        mut warnings,
    } = filtered;
    let mut kept = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let text = args[index].to_string_lossy();
        let position = index + 1;
        index += 1;
        if text == "--" {
            warnings.push(format!(
                "automated cursor execution drops the profile arguments from position {position} on; cursor-agent reads them as a prompt"
            ));
            break;
        }
        if !text.starts_with('-') || text == "-" {
            warnings.push(format!(
                "automated cursor execution drops the positional profile argument at position {position}; cursor-agent reads it as a subcommand or a prompt"
            ));
            continue;
        }
        // A short flag is its first two characters, so a warning for
        // `-ksecret` shows `-k`; a long flag ends at `=`.
        let name: String = if text.starts_with("--") {
            text.split_once('=')
                .map_or(text.as_ref(), |(name, _)| name)
                .to_string()
        } else {
            text.chars().take(2).collect()
        };
        if !KEPT_OPTIONS.contains(&name.as_str()) {
            warnings.push(format!(
                "automated cursor execution keeps only --endpoint from a profile; the profile flag {} is dropped",
                shown(&name)
            ));
            continue;
        }
        // commander takes the next token as the value whatever it looks
        // like, so a kept flag without a plain value goes too. `-e=url`
        // would give commander the value `=url`.
        let inline = text.chars().count() > name.chars().count();
        let inline_short = inline && !name.starts_with("--");
        let next_is_value = !inline
            && args
                .get(index)
                .is_some_and(|value| !value.to_string_lossy().starts_with('-'));
        if inline_short || (!inline && !next_is_value) {
            warnings.push(format!(
                "automated cursor execution drops the profile flag {} because it has no plain value",
                shown(&name)
            ));
            continue;
        }
        kept.push(args[index - 1].clone());
        if next_is_value {
            kept.push(args[index].clone());
            index += 1;
        }
    }
    FilteredArgs { kept, warnings }
}

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
        // Ask mode is Cursor's read-only mode. `--sandbox enabled` cannot
        // back it: without `--force` the sandbox asks for an approval that
        // print mode cannot give.
        AccessMode::ReadOnly => args.extend([OsString::from("--mode"), OsString::from("ask")]),
        AccessMode::WorkspaceWrite => args.extend([
            OsString::from("--force"),
            OsString::from("--sandbox"),
            OsString::from("enabled"),
        ]),
    }
    if let Some(model) = &invocation.model {
        args.push(OsString::from("--model"));
        args.push(OsString::from(model));
    }
    args.extend(invocation.extra_args.clone());
    args
}

pub(super) fn prompt_input(invocation: &SurfaceInvocation) -> String {
    let prompt = if invocation.clean_state_root.is_some() {
        format!(
            "{}\n\n{}",
            clean_system_prompt(&invocation.system_prompt),
            invocation.prompt
        )
    } else {
        combined_prompt(invocation)
    };
    format!("{prompt}\n")
}

pub(super) fn invoke(invocation: &SurfaceInvocation) -> Result<SurfaceReply, SurfaceFailure> {
    let output = run_process_request(&process_request(
        invocation,
        args(invocation),
        Some(prompt_input(invocation).into_bytes()),
    ))?;
    let sandboxed = std::env::var_os("SANDBOX_RUNTIME").is_some();
    let output =
        require_success(output).map_err(|failure| with_sandbox_hint(failure, sandboxed))?;
    parse(&output.stdout, output.stderr)
}

#[derive(Deserialize)]
struct CursorResponse {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: String,
    #[serde(default)]
    usage: Option<CursorUsage>,
}

#[derive(Deserialize)]
struct CursorUsage {
    #[serde(default, rename = "outputTokens")]
    output_tokens: Option<f64>,
}

pub(super) fn parse(stdout: &str, stderr: String) -> Result<SurfaceReply, SurfaceFailure> {
    let response = parse_jsonl::<CursorResponse>(stdout)
        .into_iter()
        .rev()
        .find(|response| response.kind.as_deref() == Some("result"))
        .ok_or_else(|| {
            SurfaceFailure::Reported("cursor-agent printed no JSON result object".to_string())
        })?;
    if response.is_error {
        let message = Some(response.result.trim())
            .filter(|result| !result.is_empty())
            .or(response.subtype.as_deref())
            .unwrap_or("no message");
        return Err(SurfaceFailure::Reported(format!(
            "cursor-agent reported an error: {message}"
        )));
    }
    Ok(SurfaceReply {
        text: nonempty_text(&response.result, "cursor-agent print")?,
        stderr,
        completion_tokens: response
            .usage
            .and_then(|usage| usage.output_tokens)
            .filter(|tokens| *tokens > 0.0),
    })
}

/// A read-only run refuses Cursor's documented `unrestricted` approval
/// mode. Ask mode held in every live check, so this is defense in depth:
/// the model keeps its Shell and Write tools under ask mode, and
/// `unrestricted` is the one approval mode that would run such a call
/// without asking.
pub(super) fn check_access(invocation: &SurfaceInvocation) -> Result<(), SurfaceFailure> {
    if invocation.mode != AccessMode::ReadOnly {
        return Ok(());
    }
    config_dir(invocation).map_or(Ok(()), |dir| check_approval_mode(&dir))
}

/// The Cursor config directory the child reads, in the order `cursor-agent`
/// resolves it: the clean one, `CURSOR_CONFIG_DIR`, `$XDG_CONFIG_HOME/cursor`
/// when that variable is set and not blank, then `$HOME/.cursor`. Each
/// variable comes from the launch environment first, then rune's own.
fn config_dir(invocation: &SurfaceInvocation) -> Option<PathBuf> {
    if let Some(root) = &invocation.clean_state_root {
        return Some(root.join(".cursor"));
    }
    let env = |name: &str| {
        invocation
            .env
            .iter()
            .rev()
            .find(|(key, _)| *key == *name)
            .map(|(_, value)| value.clone())
            .or_else(|| std::env::var_os(name))
    };
    env(CONFIG_DIR_ENV)
        .map(PathBuf::from)
        .or_else(|| {
            env(XDG_CONFIG_HOME_ENV)
                .filter(|value| !value.to_string_lossy().trim().is_empty())
                .map(|value| PathBuf::from(value).join("cursor"))
        })
        .or_else(|| {
            env("HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .or_else(dirs::home_dir)
                .map(|home| home.join(".cursor"))
        })
}

/// A missing or unreadable `cli-config.json` passes: Cursor then uses its
/// default mode, and a broken file fails in Cursor itself.
pub(super) fn check_approval_mode(config_dir: &Path) -> Result<(), SurfaceFailure> {
    let Ok(text) = std::fs::read_to_string(config_dir.join("cli-config.json")) else {
        return Ok(());
    };
    let unrestricted = serde_json::from_str::<serde_json::Value>(&text)
        .is_ok_and(|config| config["approvalMode"] == "unrestricted");
    if unrestricted {
        return Err(SurfaceFailure::Arguments(UNRESTRICTED_ERROR.to_string()));
    }
    Ok(())
}

/// Whether the child gets a nonempty API key. The launch environment, which
/// holds profile `env` values, is applied last, so it wins over rune's own.
pub(super) fn has_api_key(env: &[(OsString, OsString)]) -> bool {
    env.iter()
        .rev()
        .find(|(key, _)| *key == *API_KEY_ENV)
        .map_or_else(
            || std::env::var_os(API_KEY_ENV).is_some_and(|key| !key.is_empty()),
            |(_, value)| !value.is_empty(),
        )
}

/// The clean state for a Cursor run: refuse without an API key, otherwise
/// create the config directory that `CURSOR_CONFIG_DIR` names.
pub(super) fn prepare_clean_state(root: &Path, has_key: bool) -> Result<(), SurfaceFailure> {
    if !has_key {
        return Err(SurfaceFailure::Arguments(CLEAN_KEY_ERROR.to_string()));
    }
    let config = root.join(".cursor");
    std::fs::create_dir_all(&config).map_err(|error| {
        SurfaceFailure::Io(format!(
            "cannot create Cursor clean state {}: {error}",
            config.display()
        ))
    })
}

/// One line for a failure whose stderr shows a sandboxed process that cannot
/// read the Keychain. "Authentication required" alone is also a real missing
/// login, so it counts only inside the sandbox.
pub(super) fn sandbox_hint(stderr: &str, sandboxed: bool) -> Option<&'static str> {
    let keychain = stderr.contains("failed to copy trust settings");
    let login = sandboxed && stderr.contains("Authentication required");
    (keychain || login).then_some(SANDBOX_HINT)
}

/// The hint goes first, because the failure message cuts stderr at 500
/// characters and the Keychain errors fill that before the login error.
pub(super) fn with_sandbox_hint(failure: SurfaceFailure, sandboxed: bool) -> SurfaceFailure {
    match failure {
        SurfaceFailure::Exit {
            termination,
            stderr,
        } => {
            let stderr = match sandbox_hint(&stderr, sandboxed) {
                Some(hint) => format!("{hint}\n{stderr}"),
                None => stderr,
            };
            SurfaceFailure::Exit {
                termination,
                stderr,
            }
        }
        other => other,
    }
}
