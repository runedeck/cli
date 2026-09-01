use rune::error::Error;
use rune::ontology::{self, Source};
use std::fs;
use std::path::Path;

const CONFIG_FIX_COMMAND: &str = "rune config path";
const CONFIG_INVALID_CODE: &str = "config.invalid";
const CONFIG_UNKNOWN_KEY_CODE: &str = "config.unknown_key";

pub fn show(json: bool, no_color: bool) -> Result<i32, Error> {
    let config = ontology::load()?;
    if json {
        let output = serde_json::to_string_pretty(&config)
            .map_err(|error| Error::config(format!("cannot serialize config: {error}")))?;
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

pub fn set(key: &str, value: &str, json: bool) -> Result<i32, Error> {
    let config_path = persist_structured(key, value)?;
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

const SCALAR_KEYS: [&str; 2] = ["deck", "env"];

fn unsupported_key(key: &str) -> String {
    format!(
        "Rune does not support config key '{key}'. Use one of these keys: {}, bench, {}",
        SCALAR_KEYS.join(", "),
        ONTOLOGY_KEYS.join(", ")
    )
}

fn unknown_key_error(message: impl Into<String>) -> Error {
    Error::config(message)
        .with_code(CONFIG_UNKNOWN_KEY_CODE)
        .with_fix_command("rune config")
}

fn invalid_config_error(message: impl Into<String>) -> Error {
    Error::config(message)
        .with_code(CONFIG_INVALID_CODE)
        .with_fix_command(CONFIG_FIX_COMMAND)
}

fn supported_key(key: &str) -> bool {
    SCALAR_KEYS.contains(&key) || key == "bench" || ONTOLOGY_KEYS.contains(&key)
}

/// Write one configuration value without printing; returns the config path.
/// `bench` holds a list of workspace checkouts: set appends (first entry is
/// the primary), unset removes the whole list.
pub fn persist(key: &str, value: &str) -> Result<std::path::PathBuf, Error> {
    persist_structured(key, value)
}

pub fn persist_setup(record: &rune::ontology::SetupRecord) -> Result<std::path::PathBuf, Error> {
    let config_path = ontology::config_dir()?.join("config.yaml");
    let mut document =
        read_config_document(&config_path).map_err(|error| setup_record_error(&error))?;
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!(
            "{} must contain a YAML mapping",
            config_path.display()
        ))
    })?;
    let value = serde_yaml::to_value(record).map_err(|error| {
        Error::config(format!("cannot serialize the setup record: {error}"))
            .with_code("setup.record_invalid")
            .with_fix_command("rune setup --yes")
    })?;
    mapping.insert(serde_yaml::Value::from("setup"), value);
    write_config_document(&config_path, &document).map_err(|error| setup_record_error(&error))?;
    Ok(config_path)
}

fn setup_record_error(error: &Error) -> Error {
    Error::new(error.kind(), error.message().to_string())
        .with_code("setup.record_write_failed")
        .with_fix_command("rune setup --yes")
}

fn persist_structured(key: &str, value: &str) -> Result<std::path::PathBuf, Error> {
    let nested = ONTOLOGY_KEYS.contains(&key);
    if !supported_key(key) {
        return Err(unknown_key_error(unsupported_key(key)));
    }
    let config_dir = ontology::config_dir()?;
    let config_path = config_dir.join("config.yaml");
    if key == "bench" {
        append_to_list_in_file(&config_path, "bench", value)?;
    } else if nested {
        set_nested_in_file_structured(&config_path, "ontology", key, value)?;
    } else {
        set_in_file(&config_path, key, value)?;
    }
    Ok(config_path)
}

