use commands::ontology::{self, Source};
use std::fs;
use std::path::Path;

pub fn show(json: bool, no_color: bool) -> Result<i32, String> {
    let config = ontology::load().map_err(|error| error.to_string())?;
    if json {
        let output = serde_json::to_string_pretty(&config)
            .map_err(|error| format!("cannot serialize config: {error}"))?;
        println!("{output}");
        return Ok(0);
    }

    let sheet = crate::cli::style::Sheet::detect(no_color);
    println!("{}", sheet.heading("Config"));
    println!(
        "   {:<12} {:<8} value",
        sheet.dim("key"),
        sheet.dim("source")
    );
    for field in ontology::fields(&config) {
        let source = field.source.map_or("-", format_source);
        let badge = match source {
            "env" => sheet.yellow(&format!("{source:<8}")),
            "config" => sheet.cyan(&format!("{source:<8}")),
            _ => sheet.dim(&format!("{source:<8}")),
        };
        let value = field.value.unwrap_or_default();
        println!("   {:<12} {badge} {value}", field.key);
    }
    Ok(0)
}

const ONTOLOGY_KEYS: [&str; 12] = [
    "targets",
    "skeleton",
    "owner",
    "archive",
    "vault",
    "work",
    "lore",
    "mount",
    "developer",
    "artifacts",
    "githooks",
    "domain",
];

pub fn set(key: &str, value: &str, json: bool) -> Result<i32, String> {
    let config_path = persist(key, value)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "key": key, "value": value, "path": config_path })
        );
    } else {
        println!("set {key} = {value}");
    }
    Ok(0)
}

/// Write one configuration value without printing; returns the config path.
pub fn persist(key: &str, value: &str) -> Result<std::path::PathBuf, String> {
    let nested = ONTOLOGY_KEYS.contains(&key);
    if key != "deck" && !nested {
        return Err(format!(
            "unsupported config key '{key}'; expected: deck, {}",
            ONTOLOGY_KEYS.join(", ")
        ));
    }
    let config_dir = ontology::config_dir().map_err(|error| error.to_string())?;
    let config_path = config_dir.join("config.yaml");
    if nested {
        set_nested_in_file(&config_path, "ontology", key, value)?;
    } else {
        set_in_file(&config_path, key, value)?;
    }
    Ok(config_path)
}

pub fn get(key: &str, json: bool) -> Result<i32, String> {
    let config = ontology::load().map_err(|error| error.to_string())?;
    let field = ontology::fields(&config)
        .into_iter()
        .find(|field| field.key == key)
        .ok_or_else(|| format!("unknown config key '{key}'"))?;
    if json {
        let output = serde_json::to_string_pretty(&field)
            .map_err(|error| format!("cannot serialize config field: {error}"))?;
        println!("{output}");
        return Ok(i32::from(field.value.is_none()));
    }
    match field.value {
        Some(value) => {
            println!("{value}");
            Ok(0)
        }
        None => Ok(1),
    }
}

pub fn path(json: bool) -> Result<i32, String> {
    let config_path = ontology::config_dir()
        .map_err(|error| error.to_string())?
        .join("config.yaml");
    if json {
        println!("{}", serde_json::json!({ "path": config_path }));
    } else {
        println!("{}", config_path.display());
    }
    Ok(0)
}

pub fn unset(key: &str, json: bool) -> Result<i32, String> {
    let nested = ONTOLOGY_KEYS.contains(&key);
    if key != "deck" && !nested {
        return Err(format!(
            "unsupported config key '{key}'; expected: deck, {}",
            ONTOLOGY_KEYS.join(", ")
        ));
    }
    let config_path = ontology::config_dir()
        .map_err(|error| error.to_string())?
        .join("config.yaml");
    let mut removed = if nested {
        remove_nested_in_file(&config_path, "ontology", key)?
    } else {
        remove_in_file(&config_path, key)?
    };
    if key == "targets" {
        // The legacy key still feeds the targets fallback; leaving it would
        // silently resurrect the old value after an unset.
        removed |= remove_nested_in_file(&config_path, "ontology", "quests")?;
    }
    if json {
        println!(
            "{}",
            serde_json::json!({ "key": key, "removed": removed, "path": config_path })
        );
    } else if removed {
        println!("unset {key}");
    } else {
        println!("{key} was not set");
    }
    Ok(0)
}

fn remove_in_file(path: &Path, key: &str) -> Result<bool, String> {
    let mut document = read_config_document(path)?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| format!("{} must contain a YAML mapping", path.display()))?;
    let removed = mapping
        .remove(serde_yaml::Value::String(key.to_string()))
        .is_some();
    if removed {
        write_config_document(path, &document)?;
    }
    Ok(removed)
}

