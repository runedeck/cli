//! Bind a quest — the repo being worked on — so rune commands can find its
//! consumer manifest from anywhere. The binding lives in
//! `~/.config/rune/state.yaml`, separate from `config.yaml` because the
//! config file denies unknown fields and the binding is mutable session
//! state, not configuration.

use commands::error::{Error, ErrorKind};
use commands::ontology;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct State {
    quest: Option<String>,
}

pub fn execute(quest: Option<&str>, clone: bool, unbind: bool) -> Result<i32, Error> {
    let state_path = state_path()?;
    if unbind {
        return unbind_quest(&state_path);
    }
    let Some(requested) = quest else {
        return Ok(show_binding(&state_path));
    };
    let resolved = resolve_quest(requested, clone)?;
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
    let state: State = serde_yaml::from_str(&content)
        .map_err(|error| eprintln!("warning: {} is malformed: {error}", state_path.display()))
        .ok()?;
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
        return Ok(candidate);
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
    Ok(destination.to_path_buf())
}

fn write_binding(state_path: &Path, quest: &Path) -> Result<(), Error> {
    let mut document: serde_yaml::Value = match std::fs::read_to_string(state_path) {
        Ok(content) => serde_yaml::from_str(&content).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!("{} is malformed: {error}", state_path.display()),
            )
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
        }
        Err(error) => return Err(io_error(state_path, "read", &error)),
    };
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("{} must contain a YAML mapping", state_path.display()),
        )
    })?;
    mapping.insert(
        serde_yaml::Value::from("quest"),
        serde_yaml::Value::from(quest.to_string_lossy().into_owned()),
    );
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, "create", &error))?;
    }
    let content = serde_yaml::to_string(&document)
        .map_err(|error| Error::new(ErrorKind::Parse, format!("cannot serialize: {error}")))?;
    std::fs::write(state_path, content).map_err(|error| io_error(state_path, "write", &error))
}

fn io_error(path: &Path, action: &str, error: &dyn std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("cannot {action} {}: {error}", path.display()),
    )
}
