use super::bench::registry::{ModelConfig, Provider};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct ProviderReply {
    pub text: String,
    pub cost: Option<f64>,
    pub completion_tokens: Option<f64>,
}

pub enum Readiness {
    Ready,
    NotReady(String),
}

pub trait ModelRunner: Send + Sync {
    fn ready(&self) -> Readiness;
    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String>;
}

pub fn create_runner(config: &ModelConfig) -> Box<dyn ModelRunner> {
    match config.provider {
        Provider::Echo => Box::new(EchoRunner),
        Provider::Ollama => Box::new(HttpRunner::ollama(config)),
        Provider::OpenAiCompatible => Box::new(HttpRunner::openai_compat(config)),
        Provider::ClaudeCli => Box::new(ClaudeCliRunner {
            model: config.model.clone(),
        }),
        Provider::CodexCli => Box::new(CodexCliRunner {
            model: config.model.clone(),
            model_id: config.id.clone(),
        }),
        Provider::AgyCli => Box::new(AgyCliRunner {
            model: config.model.clone(),
            model_id: config.id.clone(),
        }),
        Provider::GrokCli => Box::new(GrokCliRunner {
            model: config.model.clone(),
            model_id: config.id.clone(),
        }),
        Provider::OpencodeCli => Box::new(OpencodeCliRunner {
            model: config.model.clone(),
            model_id: config.id.clone(),
        }),
    }
}

// Deterministic stub for tests and parity fixtures: replies with the prompt.
struct EchoRunner;

impl ModelRunner for EchoRunner {
    fn ready(&self) -> Readiness {
        Readiness::Ready
    }

    fn invoke(
        &self,
        _system_prompt: &str,
        prompt: &str,
        _temperature: Option<f64>,
        _timeout: Duration,
    ) -> Result<ProviderReply, String> {
        Ok(ProviderReply {
            text: prompt.to_string(),
            cost: None,
            completion_tokens: None,
        })
    }
}

const OLLAMA_DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

enum HttpFlavor {
    Ollama,
    OpenAiCompat,
}

struct HttpRunner {
    flavor: HttpFlavor,
    model_id: String,
    model: String,
    base_url: String,
    api_key_env: Option<String>,
}

impl HttpRunner {
    fn ollama(config: &ModelConfig) -> Self {
        Self {
            flavor: HttpFlavor::Ollama,
            model_id: config.id.clone(),
            model: config.model.clone(),
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| OLLAMA_DEFAULT_BASE_URL.to_string()),
            api_key_env: None,
        }
    }

    fn openai_compat(config: &ModelConfig) -> Self {
        Self {
            flavor: HttpFlavor::OpenAiCompat,
            model_id: config.id.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone().unwrap_or_default(),
            api_key_env: config.api_key_env.clone(),
        }
    }
}

fn url_origin(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let after_scheme = &url[scheme_end + 3..];
    match after_scheme.find('/') {
        Some(path_start) => url[..scheme_end + 3 + path_start].to_string(),
        None => url.to_string(),
    }
}

fn agent_with_timeout(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatResponseMessage>,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatUsage {
    #[serde(default)]
    completion_tokens: Option<f64>,
}

impl ModelRunner for HttpRunner {
    // Probe /api/version (ollama) so a downed server fails fast instead of
    // burning the full per-run timeout; openai-compatible checks key wiring.
    fn ready(&self) -> Readiness {
        match self.flavor {
            HttpFlavor::Ollama => {
                let version_url = format!("{}/api/version", url_origin(&self.base_url));
                let agent = agent_with_timeout(Duration::from_secs(3));
                match agent.get(&version_url).call() {
                    Ok(response) if response.status().is_success() => Readiness::Ready,
                    Ok(response) => Readiness::NotReady(format!(
                        "ollama responded HTTP {} at {version_url}",
                        response.status()
                    )),
                    Err(error) => {
                        Readiness::NotReady(format!("ollama unreachable at {version_url}: {error}"))
                    }
                }
            }
            HttpFlavor::OpenAiCompat => {
                if self.base_url.is_empty() {
                    return Readiness::NotReady(format!(
                        "model {}: base_url is required for openai-compatible",
                        self.model_id
                    ));
                }
                let Some(key_env) = &self.api_key_env else {
                    return Readiness::NotReady(format!(
                        "model {}: api_key_env is required for openai-compatible",
                        self.model_id
                    ));
                };
                match std::env::var(key_env) {
                    Ok(value) if !value.is_empty() => Readiness::Ready,
                    _ => Readiness::NotReady(format!("no key: set {key_env}")),
                }
            }
        }
    }

    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String> {
        let request = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user",
                    content: prompt,
                },
            ],
            temperature,
        };
        let body = serde_json::to_string(&request)
            .map_err(|error| format!("cannot encode chat request: {error}"))?;
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let agent = agent_with_timeout(timeout);
        let mut builder = agent.post(&url).header("content-type", "application/json");
        if let Some(key_env) = &self.api_key_env
            && let Ok(key) = std::env::var(key_env)
            && !key.is_empty()
        {
            builder = builder.header("authorization", &format!("Bearer {key}"));
        }
        let mut response = builder.send(body.as_bytes()).map_err(|error| {
            if error.to_string().to_lowercase().contains("timeout")
                || error.to_string().to_lowercase().contains("timed out")
            {
                format!("Test timeout after {}s", timeout.as_secs())
            } else {
                format!("POST {url} failed: {error}")
            }
        })?;
        if !response.status().is_success() {
            return Err(format!("POST {url} returned HTTP {}", response.status()));
        }
        let mut raw = String::new();
        response
            .body_mut()
            .as_reader()
            .read_to_string(&mut raw)
            .map_err(|error| format!("cannot read chat response: {error}"))?;
        let parsed: ChatResponse = serde_json::from_str(&raw)
            .map_err(|error| format!("invalid chat response: {error}"))?;
        let text = parsed
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .and_then(|message| message.content.clone())
            .ok_or_else(|| "chat response carried no message content".to_string())?;
        Ok(ProviderReply {
            text,
            cost: None,
            completion_tokens: parsed.usage.and_then(|usage| usage.completion_tokens),
        })
    }
}

