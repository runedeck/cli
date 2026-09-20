//! `rune draft`: write a rune under construction into the consumer's provider
//! trees and register it in `.drafts`, so the harness loads it now and
//! `rune doctor` knows it is a draft, not an orphan.

use chrono::Utc;
use rune::error::{Error, ErrorKind};
use rune::manifest::drafts::{Draft, Register};
use std::fs;
use std::path::{Path, PathBuf};

/// The rune kinds a draft can have, with their directory and file shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftKind {
    Skill,
    Agent,
    Rule,
}

impl DraftKind {
    pub fn parse(text: &str) -> Result<Self, Error> {
        match text {
            "skill" => Ok(Self::Skill),
            "agent" => Ok(Self::Agent),
            "rule" => Ok(Self::Rule),
            other => Err(Error::new(
                ErrorKind::Config,
                format!("unknown draft kind '{other}': use skill, agent, or rule"),
            )),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Agent => "agent",
            Self::Rule => "rule",
        }
    }

    /// The path of the draft inside one provider directory.
    pub fn relative_path(self, name: &str) -> String {
        match self {
            Self::Skill => format!("skills/{name}/SKILL.md"),
            Self::Agent => format!("agents/{name}.md"),
            Self::Rule => format!("rules/{name}.md"),
        }
    }

    /// The minimal body a draft starts from.
    pub fn template(self, name: &str) -> String {
        match self {
            Self::Skill => format!(
                "---\nname: {name}\ndescription: Draft. State what this skill does and when to use it.\n---\n\n# {name}\n\n## Instructions\n\nState the steps.\n"
            ),
            Self::Agent => format!(
                "---\nname: {name}\ndescription: Draft. State the bounded task this agent runs.\n---\n\n# {name}\n\nState the task, its inputs, and its output.\n"
            ),
            Self::Rule => "State one rule in one paragraph.\n".to_string(),
        }
    }
}

/// Provider directories under the consumer root that hold a `.manifest`.
/// A directory with a manifest is one the consumer deploys to.
pub fn deployed_provider_dirs(consumer_root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut dirs = Vec::new();
    let entries = fs::read_dir(consumer_root).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", consumer_root.display()),
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() && name.starts_with('.') && path.join(".manifest").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// A rune name or a domain is one identifier with no path parts: letters,
/// digits, `-` or `_`. Anything else could join into a path and escape.
/// The rule is the register's, so a stored name and a typed one agree.
pub(crate) fn validate_identifier(label: &str, value: &str) -> Result<(), Error> {
    rune::manifest::drafts::validate_name(value).map_err(|_| {
        Error::new(
            ErrorKind::Config,
            format!("{label} '{value}' must be letters, digits, '-' or '_' only"),
        )
    })
}

/// Create a draft of `kind` named `name` in every deployed provider directory.
/// Every destination is checked before the first write, and a failed write
/// removes what this call created, so a refusal never strands a file that
/// `--drop` cannot see.
pub fn create(consumer_root: &Path, kind: DraftKind, name: &str) -> Result<Vec<String>, Error> {
    validate_identifier("draft name", name)?;
    let providers = deployed_provider_dirs(consumer_root)?;
    if providers.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no deployed provider directory under {}: run rune install first",
                consumer_root.display()
            ),
        ));
    }
    let mut register =
        Register::load(consumer_root).map_err(|m| Error::new(ErrorKind::Parse, m))?;
    let relative = kind.relative_path(name);

    // Preflight: every provider, every rule, before any write.
    let mut planned = Vec::new();
    for provider in &providers {
        let target = provider.join(&relative);
        let provider_name = provider
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let consumer_relative = format!("{provider_name}/{relative}");
        if is_managed(provider, &relative)? {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{consumer_relative} is a managed rune in .manifest; a draft cannot shadow it"
                ),
            ));
        }
        if target.exists() {
            return Err(Error::new(
                ErrorKind::Config,
                format!("{consumer_relative} already exists; drop it or pick another name"),
            ));
        }
        register
            .add(Draft {
                path: consumer_relative.clone(),
                kind: kind.name().to_string(),
                name: name.to_string(),
                created: Draft::stamp(Utc::now()),
            })
            .map_err(|m| Error::new(ErrorKind::Config, m))?;
        planned.push((target, consumer_relative));
    }

    let mut written = Vec::new();
    for (target, consumer_relative) in &planned {
        if let Err(error) = write_new(target, &kind.template(name)) {
            for (done, _) in &planned[..written.len()] {
                let _ = fs::remove_file(done);
                remove_empty_parent(done, consumer_root);
            }
            return Err(error);
        }
        written.push(consumer_relative.clone());
    }
    if let Err(message) = register.save(consumer_root) {
        for (done, _) in &planned {
            let _ = fs::remove_file(done);
            remove_empty_parent(done, consumer_root);
        }
        return Err(Error::new(ErrorKind::Io, message));
    }
    Ok(written)
}

