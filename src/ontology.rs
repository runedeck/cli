use crate::error::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub deck: Option<String>,
    /// Env file consulted when a launch profile's `from_env` reference is
    /// unset in the process environment. Defaults to `~/.env`.
    pub env: Option<String>,
    pub ontology: Ontology,
    pub extensions: Vec<String>,
    pub launch: Launch,
    pub watch: Watch,
    /// Bench workspace checkouts, in priority order: the first entry is the
    /// primary (registry, default results); every entry contributes its
    /// suites, and a suite's outputs stay in the checkout that owns it.
    pub bench: Vec<String>,
    pub setup: Option<SetupRecord>,
    pub theme: Option<ThemeConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SetupRecord {
    pub version: u32,
    pub completed: Vec<String>,
}

/// Terminal theme selection: a named palette, an optional light and dark
/// pair that follows the host appearance, and single-token overrides.
#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    JsonSchema,
    Default,
    PartialEq,
    Eq
)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeConfig {
    pub name: Option<String>,
    pub auto_switch: bool,
    pub dark_name: Option<String>,
    pub light_name: Option<String>,
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub struct Ontology {
    pub targets: Option<String>,
    pub quests: Option<String>,
    pub skeleton: Option<String>,
    pub owner: Option<String>,
    pub archive: Option<String>,
    pub vault: Option<String>,
    pub work: Option<String>,
    pub lore: Option<String>,
    pub mount: Option<String>,
    pub developer: Option<String>,
    pub artifacts: Option<String>,
    pub githooks: Option<String>,
    pub domain: Option<String>,
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    JsonSchema,
    Default,
    PartialEq,
    Eq
)]
#[serde(default)]
#[schemars(transform = add_launch_schema_aliases)]
pub struct Launch {
    #[serde(alias = "default-with")]
    pub default_with: Vec<String>,
    pub tools: HashMap<String, LaunchTool>,
    pub middleware: LaunchMiddleware,
    pub models: HashMap<String, LaunchModel>,
    /// Named launch presets per tool: `launch.profiles.claude.sol` selects
    /// via `rune launch sol@claude`.
    pub profiles: HashMap<String, HashMap<String, LaunchProfile>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct LaunchModel {
    pub id: String,
    pub context: u64,
    #[serde(default)]
    pub compact: Option<u8>,
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    JsonSchema,
    Default,
    PartialEq,
    Eq
)]
#[serde(default)]
pub struct LaunchProfile {
    pub model: Option<String>,
    pub env: HashMap<String, ProfileEnvValue>,
    pub args: Vec<String>,
    pub with: Vec<String>,
}

/// A literal value or a reference resolved from the parent environment at
/// launch time (`from_env: KEY`), so secrets never live in config files.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(untagged)]
pub enum ProfileEnvValue {
    Literal(String),
    FromEnv {
        #[serde(rename = "from_env")]
        from_env: String,
    },
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    JsonSchema,
    Default,
    PartialEq,
    Eq
)]
#[serde(default)]
#[schemars(transform = add_launch_tool_schema_aliases)]
pub struct LaunchTool {
    pub binary: Option<String>,
    #[serde(alias = "base-url-env")]
    pub base_url_env: Option<String>,
}

#[derive(
    Debug,
    Clone,
    Deserialize,
    Serialize,
    JsonSchema,
    Default,
    PartialEq,
    Eq
)]
#[serde(default)]
pub struct LaunchMiddleware {
    pub pxpipe: PxpipeConfig,
    pub otel: OtelConfig,
    pub presidio: PresidioConfig,
    pub squid: SquidConfig,
    pub docker: DockerConfig,
    pub cliproxy: CliproxyConfig,
}

fn add_launch_schema_aliases(schema: &mut schemars::Schema) {
    add_schema_alias(schema, "default_with", "default-with");
}

fn add_launch_tool_schema_aliases(schema: &mut schemars::Schema) {
    add_schema_alias(schema, "base_url_env", "base-url-env");
}

