use crate::cli::launch;
use crate::cli::providers::{
    CliFailure, CliInvocation, CliProvider, ProcessFailure, ProcessTermination, SandboxMode,
    invoke_cli, prepare_clean_state,
};
use clap::ValueEnum;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

const AGY_SUPERVISOR_MARGIN: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RunMode {
    ReadOnly,
    WorkspaceWrite,
}

impl RunMode {
    fn sandbox(self) -> SandboxMode {
        match self {
            Self::ReadOnly => SandboxMode::ReadOnly,
            Self::WorkspaceWrite => SandboxMode::WorkspaceWrite,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

pub(crate) struct RunOptions {
    pub(crate) tool: String,
    pub(crate) prompt: Option<String>,
    pub(crate) prompt_file: Option<PathBuf>,
    pub(crate) system_prompt_file: Option<PathBuf>,
    pub(crate) binary: Option<PathBuf>,
    pub(crate) model: Option<String>,
    pub(crate) clean_harness_state: bool,
    pub(crate) repository: PathBuf,
    pub(crate) mode: RunMode,
    pub(crate) timeout: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) json: bool,
}

pub(crate) fn execute(options: &RunOptions) -> Result<i32, String> {
    match execute_inner(options) {
        Ok(exit_code) => Ok(exit_code),
        Err(message) if options.json => {
            println!(
                "{}",
                json!({
                    "ok": false,
                    "kind": "configuration_error",
                    "tool": options.tool,
                    "details": { "message": message },
                })
            );
            Ok(2)
        }
        Err(message) => Err(message),
    }
}

#[allow(clippy::too_many_lines)]
fn execute_inner(options: &RunOptions) -> Result<i32, String> {
    let prompt = read_prompt(options.prompt.as_deref(), options.prompt_file.as_deref())?;
    let system_prompt =
        read_optional_text_file(options.system_prompt_file.as_deref(), "system prompt")?;
    let repository = resolve_repository(&options.repository)?;
    let requested_timeout = options.timeout.as_deref().map(parse_duration).transpose()?;
    let resolve_args = if options.dry_run {
        vec![OsString::from("--dry-run")]
    } else {
        Vec::new()
    };
    let resolved = launch::resolve(&options.tool, &resolve_args)?;
    let provider = CliProvider::from_tool(&resolved.tool).ok_or_else(|| {
        format!(
            "automated execution is not supported for '{}'; use claude, codex, agy, grok, or opencode",
            resolved.tool
        )
    })?;
    let unsupported = resolved.unsupported_wrappers();
    if !unsupported.is_empty() {
        return Err(format!(
            "rune run cannot supervise {} wrappers; remove them or use rune launch",
            unsupported.join(", ")
        ));
    }
    let (resolved_binary, extra_args) = resolved
        .argv
        .split_first()
        .ok_or_else(|| "resolved launch command is empty".to_string())?;
    let binary = options.binary.as_ref().map_or_else(
        || resolved_binary.clone(),
        |path| path.as_os_str().to_os_string(),
    );
    let native_timeout = (provider == CliProvider::Agy)
        .then_some(requested_timeout)
        .flatten();
    let outer_timeout = supervisor_timeout(provider, requested_timeout)?;

    if options.dry_run || resolved.dry_run {
        println!(
            "{}\nrepository: {}\nmode: {}\ntimeout: {}\nnative_timeout: {}",
            resolved.format_dry_run(),
            repository.display(),
            options.mode.label(),
            duration_label(outer_timeout),
            duration_label(native_timeout)
        );
        return Ok(0);
    }

    for warning in resolved.warnings() {
        eprintln!("warning: {warning}");
    }
    resolved.run_pre_steps();

    let model = options
        .model
        .clone()
        .or_else(|| resolved.model.as_ref().map(|model| model.id.clone()));
    let clean_state = options
        .clean_harness_state
        .then(|| tempfile::Builder::new().prefix("rune-clean-").tempdir())
        .transpose()
        .map_err(|error| format!("cannot create clean harness state: {error}"))?;
    if let Some(state) = &clean_state {
        prepare_clean_state(provider, state.path(), model.as_deref())
            .map_err(|error| error.to_string())?;
    }
    let invocation = CliInvocation {
        provider,
        binary: binary.clone(),
        extra_args: extra_args.to_vec(),
        env: resolved.env.clone(),
        repository,
        mode: options.mode.sandbox(),
        system_prompt,
        prompt,
        model,
        native_timeout,
        timeout: outer_timeout,
        clean_state_root: clean_state.as_ref().map(|state| state.path().to_path_buf()),
    };

    let started = std::time::Instant::now();
    match invoke_cli(&invocation) {
        Ok(reply) => {
            if !reply.stderr.is_empty() {
                eprint!("{}", reply.stderr);
                if !reply.stderr.ends_with('\n') {
                    eprintln!();
                }
            }
            if options.json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "kind": "success",
                        "tool": resolved.tool,
                        "requested_route": options.tool,
                        "requested_provider": resolved.tool,
                        "resolved_route": options.tool,
                        "resolved_model": invocation.model,
                        "resolved_binary": invocation.binary.to_string_lossy(),
                        "model": invocation.model,
                        "text": reply.text,
                        "duration_ms": started.elapsed().as_millis(),
                        "clean_harness_state": options.clean_harness_state,
                        "clean_harness_state_scope": "supported_user_and_project_state",
                        "usage": {
                            "input_tokens": Value::Null,
                            "cache_creation_input_tokens": Value::Null,
                            "cache_read_input_tokens": Value::Null,
                            "output_tokens": reply.completion_tokens,
                            "total_tokens": Value::Null,
                        },
                        "completion_tokens": reply.completion_tokens,
                    })
                );
            } else {
                println!("{}", reply.text);
            }
            Ok(0)
        }
        Err(failure) => {
            if options.json {
                println!("{}", failure_json(&resolved.tool, &failure));
            } else {
                eprintln!("error: {failure}");
            }
            Ok(failure_exit_code(&failure))
        }
    }
}

