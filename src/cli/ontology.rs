use commands::ontology::{self, Source};
use std::fs;
use std::path::Path;

pub fn show(json: bool) -> Result<i32, String> {
    let config = ontology::load().map_err(|error| error.to_string())?;
    if json {
        let output = serde_json::to_string_pretty(&config)
            .map_err(|error| format!("cannot serialize config: {error}"))?;
        println!("{output}");
        return Ok(0);
    }

    println!("{:<12} {:<8} value", "key", "source");
    for field in ontology::fields(&config) {
        let source = field.source.map_or("-", format_source);
        let value = field.value.unwrap_or_default();
        println!("{:<12} {:<8} {value}", field.key, source);
    }
    Ok(0)
}

pub fn set(key: &str, value: &str, json: bool) -> Result<i32, String> {
    if key != "deck" {
        return Err(format!("unsupported config key '{key}'; expected: deck"));
    }
    let config_dir = ontology::config_dir().map_err(|error| error.to_string())?;
    let config_path = config_dir.join("config.yaml");
    set_in_file(&config_path, key, value)?;
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

fn set_in_file(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let mut document = match fs::read_to_string(path) {
        Ok(content) => serde_yaml::from_str::<serde_yaml::Value>(&content)
            .map_err(|error| format!("{} is malformed: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
        }
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| format!("{} must contain a YAML mapping", path.display()))?;
    mapping.insert(
        serde_yaml::Value::String(key.to_string()),
        serde_yaml::Value::String(value.to_string()),
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let content = serde_yaml::to_string(&document)
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
    use super::set_in_file;

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
}
