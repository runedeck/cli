//! Schema and parser for `.rune`.

use commands::error::{Error, ErrorKind};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SCHEMA_VERSION: u32 = 1;

/// Test-only escape hatch: when this env var is set, `file://` URLs pass
/// validation so integration tests can use a local bare repo as the origin
/// without spinning up an HTTPS server. Never set in production: the
/// HTTPS-only rule defends against `git://` MITM and accidental local-path
/// pulls that bypass SHA pinning.
const ALLOW_FILE_URLS_ENV: &str = "RUNE_GIT_ALLOW_FILE_URLS";
const LEGACY_ALLOW_FILE_URLS_ENV: &str = "FORGE_GIT_ALLOW_FILE_URLS";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DotRune {
    pub version: u32,
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, ArtifactList>,
}

/// Where to find a producer module. `Local` for a sibling checkout on disk,
/// `Git` for a remote HTTPS repository pinned to a 40-hex commit SHA.
///
/// A custom `Deserialize` preserves legacy `path: ../module` local sources
/// while allowing `local: ../deck` and git sources to carry an inner `path`.
#[derive(Debug)]
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
            (false, false, true) => {
                let legacy: LegacyLocalFields =
                    serde_yaml::from_value(value).map_err(D::Error::custom)?;
                Ok(Source::Local {
                    local: legacy.path,
                    path: None,
                })
            }
            (false, false, false) => Err(D::Error::custom(
                "source entry must contain `path:` (legacy local source), `local:`, or `git:`",
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
struct LegacyLocalFields {
    path: PathBuf,
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
    let allow_file = std::env::var_os(ALLOW_FILE_URLS_ENV)
        .or_else(|| std::env::var_os(LEGACY_ALLOW_FILE_URLS_ENV))
        .is_some();
    if allow_file && url.starts_with("file://") {
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

/// Per-source list of requested artifact names. Each kind defaults to empty
/// so `.rune` can request only one kind per source.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ArtifactList {
    pub skills: Vec<String>,
    pub agents: Vec<String>,
    pub rules: Vec<String>,
}

impl ArtifactList {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.agents.is_empty() && self.rules.is_empty()
    }
}

pub fn parse(content: &str) -> Result<DotRune, Error> {
    let manifest: DotRune = serde_yaml::from_str(content)
        .map_err(|error| Error::new(ErrorKind::Parse, format!(".rune: {error}")))?;

    if manifest.version != SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::Parse,
            format!(
                ".rune: schema version {} is not supported (this build only understands version {})",
                manifest.version, SCHEMA_VERSION
            ),
        ));
    }

    for source_label in manifest.artifacts.keys() {
        if !manifest.sources.contains_key(source_label) {
            return Err(Error::new(
                ErrorKind::Parse,
                format!(".rune: artifacts entry '{source_label}' has no matching `sources` entry"),
            ));
        }
    }

    Ok(manifest)
}
