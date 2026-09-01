use super::EMBEDDED_DEFAULTS;
use rune::provider;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub(super) struct SourceConfig {
    pub(super) validate: ValidateConfig,
    pub(super) dashboard: DashboardConfig,
    pub(super) spec: SpecConfig,
    pub(super) adr: AdrConfig,
    pub(super) providers: HashMap<String, provider::ProviderConfig>,
    #[serde(
        default,
        rename = "validate.exclude",
        skip_serializing_if = "Option::is_none"
    )]
    #[allow(dead_code)]
    validate_exclude_flat: Option<StringOrList>,
    #[serde(default, rename = "spec.root", skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    spec_root_flat: Option<StringOrList>,
    #[serde(
        default,
        rename = "adr.prefixes",
        skip_serializing_if = "Option::is_none"
    )]
    #[allow(dead_code)]
    adr_prefixes_flat: Option<StringOrList>,
}

impl SourceConfig {
    pub(super) fn installed_defaults() -> Result<Self, serde_yaml::Error> {
        parse(EMBEDDED_DEFAULTS)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub(super) struct ValidateConfig {
    pub(super) exclude: Option<StringOrList>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub(super) struct DashboardConfig {
    pub(super) settings_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub(super) struct SpecConfig {
    pub(super) root: Option<StringOrList>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(default)]
pub(super) struct AdrConfig {
    pub(super) prefixes: Option<StringOrList>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum StringOrList {
    String(String),
    List(Vec<String>),
}

impl Default for StringOrList {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

impl StringOrList {
    pub(super) fn joined(&self) -> Option<String> {
        match self {
            Self::String(value) => Some(value.clone()),
            Self::List(values) => (!values.is_empty()).then(|| values.join(", ")),
        }
    }

    pub(super) fn items(&self) -> Vec<String> {
        match self {
            Self::String(value) => value.split(", ").map(String::from).collect(),
            Self::List(values) => values.clone(),
        }
    }
}

pub(super) fn parse(content: &str) -> Result<SourceConfig, serde_yaml::Error> {
    serde_yaml::from_str::<Option<SourceConfig>>(content).map(Option::unwrap_or_default)
}

pub(super) fn providers(
    content: &str,
) -> Result<HashMap<String, provider::ProviderConfig>, serde_yaml::Error> {
    parse_section(content, "providers", None)
}

pub(super) fn dashboard(content: &str) -> Result<DashboardConfig, serde_yaml::Error> {
    parse_section(content, "dashboard", None)
}

pub(super) fn spec(content: &str) -> Result<SpecConfig, serde_yaml::Error> {
    parse_section(content, "spec", Some(("spec.root", "root")))
}

pub(super) fn adr(content: &str) -> Result<AdrConfig, serde_yaml::Error> {
    parse_section(content, "adr", Some(("adr.prefixes", "prefixes")))
}

pub(super) fn validate(content: &str) -> Result<ValidateConfig, serde_yaml::Error> {
    parse_section(content, "validate", Some(("validate.exclude", "exclude")))
}

fn parse_section<T>(
    content: &str,
    key: &str,
    flat_key: Option<(&str, &str)>,
) -> Result<T, serde_yaml::Error>
where
    T: DeserializeOwned + Default,
{
    let document =
        serde_yaml::from_str::<Option<serde_yaml::Mapping>>(content)?.unwrap_or_default();
    let section_key = serde_yaml::Value::String(key.to_string());
    let mut section = document
        .get(&section_key)
        .cloned()
        .unwrap_or(serde_yaml::Value::Null);
    if let Some((flat_key, field)) = flat_key {
        let flat_key = serde_yaml::Value::String(flat_key.to_string());
        if let Some(flat_value) = document.get(&flat_key) {
            let mut mapping = section.as_mapping().cloned().unwrap_or_default();
            mapping.insert(
                serde_yaml::Value::String(field.to_string()),
                flat_value.clone(),
            );
            section = serde_yaml::Value::Mapping(mapping);
        }
    }
    serde_yaml::from_value::<Option<T>>(section).map(Option::unwrap_or_default)
}