fn remove_nested_in_file(path: &Path, section: &str, key: &str) -> Result<bool, String> {
    let mut document = read_config_document(path)?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| format!("{} must contain a YAML mapping", path.display()))?;
    let section_key = serde_yaml::Value::String(section.to_string());
    let Some(section_value) = mapping.get_mut(&section_key) else {
        return Ok(false);
    };
    let section_mapping = section_value
        .as_mapping_mut()
        .ok_or_else(|| format!("{section} in {} must be a YAML mapping", path.display()))?;
    let removed = section_mapping
        .remove(serde_yaml::Value::String(key.to_string()))
        .is_some();
    if removed {
        if section_mapping.is_empty() {
            mapping.remove(&section_key);
        }
        write_config_document(path, &document)?;
    }
    Ok(removed)
}

fn set_nested_in_file(path: &Path, section: &str, key: &str, value: &str) -> Result<(), String> {
    let mut document = read_config_document(path)?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| format!("{} must contain a YAML mapping", path.display()))?;
    let section_key = serde_yaml::Value::String(section.to_string());
    let section_value = mapping
        .entry(section_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let section_mapping = section_value
        .as_mapping_mut()
        .ok_or_else(|| format!("{section} in {} must be a YAML mapping", path.display()))?;
    section_mapping.insert(
        serde_yaml::Value::String(key.to_string()),
        serde_yaml::Value::String(value.to_string()),
    );
    write_config_document(path, &document)
}

fn set_in_file(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let mut document = read_config_document(path)?;
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| format!("{} must contain a YAML mapping", path.display()))?;
    mapping.insert(
        serde_yaml::Value::String(key.to_string()),
        serde_yaml::Value::String(value.to_string()),
    );
    write_config_document(path, &document)
}

fn read_config_document(path: &Path) -> Result<serde_yaml::Value, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_yaml::from_str::<serde_yaml::Value>(&content)
            .map_err(|error| format!("{} is malformed: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
        }
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

fn write_config_document(path: &Path, document: &serde_yaml::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let content = serde_yaml::to_string(document)
        .map_err(|error| format!("cannot serialize config: {error}"))?;
    fs::write(path, content).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn format_source(source: Source) -> &'static str {
    match source {
        Source::Env => "env",
        Source::Config => "config",
        Source::Default => "default",
    }
}

#[cfg(test)]
mod tests {
    use super::{remove_in_file, remove_nested_in_file, set_in_file, set_nested_in_file};

    #[test]
    fn nested_setter_writes_under_ontology() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.yaml");
        std::fs::write(&path, "deck: /tmp/deck\n").expect("fixture");

        set_nested_in_file(&path, "ontology", "quests", "/tmp/quests").expect("set quests");
        set_nested_in_file(&path, "ontology", "lore", "/tmp/lore").expect("set lore");

        let value: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(path).expect("read updated config"))
                .expect("parse updated config");
        assert_eq!(value["ontology"]["quests"].as_str(), Some("/tmp/quests"));
        assert_eq!(value["ontology"]["lore"].as_str(), Some("/tmp/lore"));
        assert_eq!(value["deck"].as_str(), Some("/tmp/deck"));
    }

    #[test]
    fn setter_preserves_unrelated_yaml() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.yaml");
        std::fs::write(&path, "extensions:\n    - ~/Commands\n").expect("fixture");

        set_in_file(&path, "deck", "/tmp/deck").expect("set deck");

        let value: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(path).expect("read updated config"))
                .expect("parse updated config");
        assert_eq!(value["deck"].as_str(), Some("/tmp/deck"));
        assert_eq!(value["extensions"][0].as_str(), Some("~/Commands"));
    }

    #[test]
    fn remove_deletes_only_the_named_key() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.yaml");
        std::fs::write(&path, "deck: /tmp/deck\nextensions:\n    - ~/Commands\n").expect("fixture");

        assert!(remove_in_file(&path, "deck").expect("remove deck"));
        assert!(!remove_in_file(&path, "deck").expect("second remove is a no-op"));

        let value: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(path).expect("read updated config"))
                .expect("parse updated config");
        assert!(value.get("deck").is_none());
        assert_eq!(value["extensions"][0].as_str(), Some("~/Commands"));
    }

    #[test]
    fn nested_remove_drops_an_emptied_section() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.yaml");
        std::fs::write(
            &path,
            "deck: /tmp/deck\nontology:\n    quests: /tmp/quests\n",
        )
        .expect("fixture");

        assert!(remove_nested_in_file(&path, "ontology", "quests").expect("remove quests"));

        let value: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(path).expect("read updated config"))
                .expect("parse updated config");
        assert!(value.get("ontology").is_none());
        assert_eq!(value["deck"].as_str(), Some("/tmp/deck"));
    }
}
