use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::module::ModuleManifest;

#[derive(Debug, Deserialize)]
pub struct DeckManifest {
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
}

impl Deck {
    pub fn providers_for<'a>(&'a self, domain: &'a Domain) -> Option<&'a [String]> {
        domain
            .providers
            .as_deref()
            .or(self.manifest.providers.as_deref())
    }
}

pub fn is_deck(root: &Path) -> bool {
    root.join("deck.yaml").is_file()
}

pub fn load(root: &Path) -> Result<Deck, String> {
    let deck_yaml = root.join("deck.yaml");
    let content = std::fs::read_to_string(&deck_yaml)
        .map_err(|error| format!("cannot read {}: {error}", deck_yaml.display()))?;
    let manifest: DeckManifest =
        serde_yaml::from_str(&content).map_err(|error| format!("invalid deck.yaml: {error}"))?;

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

        let name = entry.file_name().to_string_lossy().into_owned();
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
    })
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