fn binary_for(env_key: &str, default: &str) -> String {
    std::env::var(env_key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_TERMINATION_GRACE: Duration = Duration::from_secs(2);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessTermination {
    Exited(i32),
    Signaled(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessOutput {
    pub(crate) termination: ProcessTermination,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl ProcessOutput {
    fn code(&self) -> Option<i32> {
        match self.termination {
            ProcessTermination::Exited(code) => Some(code),
            ProcessTermination::Signaled(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessFailure {
    Spawn(String),
    Wait(String),
    Stdin(String),
    Stdout(String),
    Stderr(String),
    Timeout(Duration),
    ForwardedSignal(i32),
    OutputLimit {
        stream: &'static str,
        limit: usize,
        tail: String,
    },
}

impl fmt::Display for ProcessFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(message)
            | Self::Wait(message)
            | Self::Stdin(message)
            | Self::Stdout(message)
            | Self::Stderr(message) => formatter.write_str(message),
            Self::Timeout(timeout) => write!(
                formatter,
                "process timed out after {} seconds",
                timeout.as_secs_f64()
            ),
            Self::ForwardedSignal(signal) => {
                write!(
                    formatter,
                    "process stopped after forwarding signal {signal}"
                )
            }
            Self::OutputLimit {
                stream,
                limit,
                tail,
            } => {
                write!(
                    formatter,
                    "process {stream} exceeded the {limit}-byte output limit"
                )?;
                if !tail.trim().is_empty() {
                    let diagnostic: String = tail.trim().chars().take(500).collect();
                    write!(formatter, ": {diagnostic}")?;
                }
                Ok(())
            }
        }
    }
}

pub(crate) struct ProcessRequest {
    pub(crate) binary: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) env_remove: Vec<OsString>,
    pub(crate) stdin: Option<Vec<u8>>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) output_limit: usize,
    pub(crate) termination_grace: Duration,
}

impl ProcessRequest {
    pub(crate) fn new(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
            args: Vec::new(),
            current_dir: None,
            env: Vec::new(),
            env_remove: Vec::new(),
            stdin: None,
            timeout: None,
            output_limit: DEFAULT_OUTPUT_LIMIT_BYTES,
            termination_grace: DEFAULT_TERMINATION_GRACE,
        }
    }
}

struct CapturedStream {
    tail: VecDeque<u8>,
    exceeded_limit: bool,
}

impl CapturedStream {
    fn into_string(self) -> String {
        String::from_utf8_lossy(&self.tail.into_iter().collect::<Vec<_>>()).into_owned()
    }
}

struct SignalFlags {
    interrupt: Arc<AtomicBool>,
    terminate: Arc<AtomicBool>,
    registrations: Vec<signal_hook::SigId>,
}

impl SignalFlags {
    #[cfg(unix)]
    fn register() -> Result<Self, ProcessFailure> {
        use signal_hook::consts::signal::{SIGINT, SIGTERM};

        let interrupt = Arc::new(AtomicBool::new(false));
        let terminate = Arc::new(AtomicBool::new(false));
        let interrupt_registration = signal_hook::flag::register(SIGINT, Arc::clone(&interrupt))
            .map_err(|error| {
                ProcessFailure::Wait(format!("cannot register SIGINT handler: {error}"))
            })?;
        let terminate_registration =
            match signal_hook::flag::register(SIGTERM, Arc::clone(&terminate)) {
                Ok(registration) => registration,
                Err(error) => {
                    signal_hook::low_level::unregister(interrupt_registration);
                    return Err(ProcessFailure::Wait(format!(
                        "cannot register SIGTERM handler: {error}"
                    )));
                }
            };
        Ok(Self {
            interrupt,
            terminate,
            registrations: vec![interrupt_registration, terminate_registration],
        })
    }

    #[cfg(not(unix))]
    fn register() -> Result<Self, ProcessFailure> {
        Ok(Self {
            interrupt: Arc::new(AtomicBool::new(false)),
            terminate: Arc::new(AtomicBool::new(false)),
            registrations: Vec::new(),
        })
    }

    fn received(&self) -> Option<i32> {
        #[cfg(unix)]
        {
            use signal_hook::consts::signal::{SIGINT, SIGTERM};

            if self.interrupt.load(Ordering::Relaxed) {
                return Some(SIGINT);
            }
            if self.terminate.load(Ordering::Relaxed) {
                return Some(SIGTERM);
            }
        }
        None
    }
}

impl Drop for SignalFlags {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            signal_hook::low_level::unregister(registration);
        }
    }
}

fn spawn_command(request: &ProcessRequest) -> Command {
    let mut command = Command::new(&request.binary);
    command.args(&request.args);
    if let Some(current_dir) = &request.current_dir {
        command.current_dir(current_dir);
    }
    command.envs(request.env.iter().cloned());
    for name in &request.env_remove {
        command.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

fn reader_thread<R: Read + Send + 'static>(
    stream_name: &'static str,
    source: Option<R>,
    limit: usize,
    exceeded_limit: Arc<AtomicBool>,
) -> std::thread::JoinHandle<Result<CapturedStream, ProcessFailure>> {
    std::thread::spawn(move || {
        let mut source = source.ok_or_else(|| {
            let message = format!("cannot capture process {stream_name}: pipe is unavailable");
            if stream_name == "stdout" {
                ProcessFailure::Stdout(message)
            } else {
                ProcessFailure::Stderr(message)
            }
        })?;
        let mut tail = VecDeque::with_capacity(limit.min(64 * 1024));
        let mut total = 0usize;
        let mut chunk = [0u8; 8192];
        loop {
            let bytes_read = source.read(&mut chunk).map_err(|error| {
                let message = format!("cannot read process {stream_name}: {error}");
                if stream_name == "stdout" {
                    ProcessFailure::Stdout(message)
                } else {
                    ProcessFailure::Stderr(message)
                }
            })?;
            if bytes_read == 0 {
                break;
            }
            total = total.saturating_add(bytes_read);
            if total > limit {
                exceeded_limit.store(true, Ordering::Relaxed);
            }
            for byte in &chunk[..bytes_read] {
                if tail.len() == limit {
                    tail.pop_front();
                }
                if limit > 0 {
                    tail.push_back(*byte);
                }
            }
        }
        Ok(CapturedStream {
            tail,
            exceeded_limit: total > limit,
        })
    })
}

fn writer_thread(
    stdin: Option<std::process::ChildStdin>,
    content: Vec<u8>,
) -> std::thread::JoinHandle<Result<(), ProcessFailure>> {
    std::thread::spawn(move || {
        use std::io::Write;

        let mut stdin = stdin.ok_or_else(|| {
            ProcessFailure::Stdin("cannot write process stdin: pipe is unavailable".to_string())
        })?;
        match stdin.write_all(&content) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(error) => Err(ProcessFailure::Stdin(format!(
                "cannot write process stdin: {error}"
            ))),
        }
    })
}

#[cfg(unix)]
fn send_process_group_signal(child: &Child, signal: i32) -> Result<(), ProcessFailure> {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let signal = Signal::try_from(signal)
        .map_err(|error| ProcessFailure::Wait(format!("invalid process signal: {error}")))?;
    match killpg(Pid::from_raw(child.id().cast_signed()), signal) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(ProcessFailure::Wait(format!(
            "cannot signal process group {}: {error}",
            child.id()
        ))),
    }
}

#[cfg(not(unix))]
fn send_process_group_signal(child: &mut Child, _signal: i32) -> Result<(), ProcessFailure> {
    child
        .kill()
        .map_err(|error| ProcessFailure::Wait(format!("cannot stop process: {error}")))
}

fn reap_after_failure(
    child: &mut Child,
    failure: ProcessFailure,
) -> Result<ExitStatus, ProcessFailure> {
    let kill_error = child.kill().err();
    match child.wait() {
        Ok(_) => Err(failure),
        Err(wait_error) => {
            let kill_detail = kill_error.map_or_else(String::new, |error| {
                format!("; direct child kill also failed: {error}")
            });
            Err(ProcessFailure::Wait(format!(
                "{failure}{kill_detail}; cannot reap process: {wait_error}"
            )))
        }
    }
}

