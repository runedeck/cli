//! The agent policy: one YAML file that grants dotted names per directory,
//! a repository file that may only take away, and a session switch file.
//! A port of the Jev reader (`jev/src/policy/`). It keeps the file format,
//! the layer order, the layer labels, and the reasons, so `rune policy
//! check` and `jev policy` print the same line for the same question.
//!
//! The user file, version 1:
//!
//! ```yaml
//! version: 1
//! disabled: [<pattern>...]       # refused everywhere, whatever a scope grants
//! grant: [<pattern>...]          # allowed everywhere unless a scope changes it
//! scopes:
//!   - path: <dir>                # `~` allowed; the closest scope decides
//!     only: [<pattern>...]       # replaces the answer with "matches only"
//!     grant: [<pattern>...]
//!     revoke: [<pattern>...]
//! ```
//!
//! A pattern is a name, or a prefix followed by `.*` that matches every
//! name below it. Nothing here reads the process environment except
//! `Environment::from_process`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_yaml::{Mapping, Value};

mod paths;
mod resolve;

#[cfg(test)]
mod tests;

pub use paths::{contains, expand_home, normalize, realpath_nearest, repository_root};
pub use resolve::matches;

/// The one file version this reader reads.
pub const VERSION: u64 = 1;
/// The variable that names another user policy file.
pub const POLICY_ENV: &str = "RUNE_POLICY";
/// The variable that refuses names for one process tree, comma-separated.
pub const DISABLE_ENV: &str = "RUNE_POLICY_DISABLE";
/// The layer of the user file's top-level `grant`.
pub const TOP_LAYER: &str = "top level";
/// The layer of the user file's `disabled` list.
pub const DISABLED_LAYER: &str = "disabled";
/// The layer of a user file that is missing or broken.
pub const FILE_LAYER: &str = "policy file";
/// The repository file, relative to the repository root.
pub const REPOSITORY_FILE: &str = ".rune/policy.yaml";

/// Whether paths compare without letter case: the default file systems of
/// macOS and Windows ignore it.
pub const FOLD_CASE: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// One scope of the user file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// As written in the file.
    pub path: String,
    /// `None` when the scope has no `only` key.
    pub only: Option<Vec<String>>,
    pub grant: Vec<String>,
    pub revoke: Vec<String>,
}

/// The user file, checked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    pub disabled: Vec<String>,
    pub grant: Vec<String>,
    pub scopes: Vec<Scope>,
}

/// A repository file, checked. Its `grant` is kept only to warn about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryPolicy {
    pub disabled: Vec<String>,
    pub grant: Vec<String>,
    pub revoke: Vec<String>,
    pub only: Option<Vec<String>>,
}

/// A user file that loaded, or the reason it did not. A reader with an
/// error refuses every name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub path: PathBuf,
    pub policy: Result<Policy, String>,
}

/// What the reader needs from the process: the home a `~` expands to, the
/// user file, the session switches, and how paths compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub home: PathBuf,
    /// The user file.
    pub file: PathBuf,
    /// The patterns of `RUNE_POLICY_DISABLE`.
    pub disable: Vec<String>,
    /// Where the session switch files live, `<id>.yaml` each. `None` reads
    /// no switch file.
    pub session_dir: Option<PathBuf>,
    pub fold_case: bool,
    /// What a relative path resolves against.
    pub cwd: PathBuf,
}

impl Environment {
    /// The environment of this process. `session_dir` is the caller's,
    /// because the rune state directory is a CLI concern.
    #[must_use]
    pub fn from_process(session_dir: Option<PathBuf>) -> Self {
        let home = crate::folders::home_dir().unwrap_or_default();
        let file = path_from(std::env::var_os(POLICY_ENV).as_deref(), &home);
        let disable = disabled_from(std::env::var(DISABLE_ENV).ok().as_deref());
        Self {
            file,
            disable,
            session_dir,
            fold_case: FOLD_CASE,
            cwd: std::env::current_dir().unwrap_or_else(|_| home.clone()),
            home,
        }
    }
}

/// The user file: `RUNE_POLICY` when it is set and not empty, else
/// `~/.config/rune/policy.yaml`.
#[must_use]
pub fn path_from(override_path: Option<&OsStr>, home: &Path) -> PathBuf {
    match override_path {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => home.join(".config").join("rune").join("policy.yaml"),
    }
}

/// The patterns of a `RUNE_POLICY_DISABLE` value.
#[must_use]
pub fn disabled_from(raw: Option<&str>) -> Vec<String> {
    raw.map_or_default(|raw| {
        raw.split(',')
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_string)
            .collect()
    })
}

/// The answer to one question: may an agent use this name at this path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub allowed: bool,
    /// `top level`, `scope <path>`, `repository <root>`, `disabled`,
    /// `session <id>`, `RUNE_POLICY_DISABLE`, or `policy file`.
    pub layer: String,
    /// The pattern that matched, `no grant`, `not in only`, or a fault.
    pub reason: String,
    /// What the reader ignored on the way, such as a repository grant.
    pub warnings: Vec<String>,
}

impl Decision {
    pub fn new(allowed: bool, layer: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            allowed,
            layer: layer.into(),
            reason: reason.into(),
            warnings: Vec::new(),
        }
    }

    /// `allowed` or `refused`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        if self.allowed { "allowed" } else { "refused" }
    }

    /// The layer and its reason, as `jev policy` prints them.
    #[must_use]
    pub fn because(&self) -> String {
        format!("{} ({})", self.layer, self.reason)
    }
}

