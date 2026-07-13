//! Bind a quest — the repo being worked on — so rune commands can find its
//! consumer manifest from anywhere. The binding lives in
//! `~/.config/rune/state.yaml`, separate from `config.yaml` because the
//! config file denies unknown fields and the binding is mutable session
//! state, not configuration.

use commands::error::{Error, ErrorKind};
use commands::ontology;
use std::path::{Path, PathBuf};

const HISTORY_LIMIT: usize = 10;

#[derive(Debug, Default)]
struct State {
    quest: Option<String>,
    quests: Vec<String>,
}

pub fn execute(quest: Option<&str>, clone: bool, unbind: bool, list: bool) -> Result<i32, Error> {
    let state_path = state_path()?;
    if list {
        return list_quests(&state_path);
    }
    if unbind {
        return unbind_quest(&state_path);
    }
    let Some(requested) = quest else {
        return Ok(show_binding(&state_path));
    };
    let resolved = if requested == "-" {
        previous_quest(&state_path)?
    } else {
        resolve_quest(requested, clone)?
    };
    write_binding(&state_path, &resolved)?;
    println!("updated {}", state_path.display());
    println!("quest bound: {}", resolved.display());
    if !resolved.join(".rune").is_file() {
        println!("note: no .rune manifest yet; `rune add <deck-or-rune>` creates it");
    }
    Ok(0)
}

pub(crate) fn bind_existing(quest: &Path) -> Result<(), Error> {
    let resolved = std::fs::canonicalize(quest).map_err(|error| io_error(quest, "read", &error))?;
    write_binding(&state_path()?, &resolved)
}

/// The bound quest root, if a binding exists and still points at a directory.
pub fn bound_quest() -> Option<PathBuf> {
    let state_path = state_path().ok()?;
    let content = std::fs::read_to_string(&state_path).ok()?;
    let document: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|error| eprintln!("warning: {} is malformed: {error}", state_path.display()))
        .ok()?;
    let state = state_from_document(&document);
    let quest = PathBuf::from(state.quest?);
    quest.is_dir().then_some(quest)
}

fn state_path() -> Result<PathBuf, Error> {
    Ok(ontology::config_dir()?.join("state.yaml"))
}

fn show_binding(state_path: &Path) -> i32 {
    if let Some(quest) = bound_quest() {
        let status = if quest.join(".rune").is_file() {
            "manifest: .rune"
        } else {
            "manifest: none"
        };
        println!("quest: {} ({status})", quest.display());
    } else {
        println!(
            "no quest bound; `rune quest <slug-or-path>` writes {}",
            state_path.display()
        );
    }
    0
}

fn list_quests(state_path: &Path) -> Result<i32, Error> {
    let mut stdout = std::io::stdout().lock();
    list_quests_to(state_path, &mut stdout)
}

fn list_quests_to(state_path: &Path, writer: &mut impl std::io::Write) -> Result<i32, Error> {
    let document = read_document(state_path)?;
    let state = state_from_document(&document);
    let history = normalized_history(&state)
        .into_iter()
        .filter(|quest| Path::new(quest).is_dir())
        .collect::<Vec<_>>();
    if history.is_empty() {
        writeln!(writer, "no recent quests").map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot write quest list: {error}"))
        })?;
        return Ok(0);
    }
    for quest in history {
        let marker = if state.quest.as_deref() == Some(quest.as_str()) {
            "*"
        } else {
            " "
        };
        writeln!(writer, "{marker} {quest}").map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot write quest list: {error}"))
        })?;
    }
    Ok(0)
}

fn previous_quest(state_path: &Path) -> Result<PathBuf, Error> {
    let document = read_document(state_path)?;
    let state = state_from_document(&document);
    normalized_history(&state)
        .into_iter()
        .find(|quest| Some(quest.as_str()) != state.quest.as_deref() && Path::new(quest).is_dir())
        .map(PathBuf::from)
        .ok_or_else(|| Error::new(ErrorKind::Config, "no previous quest".to_string()))
}

fn unbind_quest(state_path: &Path) -> Result<i32, Error> {
    if !state_path.is_file() {
        println!("no quest bound");
        return Ok(0);
    }
    let content = std::fs::read_to_string(state_path)
        .map_err(|error| io_error(state_path, "read", &error))?;
    let mut document: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("{} is malformed: {error}", state_path.display()),
        )
    })?;
    if let Some(mapping) = document.as_mapping_mut() {
        mapping.remove(serde_yaml::Value::from("quest"));
    }
    let content = serde_yaml::to_string(&document)
        .map_err(|error| Error::new(ErrorKind::Parse, format!("cannot serialize: {error}")))?;
    std::fs::write(state_path, content).map_err(|error| io_error(state_path, "write", &error))?;
    println!("quest unbound");
    Ok(0)
}

fn resolve_quest(requested: &str, clone: bool) -> Result<PathBuf, Error> {
    let as_path = PathBuf::from(requested);
    if as_path.is_dir() {
        return std::fs::canonicalize(&as_path).map_err(|error| io_error(&as_path, "read", &error));
    }

    let quests_root = quests_root()?;
    let name = requested.rsplit('/').next().unwrap_or(requested);
    if name.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("quest '{requested}' has no name segment"),
        ));
    }
    let candidate = quests_root.join(name);
    if candidate.is_dir() {
        return std::fs::canonicalize(&candidate)
            .map_err(|error| io_error(&candidate, "read", &error));
    }

    if clone {
        return clone_quest(requested, &candidate);
    }
    Err(Error::new(
        ErrorKind::Config,
        format!(
            "quest '{requested}' not found at {}; pass --clone to clone https://github.com/{requested}",
            candidate.display()
        ),
    ))
}

