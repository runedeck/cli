//! Noninteractive adapters for the coding surfaces `rune run` drives:
//! Claude, Codex, Grok, `agy`, and `OpenCode`. Each adapter owns its argument
//! contract, prompt transport, and output parsing; process lifecycle comes
//! from [`crate::cli::process`]. Serde field names on the event types mirror
//! each tool's real stdout and must not change.
use crate::cli::process::{
    ProcessFailure, ProcessOutput, ProcessRequest, ProcessTermination, run_process_request,
};
use serde::Deserialize;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
}

impl AccessMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
        }
    }
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
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SurfaceReply {
    pub(crate) text: String,
    pub(crate) stderr: String,
    pub(crate) completion_tokens: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SurfaceFailure {
    Process(ProcessFailure),
    Exit {
        termination: ProcessTermination,
        stderr: String,
    },
    Reported(String),
    Io(String),
    Arguments(String),
}

impl fmt::Display for SurfaceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(failure) => failure.fmt(formatter),
            Self::Exit {
                termination,
                stderr,
            } => {
                let detail: String = stderr.trim().chars().take(500).collect();
                write!(formatter, "process {termination:?}")?;
                if !detail.is_empty() {
                    write!(formatter, ": {detail}")?;
                }
                Ok(())
            }
            Self::Reported(message) | Self::Io(message) | Self::Arguments(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl From<ProcessFailure> for SurfaceFailure {
    fn from(failure: ProcessFailure) -> Self {
        Self::Process(failure)
    }
}

fn is_short_option(option: &str) -> bool {
    option.len() == 2 && option.starts_with('-') && !option.starts_with("--")
}

fn extra_arg_matches(argument: &OsString, option: &str) -> bool {
    let argument = argument.to_string_lossy();
    argument == option
        || argument.starts_with(&format!("{option}="))
        || (is_short_option(option) && argument.starts_with(option))
}

fn reject_owned_args(
    invocation: &SurfaceInvocation,
    owned_options: &[&str],
) -> Result<(), SurfaceFailure> {
    let conflicts: Vec<String> = invocation
        .extra_args
        .iter()
        .filter(|argument| {
            owned_options
                .iter()
                .any(|option| extra_arg_matches(argument, option))
        })
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    if conflicts.is_empty() {
        return Ok(());
    }
    Err(SurfaceFailure::Arguments(format!(
        "automated {} execution owns these profile arguments: {}; remove them from the launch profile",
        match invocation.surface {
            Surface::Claude => "claude",
            Surface::Codex => "codex",
            Surface::Agy => "agy",
            Surface::Grok => "grok",
            Surface::Opencode => "opencode",
        },
        conflicts.join(", ")
    )))
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

fn copy_codex_route_config(source: &Path, target: &Path) -> Result<(), SurfaceFailure> {
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
    let provider_name = config
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            SurfaceFailure::Arguments("Codex clean state requires model_provider".to_string())
        })?;
    let provider = config
        .get("model_providers")
        .and_then(toml::Value::as_table)
        .and_then(|providers| providers.get(provider_name))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            SurfaceFailure::Arguments(format!(
                "Codex provider configuration not found: {provider_name}"
            ))
        })?;
    let mut clean_provider = toml::map::Map::new();
    for key in [
        "name",
        "base_url",
        "wire_api",
        "env_key",
        "http_headers",
        "env_http_headers",
        "query_params",
    ] {
        if let Some(value) = provider.get(key) {
            clean_provider.insert(key.to_string(), value.clone());
        }
    }
    let mut providers = toml::map::Map::new();
    providers.insert(
        provider_name.to_string(),
        toml::Value::Table(clean_provider),
    );
    let mut clean = toml::map::Map::new();
    clean.insert(
        "model_provider".to_string(),
        toml::Value::String(provider_name.to_string()),
    );
    clean.insert("model_providers".to_string(), toml::Value::Table(providers));
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
            let source = home.join(".grok/auth.json");
            let target_root = root.join(".grok");
            std::fs::create_dir_all(&target_root).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot create Grok clean state {}: {error}",
                    target_root.display()
                ))
            })?;
            std::fs::copy(&source, target_root.join("auth.json")).map_err(|error| {
                SurfaceFailure::Io(format!(
                    "cannot bridge Grok authentication {}: {error}",
                    source.display()
                ))
            })?;
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