/// A loaded user file and the environment it answers in.
#[derive(Debug, Clone)]
pub struct Reader {
    pub env: Environment,
    pub loaded: Loaded,
}

impl Reader {
    /// Read the user file the environment names. Never fails: a fault
    /// becomes a reader that refuses every name.
    #[must_use]
    pub fn load(env: Environment) -> Self {
        let loaded = load(&env.file);
        Self { env, loaded }
    }

    /// Whether `name` is allowed at `subject` for the session, if known.
    #[must_use]
    pub fn decide(&self, name: &str, subject: &Path, session: Option<&str>) -> Decision {
        resolve::decide(self, name, subject, session)
    }
}

/// Read and check the user file at `path`. Never fails.
#[must_use]
pub fn load(path: &Path) -> Loaded {
    let policy = match std::fs::read_to_string(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("no policy file at {}", path.display()))
        }
        Err(error) => Err(format!(
            "cannot read policy file: {error} at {}",
            path.display()
        )),
        Ok(text) => parse(&text).map_err(|why| format!("{}: {why}", path.display())),
    };
    Loaded {
        path: path.to_path_buf(),
        policy,
    }
}

/// Check the text of a user file. The error names the first fault.
pub fn parse(text: &str) -> Result<Policy, String> {
    let doc = yaml(text)?;
    let map = doc.as_mapping().ok_or("policy is not a mapping")?;
    known_keys(map, &["version", "disabled", "grant", "scopes"], "")?;
    version(map)?;
    let scopes = match map.get("scopes") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Sequence(items)) => items
            .iter()
            .enumerate()
            .map(|(i, item)| scope_of(item, i))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("scopes is not a list".to_string()),
    };
    Ok(Policy {
        disabled: patterns(map.get("disabled"), "disabled")?,
        grant: patterns(map.get("grant"), "grant")?,
        scopes,
    })
}

/// Check the text of a repository file: `version`, `disabled`, `grant`,
/// `revoke`, and `only`. A grant parses, so the reader can warn about it.
pub fn parse_repository(text: &str) -> Result<RepositoryPolicy, String> {
    let doc = yaml(text)?;
    let map = doc.as_mapping().ok_or("policy is not a mapping")?;
    known_keys(map, &["version", "disabled", "grant", "revoke", "only"], "")?;
    version(map)?;
    Ok(RepositoryPolicy {
        disabled: patterns(map.get("disabled"), "disabled")?,
        grant: patterns(map.get("grant"), "grant")?,
        revoke: patterns(map.get("revoke"), "revoke")?,
        only: map
            .get("only")
            .map(|value| patterns(Some(value), "only"))
            .transpose()?,
    })
}

/// Check the text of a session switch file: `disabled`, and `version`
/// when present. An empty file disables nothing.
pub fn parse_session(text: &str) -> Result<Vec<String>, String> {
    let doc = yaml(text)?;
    if doc.is_null() {
        return Ok(Vec::new());
    }
    let map = doc.as_mapping().ok_or("session switch is not a mapping")?;
    known_keys(map, &["version", "disabled"], "")?;
    if map.contains_key("version") {
        version(map)?;
    }
    patterns(map.get("disabled"), "disabled")
}

fn yaml(text: &str) -> Result<Value, String> {
    serde_yaml::from_str(text).map_err(|error| format!("YAML: {error}"))
}

fn scope_of(value: &Value, i: usize) -> Result<Scope, String> {
    let at = format!("scopes[{i}]");
    let map = value
        .as_mapping()
        .ok_or_else(|| format!("{at} is not a mapping"))?;
    known_keys(map, &["path", "only", "grant", "revoke"], &format!("{at}."))?;
    let path = map
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| format!("{at} has no path"))?;
    Ok(Scope {
        path: path.to_string(),
        only: map
            .get("only")
            .map(|value| patterns(Some(value), &format!("{at}.only")))
            .transpose()?,
        grant: patterns(map.get("grant"), &format!("{at}.grant"))?,
        revoke: patterns(map.get("revoke"), &format!("{at}.revoke"))?,
    })
}

/// The first key of `map` that is not in `known`.
fn known_keys(map: &Mapping, known: &[&str], prefix: &str) -> Result<(), String> {
    for key in map.keys() {
        match key.as_str() {
            Some(name) if known.contains(&name) => {}
            Some(name) => return Err(format!("unknown key {prefix}{name}")),
            None => {
                let shown = serde_yaml::to_string(key).unwrap_or_default();
                return Err(format!("unknown key {prefix}{}", shown.trim()));
            }
        }
    }
    Ok(())
}

fn version(map: &Mapping) -> Result<(), String> {
    let value = map.get("version").unwrap_or(&Value::Null);
    if value.as_u64() == Some(VERSION) {
        return Ok(());
    }
    let shown = serde_json::to_string(value).unwrap_or_else(|_| "?".to_string());
    Err(format!("version {shown}; this reader reads {VERSION}"))
}

/// A list of names or patterns. Absent or null is empty.
fn patterns(value: Option<&Value>, at: &str) -> Result<Vec<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Sequence(items)) => items
            .iter()
            .enumerate()
            .map(|(i, item)| match item.as_str().map(str::trim) {
                Some(text) if !text.is_empty() => Ok(text.to_string()),
                _ => Err(format!("{at}[{i}] is not a name")),
            })
            .collect(),
        Some(_) => Err(format!("{at} is not a list")),
    }
}