fn quests_root() -> Result<PathBuf, Error> {
    let config = ontology::load()?;
    config
        .ontology
        .quests
        .map(|value| PathBuf::from(value.value))
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                "quests root is not configured".to_string(),
            )
        })
}

fn clone_quest(slug: &str, destination: &Path) -> Result<PathBuf, Error> {
    let segments = slug.split('/').filter(|s| !s.is_empty()).count();
    if segments != 2 {
        return Err(Error::new(
            ErrorKind::Config,
            format!("--clone needs an <owner>/<name> slug, got '{slug}'"),
        ));
    }
    let url = format!("https://github.com/{slug}.git");
    let status = std::process::Command::new("git")
        .args(["clone", &url])
        .arg(destination)
        .status()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git clone: {error}")))?;
    if !status.success() {
        return Err(Error::new(
            ErrorKind::Io,
            format!("git clone {url} failed with {status}"),
        ));
    }
    std::fs::canonicalize(destination).map_err(|error| io_error(destination, "read", &error))
}

fn write_binding(state_path: &Path, quest: &Path) -> Result<(), Error> {
    let mut document = read_document(state_path)?;
    let state = state_from_document(&document);
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("{} must contain a YAML mapping", state_path.display()),
        )
    })?;
    let quest = quest.to_string_lossy().into_owned();
    mapping.insert(
        serde_yaml::Value::from("quest"),
        serde_yaml::Value::from(quest.clone()),
    );
    let mut history = vec![quest];
    if let Some(active) = state.quest {
        history.push(active);
    }
    history.extend(state.quests);
    deduplicate_and_cap(&mut history);
    mapping.insert(
        serde_yaml::Value::from("quests"),
        serde_yaml::to_value(history)
            .map_err(|error| Error::new(ErrorKind::Parse, format!("cannot serialize: {error}")))?,
    );
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, "create", &error))?;
    }
    let content = serde_yaml::to_string(&document)
        .map_err(|error| Error::new(ErrorKind::Parse, format!("cannot serialize: {error}")))?;
    std::fs::write(state_path, content).map_err(|error| io_error(state_path, "write", &error))
}

fn read_document(state_path: &Path) -> Result<serde_yaml::Value, Error> {
    match std::fs::read_to_string(state_path) {
        Ok(content) => serde_yaml::from_str(&content).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!("{} is malformed: {error}", state_path.display()),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
        }
        Err(error) => Err(io_error(state_path, "read", &error)),
    }
}

fn state_from_document(document: &serde_yaml::Value) -> State {
    let Some(mapping) = document.as_mapping() else {
        return State::default();
    };
    let quest = mapping
        .get(serde_yaml::Value::from("quest"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string);
    let quests = mapping
        .get(serde_yaml::Value::from("quests"))
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_yaml::Value::as_str)
        .map(str::to_string)
        .collect();
    State { quest, quests }
}

fn normalized_history(state: &State) -> Vec<String> {
    let mut history = state.quest.iter().cloned().collect::<Vec<_>>();
    history.extend(state.quests.iter().cloned());
    deduplicate_and_cap(&mut history);
    history
}

fn deduplicate_and_cap(history: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    history.retain(|quest| seen.insert(quest.clone()));
    history.truncate(HISTORY_LIMIT);
}

fn io_error(path: &Path, action: &str, error: &dyn std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("cannot {action} {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_quest_skips_deleted_history_entries() {
        let root = tempfile::tempdir().unwrap();
        let active = root.path().join("active");
        let deleted = root.path().join("deleted");
        let valid = root.path().join("valid");
        std::fs::create_dir(&active).unwrap();
        std::fs::create_dir(&valid).unwrap();
        let state_path = root.path().join("state.yaml");
        std::fs::write(
            &state_path,
            serde_yaml::to_string(&serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter(
                [
                    (
                        serde_yaml::Value::from("quest"),
                        serde_yaml::Value::from(active.to_string_lossy().into_owned()),
                    ),
                    (
                        serde_yaml::Value::from("quests"),
                        serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::from(deleted.to_string_lossy().into_owned()),
                            serde_yaml::Value::from(valid.to_string_lossy().into_owned()),
                        ]),
                    ),
                ],
            )))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(previous_quest(&state_path).unwrap(), valid);
    }

    #[test]
    fn list_quests_omits_deleted_history_entries() {
        let root = tempfile::tempdir().unwrap();
        let active = root.path().join("active");
        let deleted = root.path().join("deleted");
        let valid = root.path().join("valid");
        std::fs::create_dir(&active).unwrap();
        std::fs::create_dir(&valid).unwrap();
        let state_path = root.path().join("state.yaml");
        std::fs::write(
            &state_path,
            format!(
                "quest: {}\nquests:\n  - {}\n  - {}\n",
                active.display(),
                deleted.display(),
                valid.display()
            ),
        )
        .unwrap();
        let mut output = Vec::new();

        list_quests_to(&state_path, &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("* {}\n  {}\n", active.display(), valid.display())
        );
    }
}