pub fn get(key: &str, json: bool) -> Result<i32, Error> {
    if !supported_key(key) {
        return Err(unknown_key_error(format!(
            "Rune does not recognize config key '{key}'."
        )));
    }
    let config = ontology::load()?;
    let field = ontology::fields(&config)
        .into_iter()
        .find(|field| field.key == key)
        .ok_or_else(|| unknown_key_error(format!("Rune does not recognize config key '{key}'.")))?;
    if json {
        let output = serde_json::to_string_pretty(&field)
            .map_err(|error| Error::config(format!("cannot serialize config field: {error}")))?;
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

pub fn path(json: bool) -> Result<i32, Error> {
    let config_path = ontology::config_dir()?.join("config.yaml");
    if json {
        println!("{}", serde_json::json!({ "path": config_path }));
    } else {
        println!("{}", config_path.display());
    }
    Ok(0)
}

pub fn unset(key: &str, json: bool) -> Result<i32, Error> {
    let nested = ONTOLOGY_KEYS.contains(&key);
    if !supported_key(key) {
        return Err(unknown_key_error(unsupported_key(key)));
    }
    let config_path = ontology::config_dir()?.join("config.yaml");
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

fn remove_in_file(path: &Path, key: &str) -> Result<bool, Error> {
    let mut document = read_config_document(path)?;
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!("{} must contain a YAML mapping", path.display()))
    })?;
    let removed = mapping
        .remove(serde_yaml::Value::String(key.to_string()))
        .is_some();
    if removed {
        write_config_document(path, &document)?;
    }
    Ok(removed)
}

fn remove_nested_in_file(path: &Path, section: &str, key: &str) -> Result<bool, Error> {
    let mut document = read_config_document(path)?;
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!("{} must contain a YAML mapping", path.display()))
    })?;
    let section_key = serde_yaml::Value::String(section.to_string());
    let Some(section_value) = mapping.get_mut(&section_key) else {
        return Ok(false);
    };
    let section_mapping = section_value.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!(
            "{section} in {} must be a YAML mapping",
            path.display()
        ))
    })?;
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

pub(crate) fn set_nested_in_file(
    path: &Path,
    section: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    set_nested_in_file_structured(path, section, key, value)
        .map_err(|error| error.message().to_string())
}

fn set_nested_in_file_structured(
    path: &Path,
    section: &str,
    key: &str,
    value: &str,
) -> Result<(), Error> {
    let mut document = read_config_document(path)?;
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!("{} must contain a YAML mapping", path.display()))
    })?;
    let section_key = serde_yaml::Value::String(section.to_string());
    let section_value = mapping
        .entry(section_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let section_mapping = section_value.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!(
            "{section} in {} must be a YAML mapping",
            path.display()
        ))
    })?;
    section_mapping.insert(
        serde_yaml::Value::String(key.to_string()),
        serde_yaml::Value::String(value.to_string()),
    );
    write_config_document(path, &document)
}

fn append_to_list_in_file(path: &Path, key: &str, value: &str) -> Result<(), Error> {
    let mut document = read_config_document(path)?;
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!("{} must contain a YAML mapping", path.display()))
    })?;
    let list_key = serde_yaml::Value::String(key.to_string());
    let list_value = mapping
        .entry(list_key)
        .or_insert_with(|| serde_yaml::Value::Sequence(Vec::new()));
    let sequence = list_value.as_sequence_mut().ok_or_else(|| {
        invalid_config_error(format!("{key} in {} must be a YAML list", path.display()))
    })?;
    let entry = serde_yaml::Value::String(value.to_string());
    if !sequence.contains(&entry) {
        sequence.push(entry);
    }
    write_config_document(path, &document)
}

fn set_in_file(path: &Path, key: &str, value: &str) -> Result<(), Error> {
    let mut document = read_config_document(path)?;
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        invalid_config_error(format!("{} must contain a YAML mapping", path.display()))
    })?;
    mapping.insert(
        serde_yaml::Value::String(key.to_string()),
        serde_yaml::Value::String(value.to_string()),
    );
    write_config_document(path, &document)
}