fn read_prompt(argument: Option<&str>, file: Option<&Path>) -> Result<String, String> {
    let prompt = match (argument, file) {
        (Some(_), Some(_)) => {
            return Err("pass a prompt argument or --prompt-file, not both".to_string());
        }
        (Some(prompt), None) => prompt.to_string(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read prompt file {}: {error}", path.display()))?,
        (None, None) => {
            let mut stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Err(
                    "provide a prompt argument, --prompt-file, or pipe a prompt on stdin"
                        .to_string(),
                );
            }
            let mut prompt = String::new();
            stdin
                .read_to_string(&mut prompt)
                .map_err(|error| format!("cannot read prompt from stdin: {error}"))?;
            prompt
        }
    };
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".to_string());
    }
    Ok(prompt)
}

fn read_optional_text_file(path: Option<&Path>, label: &str) -> Result<String, String> {
    let Some(path) = path else {
        return Ok(String::new());
    };
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {label} file {}: {error}", path.display()))?;
    if text.trim().is_empty() {
        return Err(format!(
            "{label} file must not be empty: {}",
            path.display()
        ));
    }
    Ok(text)
}

fn resolve_repository(path: &Path) -> Result<PathBuf, String> {
    let repository = std::fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve repository {}: {error}", path.display()))?;
    if !repository.is_dir() {
        return Err(format!(
            "repository is not a directory: {}",
            repository.display()
        ));
    }
    Ok(repository)
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1_u64)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600_000)
    } else {
        (value, 1_000)
    };
    let amount = number
        .parse::<u64>()
        .map_err(|_| format!("invalid timeout '{value}'; use values such as 30s, 5m, or 1h"))?;
    let milliseconds = amount
        .checked_mul(multiplier)
        .ok_or_else(|| format!("timeout '{value}' is too large"))?;
    if milliseconds == 0 {
        return Err("timeout must be greater than zero".to_string());
    }
    Ok(Duration::from_millis(milliseconds))
}

fn supervisor_timeout(
    provider: CliProvider,
    requested: Option<Duration>,
) -> Result<Option<Duration>, String> {
    if provider != CliProvider::Agy {
        return Ok(requested);
    }
    requested
        .map(|timeout| {
            timeout
                .checked_add(AGY_SUPERVISOR_MARGIN)
                .ok_or_else(|| "agy supervisor timeout is too large".to_string())
        })
        .transpose()
}

