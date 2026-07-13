use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::module::ModuleManifest;

#[derive(Debug, Deserialize)]
pub struct DeckManifest {
    pub schema: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub providers: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct DomainDefaults {
    providers: Option<BTreeMap<String, serde_yaml::Value>>,
}

#[derive(Debug)]
pub struct Domain {
    pub name: String,
    pub root: PathBuf,
    pub manifest: ModuleManifest,
    providers: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct Deck {
    pub root: PathBuf,
    pub manifest: DeckManifest,
    pub domains: Vec<Domain>,
    pub warnings: Vec<String>,
    casts: BTreeMap<String, Cast>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cast {
    name: String,
    #[serde(rename = "description")]
    _description: String,
    #[serde(default)]
    extends: Vec<String>,
    runes: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default, rename = "plugin")]
    _plugin: Option<serde_yaml::Value>,
}

impl Deck {
    pub fn providers_for<'a>(&'a self, domain: &'a Domain) -> Option<&'a [String]> {
        domain
            .providers
            .as_deref()
            .or(self.manifest.providers.as_deref())
    }

    pub fn resolve_cast<'a>(
        &self,
        name: &str,
        artifact_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<Vec<String>, String> {
        let artifacts = artifact_ids
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let mut stack = Vec::new();
        let selected = self.resolve_cast_inner(name, &artifacts, &mut stack)?;
        let mut ordered = selected.into_iter().collect::<Vec<_>>();
        ordered.sort_by_key(|id| artifact_order_key(id));
        Ok(ordered)
    }

    fn resolve_cast_inner(
        &self,
        name: &str,
        artifacts: &BTreeSet<String>,
        stack: &mut Vec<String>,
    ) -> Result<BTreeSet<String>, String> {
        if let Some(index) = stack.iter().position(|item| item == name) {
            let mut cycle = stack[index..].to_vec();
            cycle.push(name.to_string());
            return Err(format!("cast extension cycle: {}", cycle.join(" -> ")));
        }
        let cast = self
            .casts
            .get(name)
            .ok_or_else(|| format!("unknown cast '{name}'"))?;
        stack.push(name.to_string());
        let mut selected = BTreeSet::new();
        for parent in &cast.extends {
            selected.extend(self.resolve_cast_inner(parent, artifacts, stack)?);
        }
        for pattern in &cast.runes {
            let matched = artifacts
                .iter()
                .filter(|id| matches_artifact_glob(pattern, id))
                .cloned()
                .collect::<Vec<_>>();
            if matched.is_empty() {
                return Err(format!(
                    "cast '{}' rune pattern '{pattern}' matches no artifact",
                    cast.name
                ));
            }
            selected.extend(matched);
        }
        for pattern in &cast.exclude {
            selected.retain(|id| !matches_artifact_glob(pattern, id));
        }
        let popped = stack.pop();
        debug_assert_eq!(popped.as_deref(), Some(name));
        Ok(selected)
    }
}

pub fn is_deck(root: &Path) -> bool {
    root.join("deck.yaml").is_file()
}

pub fn load(root: &Path) -> Result<Deck, String> {
    let deck_yaml = root.join("deck.yaml");
    let content = std::fs::read_to_string(&deck_yaml)
        .map_err(|error| format!("cannot read {}: {error}", deck_yaml.display()))?;
    let value: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|error| format!("invalid deck.yaml: {error}"))?;
    let found_schema = value.get("schema");
    if found_schema.and_then(serde_yaml::Value::as_u64) != Some(1) {
        let found = found_schema.map_or_else(
            || "missing".to_string(),
            |value| {
                serde_yaml::to_string(value)
                    .unwrap_or_else(|_| format!("{value:?}"))
                    .trim()
                    .to_string()
            },
        );
        return Err(format!(
            "unsupported deck schema: found {found}; supported schema is 1"
        ));
    }
    let manifest: DeckManifest =
        serde_yaml::from_value(value).map_err(|error| format!("invalid deck.yaml: {error}"))?;
    let casts = load_casts(root)?;