fn read_config_document(path: &Path) -> Result<serde_yaml::Value, Error> {
    match fs::read_to_string(path) {
        Ok(content) => serde_yaml::from_str::<serde_yaml::Value>(&content).map_err(|error| {
            invalid_config_error(format!("{} is malformed: {error}", path.display()))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
        }
        Err(error) => Err(Error::io(format!(
            "cannot read {}: {error}",
            path.display()
        ))),
    }
}

fn write_config_document(path: &Path, document: &serde_yaml::Value) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| Error::io(format!("cannot create {}: {error}", parent.display())))?;
    }
    let content = serde_yaml::to_string(document)
        .map_err(|error| Error::config(format!("cannot serialize config: {error}")))?;
    fs::write(path, content)
        .map_err(|error| Error::io(format!("cannot write {}: {error}", path.display())))
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
    use super::{
        append_to_list_in_file, get, read_config_document, remove_in_file, remove_nested_in_file,
        set, set_in_file, set_nested_in_file, set_nested_in_file_structured, unset,
    };

    fn assert_unknown_key(error: &rune::error::Error) {
        assert_eq!(error.kind(), rune::error::ErrorKind::Config);
        assert_eq!(error.code(), "config.unknown_key");
        assert_eq!(error.fix_command(), Some("rune config"));
    }

    fn assert_invalid_config(error: &rune::error::Error) {
        assert_eq!(error.kind(), rune::error::ErrorKind::Config);
        assert_eq!(error.code(), "config.invalid");
        assert_eq!(error.fix_command(), Some("rune config path"));
    }

    #[test]
    fn unknown_key_commands_have_stable_repairs() {
        let first = get("not-a-key", false).expect_err("key must be unknown");
        let second = get("not-a-key", false).expect_err("key must stay unknown");
        assert_unknown_key(&first);
        assert_unknown_key(&second);
        assert_eq!(first.code(), second.code());
        assert_eq!(first.message(), second.message());

        let set_error = set("not-a-key", "value", false).expect_err("set must fail");
        let unset_error = unset("not-a-key", false).expect_err("unset must fail");
        assert_unknown_key(&set_error);
        assert_unknown_key(&unset_error);
    }

    #[test]
    fn malformed_config_has_a_repair_command() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.yaml");
        std::fs::write(&path, "ontology: [\n").expect("fixture");

        let error = set_in_file(&path, "deck", "/tmp/deck").expect_err("config must fail");

        assert_invalid_config(&error);
        assert!(
            error
                .message()
                .starts_with(&format!("{} is malformed:", path.display()))
        );
    }

    #[test]
    fn invalid_config_shapes_have_a_repair_command() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root_path = directory.path().join("root.yaml");
        std::fs::write(&root_path, "- item\n").expect("root fixture");
        let root_error =
            set_in_file(&root_path, "deck", "/tmp/deck").expect_err("root must be a mapping");
        assert_invalid_config(&root_error);
        assert_eq!(
            root_error.message(),
            format!("{} must contain a YAML mapping", root_path.display())
        );

        let nested_path = directory.path().join("nested.yaml");
        std::fs::write(&nested_path, "ontology: []\n").expect("nested fixture");
        let nested_error =
            set_nested_in_file_structured(&nested_path, "ontology", "lore", "/tmp/lore")
                .expect_err("ontology must be a mapping");
        assert_invalid_config(&nested_error);
        assert_eq!(
            nested_error.message(),
            format!(
                "ontology in {} must be a YAML mapping",
                nested_path.display()
            )
        );

        let list_path = directory.path().join("list.yaml");
        std::fs::write(&list_path, "bench: /tmp/bench\n").expect("list fixture");
        let list_error = append_to_list_in_file(&list_path, "bench", "/tmp/other")
            .expect_err("bench must be a list");
        assert_invalid_config(&list_error);
        assert_eq!(
            list_error.message(),
            format!("bench in {} must be a YAML list", list_path.display())
        );
    }

    #[test]
    fn config_read_error_has_no_repair_command() {
        let directory = tempfile::tempdir().expect("tempdir");
        let parent_file = directory.path().join("not-a-directory");
        std::fs::write(&parent_file, "fixture").expect("parent fixture");
        let path = parent_file.join("config.yaml");

        let error = read_config_document(&path).expect_err("read must fail");

        assert_eq!(error.kind(), rune::error::ErrorKind::Io);
        assert_eq!(error.code(), "error.io");
        assert_eq!(error.fix_command(), None);
        assert!(error.message().starts_with("cannot read "));
    }

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
