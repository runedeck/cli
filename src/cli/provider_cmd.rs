//! `rune provider`: inspect and toggle deploy providers for the current
//! source. Listing reads the merged config; enable/disable write the
//! `providers.<name>.enabled` key into the local `config.yaml`.

use rune::error::{Error, ErrorKind};
use std::path::Path;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ProviderAction {
    /// Enable a provider for the default deploy set.
    Enable { name: String },
    /// Disable a provider; `--provider <name>` still selects it explicitly.
    Disable { name: String },
}

pub fn execute(action: Option<ProviderAction>, json: bool) -> Result<i32, Error> {
    match action {
        None => list(json),
        Some(ProviderAction::Enable { name }) => set_enabled(&name, true, json),
        Some(ProviderAction::Disable { name }) => set_enabled(&name, false, json),
    }
}

fn list(json: bool) -> Result<i32, Error> {
    let merged = crate::cli::config::load_merged_config(Path::new("."))?;
    let providers = crate::cli::config::load_providers(&merged)?;
    let mut names: Vec<&String> = providers.keys().collect();
    names.sort();

    if json {
        let rows: Vec<serde_json::Value> = names
            .iter()
            .map(|name| {
                let provider = &providers[*name];
                serde_json::json!({
                    "name": name,
                    "enabled": provider.enabled,
                    "target": provider.default_target(),
                    "plugin": provider.plugin,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "providers": rows }));
        return Ok(0);
    }

    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading("providers"));
    for name in names {
        let provider = &providers[name];
        let state = if provider.enabled {
            sheet.green("enabled")
        } else {
            sheet.dim("disabled")
        };
        let plugin = provider
            .plugin
            .as_ref()
            .map(|plugin| sheet.dim(&format!("  plugin: {plugin}")))
            .unwrap_or_default();
        println!(
            "   {:<12} {state:<10} {}{plugin}",
            sheet.bold(name),
            sheet.dim(provider.default_target()),
        );
    }
    Ok(0)
}

fn set_enabled(name: &str, enabled: bool, json: bool) -> Result<i32, Error> {
    set_enabled_at(Path::new("."), name, enabled, json)
}

fn set_enabled_at(root: &Path, name: &str, enabled: bool, json: bool) -> Result<i32, Error> {
    let merged = crate::cli::config::load_merged_config(root)?;
    let providers = crate::cli::config::load_providers(&merged)?;
    if !providers.contains_key(name) {
        let mut known: Vec<&String> = providers.keys().collect();
        known.sort();
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "Rune does not recognize provider '{name}'. Known providers: {}.",
                known
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with_code("provider.unknown")
        .with_fix_command("rune provider"));
    }

    let config_path = root.join("config.yaml");
    let mut document: serde_yaml::Mapping = if config_path.is_file() {
        let content = crate::cli::config::read_file(&config_path)?;
        serde_yaml::from_str(&content)
            .map_err(|error| Error::new(ErrorKind::Config, format!("config.yaml: {error}")))?
    } else {
        serde_yaml::Mapping::new()
    };

    let providers_key = serde_yaml::Value::from("providers");
    let provider_entry = document
        .entry(providers_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let Some(provider_map) = provider_entry.as_mapping_mut() else {
        return Err(Error::new(
            ErrorKind::Config,
            "config.yaml providers: is not a map".to_string(),
        ));
    };
    let entry = provider_map
        .entry(serde_yaml::Value::from(name))
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let Some(entry_map) = entry.as_mapping_mut() else {
        return Err(Error::new(
            ErrorKind::Config,
            format!("config.yaml providers.{name}: is not a map"),
        ));
    };
    entry_map.insert(
        serde_yaml::Value::from("enabled"),
        serde_yaml::Value::from(enabled),
    );

    let mut rendered = serde_yaml::to_string(&document)
        .map_err(|error| Error::new(ErrorKind::Config, format!("cannot render config: {error}")))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    crate::cli::config::write_atomic(&config_path, &rendered)?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "provider": name, "enabled": enabled })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        let state = if enabled { "enabled" } else { "disabled" };
        println!("{}", sheet.ok(&format!("{name} {state} in ./config.yaml")));
    }
    Ok(0)
}

#[cfg(test)]
mod tests;