fn wait_after_signal(
    child: &mut Child,
    initial_signal: i32,
    grace: Duration,
) -> Result<ExitStatus, ProcessFailure> {
    #[cfg(unix)]
    if let Err(failure) = send_process_group_signal(child, initial_signal) {
        return reap_after_failure(child, failure);
    }
    #[cfg(not(unix))]
    if let Err(failure) = send_process_group_signal(child, initial_signal) {
        return reap_after_failure(child, failure);
    }

    let deadline = Instant::now() + grace;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(PROCESS_POLL_INTERVAL);
            }
            Ok(None) => break,
            Err(error) => {
                return reap_after_failure(
                    child,
                    ProcessFailure::Wait(format!("cannot wait for process after signal: {error}")),
                );
            }
        }
    }

    #[cfg(unix)]
    {
        use signal_hook::consts::signal::SIGKILL;
        if let Err(failure) = send_process_group_signal(child, SIGKILL) {
            return reap_after_failure(child, failure);
        }
    }
    #[cfg(not(unix))]
    child
        .kill()
        .map_err(|error| ProcessFailure::Wait(format!("cannot kill process: {error}")))?;
    child
        .wait()
        .map_err(|error| ProcessFailure::Wait(format!("cannot reap process: {error}")))
}

fn termination_from_status(status: ExitStatus) -> ProcessTermination {
    if let Some(code) = status.code() {
        return ProcessTermination::Exited(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ProcessTermination::Signaled(status.signal().unwrap_or_default())
    }
    #[cfg(not(unix))]
    ProcessTermination::Signaled(0)
}

fn join_writer(
    handle: Option<std::thread::JoinHandle<Result<(), ProcessFailure>>>,
) -> Result<(), ProcessFailure> {
    let Some(handle) = handle else {
        return Ok(());
    };
    handle
        .join()
        .map_err(|_| ProcessFailure::Stdin("process stdin writer panicked".to_string()))?
}

fn join_reader(
    stream_name: &'static str,
    handle: std::thread::JoinHandle<Result<CapturedStream, ProcessFailure>>,
) -> Result<CapturedStream, ProcessFailure> {
    handle.join().map_err(|_| {
        let message = format!("process {stream_name} reader panicked");
        if stream_name == "stdout" {
            ProcessFailure::Stdout(message)
        } else {
            ProcessFailure::Stderr(message)
        }
    })?
}

fn requested_stop(
    request: &ProcessRequest,
    signal_flags: &SignalFlags,
    stdout_exceeded: &AtomicBool,
    stderr_exceeded: &AtomicBool,
    started: Instant,
) -> Option<ProcessFailure> {
    if stdout_exceeded.load(Ordering::Relaxed) {
        return Some(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: request.output_limit,
            tail: String::new(),
        });
    }
    if stderr_exceeded.load(Ordering::Relaxed) {
        return Some(ProcessFailure::OutputLimit {
            stream: "stderr",
            limit: request.output_limit,
            tail: String::new(),
        });
    }
    if let Some(signal) = signal_flags.received() {
        return Some(ProcessFailure::ForwardedSignal(signal));
    }
    request
        .timeout
        .filter(|timeout| started.elapsed() >= *timeout)
        .map(ProcessFailure::Timeout)
}

fn stopping_signal(failure: &ProcessFailure) -> i32 {
    #[cfg(unix)]
    {
        use signal_hook::consts::signal::SIGTERM;

        match failure {
            ProcessFailure::ForwardedSignal(signal) => *signal,
            _ => SIGTERM,
        }
    }
    #[cfg(not(unix))]
    0
}

