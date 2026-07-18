use serde::Deserialize;
use std::collections::HashMap;

// --- Content Kind ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentKind {
    Agents,
    Skills,
    Rules,
    Hooks,
}

impl ContentKind {
    pub const ALL: &[ContentKind] = &[Self::Agents, Self::Skills, Self::Rules];
    pub const DECK_ALL: &[ContentKind] = &[Self::Skills, Self::Agents, Self::Rules, Self::Hooks];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agents => "agents",
            Self::Skills => "skills",
            Self::Rules => "rules",
            Self::Hooks => "hooks",
        }
    }
}

impl std::fmt::Display for ContentKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

// --- Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyRule {
    KebabCase,
    KebabCaseAgents,
    RemapTools,
    AgentsToToml,
    StripLinks,
}

impl AssemblyRule {
    pub fn from_name(name: &str) -> Result<Self, String> {
        match name {
            "kebab-case" => Ok(Self::KebabCase),
            "kebab-case-agents" => Ok(Self::KebabCaseAgents),
            "remap-tools" => Ok(Self::RemapTools),
            "agents-to-toml" => Ok(Self::AgentsToToml),
            "strip-links" => Ok(Self::StripLinks),
            other => Err(format!("unknown assembly rule: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub target: ProviderTarget,
    pub assembly: Option<Vec<String>>,
    pub deploy: Option<Vec<String>>,
    pub keep_fields: Option<HashMap<String, Vec<String>>>,
    pub models: Option<HashMap<String, Vec<String>>>,
    pub effort: Option<HashMap<String, String>>,
    pub aliases: Option<Vec<String>>,
    /// Default target model ID for this provider (an exact ID from
    /// `config/models.yaml`). Selects which `provider/<model>/` variant
    /// directory wins during assembly; `--model` overrides it.
    pub model: Option<String>,
    /// Deploy skills, agents, and hooks as a skills-directory plugin of
    /// this name (`<target>/skills/<plugin>/…` with a `.claude-plugin/`
    /// manifest), so the harness namespaces them as `<plugin>:<skill>`.
    /// Rules keep their loose path. Absent means the loose layout.
    pub plugin: Option<String>,
    /// Opt-in providers (`enabled: false`) deploy only when named
    /// explicitly with `--provider`; the default set skips them.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl ProviderConfig {
    pub fn default_target(&self) -> &str {
        self.target.default_target()
    }

    pub fn target_for_kind(&self, kind: ContentKind) -> &str {
        self.target.target_for_kind(kind)
    }

    pub fn target_roots(&self) -> Vec<&str> {
        self.target.roots()
    }

    pub fn matches_target(&self, target_name: &str, provider_key: &str) -> bool {
        if target_name == provider_key {
            return true;
        }

        if self
            .target_roots()
            .iter()
            .any(|target| target_name == *target || target_name == target.trim_start_matches('.'))
        {
            return true;
        }

        self.aliases
            .as_ref()
            .is_some_and(|aliases| aliases.iter().any(|alias| alias == target_name))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ProviderTarget {
    Single(String),
    ByKind(ProviderTargetMap),
}

impl ProviderTarget {
    pub fn default_target(&self) -> &str {
        match self {
            Self::Single(target) => target,
            Self::ByKind(targets) => &targets.default,
        }
    }

    pub fn target_for_kind(&self, kind: ContentKind) -> &str {
        match self {
            Self::Single(target) => target,
            Self::ByKind(targets) => match kind {
                ContentKind::Agents => targets.agents.as_deref().unwrap_or(&targets.default),
                ContentKind::Skills => targets.skills.as_deref().unwrap_or(&targets.default),
                ContentKind::Rules => targets.rules.as_deref().unwrap_or(&targets.default),
                ContentKind::Hooks => &targets.default,
            },
        }
    }

    pub fn roots(&self) -> Vec<&str> {
        let mut roots = vec![self.default_target()];
        if let Self::ByKind(targets) = self {
            for target in [&targets.agents, &targets.skills, &targets.rules]
                .into_iter()
                .flatten()
            {
                if !roots.contains(&target.as_str()) {
                    roots.push(target.as_str());
                }
            }
        }
        roots
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderTargetMap {
    pub default: String,
    pub agents: Option<String>,
    pub skills: Option<String>,
    pub rules: Option<String>,
}

#[cfg(test)]
mod plugin_layout_tests {
    use super::*;

    #[test]
    fn plugin_derives_the_skills_directory_layout() {
        let source = "providers:\n    claude:\n        target: .claude\n        plugin: rune\n";
        let providers = load_providers(source).unwrap();
        let claude = &providers["claude"];
        assert_eq!(claude.default_target(), ".claude/skills/rune");
        assert_eq!(
            claude.target_for_kind(ContentKind::Skills),
            ".claude/skills/rune"
        );
        assert_eq!(
            claude.target_for_kind(ContentKind::Hooks),
            ".claude/skills/rune"
        );
        assert_eq!(claude.target_for_kind(ContentKind::Rules), ".claude");
    }

    #[test]
    fn plugin_with_a_by_kind_target_map_is_a_config_error() {
        let source = "providers:\n    claude:\n        plugin: rune\n        target:\n            default: .claude\n            skills: .custom\n";
        let error = load_providers(source).unwrap_err();
        assert!(error.contains("plugin: null"), "actionable error: {error}");
    }
}

// --- Loading ---

#[derive(Deserialize)]
struct Wrapper {
    providers: HashMap<String, ProviderConfig>,
}

pub fn load_providers(defaults_content: &str) -> Result<HashMap<String, ProviderConfig>, String> {
    let wrapper: Wrapper = parse_yaml(defaults_content, "providers")?;
    let mut providers = wrapper.providers;
    for (name, provider) in &mut providers {
        apply_plugin_layout(name, provider)?;
    }
    Ok(providers)
}

/// Rewrite the target map of a plugin-mode provider: skills, agents, and
/// hooks land under the plugin root (`<target>/skills/<plugin>`), rules keep
/// their configured loose path. `plugin` combines only with a `Single`
/// target; pairing it with an explicit by-kind map is a config error, since
/// the two disagree about where every kind lives (set `plugin: null` to keep
/// a custom map).
fn apply_plugin_layout(provider_name: &str, provider: &mut ProviderConfig) -> Result<(), String> {
    let Some(plugin) = &provider.plugin else {
        return Ok(());
    };
    if matches!(provider.target, ProviderTarget::ByKind(_)) {
        return Err(format!(
            "provider '{provider_name}': `plugin: {plugin}` cannot combine with a by-kind target map; set `plugin: null` to use the custom map, or a single `target:` to use the plugin layout"
        ));
    }
    let base = provider.default_target().trim_end_matches('/');
    let plugin_root = format!("{base}/skills/{plugin}");
    let rules = provider.target_for_kind(ContentKind::Rules).to_string();
    provider.target = ProviderTarget::ByKind(ProviderTargetMap {
        default: plugin_root,
        agents: None,
        skills: None,
        rules: Some(rules),
    });
    Ok(())
}

pub fn load_models(models_content: &str) -> Result<HashMap<String, Vec<String>>, String> {
    parse_yaml(models_content, "models")
}

/// Load tool name mappings for a specific provider from remap-tools YAML.
///
/// The YAML file is structured as:
///
/// ```yaml
/// gemini:
///     Read: read_file
///     Write: write_file
/// ```
///
/// Returns the mapping for the given provider, or an empty map if the
/// provider has no entry.
pub fn load_tool_mappings(
    remap_content: &str,
    provider_name: &str,
) -> Result<HashMap<String, String>, String> {
    let parsed: HashMap<String, HashMap<String, String>> =
        parse_yaml(remap_content, "remap-tools")?;

    match parsed.get(provider_name) {
        Some(mappings) => Ok(mappings.clone()),
        None => Ok(HashMap::new()),
    }
}

// --- Lookup ---

pub fn map_tool(tool: &str, mappings: &HashMap<String, String>) -> String {
    if let Some(mapped) = mappings.get(tool) {
        return mapped.clone();
    }
    tool.to_string()
}

// --- Validation ---

pub fn validate_qualifier(
    qualifier_name: &str,
    models: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    if qualifier_name == "user" {
        return Ok(());
    }

    if models.contains_key(qualifier_name) {
        return Ok(());
    }

    let is_known_model = models.values().flatten().any(|id| id == qualifier_name);

    if is_known_model {
        return Ok(());
    }

    Err(format!(
        "unknown qualifier '{qualifier_name}': not a provider or model"
    ))
}

// --- Internal ---

fn parse_yaml<T: serde::de::DeserializeOwned>(content: &str, label: &str) -> Result<T, String> {
    match serde_yaml::from_str(content) {
        Ok(parsed) => Ok(parsed),
        Err(err) => Err(format!("failed to parse {label}: {err}")),
    }
}

#[cfg(test)]
mod tests;
