//! Bind a target — the repo being worked on — so rune commands can find its
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
    target: Option<String>,
    targets: Vec<String>,
}

pub fn execute(target: Option<&str>, clone: bool, unbind: bool, list: bool) -> Result<i32, Error> {
    let state_path = state_path()?;
    if list {
        return list_targets(&state_path);
    }
    if unbind {
        return unbind_target(&state_path);
    }
    let Some(requested) = target else {
        return Ok(show_binding(&state_path));
    };
    let resolved = if requested == "-" {
        previous_target(&state_path)?
    } else {
        resolve_target(requested, clone)?
    };
    write_binding(&state_path, &resolved)?;
    let label = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(requested);
    println!("bound target '{label}' → {}", resolved.display());
    if resolved.join(".rune").is_file() {
        println!("next: rune tui --edit to review (or: rune install)");
    } else {
        println!("next: rune add <deck-or-rune> to stage runes (no .rune manifest yet)");
    }
    Ok(0)
}

pub(crate) fn bind_existing(target: &Path) -> Result<(), Error> {
    let resolved =
        std::fs::canonicalize(target).map_err(|error| io_error(target, "read", &error))?;
    write_binding(&state_path()?, &resolved)
}

/// The bound target root, if a binding exists and still points at a directory.
pub fn bound_target() -> Option<PathBuf> {
    bound_quest_with_warnings(true)
}

pub(crate) fn bound_target_silent() -> Option<PathBuf> {
    bound_quest_with_warnings(false)
}

fn bound_quest_with_warnings(show_warning: bool) -> Option<PathBuf> {
    let state_path = state_path().ok()?;
    let content = std::fs::read_to_string(&state_path).ok()?;
    let document: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(document) => document,
        Err(error) => {
            if show_warning {
                eprintln!("warning: {} is malformed: {error}", state_path.display());
            }
            return None;
        }
    };
    let state = state_from_document(&document);
    let target = PathBuf::from(state.target?);
    target.is_dir().then_some(target)
}

fn state_path() -> Result<PathBuf, Error> {
    Ok(ontology::config_dir()?.join("state.yaml"))
}

fn show_binding(state_path: &Path) -> i32 {
    if let Some(target) = bound_target() {
        let status = if target.join(".rune").is_file() {
            "manifest: .rune"
        } else {
            "manifest: none"
        };
        println!("target: {} ({status})", target.display());
    } else {
        println!(
            "no target bound; `rune target <slug-or-path>` writes {}",
            state_path.display()
        );
    }
    0
}

fn list_targets(state_path: &Path) -> Result<i32, Error> {
    let mut stdout = std::io::stdout().lock();
    list_targets_to(state_path, &mut stdout)
}

fn list_targets_to(state_path: &Path, writer: &mut impl std::io::Write) -> Result<i32, Error> {
    let document = read_document(state_path)?;
    let state = state_from_document(&document);
    let history = normalized_history(&state)
        .into_iter()
        .filter(|target| Path::new(target).is_dir())
        .collect::<Vec<_>>();
    if history.is_empty() {
        writeln!(writer, "no recent targets").map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot write target list: {error}"))
        })?;
        return Ok(0);
    }
    for target in history {
        let marker = if state.target.as_deref() == Some(target.as_str()) {
            "*"
        } else {
            " "
        };
        writeln!(writer, "{marker} {target}").map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot write target list: {error}"))
        })?;
    }
    Ok(0)
}

fn previous_target(state_path: &Path) -> Result<PathBuf, Error> {
    let document = read_document(state_path)?;
    let state = state_from_document(&document);
    normalized_history(&state)
        .into_iter()
        .find(|target| {
            Some(target.as_str()) != state.target.as_deref() && Path::new(target).is_dir()
        })
        .map(PathBuf::from)
        .ok_or_else(|| Error::new(ErrorKind::Config, "no previous target".to_string()))
}

fn unbind_target(state_path: &Path) -> Result<i32, Error> {
    if !state_path.is_file() {
        println!("no target bound");
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
        mapping.remove(serde_yaml::Value::from("target"));
        mapping.remove(serde_yaml::Value::from("quest"));
    }
    let content = serde_yaml::to_string(&document)
        .map_err(|error| Error::new(ErrorKind::Parse, format!("cannot serialize: {error}")))?;
    std::fs::write(state_path, content).map_err(|error| io_error(state_path, "write", &error))?;
    println!("target unbound");
    Ok(0)
}

fn resolve_target(requested: &str, clone: bool) -> Result<PathBuf, Error> {
    let as_path = PathBuf::from(requested);
    if as_path.is_dir() {
        return std::fs::canonicalize(&as_path).map_err(|error| io_error(&as_path, "read", &error));
    }

    let targets_root = targets_root()?;
    let name = requested.rsplit('/').next().unwrap_or(requested);
    if name.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("target '{requested}' has no name segment"),
        ));
    }
    let candidate = targets_root.join(name);
    if candidate.is_dir() {
        return std::fs::canonicalize(&candidate)
            .map_err(|error| io_error(&candidate, "read", &error));
    }

    if clone {
        return clone_target(requested, &candidate);
    }
    Err(Error::new(
        ErrorKind::Config,
        format!(
            "target '{requested}' not found at {}; pass --clone to clone https://github.com/{requested}",
            candidate.display()
        ),
    ))
}

fn targets_root() -> Result<PathBuf, Error> {
    let config = ontology::load()?;
    config
        .ontology
        .targets
        .map(|value| PathBuf::from(value.value))
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                "targets root is not configured".to_string(),
            )
        })
}

fn clone_target(slug: &str, destination: &Path) -> Result<PathBuf, Error> {
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

fn write_binding(state_path: &Path, target: &Path) -> Result<(), Error> {
    let mut document = read_document(state_path)?;
    let state = state_from_document(&document);
    let mapping = document.as_mapping_mut().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("{} must contain a YAML mapping", state_path.display()),
        )
    })?;
    let target = target.to_string_lossy().into_owned();
    mapping.insert(
        serde_yaml::Value::from("target"),
        serde_yaml::Value::from(target.clone()),
    );
    let mut history = vec![target];
    if let Some(active) = state.target {
        history.push(active);
    }
    history.extend(state.targets);
    deduplicate_and_cap(&mut history);
    mapping.insert(
        serde_yaml::Value::from("targets"),
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
    let target = mapping
        .get(serde_yaml::Value::from("target"))
        .or_else(|| mapping.get(serde_yaml::Value::from("quest")))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string);
    let targets = mapping
        .get(serde_yaml::Value::from("targets"))
        .or_else(|| mapping.get(serde_yaml::Value::from("quests")))
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_yaml::Value::as_str)
        .map(str::to_string)
        .collect();
    State { target, targets }
}

fn normalized_history(state: &State) -> Vec<String> {
    let mut history = state.target.iter().cloned().collect::<Vec<_>>();
    history.extend(state.targets.iter().cloned());
    deduplicate_and_cap(&mut history);
    history
}

fn deduplicate_and_cap(history: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    history.retain(|target| seen.insert(target.clone()));
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
                        serde_yaml::Value::from("target"),
                        serde_yaml::Value::from(active.to_string_lossy().into_owned()),
                    ),
                    (
                        serde_yaml::Value::from("targets"),
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

        assert_eq!(previous_target(&state_path).unwrap(), valid);
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
                "target: {}\nquests:\n  - {}\n  - {}\n",
                active.display(),
                deleted.display(),
                valid.display()
            ),
        )
        .unwrap();
        let mut output = Vec::new();

        list_targets_to(&state_path, &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("* {}\n  {}\n", active.display(), valid.display())
        );
    }
}