fn duration_label(timeout: Option<Duration>) -> String {
    timeout.map_or_else(
        || "none".to_string(),
        |duration| format!("{}ms", duration.as_millis()),
    )
}

fn failure_exit_code(failure: &CliFailure) -> i32 {
    match failure {
        CliFailure::Process(ProcessFailure::Timeout(_)) => 124,
        CliFailure::Process(ProcessFailure::ForwardedSignal(signal))
        | CliFailure::Exit {
            termination: ProcessTermination::Signaled(signal),
            ..
        } => 128 + signal,
        CliFailure::Process(ProcessFailure::Spawn(_)) => 127,
        CliFailure::Process(ProcessFailure::OutputLimit { .. }) => 70,
        CliFailure::Exit {
            termination: ProcessTermination::Exited(code),
            ..
        } => *code,
        CliFailure::Process(_)
        | CliFailure::Provider(_)
        | CliFailure::Io(_)
        | CliFailure::Arguments(_) => 1,
    }
}

fn failure_json(tool: &str, failure: &CliFailure) -> Value {
    let (kind, details) = match failure {
        CliFailure::Process(ProcessFailure::Timeout(timeout)) => {
            ("timeout", json!({ "timeout_ms": timeout.as_millis() }))
        }
        CliFailure::Process(ProcessFailure::ForwardedSignal(signal)) => {
            ("signal", json!({ "signal": signal }))
        }
        CliFailure::Process(ProcessFailure::OutputLimit {
            stream,
            limit,
            tail,
        }) => (
            "output_limit",
            json!({ "stream": stream, "limit": limit, "tail": tail }),
        ),
        CliFailure::Process(failure) => {
            ("process_failure", json!({ "message": failure.to_string() }))
        }
        CliFailure::Exit {
            termination: ProcessTermination::Exited(code),
            stderr,
        } => ("exit", json!({ "exit_code": code, "stderr": stderr })),
        CliFailure::Exit {
            termination: ProcessTermination::Signaled(signal),
            stderr,
        } => ("signal", json!({ "signal": signal, "stderr": stderr })),
        CliFailure::Provider(message) => ("provider_error", json!({ "message": message })),
        CliFailure::Io(message) => ("io_error", json!({ "message": message })),
        CliFailure::Arguments(message) => ("argument_error", json!({ "message": message })),
    };
    json!({
        "ok": false,
        "kind": kind,
        "tool": tool,
        "details": details,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_absent_by_default() {
        assert_eq!(supervisor_timeout(CliProvider::Codex, None), Ok(None));
    }

    #[test]
    fn system_prompt_file_requires_content() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("system.md");
        std::fs::write(&path, "Use the rule.\n").expect("write system prompt");
        assert_eq!(
            read_optional_text_file(Some(&path), "system prompt"),
            Ok("Use the rule.\n".to_string())
        );
        std::fs::write(&path, " \n").expect("write empty system prompt");
        assert!(read_optional_text_file(Some(&path), "system prompt").is_err());
    }

    #[test]
    fn duration_parser_accepts_documented_units() {
        assert_eq!(parse_duration("250ms"), Ok(Duration::from_millis(250)));
        assert_eq!(parse_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_duration("5m"), Ok(Duration::from_mins(5)));
        assert_eq!(parse_duration("1h"), Ok(Duration::from_hours(1)));
    }

    #[test]
    fn agy_supervisor_deadline_follows_native_timeout() {
        let native = Duration::from_mins(5);
        assert_eq!(
            supervisor_timeout(CliProvider::Agy, Some(native)),
            Ok(Some(native + AGY_SUPERVISOR_MARGIN))
        );
    }

    #[test]
    fn missing_input_usage_keeps_total_unknown() {
        let usage = json!({
            "input_tokens": Value::Null,
            "output_tokens": 20,
            "total_tokens": Value::Null,
        });
        assert!(usage["total_tokens"].is_null());
    }

    #[test]
    fn typed_json_distinguishes_output_limit() {
        let failure = CliFailure::Process(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: 64,
            tail: "last bytes".to_string(),
        });
        assert_eq!(
            failure_json("codex", &failure),
            json!({
                "ok": false,
                "kind": "output_limit",
                "tool": "codex",
                "details": {
                    "stream": "stdout",
                    "limit": 64,
                    "tail": "last bytes",
                },
            })
        );
    }
}