fn process_request(
    invocation: &SurfaceInvocation,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
) -> ProcessRequest {
    let mut request = ProcessRequest::new(invocation.binary.clone());
    request.args = args;
    request.current_dir = Some(invocation.repository.clone());
    request.env.clone_from(&invocation.env);
    request.env_remove = vec![OsString::from("HARNESS_AUTOMATED")];
    if let Some(root) = &invocation.clean_state_root {
        let root = root.as_os_str().to_os_string();
        match invocation.surface {
            Surface::Claude => {
                request
                    .env
                    .push((OsString::from("CLAUDE_CONFIG_DIR"), root));
                request.env_remove.push(OsString::from("CLAUDECODE"));
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
    request.stdin = stdin;
    request.timeout = invocation.timeout;
    request
}

fn require_success(output: ProcessOutput) -> Result<ProcessOutput, SurfaceFailure> {
    if output.termination == ProcessTermination::Exited(0) {
        return Ok(output);
    }
    Err(SurfaceFailure::Exit {
        termination: output.termination,
        stderr: output.stderr,
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

fn finish_scratch<T>(
    result: Result<T, SurfaceFailure>,
    scratch: &PathBuf,
) -> Result<T, SurfaceFailure> {
    match std::fs::remove_dir_all(scratch) {
        Ok(()) => result,
        Err(cleanup_error) => {
            let message = match result {
                Ok(_) => format!(
                    "cannot remove scratch directory {}: {cleanup_error}",
                    scratch.display()
                ),
                Err(failure) => format!(
                    "{failure}; cannot remove scratch directory {}: {cleanup_error}",
                    scratch.display()
                ),
            };
            Err(SurfaceFailure::Io(message))
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
            AccessMode::ReadOnly => "plan",
            AccessMode::WorkspaceWrite => "acceptEdits",
        }),
    ];
    if invocation.clean_state_root.is_some() {
        args.extend([OsString::from("--setting-sources"), OsString::from("")]);
    }
    if invocation.mode == AccessMode::ReadOnly {
        args.extend([
            OsString::from("--tools"),
            OsString::from(READ_ONLY_TOOLS),
            OsString::from("--allowedTools"),
            OsString::from(READ_ONLY_TOOLS),
        ]);
    }
    if let Some(model) = &invocation.model {
        args.push(OsString::from("--model"));
        args.push(OsString::from(model));
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
    args.extend(invocation.extra_args.clone());
    args
}

fn invoke_claude(invocation: &SurfaceInvocation) -> Result<SurfaceReply, SurfaceFailure> {
    reject_owned_args(
        invocation,
        &[
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
        ],
    )?;
    let output = require_success(run_process_request(&process_request(
        invocation,
        claude_args(invocation),
        Some(format!("{}\n", invocation.prompt).into_bytes()),
    ))?)?;
    Ok(SurfaceReply {
        text: nonempty_text(&output.stdout, "claude print")?,
        stderr: output.stderr,
        completion_tokens: None,
    })
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

fn with_surface_diagnostics(messages: &[String], stderr: &str) -> String {
    let mut sections: Vec<&str> = messages.iter().map(String::as_str).collect();
    if !stderr.trim().is_empty() {
        sections.push(stderr.trim());
    }
    sections.join("\n")
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

fn invoke_codex(invocation: &SurfaceInvocation) -> Result<SurfaceReply, SurfaceFailure> {
    reject_owned_args(
        invocation,
        &[
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
    )?;
    let scratch = create_scratch_directory("codex")?;
    let result = (|| {
        let final_message_path = scratch.join("last-message.txt");
        let mut args = vec![
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
                AccessMode::ReadOnly => "read-only",
                AccessMode::WorkspaceWrite => "workspace-write",
            }),
        ];
        if invocation.clean_state_root.is_some() {
            args.push(OsString::from("--ignore-rules"));
        }
        if let Some(model) = &invocation.model {
            args.push(OsString::from("-m"));
            args.push(OsString::from(model));
        }
        args.extend(invocation.extra_args.clone());
        args.push(OsString::from("-o"));
        args.push(final_message_path.as_os_str().to_os_string());
        args.push(OsString::from("-"));
        let output = run_process_request(&process_request(
            invocation,
            args,
            Some(format!("{}\n", combined_prompt(invocation)).into_bytes()),
        ))?;
        let events: Vec<CodexEvent> = parse_jsonl(&output.stdout);
        let reported_errors = codex_error_messages(&events);
        if output.termination != ProcessTermination::Exited(0) {
            return Err(SurfaceFailure::Exit {
                termination: output.termination,
                stderr: with_surface_diagnostics(&reported_errors, &output.stderr),
            });
        }
        if !reported_errors.is_empty() && !final_message_path.exists() {
            return Err(SurfaceFailure::Reported(format!(
                "codex exec error: {}",
                reported_errors.join("; ")
            )));
        }
        let final_message = read_codex_final_message(&final_message_path, &events)?;
        let text = nonempty_text(&final_message, "codex exec")?;
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
            AccessMode::ReadOnly => "read-only",
            AccessMode::WorkspaceWrite => "workspace",
        }),
        OsString::from("--permission-mode"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly => "dontAsk",
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

fn invoke_grok(invocation: &SurfaceInvocation) -> Result<SurfaceReply, SurfaceFailure> {
    reject_owned_args(
        invocation,
        &[
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
    )?;
    let scratch = create_scratch_directory("grok")?;
    let result = (|| {
        let prompt_path = scratch.join("prompt.txt");
        std::fs::write(&prompt_path, &invocation.prompt).map_err(|error| {
            SurfaceFailure::Io(format!(
                "cannot write grok prompt {}: {error}",
                prompt_path.display()
            ))
        })?;
        let output = require_success(run_process_request(&process_request(
            invocation,
            grok_args(invocation, &prompt_path),
            None,
        ))?)?;
        Ok(SurfaceReply {
            text: grok_final_text(&output.stdout)?,
            stderr: output.stderr,
            completion_tokens: None,
        })
    })();
    finish_scratch(result, &scratch)
}

fn invoke_agy(invocation: &SurfaceInvocation) -> Result<SurfaceReply, SurfaceFailure> {
    reject_owned_args(
        invocation,
        &[
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
        ],
    )?;
    let mut args = vec![
        OsString::from("--sandbox"),
        OsString::from("--mode"),
        OsString::from(match invocation.mode {
            AccessMode::ReadOnly => "plan",
            AccessMode::WorkspaceWrite => "accept-edits",
        }),
        OsString::from("--print"),
        OsString::from(combined_prompt(invocation)),
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
    let output = require_success(run_process_request(&process_request(
        invocation, args, None,
    ))?)?;
    let response: AgyResponse = serde_json::from_str(&output.stdout).map_err(|error| {
        SurfaceFailure::Reported(format!("agy returned invalid JSON output: {error}"))
    })?;
    Ok(SurfaceReply {
        text: nonempty_text(&response.response, "agy print")?,
        stderr: output.stderr,
        completion_tokens: response
            .usage
            .and_then(|usage| usage.output_tokens)
            .filter(|tokens| *tokens > 0.0),
    })
}

#[derive(Deserialize)]
struct AgyResponse {
    response: String,
    #[serde(default)]
    usage: Option<AgyUsage>,
}

#[derive(Deserialize)]
struct AgyUsage {
    #[serde(default)]
    output_tokens: Option<f64>,
}

fn invoke_opencode(invocation: &SurfaceInvocation) -> Result<SurfaceReply, SurfaceFailure> {
    reject_owned_args(
        invocation,
        &[
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
        ],
    )?;
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
    args.extend(invocation.extra_args.clone());
    args.push(OsString::from(combined_prompt(invocation)));
    let mut request = process_request(invocation, args, None);
    if invocation.mode == AccessMode::ReadOnly {
        let permissions = if invocation.clean_state_root.is_some() {
            r#"{"*":"deny","read":"allow","glob":"allow","grep":"allow","list":"allow","lsp":"allow"}"#
        } else {
            r#"{"*":"deny","read":"allow","glob":"allow","grep":"allow","list":"allow","lsp":"allow","skill":"allow"}"#
        };
        request.env.push((
            OsString::from("OPENCODE_PERMISSION"),
            OsString::from(permissions),
        ));
    }
    let output = require_success(run_process_request(&request)?)?;
    let events: Vec<OpencodeEvent> = parse_jsonl(&output.stdout);
    if let Some(message) = opencode_session_error(&events) {
        return Err(SurfaceFailure::Reported(format!(
            "opencode session error: {message}"
        )));
    }
    let text = opencode_final_text(&events)?;
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
    })
}

pub(crate) fn invoke_surface(
    invocation: &SurfaceInvocation,
) -> Result<SurfaceReply, SurfaceFailure> {
    match invocation.surface {
        Surface::Claude => invoke_claude(invocation),
        Surface::Codex => invoke_codex(invocation),
        Surface::Agy => invoke_agy(invocation),
        Surface::Grok => invoke_grok(invocation),
        Surface::Opencode => invoke_opencode(invocation),
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
