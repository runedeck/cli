use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Provider {
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
    #[serde(rename = "codex-cli")]
    CodexCli,
    #[serde(rename = "claude-cli")]
    ClaudeCli,
    #[serde(rename = "agy-cli")]
    AgyCli,
    #[serde(rename = "grok-cli")]
    GrokCli,
    #[serde(rename = "opencode-cli")]
    OpencodeCli,
    #[serde(rename = "echo")]
    Echo,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenAiCompatible => "openai-compatible",
            Self::CodexCli => "codex-cli",
            Self::ClaudeCli => "claude-cli",
            Self::AgyCli => "agy-cli",
            Self::GrokCli => "grok-cli",
            Self::OpencodeCli => "opencode-cli",
            Self::Echo => "echo",
        }
    }

    pub fn is_cli(self) -> bool {
        matches!(
            self,
            Self::CodexCli | Self::ClaudeCli | Self::AgyCli | Self::GrokCli | Self::OpencodeCli
        )
    }

    fn default_concurrency(self) -> u32 {
        match self {
            Self::Ollama | Self::Echo => 2,
            Self::OpenAiCompatible => 4,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub id: String,
    pub provider: Provider,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub runs: u32,
    pub concurrency: u32,
    pub temperature: Option<f64>,
    pub enabled: bool,
}

#[derive(Deserialize)]
struct RegistryFile {
    models: Vec<RegistryEntry>,
}

#[derive(Deserialize)]
struct RegistryEntry {
    id: String,
    provider: Provider,
    model: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    runs: Option<u32>,
    #[serde(default)]
    concurrency: Option<u32>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    enabled: Option<bool>,
}

fn expand_environment(
    value: &str,
    model_id: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let mut expanded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        expanded.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            expanded.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = &after[..end];
        let valid = !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if valid {
            match env(name) {
                Some(resolved) if !resolved.is_empty() => expanded.push_str(&resolved),
                _ => {
                    return Err(format!(
                        "model {model_id}: base_url references ${{{name}}} but {name} is not set"
                    ));
                }
            }
        } else {
            expanded.push_str(&rest[start..=start + 2 + end]);
        }
        rest = &after[end + 1..];
    }
    expanded.push_str(rest);
    Ok(expanded)
}

// Registry contract and hard-error list mirror the bun implementation: the
// same models.yaml drives both runners.
pub fn load_model_registry(config_path: &Path) -> Result<Vec<ModelConfig>, String> {
    let raw = std::fs::read_to_string(config_path)
        .map_err(|error| format!("cannot read {}: {error}", config_path.display()))?;
    let registry: RegistryFile = serde_yaml::from_str(&raw)
        .map_err(|error| format!("invalid registry {}: {error}", config_path.display()))?;

    let mut seen = HashSet::new();
    let mut models = Vec::with_capacity(registry.models.len());

    for entry in registry.models {
        if entry.id.is_empty() || entry.model.is_empty() {
            return Err(format!(
                "invalid registry {}: id and model must be non-empty",
                config_path.display()
            ));
        }
        if !seen.insert(entry.id.clone()) {
            return Err(format!(
                "duplicate model id in {}: {}",
                config_path.display(),
                entry.id
            ));
        }
        if entry.runs == Some(0) || entry.concurrency == Some(0) {
            return Err(format!(
                "model {}: runs and concurrency must be positive",
                entry.id
            ));
        }
        if entry.provider.is_cli() && entry.temperature.is_some() {
            return Err(format!(
                "model {}: temperature must be omitted for {} (no sampling controls)",
                entry.id,
                entry.provider.name()
            ));
        }
        if entry.provider == Provider::OpenAiCompatible && entry.base_url.is_none() {
            return Err(format!(
                "model {}: base_url is required for openai-compatible",
                entry.id
            ));
        }

        let enabled = entry.enabled.unwrap_or(true);
        // base_url stays raw at load so `list` and `doctor` can describe a
        // machine that lacks one provider's environment variable; run and
        // report expand (and hard-error) via `expand_base_urls` after model
        // selection.
        let base_url = entry.base_url.clone();

        models.push(ModelConfig {
            concurrency: entry
                .concurrency
                .unwrap_or(entry.provider.default_concurrency()),
            temperature: if entry.provider.is_cli() {
                None
            } else {
                Some(entry.temperature.unwrap_or(1.0))
            },
            id: entry.id,
            provider: entry.provider,
            model: entry.model,
            base_url,
            api_key_env: entry.api_key_env,
            runs: entry.runs.unwrap_or(30),
            enabled,
        });
    }

    Ok(models)
}

/// Resolve `${VAR}` references in the selected models' base URLs from the
/// process environment. A missing or empty variable is a hard error, matching
/// the bun harness's load-time strictness for models that are about to run.
pub fn expand_base_urls(models: &mut [ModelConfig]) -> Result<(), String> {
    expand_base_urls_with_env(models, &|name| std::env::var(name).ok())
}

pub fn expand_base_urls_with_env(
    models: &mut [ModelConfig],
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(), String> {
    for model in models {
        if let Some(url) = &model.base_url {
            model.base_url = Some(expand_environment(url, &model.id, env)?);
        }
    }
    Ok(())
}

pub fn select_models(
    registry: &[ModelConfig],
    models_flag: Option<&str>,
) -> Result<Vec<ModelConfig>, String> {
    let Some(flag) = models_flag else {
        return Ok(registry
            .iter()
            .filter(|model| model.enabled)
            .cloned()
            .collect());
    };
    let wanted: Vec<&str> = flag
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect();
    let mut selected = Vec::with_capacity(wanted.len());
    for id in wanted {
        let Some(model) = registry.iter().find(|model| model.id == id) else {
            return Err(format!(
                "--models: unknown model id '{id}' (see 'rune bench list')"
            ));
        };
        selected.push(model.clone());
    }
    Ok(selected)
}