fn add_schema_alias(schema: &mut schemars::Schema, canonical: &str, alias: &str) {
    let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let Some(mut alias_schema) = properties.get(canonical).cloned() else {
        return;
    };
    if let Some(mapping) = alias_schema.as_object_mut() {
        mapping.insert(
            "x-rune-alias-for".to_string(),
            serde_json::Value::String(canonical.to_string()),
        );
    }
    properties.insert(alias.to_string(), alias_schema);
}

/// Liveness check for a local AI-API proxy (CLIProxyAPI-shaped), the
/// process a cross-harness profile's `ANTHROPIC_BASE_URL` points at. It
/// probes the proxy port and warns when it is down. `command` is empty by
/// default (check-only); set it to a start command (e.g. `brew services
/// run cliproxyapi`) to opt into self-heal.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct CliproxyConfig {
    pub host: String,
    pub port: u16,
    pub command: String,
}

impl Default for CliproxyConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8317,
            command: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct PxpipeConfig {
    pub base_url: String,
    pub host: String,
    pub port: u16,
    pub command: String,
    pub log_path: String,
}

impl Default for PxpipeConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:47821".to_string(),
            host: "127.0.0.1".to_string(),
            port: 47_821,
            command: "pxpipe".to_string(),
            log_path: "~/.pxpipe/proxy.log".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct OtelConfig {
    pub endpoint: String,
    pub service_name: String,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:4318".to_string(),
            service_name: "rune-launch".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct PresidioConfig {
    pub base_url: String,
    pub host: String,
    pub port: u16,
}

impl Default for PresidioConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:47822".to_string(),
            host: "127.0.0.1".to_string(),
            port: 47_822,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct SquidConfig {
    pub http_proxy: String,
    pub https_proxy: String,
}

impl Default for SquidConfig {
    fn default() -> Self {
        Self {
            http_proxy: "http://127.0.0.1:3128".to_string(),
            https_proxy: "http://127.0.0.1:3128".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct DockerConfig {
    pub image: String,
    pub args: Vec<String>,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "ghcr.io/runedeck/rune-coding-tool:latest".to_string(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub struct Watch {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Env,
    Config,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedField {
    pub key: &'static str,
    pub env: &'static str,
    pub value: Option<String>,
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResolvedOntology {
    pub targets: Option<ResolvedValue>,
    pub skeleton: Option<ResolvedValue>,
    pub owner: Option<ResolvedValue>,
    pub archive: Option<ResolvedValue>,
    pub vault: Option<ResolvedValue>,
    pub work: Option<ResolvedValue>,
    pub lore: Option<ResolvedValue>,
    pub mount: Option<ResolvedValue>,
    pub developer: Option<ResolvedValue>,
    pub artifacts: Option<ResolvedValue>,
    pub githooks: Option<ResolvedValue>,
    pub domain: Option<ResolvedValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedValue {
    pub value: String,
    pub source: Source,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResolvedConfig {
    pub deck: Option<ResolvedValue>,
    pub env: Option<ResolvedValue>,
    pub ontology: ResolvedOntology,
    pub extensions: Vec<PathBuf>,
    pub setup: Option<SetupRecord>,
    #[serde(skip)]
    pub launch: Launch,
    #[serde(skip)]
    pub bench: Vec<String>,
    #[serde(skip)]
    pub theme: Option<ThemeConfig>,
}

impl ResolvedConfig {
    /// The env file consulted for `from_env` fallback: the configured
    /// override, or `~/.env`.
    #[must_use]
    pub fn env_file(&self) -> Option<PathBuf> {
        match &self.env {
            Some(value) => Some(expand_tilde(&value.value)),
            None => dirs::home_dir().map(|home| home.join(".env")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Key {
    Targets,
    Skeleton,
    Owner,
    Archive,
    Vault,
    Work,
    Lore,
    Mount,
    Developer,
    Artifacts,
    Githooks,
    Domain,
}

impl Key {
    const ALL: [Self; 12] = [
        Self::Targets,
        Self::Skeleton,
        Self::Owner,
        Self::Archive,
        Self::Vault,
        Self::Work,
        Self::Lore,
        Self::Mount,
        Self::Developer,
        Self::Artifacts,
        Self::Githooks,
        Self::Domain,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Targets => "targets",
            Self::Skeleton => "skeleton",
            Self::Owner => "owner",
            Self::Archive => "archive",
            Self::Vault => "vault",
            Self::Work => "work",
            Self::Lore => "lore",
            Self::Mount => "mount",
            Self::Developer => "developer",
            Self::Artifacts => "artifacts",
            Self::Githooks => "githooks",
            Self::Domain => "domain",
        }
    }

    fn env(self) -> &'static str {
        match self {
            Self::Targets => "RUNE_TARGETS",
            Self::Skeleton => "RUNE_SKELETON",
            Self::Owner => "RUNE_OWNER",
            Self::Archive => "RUNE_ARCHIVE",
            Self::Vault => "RUNE_VAULT",
            Self::Work => "RUNE_WORK",
            Self::Lore => "RUNE_LORE",
            Self::Mount => "RUNE_MOUNT",
            Self::Developer => "RUNE_DEVELOPER",
            Self::Artifacts => "RUNE_ARTIFACTS",
            Self::Githooks => "RUNE_GITHOOKS",
            Self::Domain => "RUNE_DOMAIN",
        }
    }

    // Only product-generic defaults ship in the binary: the targets root and
    // its archive. Machine-specific locations (skeleton checkout, vault,
    // data layer, mount, domain) come from config or environment; `rune init`
    // falls back to the embedded skeleton when none is configured.
    fn default(self) -> Option<&'static str> {
        match self {
            Self::Targets => Some("~/Agents"),
            Self::Archive => Some("~/Agents/archive"),
            Self::Skeleton
            | Self::Vault
            | Self::Lore
            | Self::Mount
            | Self::Domain
            | Self::Owner
            | Self::Work
            | Self::Developer
            | Self::Artifacts
            | Self::Githooks => None,
        }
    }

    /// Legacy environment variable still honored when the primary is unset.
    fn env_legacy(self) -> Option<&'static str> {
        match self {
            Self::Targets => Some("RUNE_QUESTS"),
            _ => None,
        }
    }

    fn configured(self, ontology: &Ontology) -> Option<&String> {
        match self {
            Self::Targets => ontology.targets.as_ref().or(ontology.quests.as_ref()),
            Self::Skeleton => ontology.skeleton.as_ref(),
            Self::Owner => ontology.owner.as_ref(),
            Self::Archive => ontology.archive.as_ref(),
            Self::Vault => ontology.vault.as_ref(),
            Self::Work => ontology.work.as_ref(),
            Self::Lore => ontology.lore.as_ref(),
            Self::Mount => ontology.mount.as_ref(),
            Self::Developer => ontology.developer.as_ref(),
            Self::Artifacts => ontology.artifacts.as_ref(),
            Self::Githooks => ontology.githooks.as_ref(),
            Self::Domain => ontology.domain.as_ref(),
        }
    }

    fn is_path(self) -> bool {
        !matches!(self, Self::Owner | Self::Domain)
    }
}

impl ResolvedOntology {
    pub fn fields(&self) -> Vec<ResolvedField> {
        Key::ALL
            .iter()
            .map(|key| {
                let resolved = self.value(*key);
                ResolvedField {
                    key: key.name(),
                    env: key.env(),
                    value: resolved.map(|value| value.value.clone()),
                    source: resolved.map(|value| value.source),
                }
            })
            .collect()
    }

    fn value(&self, key: Key) -> Option<&ResolvedValue> {
        match key {
            Key::Targets => self.targets.as_ref(),
            Key::Skeleton => self.skeleton.as_ref(),
            Key::Owner => self.owner.as_ref(),
            Key::Archive => self.archive.as_ref(),
            Key::Vault => self.vault.as_ref(),
            Key::Work => self.work.as_ref(),
            Key::Lore => self.lore.as_ref(),
            Key::Mount => self.mount.as_ref(),
            Key::Developer => self.developer.as_ref(),
            Key::Artifacts => self.artifacts.as_ref(),
            Key::Githooks => self.githooks.as_ref(),
            Key::Domain => self.domain.as_ref(),
        }
    }
}

/// Return the effective user defaults that ship in this binary.
#[must_use]
pub fn installed_defaults() -> Config {
    Config {
        env: Some("~/.env".to_string()),
        ontology: Ontology {
            targets: Key::Targets.default().map(str::to_string),
            archive: Key::Archive.default().map(str::to_string),
            ..Ontology::default()
        },
        launch: resolve_launch(&Launch::default()),
        ..Config::default()
    }
}

pub fn load() -> Result<ResolvedConfig, Error> {
    let config_dir = config_dir()?;
    load_from_dir_with_env(&config_dir, &|name| std::env::var(name).ok())
}

pub fn config_dir() -> Result<PathBuf, Error> {
    dirs::home_dir()
        .map(|home| home.join(".config/rune"))
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))
}

pub fn env_vars(config: &ResolvedConfig) -> Vec<(String, String)> {
    config
        .ontology
        .fields()
        .into_iter()
        .filter_map(|field| {
            field
                .value
                .map(|value| (field.env.to_string(), value.clone()))
        })
        .collect()
}

#[must_use]
pub fn fields(config: &ResolvedConfig) -> Vec<ResolvedField> {
    let mut fields = vec![ResolvedField {
        key: "deck",
        env: "RUNE_DECK",
        value: config.deck.as_ref().map(|value| value.value.clone()),
        source: config.deck.as_ref().map(|value| value.source),
    }];
    fields.push(ResolvedField {
        key: "env",
        env: "RUNE_ENV",
        value: config.env.as_ref().map(|value| value.value.clone()),
        source: config.env.as_ref().map(|value| value.source),
    });
    fields.push(ResolvedField {
        key: "bench",
        env: "",
        value: (!config.bench.is_empty()).then(|| config.bench.join(", ")),
        source: (!config.bench.is_empty()).then_some(Source::Config),
    });
    fields.extend(config.ontology.fields());
    fields
}

pub fn expand_tilde(value: &str) -> PathBuf {
    if value == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return dirs::home_dir().map_or_else(|| PathBuf::from(value), |home| home.join(rest));
    }
    PathBuf::from(value)
}

fn load_from_dir_with_env(
    config_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<ResolvedConfig, Error> {
    let config = load_raw_config(config_dir)?;
    Ok(resolve_config(&config, env))
}

fn load_raw_config(config_dir: &Path) -> Result<Config, Error> {
    let config_path = config_dir.join("config.yaml");
    match fs::read_to_string(&config_path) {
        Ok(content) => parse_config(&content, &config_path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", config_path.display()),
        )),
    }
}

fn parse_config(content: &str, path: &Path) -> Result<Config, Error> {
    serde_yaml::from_str(content).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("{} is malformed: {error}", path.display()),
        )
        .with_code("config.invalid")
        .with_fix_command("rune config path")
    })
}

fn default_launch(cliproxy: &CliproxyConfig) -> Launch {
    let proxy_url = format!("http://{}:{}", cliproxy.host, cliproxy.port);
    let proxy_environment = |small_model: &str| {
        [
            (
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                ProfileEnvValue::FromEnv {
                    from_env: "CLIPROXY_API_KEY".to_string(),
                },
            ),
            (
                "ANTHROPIC_BASE_URL".to_string(),
                ProfileEnvValue::Literal(proxy_url.clone()),
            ),
            (
                "ANTHROPIC_SMALL_FAST_MODEL".to_string(),
                ProfileEnvValue::Literal(small_model.to_string()),
            ),
        ]
        .into_iter()
        .collect()
    };
    let claude_profiles = [
        (
            "grok".to_string(),
            LaunchProfile {
                model: Some("grok".to_string()),
                env: proxy_environment("grok-composer-2.5-fast"),
                with: vec!["cliproxy".to_string()],
                ..LaunchProfile::default()
            },
        ),
        (
            "sol".to_string(),
            LaunchProfile {
                model: Some("sol".to_string()),
                env: proxy_environment("gpt-5.6-luna"),
                with: vec!["cliproxy".to_string()],
                ..LaunchProfile::default()
            },
        ),
    ]
    .into_iter()
    .collect();

    Launch {
        profiles: [("claude".to_string(), claude_profiles)]
            .into_iter()
            .collect(),
        ..Launch::default()
    }
}

/// Overlay user launch configuration on Rune's built-in profiles and routes.
#[must_use]
pub fn resolve_launch(configured: &Launch) -> Launch {
    let mut resolved = default_launch(&configured.middleware.cliproxy);
    resolved.default_with.clone_from(&configured.default_with);
    resolved.tools.clone_from(&configured.tools);
    resolved.middleware.clone_from(&configured.middleware);
    resolved.models.extend(configured.models.clone());
    for (tool, configured_profiles) in &configured.profiles {
        resolved
            .profiles
            .entry(tool.clone())
            .or_default()
            .extend(configured_profiles.clone());
    }
    resolved
}

fn resolve_config(config: &Config, env: &dyn Fn(&str) -> Option<String>) -> ResolvedConfig {
    let deck = env("RUNE_DECK")
        .map(|value| ResolvedValue {
            value,
            source: Source::Env,
        })
        .or_else(|| {
            config.deck.as_ref().map(|value| ResolvedValue {
                value: value.clone(),
                source: Source::Config,
            })
        });
    let env_file = env("RUNE_ENV")
        .map(|value| ResolvedValue {
            value,
            source: Source::Env,
        })
        .or_else(|| {
            config.env.as_ref().map(|value| ResolvedValue {
                value: value.clone(),
                source: Source::Config,
            })
        });
    let ontology = ResolvedOntology {
        targets: resolve_key(Key::Targets, &config.ontology, env),
        skeleton: resolve_key(Key::Skeleton, &config.ontology, env),
        owner: resolve_key(Key::Owner, &config.ontology, env),
        archive: resolve_key(Key::Archive, &config.ontology, env),
        vault: resolve_key(Key::Vault, &config.ontology, env),
        work: resolve_key(Key::Work, &config.ontology, env),
        lore: resolve_key(Key::Lore, &config.ontology, env),
        mount: resolve_key(Key::Mount, &config.ontology, env),
        developer: resolve_key(Key::Developer, &config.ontology, env),
        artifacts: resolve_key(Key::Artifacts, &config.ontology, env),
        githooks: resolve_key(Key::Githooks, &config.ontology, env),
        domain: resolve_key(Key::Domain, &config.ontology, env),
    };
    let extensions = config
        .extensions
        .iter()
        .map(|extension| expand_tilde(extension))
        .collect();
    ResolvedConfig {
        deck,
        env: env_file,
        ontology,
        extensions,
        setup: config.setup.clone(),
        launch: resolve_launch(&config.launch),
        bench: config.bench.clone(),
        theme: config.theme.clone(),
    }
}

fn resolve_key(
    key: Key,
    ontology: &Ontology,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<ResolvedValue> {
    if let Some(value) = env(key.env()).or_else(|| key.env_legacy().and_then(env)) {
        return Some(resolved_value(key, value, Source::Env));
    }
    if let Some(value) = key.configured(ontology) {
        return Some(resolved_value(key, value.clone(), Source::Config));
    }
    key.default()
        .map(|value| resolved_value(key, value.to_string(), Source::Default))
}

fn resolved_value(key: Key, value: String, source: Source) -> ResolvedValue {
    let value = if key.is_path() {
        expand_tilde(&value).to_string_lossy().to_string()
    } else {
        value
    };
    ResolvedValue { value, source }
}

#[cfg(test)]
mod tests;
