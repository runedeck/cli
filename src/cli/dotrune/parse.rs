//! Schema and parser for `.rune`.

use commands::error::{Error, ErrorKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SCHEMA_VERSION: u32 = 1;
/// Version 2 adds the `dirs:` workspace-member section; version 1 files
/// keep parsing unchanged.
pub const SCHEMA_VERSION_DIRS: u32 = 2;

/// Test-only escape hatch: when this env var is set, `file://` URLs pass
/// validation so integration tests can use a local bare repo as the origin
/// without spinning up an HTTPS server. Never set in production: the
/// HTTPS-only rule defends against `git://` MITM and accidental local-path
/// pulls that bypass SHA pinning.
#[cfg(feature = "test-file-urls")]
const ALLOW_FILE_URLS_ENV: &str = "RUNE_GIT_ALLOW_FILE_URLS";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DotRune {
    pub version: u32,
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub runes: BTreeMap<String, RuneList>,
    /// Workspace directories associated with this consumer (schema v2):
    /// wiki, data, out, and other members that tools like `rune todo --all`
    /// aggregate over. Committed paths are relative to the `.rune` file;
    /// machine-specific absolute paths belong in a gitignored `.rune.local`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirs: Vec<WorkspaceDir>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDir {
    pub path: String,
    /// What the directory is to this workspace: wiki, data, out, or free text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub role: String,
    /// Missing required members are errors for aggregating tools; optional
    /// members warn and skip.
    #[serde(default)]
    pub required: bool,
}

/// Where to find a producer module. `Local` for a sibling checkout on disk,
/// `Git` for a remote HTTPS repository pinned to a 40-hex commit SHA.
///
/// A custom `Deserialize` allows local and git sources to carry an inner `path`.
#[derive(Debug, Clone)]
pub enum Source {
    Local {
        local: PathBuf,
        path: Option<PathBuf>,
    },
    Git {
        git: String,
        commit: String,
        path: Option<PathBuf>,
    },
}

impl Serialize for Source {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(None)?;
        match self {
            Self::Local { local, path } => {
                map.serialize_entry("local", local)?;
                if let Some(path) = path {
                    map.serialize_entry("path", path)?;
                }
            }
            Self::Git { git, commit, path } => {
                map.serialize_entry("git", git)?;
                map.serialize_entry("ref", commit)?;
                if let Some(path) = path {
                    map.serialize_entry("path", path)?;
                }
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let value: serde_yaml::Value = Deserialize::deserialize(deserializer)?;
        let mapping = value.as_mapping().ok_or_else(|| {
            D::Error::custom(
                "source entry must be a mapping (e.g. `path: ../foo` or `git: https://...`)",
            )
        })?;
        let has_local = mapping.contains_key(serde_yaml::Value::from("local"));
        let has_git = mapping.contains_key(serde_yaml::Value::from("git"));
        let has_path = mapping.contains_key(serde_yaml::Value::from("path"));
        match (has_local, has_git, has_path) {
            (true, true, _) => Err(D::Error::custom(
                "source entry cannot have both `local` and `git`; pick one",
            )),
            (false, true, _) => {
                let git: GitFields = serde_yaml::from_value(value).map_err(D::Error::custom)?;
                validate_git_url(&git.git).map_err(D::Error::custom)?;
                validate_commit_sha(&git.commit).map_err(D::Error::custom)?;
                validate_subpath(git.path.as_deref()).map_err(D::Error::custom)?;
                Ok(Source::Git {
                    git: git.git,
                    commit: git.commit,
                    path: git.path,
                })
            }
            (true, false, _) => {
                let local: LocalFields = serde_yaml::from_value(value).map_err(D::Error::custom)?;
                validate_subpath(local.path.as_deref()).map_err(D::Error::custom)?;
                Ok(Source::Local {
                    local: local.local,
                    path: local.path,
                })
            }
            (false, false, _) => Err(D::Error::custom(
                "source entry must contain exactly one of `local:` or `git:`",
            )),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalFields {
    local: PathBuf,
    path: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitFields {
    git: String,
    #[serde(rename = "ref")]
    commit: String,
    path: Option<PathBuf>,
}

fn validate_subpath(path: Option<&std::path::Path>) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "source path must be a non-empty directory inside the materialized source: {}",
            path.display()
        ));
    }
    Ok(())
}

pub fn validate_git_url(url: &str) -> Result<(), String> {
    #[cfg(feature = "test-file-urls")]
    if std::env::var_os(ALLOW_FILE_URLS_ENV).is_some() && url.starts_with("file://") {
        return Ok(());
    }
    let Some(after_scheme) = url.strip_prefix("https://") else {
        return Err(format!("git URL must start with https://, got: {url}"));
    };
    let host_segment = after_scheme.split('/').next().unwrap_or_default();
    if host_segment.contains('@') {
        return Err(format!(
            "git URL must not embed user@ credentials in the host: {url}"
        ));
    }
    if host_segment.is_empty() {
        return Err(format!("git URL has no host: {url}"));
    }
    Ok(())
}

pub fn validate_commit_sha(sha: &str) -> Result<(), String> {
    if sha.len() != 40 {
        return Err(format!(
            "ref must be a 40-char commit SHA; got {} chars: {sha}",
            sha.len()
        ));
    }
    if !sha
        .chars()
        .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err(format!(
            "ref must be 40 lowercase hex chars (no branch names, tags, or uppercase): {sha}"
        ));
    }
    Ok(())
}

/// Per-source list of requested rune names. Each kind defaults to empty
/// so `.rune` can request only one kind per source.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuneList {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "string_or_list"
    )]
    pub casts: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<String>,
}