fn wait_for_process(
    child: &mut Child,
    request: &ProcessRequest,
    signal_flags: &SignalFlags,
    stdout_exceeded: &AtomicBool,
    stderr_exceeded: &AtomicBool,
    binary: &str,
) -> Result<(ExitStatus, Option<ProcessFailure>), ProcessFailure> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, None)),
            Ok(None) => {}
            Err(error) => {
                let failure = ProcessFailure::Wait(format!("{binary} wait failed: {error}"));
                let status =
                    wait_after_signal(child, stopping_signal(&failure), request.termination_grace)?;
                return Ok((status, Some(failure)));
            }
        }

        if let Some(failure) = requested_stop(
            request,
            signal_flags,
            stdout_exceeded,
            stderr_exceeded,
            started,
        ) {
            let status =
                wait_after_signal(child, stopping_signal(&failure), request.termination_grace)?;
            return Ok((status, Some(failure)));
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

pub(crate) fn run_process_request(
    request: &ProcessRequest,
) -> Result<ProcessOutput, ProcessFailure> {
    let binary = request.binary.to_string_lossy();
    let mut command = spawn_command(request);
    command
        .stdin(if request.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let signal_flags = SignalFlags::register()?;
    let mut child = command
        .spawn()
        .map_err(|error| ProcessFailure::Spawn(format!("{binary} not runnable: {error}")))?;
    let stdout_exceeded = Arc::new(AtomicBool::new(false));
    let stderr_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_handle = reader_thread(
        "stdout",
        child.stdout.take(),
        request.output_limit,
        Arc::clone(&stdout_exceeded),
    );
    let stderr_handle = reader_thread(
        "stderr",
        child.stderr.take(),
        request.output_limit,
        Arc::clone(&stderr_exceeded),
    );
    let stdin_handle = request
        .stdin
        .clone()
        .map(|content| writer_thread(child.stdin.take(), content));
    let wait_result = wait_for_process(
        &mut child,
        request,
        &signal_flags,
        &stdout_exceeded,
        &stderr_exceeded,
        &binary,
    );
    let stdin_result = join_writer(stdin_handle);
    let stdout_result = join_reader("stdout", stdout_handle);
    let stderr_result = join_reader("stderr", stderr_handle);

    let (status, stop_failure) = wait_result?;
    stdin_result?;
    let stdout_capture = stdout_result?;
    let stderr_capture = stderr_result?;
    let stdout_exceeded_limit = stdout_capture.exceeded_limit;
    let stderr_exceeded_limit = stderr_capture.exceeded_limit;
    let stdout = stdout_capture.into_string();
    let stderr = stderr_capture.into_string();
    if let Some(mut failure) = stop_failure {
        if let ProcessFailure::OutputLimit { stream, tail, .. } = &mut failure {
            *tail = if *stream == "stdout" {
                stdout.clone()
            } else {
                stderr.clone()
            };
        }
        return Err(failure);
    }
    if stdout_exceeded_limit {
        return Err(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: request.output_limit,
            tail: stdout,
        });
    }
    if stderr_exceeded_limit {
        return Err(ProcessFailure::OutputLimit {
            stream: "stderr",
            limit: request.output_limit,
            tail: stderr,
        });
    }
    Ok(ProcessOutput {
        termination: termination_from_status(status),
        stdout,
        stderr,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliProvider {
    Claude,
    Codex,
    Agy,
    Grok,
    Opencode,
}

impl CliProvider {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
}

#[derive(Debug, Clone)]
pub(crate) struct CliInvocation {
    pub(crate) provider: CliProvider,
    pub(crate) binary: OsString,
    pub(crate) extra_args: Vec<OsString>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) repository: PathBuf,
    pub(crate) mode: SandboxMode,
    pub(crate) system_prompt: String,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    pub(crate) native_timeout: Option<Duration>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) clean_state_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CliReply {
    pub(crate) text: String,
    pub(crate) stderr: String,
    pub(crate) completion_tokens: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliFailure {
    Process(ProcessFailure),
    Exit {
        termination: ProcessTermination,
        stderr: String,
    },
    Provider(String),
    Io(String),
    Arguments(String),
}

impl fmt::Display for CliFailure {
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
            Self::Provider(message) | Self::Io(message) | Self::Arguments(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl From<ProcessFailure> for CliFailure {
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

fn reject_owned_args(invocation: &CliInvocation, owned_options: &[&str]) -> Result<(), CliFailure> {
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
    Err(CliFailure::Arguments(format!(
        "automated {} execution owns these profile arguments: {}; remove them from the launch profile",
        match invocation.provider {
            CliProvider::Claude => "claude",
            CliProvider::Codex => "codex",
            CliProvider::Agy => "agy",
            CliProvider::Grok => "grok",
            CliProvider::Opencode => "opencode",
        },
        conflicts.join(", ")
    )))
}

fn combined_prompt(invocation: &CliInvocation) -> String {
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

fn copy_codex_route_config(source: &Path, target: &Path) -> Result<(), CliFailure> {
    let text = std::fs::read_to_string(source).map_err(|error| {
        CliFailure::Io(format!(
            "cannot read Codex route configuration {}: {error}",
            source.display()
        ))
    })?;
    let config: toml::Value = toml::from_str(&text).map_err(|error| {
        CliFailure::Arguments(format!(
            "cannot parse Codex route configuration {}: {error}",
            source.display()
        ))
    })?;
    let provider_name = config
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            CliFailure::Arguments("Codex clean state requires model_provider".to_string())
        })?;
    let provider = config
        .get("model_providers")
        .and_then(toml::Value::as_table)
        .and_then(|providers| providers.get(provider_name))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            CliFailure::Arguments(format!(
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
        CliFailure::Arguments(format!("cannot render Codex clean configuration: {error}"))
    })?;
    std::fs::write(target, rendered).map_err(|error| {
        CliFailure::Io(format!(
            "cannot write Codex clean configuration {}: {error}",
            target.display()
        ))
    })
}

fn copy_opencode_route_config(
    source: &Path,
    target: &Path,
    model: Option<&str>,
) -> Result<(), CliFailure> {
    let text = match std::fs::read_to_string(source) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(error) => {
            return Err(CliFailure::Io(format!(
                "cannot read OpenCode route configuration {}: {error}",
                source.display()
            )));
        }
    };
    let source_config: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        CliFailure::Arguments(format!(
            "cannot parse OpenCode route configuration {}: {error}",
            source.display()
        ))
    })?;
    let mut clean = serde_json::Map::new();
    if let Some(schema) = source_config.get("$schema") {
        clean.insert("$schema".to_string(), schema.clone());
    }
    if let Some(provider_name) = model.and_then(|value| value.split_once('/').map(|pair| pair.0))
        && let Some(provider) = source_config
            .get("provider")
            .and_then(serde_json::Value::as_object)
            .and_then(|providers| providers.get(provider_name))
    {
        let mut providers = serde_json::Map::new();
        providers.insert(provider_name.to_string(), provider.clone());
        clean.insert("provider".to_string(), serde_json::Value::Object(providers));
    }
    let rendered = serde_json::to_vec(&serde_json::Value::Object(clean)).map_err(|error| {
        CliFailure::Arguments(format!(
            "cannot render OpenCode clean configuration: {error}"
        ))
    })?;
    std::fs::write(target, rendered).map_err(|error| {
        CliFailure::Io(format!(
            "cannot write OpenCode clean configuration {}: {error}",
            target.display()
        ))
    })
}

pub(crate) fn prepare_clean_state(
    provider: CliProvider,
    root: &Path,
    model: Option<&str>,
) -> Result<(), CliFailure> {
    let home = dirs::home_dir()
        .ok_or_else(|| CliFailure::Io("cannot resolve home directory".to_string()))?;
    match provider {
        CliProvider::Codex => {
            let source_root =
                std::env::var_os("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from);
            copy_codex_route_config(&source_root.join("config.toml"), &root.join("config.toml"))
        }
        CliProvider::Grok => {
            let source = home.join(".grok/auth.json");
            let target_root = root.join(".grok");
            std::fs::create_dir_all(&target_root).map_err(|error| {
                CliFailure::Io(format!(
                    "cannot create Grok clean state {}: {error}",
                    target_root.display()
                ))
            })?;
            std::fs::copy(&source, target_root.join("auth.json")).map_err(|error| {
                CliFailure::Io(format!(
                    "cannot bridge Grok authentication {}: {error}",
                    source.display()
                ))
            })?;
            Ok(())
        }
        CliProvider::Opencode => {
            let source_config_root = std::env::var_os("XDG_CONFIG_HOME")
                .map_or_else(|| home.join(".config"), PathBuf::from);
            let target_config_root = root.join("config/opencode");
            std::fs::create_dir_all(&target_config_root).map_err(|error| {
                CliFailure::Io(format!(
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
                CliFailure::Io(format!(
                    "cannot create OpenCode clean state {}: {error}",
                    target_data_root.display()
                ))
            })?;
            if source.is_file() {
                std::fs::copy(&source, target_data_root.join("auth.json")).map_err(|error| {
                    CliFailure::Io(format!(
                        "cannot bridge OpenCode authentication {}: {error}",
                        source.display()
                    ))
                })?;
            }
            Ok(())
        }
        CliProvider::Claude | CliProvider::Agy => Ok(()),
    }
}

fn process_request(
    invocation: &CliInvocation,
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
        match invocation.provider {
            CliProvider::Claude => {
                request
                    .env
                    .push((OsString::from("CLAUDE_CONFIG_DIR"), root));
                request.env_remove.push(OsString::from("CLAUDECODE"));
            }
            CliProvider::Codex => {
                request.env.push((OsString::from("CODEX_HOME"), root));
            }
            CliProvider::Agy => {
                request
                    .env
                    .push((OsString::from("ANTIGRAVITY_EXECUTABLE_DATA_DIR"), root));
            }
            CliProvider::Grok => {
                request.env.push((OsString::from("HOME"), root));
            }
            CliProvider::Opencode => {
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
                request.env.push((
                    OsString::from("OPENCODE_DISABLE_PROJECT_CONFIG"),
                    OsString::from("1"),
                ));
                request.env.push((
                    OsString::from("OPENCODE_DISABLE_EXTERNAL_SKILLS"),
                    OsString::from("1"),
                ));
                request.env.push((
                    OsString::from("OPENCODE_DISABLE_CLAUDE_CODE_PROMPT"),
                    OsString::from("1"),
                ));
                request.env.push((
                    OsString::from("OPENCODE_DISABLE_CLAUDE_CODE_SKILLS"),
                    OsString::from("1"),
                ));
            }
        }
    }
    request.stdin = stdin;
    request.timeout = invocation.timeout;
    request
}

fn require_success(output: ProcessOutput) -> Result<ProcessOutput, CliFailure> {
    if output.termination == ProcessTermination::Exited(0) {
        return Ok(output);
    }
    Err(CliFailure::Exit {
        termination: output.termination,
        stderr: output.stderr,
    })
}

fn nonempty_text(text: &str, provider: &str) -> Result<String, CliFailure> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(CliFailure::Provider(format!(
            "{provider} produced no final response"
        )));
    }
    Ok(text)
}

fn create_scratch_directory(kind: &str) -> Result<PathBuf, CliFailure> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| CliFailure::Io(format!("cannot timestamp scratch directory: {error}")))?;
    let scratch = std::env::temp_dir().join(format!(
        "rune-{kind}-{}-{}",
        std::process::id(),
        timestamp.as_nanos()
    ));
    std::fs::create_dir_all(&scratch).map_err(|error| {
        CliFailure::Io(format!(
            "cannot create scratch directory {}: {error}",
            scratch.display()
        ))
    })?;
    Ok(scratch)
}

fn finish_scratch<T>(result: Result<T, CliFailure>, scratch: &PathBuf) -> Result<T, CliFailure> {
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
            Err(CliFailure::Io(message))
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

fn claude_args(invocation: &CliInvocation) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--print"),
        OsString::from("--output-format"),
        OsString::from("text"),
        OsString::from("--permission-mode"),
        OsString::from(match invocation.mode {
            SandboxMode::ReadOnly => "plan",
            SandboxMode::WorkspaceWrite => "acceptEdits",
        }),
    ];
    if invocation.clean_state_root.is_some() {
        args.extend([OsString::from("--setting-sources"), OsString::from("")]);
    }
    if invocation.mode == SandboxMode::ReadOnly {
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

fn invoke_claude(invocation: &CliInvocation) -> Result<CliReply, CliFailure> {
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
    Ok(CliReply {
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

fn with_provider_diagnostics(messages: &[String], stderr: &str) -> String {
    let mut sections: Vec<&str> = messages.iter().map(String::as_str).collect();
    if !stderr.trim().is_empty() {
        sections.push(stderr.trim());
    }
    sections.join("\n")
}

fn read_codex_final_message(path: &PathBuf, events: &[CodexEvent]) -> Result<String, CliFailure> {
    let file_message = match std::fs::read_to_string(path) {
        Ok(content) if !content.trim().is_empty() => Some(content),
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(CliFailure::Io(format!(
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
            CliFailure::Provider("codex exec produced no final agent message".to_string())
        })
}

fn invoke_codex(invocation: &CliInvocation) -> Result<CliReply, CliFailure> {
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
                SandboxMode::ReadOnly => "read-only",
                SandboxMode::WorkspaceWrite => "workspace-write",
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
            return Err(CliFailure::Exit {
                termination: output.termination,
                stderr: with_provider_diagnostics(&reported_errors, &output.stderr),
            });
        }
        if !reported_errors.is_empty() && !final_message_path.exists() {
            return Err(CliFailure::Provider(format!(
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
        Ok(CliReply {
            text,
            stderr: output.stderr,
            completion_tokens,
        })
    })();
    finish_scratch(result, &scratch)
}

fn grok_args(invocation: &CliInvocation, prompt_path: &Path) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--cwd"),
        invocation.repository.as_os_str().to_os_string(),
        OsString::from("--prompt-file"),
        prompt_path.as_os_str().to_os_string(),
        OsString::from("--output-format"),
        OsString::from("plain"),
        OsString::from("--no-memory"),
        OsString::from("--sandbox"),
        OsString::from(match invocation.mode {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite => "workspace",
        }),
        OsString::from("--permission-mode"),
        OsString::from(match invocation.mode {
            SandboxMode::ReadOnly => "dontAsk",
            SandboxMode::WorkspaceWrite => "acceptEdits",
        }),
    ];
    // Grok's read-only sandbox profile still permits writes through allowed
    // tools, so the read-only contract comes from the tool set and deny rules.
    if invocation.mode == SandboxMode::ReadOnly {
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
    if invocation.clean_state_root.is_some() || !invocation.system_prompt.trim().is_empty() {
        args.push(OsString::from(format!(
            "--system-prompt-override={}",
            clean_system_prompt(&invocation.system_prompt)
        )));
    }
    args.extend(invocation.extra_args.clone());
    args
}

fn invoke_grok(invocation: &CliInvocation) -> Result<CliReply, CliFailure> {
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
            CliFailure::Io(format!(
                "cannot write grok prompt {}: {error}",
                prompt_path.display()
            ))
        })?;
        let output = require_success(run_process_request(&process_request(
            invocation,
            grok_args(invocation, &prompt_path),
            None,
        ))?)?;
        Ok(CliReply {
            text: nonempty_text(&output.stdout, "grok single-prompt")?,
            stderr: output.stderr,
            completion_tokens: None,
        })
    })();
    finish_scratch(result, &scratch)
}

fn invoke_agy(invocation: &CliInvocation) -> Result<CliReply, CliFailure> {
    reject_owned_args(
        invocation,
        &[
            "-p",
            "--print",
            "--prompt",
            "--print-timeout",
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
            SandboxMode::ReadOnly => "plan",
            SandboxMode::WorkspaceWrite => "accept-edits",
        }),
        OsString::from("--print"),
        OsString::from(combined_prompt(invocation)),
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
    Ok(CliReply {
        text: nonempty_text(&output.stdout, "agy print")?,
        stderr: output.stderr,
        completion_tokens: None,
    })
}

fn invoke_opencode(invocation: &CliInvocation) -> Result<CliReply, CliFailure> {
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
    let mut args = vec![OsString::from("run")];
    args.extend([
        OsString::from("--dir"),
        invocation.repository.as_os_str().to_os_string(),
        OsString::from("--format"),
        OsString::from("json"),
    ]);
    if let Some(model) = &invocation.model {
        args.push(OsString::from("-m"));
        args.push(OsString::from(model));
    }
    if invocation.mode == SandboxMode::WorkspaceWrite {
        args.push(OsString::from("--auto"));
    }
    args.extend(invocation.extra_args.clone());
    args.push(OsString::from(combined_prompt(invocation)));
    let mut request = process_request(invocation, args, None);
    if invocation.mode == SandboxMode::ReadOnly {
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
        return Err(CliFailure::Provider(format!(
            "opencode session error: {message}"
        )));
    }
    let text = events
        .iter()
        .filter(|event| event.kind.as_deref() == Some("text"))
        .filter_map(|event| event.part.as_ref())
        .filter_map(|part| part.text.as_deref())
        .collect::<Vec<_>>()
        .join("");
    let completion_tokens = events
        .iter()
        .filter(|event| event.kind.as_deref() == Some("step_finish"))
        .filter_map(|event| event.part.as_ref())
        .filter_map(|part| part.tokens.as_ref())
        .filter_map(|tokens| tokens.output)
        .map(|tokens| tokens.max(0.0))
        .sum::<f64>();
    Ok(CliReply {
        text: nonempty_text(&text, "opencode run")?,
        stderr: output.stderr,
        completion_tokens: (completion_tokens > 0.0).then_some(completion_tokens),
    })
}

pub(crate) fn invoke_cli(invocation: &CliInvocation) -> Result<CliReply, CliFailure> {
    match invocation.provider {
        CliProvider::Claude => invoke_claude(invocation),
        CliProvider::Codex => invoke_codex(invocation),
        CliProvider::Agy => invoke_agy(invocation),
        CliProvider::Grok => invoke_grok(invocation),
        CliProvider::Opencode => invoke_opencode(invocation),
    }
}

fn run_process(
    binary: &str,
    args: &[String],
    stdin_text: Option<&str>,
    timeout: Duration,
) -> Result<ProcessOutput, String> {
    let mut request = ProcessRequest::new(binary);
    request.args = args.iter().map(OsString::from).collect();
    request.stdin = stdin_text.map(|text| text.as_bytes().to_vec());
    request.timeout = Some(timeout);
    run_process_request(&request).map_err(|failure| failure.to_string())
}

const READY_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

fn benchmark_cli_invocation(
    provider: CliProvider,
    binary: String,
    model: Option<String>,
    system_prompt: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<CliInvocation, String> {
    Ok(CliInvocation {
        provider,
        binary: OsString::from(binary),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: std::env::current_dir()
            .map_err(|error| format!("cannot resolve benchmark repository: {error}"))?,
        mode: SandboxMode::ReadOnly,
        system_prompt: system_prompt.to_string(),
        prompt: prompt.to_string(),
        model,
        native_timeout: None,
        timeout: Some(timeout),
        clean_state_root: None,
    })
}

fn benchmark_provider_reply(reply: CliReply) -> ProviderReply {
    ProviderReply {
        text: reply.text,
        cost: None,
        completion_tokens: reply.completion_tokens,
    }
}

struct ClaudeCliRunner {
    model: String,
}

// Non-interactive Claude Code as a model backend, primarily for the LLM judge.
// Uses the user's existing auth; the system prompt travels via
// --append-system-prompt so the judge's JSON-only contract is honoured.
impl ModelRunner for ClaudeCliRunner {
    fn ready(&self) -> Readiness {
        let binary = binary_for("CLAUDE_BIN", "claude");
        match run_process(
            &binary,
            &["--version".to_string()],
            None,
            READY_PROBE_TIMEOUT,
        ) {
            Ok(outcome) if outcome.code() == Some(0) => Readiness::Ready,
            Ok(outcome) => Readiness::NotReady(format!(
                "claude --version exited {:?}; ensure Claude Code is installed and authenticated",
                outcome.code()
            )),
            Err(error) => Readiness::NotReady(format!("claude binary not runnable: {error}")),
        }
    }

    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        _temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String> {
        let invocation = benchmark_cli_invocation(
            CliProvider::Claude,
            binary_for("CLAUDE_BIN", "claude"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_cli(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
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

struct CodexCliRunner {
    model: String,
    model_id: String,
}

// The prompt travels over stdin; codex exec has no system-prompt flag, so the
// suite system_prompt is prepended. Auth is never touched: ready() only checks
// login status.
impl ModelRunner for CodexCliRunner {
    fn ready(&self) -> Readiness {
        let binary = binary_for("CODEX_BIN", "codex");
        if binary.contains('/') && !PathBuf::from(&binary).exists() {
            return Readiness::NotReady(format!("codex binary not found at {binary}"));
        }
        match run_process(
            &binary,
            &["login".to_string(), "status".to_string()],
            None,
            READY_PROBE_TIMEOUT,
        ) {
            Ok(outcome) if outcome.code() == Some(0) => Readiness::Ready,
            Ok(outcome) => Readiness::NotReady(format!(
                "codex login status exited {:?}; authenticate in your own terminal before enabling {}",
                outcome.code(),
                self.model_id
            )),
            Err(error) => Readiness::NotReady(format!("codex login status failed: {error}")),
        }
    }

    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        _temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String> {
        let invocation = benchmark_cli_invocation(
            CliProvider::Codex,
            binary_for("CODEX_BIN", "codex"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_cli(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
}

struct AgyCliRunner {
    model: String,
    model_id: String,
}

// agy exposes no system-prompt flag, so the suite system prompt is prepended
// to the user prompt — a documented divergence from API-backed providers.
impl ModelRunner for AgyCliRunner {
    fn ready(&self) -> Readiness {
        let binary = binary_for("AGY_BIN", "agy");
        match run_process(&binary, &["models".to_string()], None, READY_PROBE_TIMEOUT) {
            Ok(outcome) if outcome.code() == Some(0) => Readiness::Ready,
            Ok(outcome) => Readiness::NotReady(format!(
                "agy models exited {:?}; ensure agy is installed and authenticated for {}",
                outcome.code(),
                self.model_id
            )),
            Err(error) => Readiness::NotReady(format!("agy binary not runnable: {error}")),
        }
    }

    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        _temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String> {
        let invocation = benchmark_cli_invocation(
            CliProvider::Agy,
            binary_for("AGY_BIN", "agy"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_cli(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
}

struct GrokCliRunner {
    model: String,
    model_id: String,
}

// `model: default` omits -m and uses the CLI's configured default model; the
// suite system prompt goes through --system-prompt-override.
impl ModelRunner for GrokCliRunner {
    fn ready(&self) -> Readiness {
        let binary = binary_for("GROK_BIN", "grok");
        match run_process(
            &binary,
            &["--version".to_string()],
            None,
            READY_PROBE_TIMEOUT,
        ) {
            Ok(outcome) if outcome.code() == Some(0) => Readiness::Ready,
            Ok(outcome) => Readiness::NotReady(format!(
                "grok --version exited {:?}; ensure grok is installed and authenticated for {}",
                outcome.code(),
                self.model_id
            )),
            Err(error) => Readiness::NotReady(format!("grok binary not runnable: {error}")),
        }
    }

    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        _temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String> {
        let model = (self.model != "default").then(|| self.model.clone());
        let invocation = benchmark_cli_invocation(
            CliProvider::Grok,
            binary_for("GROK_BIN", "grok"),
            model,
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_cli(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
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
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    tokens: Option<OpencodeTokens>,
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

struct OpencodeCliRunner {
    model: String,
    model_id: String,
}

// The route to models with no public API (proton-lumo/lumo-max): JSONL events
// from `opencode run --format json`; text parts concatenate and step_finish
// carries token usage. No system-prompt flag, so it is prepended.
impl ModelRunner for OpencodeCliRunner {
    fn ready(&self) -> Readiness {
        let binary = binary_for("OPENCODE_BIN", "opencode");
        match run_process(
            &binary,
            &["--version".to_string()],
            None,
            READY_PROBE_TIMEOUT,
        ) {
            Ok(outcome) if outcome.code() == Some(0) => Readiness::Ready,
            Ok(outcome) => Readiness::NotReady(format!(
                "opencode --version exited {:?}; ensure opencode is installed and its provider configured for {}",
                outcome.code(),
                self.model_id
            )),
            Err(error) => Readiness::NotReady(format!("opencode binary not runnable: {error}")),
        }
    }

    fn invoke(
        &self,
        system_prompt: &str,
        prompt: &str,
        _temperature: Option<f64>,
        timeout: Duration,
    ) -> Result<ProviderReply, String> {
        let invocation = benchmark_cli_invocation(
            CliProvider::Opencode,
            binary_for("OPENCODE_BIN", "opencode"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_cli(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_request(script: &str) -> ProcessRequest {
        let mut request = ProcessRequest::new("/bin/sh");
        request.args = vec![OsString::from("-c"), OsString::from(script)];
        request
    }

    #[test]
    fn process_request_has_no_default_timeout() {
        let request = ProcessRequest::new("/bin/true");
        assert_eq!(request.timeout, None);
    }

    #[test]
    fn process_captures_stdout_stderr_and_exit_code() {
        let request = shell_request("printf answer; printf warning >&2; exit 7");
        let output = run_process_request(&request).expect("process output");

        assert_eq!(output.termination, ProcessTermination::Exited(7));
        assert_eq!(output.stdout, "answer");
        assert_eq!(output.stderr, "warning");
    }

    #[test]
    fn process_captures_stdin_without_deadlock() {
        let mut request = ProcessRequest::new("/bin/cat");
        request.stdin = Some(b"question\n".to_vec());
        let output = run_process_request(&request).expect("process output");

        assert_eq!(output.termination, ProcessTermination::Exited(0));
        assert_eq!(output.stdout, "question\n");
    }

    #[test]
    fn process_preserves_exit_when_child_closes_stdin_early() {
        let mut request = shell_request("exit 23");
        request.stdin = Some(vec![b'x'; DEFAULT_OUTPUT_LIMIT_BYTES * 2]);
        let output = run_process_request(&request).expect("process output");

        assert_eq!(output.termination, ProcessTermination::Exited(23));
    }

    #[test]
    fn process_reports_signal_termination() {
        let request = shell_request("kill -TERM $$");
        let output = run_process_request(&request).expect("process output");

        assert!(matches!(
            output.termination,
            ProcessTermination::Signaled(signal)
                if signal == signal_hook::consts::signal::SIGTERM
        ));
    }

    #[test]
    fn process_timeout_uses_grace_then_reports_timeout() {
        let mut request = shell_request("trap '' TERM; sleep 5");
        request.stdin = Some(vec![b'x'; DEFAULT_OUTPUT_LIMIT_BYTES * 2]);
        request.timeout = Some(Duration::from_millis(50));
        request.termination_grace = Duration::from_millis(50);
        let started = Instant::now();
        let failure = run_process_request(&request).expect_err("timeout");

        assert_eq!(failure, ProcessFailure::Timeout(Duration::from_millis(50)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn process_drains_output_after_limit_is_crossed() {
        let mut request = shell_request(
            "i=0; while [ \"$i\" -lt 100 ]; do printf 0123456789; i=$((i + 1)); done",
        );
        request.output_limit = 64;
        request.termination_grace = Duration::from_millis(50);
        let failure = run_process_request(&request).expect_err("output limit");

        let ProcessFailure::OutputLimit {
            stream,
            limit,
            tail,
        } = failure
        else {
            panic!("expected output limit failure");
        };
        assert_eq!(stream, "stdout");
        assert_eq!(limit, 64);
        assert_eq!(tail, format!("6789{}", "0123456789".repeat(6)));
    }

    #[test]
    fn process_request_removes_inherited_automation_mode() {
        let mut request = shell_request("printf %s \"${HARNESS_AUTOMATED-unset}\"");
        request.env = vec![(OsString::from("HARNESS_AUTOMATED"), OsString::from("1"))];
        request.env_remove = vec![OsString::from("HARNESS_AUTOMATED")];
        let output = run_process_request(&request).expect("process output");

        assert_eq!(output.termination, ProcessTermination::Exited(0));
        assert_eq!(output.stdout, "unset");
    }

    #[test]
    fn automated_provider_rejects_profile_owned_flags() {
        let invocation = CliInvocation {
            provider: CliProvider::Codex,
            binary: OsString::from("codex"),
            extra_args: vec![OsString::from("--sandbox=workspace-write")],
            env: Vec::new(),
            repository: PathBuf::from("."),
            mode: SandboxMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: None,
        };

        assert_eq!(
            reject_owned_args(&invocation, &["-s", "--sandbox"]),
            Err(CliFailure::Arguments(
                "automated codex execution owns these profile arguments: --sandbox=workspace-write; remove them from the launch profile".to_string()
            ))
        );
    }

    #[test]
    fn clean_codex_config_keeps_only_route_fields() {
        let directory = tempfile::tempdir().expect("tempdir");
        let source = directory.path().join("source.toml");
        let target = directory.path().join("clean.toml");
        std::fs::write(
            &source,
            "model_provider = \"proxy\"\nhooks = [\"secret\"]\n[model_providers.proxy]\nname = \"Proxy\"\nbase_url = \"http://localhost\"\nwire_api = \"responses\"\nenv_key = \"PROXY_KEY\"\nhttp_headers = { version = \"1\" }\nenv_http_headers = { authorization = \"PROXY_AUTH\" }\nquery_params = { beta = \"true\" }\n[plugins.test]\nenabled = true\n",
        )
        .expect("write config");

        copy_codex_route_config(&source, &target).expect("copy clean config");
        let clean = std::fs::read_to_string(target).expect("read clean config");

        assert!(clean.contains("model_provider = \"proxy\""));
        assert!(clean.contains("env_key = \"PROXY_KEY\""));
        assert!(clean.contains("http_headers"));
        assert!(clean.contains("env_http_headers"));
        assert!(clean.contains("query_params"));
        assert!(!clean.contains("hooks"));
        assert!(!clean.contains("plugins"));
    }

    #[test]
    fn claude_read_only_limits_available_tools() {
        let invocation = CliInvocation {
            provider: CliProvider::Claude,
            binary: OsString::from("claude"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("."),
            mode: SandboxMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: None,
        };

        assert_eq!(
            claude_args(&invocation),
            [
                "--print",
                "--output-format",
                "text",
                "--permission-mode",
                "plan",
                "--tools",
                "Read,Glob,Grep",
                "--allowedTools",
                "Read,Glob,Grep",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn clean_claude_overrides_system_prompt() {
        let invocation = CliInvocation {
            provider: CliProvider::Claude,
            binary: OsString::from("claude"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("."),
            mode: SandboxMode::ReadOnly,
            system_prompt: "Use this rule.".to_string(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: Some(PathBuf::from("/clean")),
        };
        let args = claude_args(&invocation);

        assert!(args.windows(2).any(|pair| pair
            == [
                OsString::from("--system-prompt"),
                OsString::from(format!("{CLEAN_SYSTEM_PROMPT}\n\nUse this rule.")),
            ]));
        assert!(!args.contains(&OsString::from("--append-system-prompt")));
    }

    #[test]
    fn grok_read_only_limits_tools_and_denies_writes() {
        let invocation = CliInvocation {
            provider: CliProvider::Grok,
            binary: OsString::from("grok"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("/repo"),
            mode: SandboxMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: None,
        };

        assert_eq!(
            grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt")),
            [
                "--cwd",
                "/repo",
                "--prompt-file",
                "/scratch/prompt.txt",
                "--output-format",
                "plain",
                "--no-memory",
                "--sandbox",
                "read-only",
                "--permission-mode",
                "dontAsk",
                "--tools",
                "Read,Glob,Grep",
                "--deny",
                "Write(**)",
                "--deny",
                "Edit(**)",
                "--deny",
                "Bash(**)",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn clean_grok_uses_neutral_system_prompt() {
        let invocation = CliInvocation {
            provider: CliProvider::Grok,
            binary: OsString::from("grok"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("/repo"),
            mode: SandboxMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: Some(PathBuf::from("/clean")),
        };
        let args = grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt"));

        assert!(args.contains(&OsString::from(format!(
            "--system-prompt-override={CLEAN_SYSTEM_PROMPT}"
        ))));
    }

    #[test]
    fn grok_system_prompt_accepts_frontmatter() {
        let invocation = CliInvocation {
            provider: CliProvider::Grok,
            binary: OsString::from("grok"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("/repo"),
            mode: SandboxMode::ReadOnly,
            system_prompt: "---\nname: Example".to_string(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: None,
        };
        let args = grok_args(&invocation, &PathBuf::from("/scratch/prompt.txt"));

        assert!(args.contains(&OsString::from(format!(
            "--system-prompt-override={CLEAN_SYSTEM_PROMPT}\n\n---\nname: Example"
        ))));
    }

    #[test]
    fn clean_state_redirects_codex_and_agy_data() {
        for (provider, key) in [
            (CliProvider::Codex, "CODEX_HOME"),
            (CliProvider::Agy, "ANTIGRAVITY_EXECUTABLE_DATA_DIR"),
            (CliProvider::Grok, "HOME"),
        ] {
            let invocation = CliInvocation {
                provider,
                binary: OsString::from("provider"),
                extra_args: Vec::new(),
                env: Vec::new(),
                repository: PathBuf::from("."),
                mode: SandboxMode::ReadOnly,
                system_prompt: String::new(),
                prompt: "Inspect only".to_string(),
                model: None,
                native_timeout: None,
                timeout: None,
                clean_state_root: Some(PathBuf::from("/clean")),
            };
            let request = process_request(&invocation, Vec::new(), None);

            assert!(
                request
                    .env
                    .contains(&(OsString::from(key), OsString::from("/clean")))
            );
        }
    }

    #[test]
    fn clean_opencode_config_keeps_only_selected_provider() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source.json");
        let target = temporary.path().join("target.json");
        std::fs::write(
            &source,
            r#"{"$schema":"schema","instructions":["rules/*.md"],"plugin":["plugin"],"provider":{"proton-lumo":{"npm":"provider"},"other":{"npm":"other"}}}"#,
        )
        .expect("source configuration");

        copy_opencode_route_config(&source, &target, Some("proton-lumo/lumo-max"))
            .expect("clean configuration");
        let clean: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target).expect("target configuration"))
                .expect("valid target JSON");

        assert_eq!(clean["$schema"], "schema");
        assert_eq!(clean["provider"]["proton-lumo"]["npm"], "provider");
        assert!(clean.get("instructions").is_none());
        assert!(clean.get("plugin").is_none());
        assert!(clean["provider"].get("other").is_none());
    }

    #[test]
    fn clean_opencode_redirects_xdg_state() {
        let invocation = CliInvocation {
            provider: CliProvider::Opencode,
            binary: OsString::from("opencode"),
            extra_args: Vec::new(),
            env: Vec::new(),
            repository: PathBuf::from("."),
            mode: SandboxMode::ReadOnly,
            system_prompt: String::new(),
            prompt: "Inspect only".to_string(),
            model: None,
            native_timeout: None,
            timeout: None,
            clean_state_root: Some(PathBuf::from("/clean")),
        };
        let request = process_request(&invocation, Vec::new(), None);

        for (key, value) in [
            ("XDG_CONFIG_HOME", "/clean/config"),
            ("XDG_DATA_HOME", "/clean/data"),
            ("XDG_STATE_HOME", "/clean/state"),
        ] {
            assert!(
                request
                    .env
                    .contains(&(OsString::from(key), OsString::from(value)))
            );
        }
    }

    #[test]
    fn automated_providers_reject_read_only_bypass_arguments() {
        for (provider, argument) in [
            (CliProvider::Claude, "--dangerously-skip-permissions"),
            (
                CliProvider::Codex,
                "--config=sandbox_mode=\"danger-full-access\"",
            ),
            (CliProvider::Codex, "-sdanger-full-access"),
            (CliProvider::Codex, "-csandbox_mode=\"danger-full-access\""),
            (CliProvider::Codex, "-C/tmp"),
            (CliProvider::Codex, "-mgpt-5.6-sol"),
            (CliProvider::Grok, "--always-approve"),
            (CliProvider::Grok, "-mgrok-4"),
            (CliProvider::Agy, "--dangerously-skip-permissions"),
            (CliProvider::Opencode, "--attach=http://127.0.0.1:4096"),
            (CliProvider::Opencode, "-mproton-lumo/lumo-max"),
        ] {
            let invocation = CliInvocation {
                provider,
                binary: OsString::from("missing-provider-binary"),
                extra_args: vec![OsString::from(argument)],
                env: Vec::new(),
                repository: PathBuf::from("."),
                mode: SandboxMode::ReadOnly,
                system_prompt: String::new(),
                prompt: "Inspect only".to_string(),
                model: None,
                native_timeout: None,
                timeout: None,
                clean_state_root: None,
            };

            assert!(
                matches!(invoke_cli(&invocation), Err(CliFailure::Arguments(_))),
                "{provider:?} accepted {argument}"
            );
        }
    }

    #[test]
    fn codex_failure_reports_its_own_error_events() {
        let events: Vec<CodexEvent> = parse_jsonl(concat!(
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"error\",\"message\":\"hooks loaded twice\"}}\n",
            "{\"type\":\"error\",\"message\":\"You've hit your usage limit.\"}\n",
            "{\"type\":\"turn.failed\",\"error\":{\"message\":\"turn aborted\"}}",
        ));

        assert_eq!(
            codex_error_messages(&events),
            vec![
                "hooks loaded twice".to_string(),
                "You've hit your usage limit.".to_string(),
                "turn aborted".to_string(),
            ]
        );
        assert_eq!(
            with_provider_diagnostics(&codex_error_messages(&events), "cache warning\n"),
            "hooks loaded twice\nYou've hit your usage limit.\nturn aborted\ncache warning"
        );
    }

    #[test]
    fn opencode_session_error_precedes_text_output() {
        let events: Vec<OpencodeEvent> = parse_jsonl(
            "{\"type\":\"session.error\",\"properties\":{\"error\":{\"data\":{\"message\":\"route failed\"}}}}",
        );

        assert_eq!(opencode_session_error(&events), Some("route failed"));
    }
}
