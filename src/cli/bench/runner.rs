//! Model backends for `rune bench`: HTTP chat endpoints (Ollama,
//! OpenAI-compatible), a deterministic echo stub, and the coding surfaces
//! reused through [`crate::cli::surface`]. This is the bench model-runner
//! layer, not the deploy-provider list behind `rune provider` and not the
//! deploy target conventions in the library's `provider` module.
use super::registry::{ModelConfig, Provider};
use crate::cli::process::{ProcessOutput, ProcessRequest, run_process_request};
use crate::cli::surface::{AccessMode, Surface, SurfaceInvocation, SurfaceReply, invoke_surface};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

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

fn benchmark_surface_invocation(
    surface: Surface,
    binary: String,
    model: Option<String>,
    system_prompt: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<SurfaceInvocation, String> {
    Ok(SurfaceInvocation {
        surface,
        binary: OsString::from(binary),
        extra_args: Vec::new(),
        env: Vec::new(),
        repository: std::env::current_dir()
            .map_err(|error| format!("cannot resolve benchmark repository: {error}"))?,
        mode: AccessMode::ReadOnly,
        system_prompt: system_prompt.to_string(),
        prompt: prompt.to_string(),
        model,
        native_timeout: None,
        timeout: Some(timeout),
        clean_state_root: None,
    })
}

fn benchmark_provider_reply(reply: SurfaceReply) -> ProviderReply {
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
        let invocation = benchmark_surface_invocation(
            Surface::Claude,
            binary_for("CLAUDE_BIN", "claude"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_surface(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
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
        let invocation = benchmark_surface_invocation(
            Surface::Codex,
            binary_for("CODEX_BIN", "codex"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_surface(&invocation)
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
        let invocation = benchmark_surface_invocation(
            Surface::Agy,
            binary_for("AGY_BIN", "agy"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_surface(&invocation)
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
        let invocation = benchmark_surface_invocation(
            Surface::Grok,
            binary_for("GROK_BIN", "grok"),
            model,
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_surface(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
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
        let invocation = benchmark_surface_invocation(
            Surface::Opencode,
            binary_for("OPENCODE_BIN", "opencode"),
            Some(self.model.clone()),
            system_prompt,
            prompt,
            timeout,
        )?;
        invoke_surface(&invocation)
            .map(benchmark_provider_reply)
            .map_err(|failure| failure.to_string())
    }
}