fn string_or_list<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    use serde::de::Error as _;
    let value: serde_yaml::Value = Deserialize::deserialize(deserializer)?;
    match value {
        serde_yaml::Value::String(single) => Ok(vec![single]),
        serde_yaml::Value::Sequence(_) => serde_yaml::from_value(value).map_err(D::Error::custom),
        other => Err(D::Error::custom(format!(
            "casts must be a cast name or a list of cast names, got: {other:?}"
        ))),
    }
}

impl RuneList {
    pub fn is_empty(&self) -> bool {
        self.casts.is_empty()
            && self.include.is_empty()
            && self.skills.is_empty()
            && self.agents.is_empty()
            && self.rules.is_empty()
            && self.hooks.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.include
            .iter()
            .chain(&self.skills)
            .chain(&self.agents)
            .chain(&self.rules)
            .chain(&self.hooks)
    }
}

pub fn parse(content: &str) -> Result<DotRune, Error> {
    let manifest: DotRune = serde_yaml::from_str(content)
        .map_err(|error| Error::new(ErrorKind::Parse, format!(".rune: {error}")))?;

    if manifest.version != SCHEMA_VERSION && manifest.version != SCHEMA_VERSION_DIRS {
        return Err(Error::new(
            ErrorKind::Parse,
            format!(
                ".rune: schema version {} is not supported (this build understands versions {SCHEMA_VERSION} and {SCHEMA_VERSION_DIRS})",
                manifest.version
            ),
        ));
    }
    if manifest.version == SCHEMA_VERSION && !manifest.dirs.is_empty() {
        return Err(Error::new(
            ErrorKind::Parse,
            ".rune: dirs requires version 2".to_string(),
        ));
    }
    for member in &manifest.dirs {
        let path = std::path::Path::new(&member.path);
        if path.is_absolute() || member.path.starts_with('~') {
            return Err(Error::new(
                ErrorKind::Parse,
                format!(
                    ".rune: dirs path '{}' must be relative to the .rune file; machine paths belong in .rune.local",
                    member.path
                ),
            ));
        }
        // Leading `..` components reach sibling members (`../wiki`); interior
        // `..` after a normal segment is a traversal smell and is rejected.
        let mut seen_normal = false;
        for component in path.components() {
            match component {
                std::path::Component::ParentDir if seen_normal => {
                    return Err(Error::new(
                        ErrorKind::Parse,
                        format!(
                            ".rune: dirs path '{}' mixes .. inside the path",
                            member.path
                        ),
                    ));
                }
                std::path::Component::Normal(_) => seen_normal = true,
                _ => {}
            }
        }
    }

    for source_label in manifest.runes.keys() {
        if !manifest.sources.contains_key(source_label) {
            return Err(Error::new(
                ErrorKind::Parse,
                format!(".rune: runes entry '{source_label}' has no matching `sources` entry"),
            ));
        }
    }

    Ok(manifest)
}