/// Create the file exclusively: a file that appears between preflight and
/// write is a refusal, never an overwrite.
fn write_new(target: &Path, content: &str) -> Result<(), Error> {
    use std::io::Write;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", parent.display()),
            )
        })?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", target.display()),
            )
        })?;
    if let Err(error) = file.write_all(content.as_bytes()) {
        std::mem::drop(file);
        let _ = fs::remove_file(target);
        return Err(Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", target.display()),
        ));
    }
    Ok(())
}

/// Whether a provider's `.manifest` lists the relative path.
fn is_managed(provider_dir: &Path, relative: &str) -> Result<bool, Error> {
    let manifest = provider_dir.join(".manifest");
    let content = fs::read_to_string(&manifest).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", manifest.display()),
        )
    })?;
    let entries = rune::manifest::read(&content).map_err(|m| Error::new(ErrorKind::Parse, m))?;
    Ok(entries.contains_key(relative))
}

/// Remove every provider copy of the draft named `name` and deregister it.
pub fn drop(consumer_root: &Path, name: &str) -> Result<Vec<String>, Error> {
    let mut register =
        Register::load(consumer_root).map_err(|m| Error::new(ErrorKind::Parse, m))?;
    let removed = register.remove_name(name);
    if removed.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("no draft named '{name}'"),
        ));
    }
    let mut paths = Vec::new();
    for draft in &removed {
        let file = consumer_root.join(&draft.path);
        if file.exists() {
            fs::remove_file(&file).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot remove {}: {error}", file.display()),
                )
            })?;
            remove_empty_parent(&file, consumer_root);
        }
        paths.push(draft.path.clone());
    }
    register
        .save(consumer_root)
        .map_err(|m| Error::new(ErrorKind::Io, m))?;
    Ok(paths)
}

/// A skill draft leaves an empty `skills/<name>/` behind; remove it.
fn remove_empty_parent(file: &Path, stop_at: &Path) {
    if let Some(parent) = file.parent()
        && parent != stop_at
        && fs::read_dir(parent).is_ok_and(|mut entries| entries.next().is_none())
    {
        let _ = fs::remove_dir(parent);
    }
}

/// The register as lines for `--list`: name, kind, age, path.
pub fn list(consumer_root: &Path) -> Result<Vec<String>, Error> {
    let register = Register::load(consumer_root).map_err(|m| Error::new(ErrorKind::Parse, m))?;
    let now = Utc::now();
    Ok(register
        .drafts
        .iter()
        .map(|draft| {
            let age = draft
                .age_days(now)
                .map_or_else(|| "unknown age".to_string(), |d| format!("{d} days"));
            let state = if consumer_root.join(&draft.path).exists() {
                ""
            } else {
                "  (missing)"
            };
            format!(
                "{}  {}  {}  {}{}",
                draft.name, draft.kind, age, draft.path, state
            )
        })
        .collect())
}

/// The command entry: dispatch on the flags and print the outcome.
pub fn run(
    target: &str,
    kind: Option<&str>,
    name: Option<&str>,
    list_flag: bool,
    drop_name: Option<&str>,
    json: bool,
) -> Result<i32, Error> {
    let root = Path::new(target);
    if list_flag {
        let lines = list(root)?;
        if json {
            let register = Register::load(root).map_err(|m| Error::new(ErrorKind::Parse, m))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&register.drafts).unwrap_or_default()
            );
        } else if lines.is_empty() {
            println!("no drafts");
        } else {
            for line in lines {
                println!("{line}");
            }
        }
        return Ok(0);
    }
    if let Some(name) = drop_name {
        let removed = drop(root, name)?;
        if json {
            println!(
                "{}",
                serde_json::json!({ "dropped": name, "removed": removed })
            );
        } else {
            for path in &removed {
                println!("removed {path}");
            }
        }
        return Ok(0);
    }
    let kind = DraftKind::parse(kind.unwrap_or_default())?;
    let name =
        name.ok_or_else(|| Error::new(ErrorKind::Config, "draft needs a name".to_string()))?;
    let written = create(root, kind, name)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "draft": name, "kind": kind.name(), "written": written })
        );
    } else {
        for path in &written {
            println!("draft {path}");
        }
        println!(
            "registered in .drafts; promote with: rune promote {name} --domain <domain> --change <id>"
        );
    }
    Ok(0)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