    let runes_root = root.join("runes");
    let mut entries = if runes_root.is_dir() {
        std::fs::read_dir(&runes_root)
            .map_err(|error| format!("cannot read {}: {error}", runes_root.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("cannot read {}: {error}", runes_root.display()))?
    } else {
        Vec::new()
    };
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut domains = Vec::new();
    let mut warnings = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let domain_root = entry.path();
        let module_yaml = domain_root.join("module.yaml");
        if !domain_root.is_dir() || !module_yaml.is_file() {
            let warning = format!(
                "warning: skipping deck entry {} without module.yaml",
                domain_root.display()
            );
            eprintln!("{warning}");
            warnings.push(warning);
            continue;
        }

        let domain_manifest = crate::module::load(&domain_root)?;
        if domain_manifest.name != name {
            return Err(format!(
                "deck domain directory '{name}' does not match module.yaml name '{}'",
                domain_manifest.name
            ));
        }
        let providers = domain_manifest
            .providers
            .clone()
            .or(load_default_provider_names(&domain_root)?);
        domains.push(Domain {
            name,
            root: domain_root,
            manifest: domain_manifest,
            providers,
        });
    }

    Ok(Deck {
        root: root.to_path_buf(),
        manifest,
        domains,
        warnings,
        casts,
    })
}

fn load_casts(root: &Path) -> Result<BTreeMap<String, Cast>, String> {
    let casts_root = root.join("casts");
    if !casts_root.is_dir() {
        return Ok(BTreeMap::new());
    }
    let mut paths = std::fs::read_dir(&casts_root)
        .map_err(|error| format!("cannot read {}: {error}", casts_root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot read {}: {error}", casts_root.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut casts = BTreeMap::new();
    for path in paths {
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let cast: Cast = serde_yaml::from_str(&content)
            .map_err(|error| format!("invalid {}: {error}", path.display()))?;
        let name = cast.name.clone();
        if casts.insert(name.clone(), cast).is_some() {
            return Err(format!("duplicate cast name '{name}'"));
        }
    }
    Ok(casts)
}

fn artifact_order_key(id: &str) -> (String, u8, String, String) {
    let mut parts = id.splitn(4, '/');
    let domain = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let remainder = parts.next().unwrap_or_default();
    let kind_order = match kind {
        "skills" => 0,
        "agents" => 1,
        "rules" => 2,
        "hooks" => 3,
        _ => 4,
    };
    (
        domain.to_string(),
        kind_order,
        name.to_string(),
        remainder.to_string(),
    )
}

pub fn matches_artifact_glob(pattern: &str, value: &str) -> bool {
    fn matches(pattern: &[u8], value: &[u8], memo: &mut BTreeMap<(usize, usize), bool>) -> bool {
        let key = (pattern.len(), value.len());
        if let Some(result) = memo.get(&key) {
            return *result;
        }
        let result = match pattern {
            [] => value.is_empty(),
            [b'*', b'*', rest @ ..] => {
                matches(rest, value, memo)
                    || (!value.is_empty() && matches(pattern, &value[1..], memo))
            }
            [b'*', rest @ ..] => {
                matches(rest, value, memo)
                    || (!value.is_empty()
                        && value[0] != b'/'
                        && matches(pattern, &value[1..], memo))
            }
            [b'?', rest @ ..] => {
                !value.is_empty() && value[0] != b'/' && matches(rest, &value[1..], memo)
            }
            [literal, rest @ ..] => {
                value.first() == Some(literal) && matches(rest, &value[1..], memo)
            }
        };
        memo.insert(key, result);
        result
    }

    matches(pattern.as_bytes(), value.as_bytes(), &mut BTreeMap::new())
}

fn load_default_provider_names(domain_root: &Path) -> Result<Option<Vec<String>>, String> {
    let defaults_yaml = domain_root.join("defaults.yaml");
    if !defaults_yaml.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&defaults_yaml)
        .map_err(|error| format!("cannot read {}: {error}", defaults_yaml.display()))?;
    if content.trim().is_empty() {
        return Ok(None);
    }
    let defaults: DomainDefaults = serde_yaml::from_str(&content)
        .map_err(|error| format!("invalid {}: {error}", defaults_yaml.display()))?;
    Ok(defaults
        .providers
        .map(|providers| providers.into_keys().collect()))
}

#[cfg(test)]
mod tests;
