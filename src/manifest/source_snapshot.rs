//! Read-only snapshots of explicitly authorized authored source roots.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

pub const SOURCE_SNAPSHOT_PATH: &str = ".provenance/source-snapshot.json";
pub const SOURCE_SNAPSHOT_VERSION: &str = "rune-source-snapshot/v1";
pub const SOURCE_SNAPSHOT_RECORD_VERSION: &str = "rune-provider-source-snapshot/v1";

/// Callers derive these paths from current configuration, never from a saved record.
#[derive(Debug, Clone)]
pub struct SourceInput {
    pub label: String,
    pub path: PathBuf,
    pub required: bool,
    /// Inline effective configuration has no filesystem path.
    pub bytes: Option<Vec<u8>>,
}

impl SourceInput {
    #[must_use]
    pub fn required(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            path: path.into(),
            required: true,
            bytes: None,
        }
    }

    #[must_use]
    pub fn optional(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            path: path.into(),
            required: false,
            bytes: None,
        }
    }

    #[must_use]
    pub fn value(label: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        Self {
            label: label.into(),
            path: PathBuf::new(),
            required: true,
            bytes: Some(bytes.as_ref().to_vec()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub version: String,
    pub digest: String,
    /// Semantic labels only. Saved records cannot authorize filesystem paths.
    pub roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshotRecord {
    pub version: String,
    pub source: SourceSnapshot,
    /// Provider-relative skill directories and their complete bundle digests.
    pub selected_skills: BTreeMap<String, String>,
    pub model_override: Option<String>,
}

impl SourceSnapshotRecord {
    #[must_use]
    pub fn new(source: SourceSnapshot, selected_skills: BTreeMap<String, String>) -> Self {
        Self {
            version: SOURCE_SNAPSHOT_RECORD_VERSION.into(),
            source,
            selected_skills,
            model_override: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != SOURCE_SNAPSHOT_RECORD_VERSION
            || self.source.version != SOURCE_SNAPSHOT_VERSION
            || !valid_digest(&self.source.digest)
            || self.source.roots.is_empty()
            || self.source.roots.windows(2).any(|pair| pair[0] >= pair[1])
            || self.source.roots.iter().any(|label| !valid_label(label))
            || self
                .model_override
                .as_ref()
                .is_some_and(|model| model.trim().is_empty() || model.chars().any(char::is_control))
        {
            return Err("invalid source snapshot identity".into());
        }
        for (path, digest) in &self.selected_skills {
            let components: Vec<_> = Path::new(path).components().collect();
            if components.len() < 2
                || components[0] != Component::Normal("skills".as_ref())
                || components
                    .iter()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || path.contains('\\')
                || path.chars().any(char::is_control)
                || !valid_digest(digest)
            {
                return Err("invalid selected skill snapshot".into());
            }
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct Entry {
    path: String,
    kind: &'static str,
    mode: u32,
    content_digest: Option<String>,
    symlink_target: Option<String>,
}

#[derive(Serialize)]
struct RootObservation {
    label: String,
    path: Option<String>,
    required: bool,
    entries: Vec<Entry>,
}

/// Hash every entry in each supplied source root, including optional absence.
///
/// Inputs must name authored trees or explicit configuration files, not whole
/// consumer checkouts. Nested symlinks are recorded without reading their targets.
/// Reserved VCS, provenance, and workspace directories do not enter the snapshot.
pub fn inspect(inputs: &[SourceInput]) -> Result<SourceSnapshot, String> {
    if inputs.is_empty() {
        return Err("source snapshot has no authoritative inputs".into());
    }
    let mut ordered = BTreeMap::new();
    for input in inputs {
        if !valid_label(&input.label) || ordered.insert(&input.label, input).is_some() {
            return Err("source snapshot labels must be non-empty and unique".into());
        }
    }
    let mut observations = Vec::new();
    for input in ordered.values() {
        if let Some(bytes) = &input.bytes {
            observations.push(RootObservation {
                label: input.label.clone(),
                path: None,
                required: true,
                entries: vec![Entry {
                    path: String::new(),
                    kind: "inline",
                    mode: 0,
                    content_digest: Some(super::content_sha256_bytes(bytes)),
                    symlink_target: None,
                }],
            });
            continue;
        }
        let absolute = std::path::absolute(&input.path).map_err(|error| error.to_string())?;
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !input.required => None,
            Err(error) => {
                return Err(format!(
                    "source input {} is unavailable: {error}",
                    input.label
                ));
            }
        };
        let mut entries = Vec::new();
        if let Some(metadata) = metadata {
            if metadata.is_symlink() {
                return Err(format!(
                    "source input {} must not be a symlink",
                    input.label
                ));
            }
            collect(&absolute, Path::new(""), &metadata, &mut entries)?;
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        observations.push(RootObservation {
            label: input.label.clone(),
            path: Some(path_text(&absolute)?),
            required: input.required,
            entries,
        });
    }
    let bytes = serde_json::to_vec(&(SOURCE_SNAPSHOT_VERSION, &observations))
        .map_err(|error| error.to_string())?;
    Ok(SourceSnapshot {
        version: SOURCE_SNAPSHOT_VERSION.into(),
        digest: super::content_sha256_bytes(&bytes),
        roots: observations.into_iter().map(|root| root.label).collect(),
    })
}

fn collect(
    root: &Path,
    relative: &Path,
    metadata: &fs::Metadata,
    entries: &mut Vec<Entry>,
) -> Result<(), String> {
    let path = if relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };
    let mut entry = Entry {
        path: path_text(relative)?,
        kind: "file",
        mode: mode(metadata),
        content_digest: None,
        symlink_target: None,
    };
    if metadata.is_symlink() {
        entry.kind = "symlink";
        entry.mode = 0;
        let target = fs::read_link(&path).map_err(|error| error.to_string())?;
        validate_symlink(relative, &target)?;
        entry.symlink_target = Some(path_text(&target)?);
    } else if metadata.is_file() {
        entry.content_digest = Some(file_digest(&path)?);
    } else if metadata.is_dir() {
        entry.kind = "directory";
        for child in fs::read_dir(&path)
            .map_err(|error| format!("cannot read source {}: {error}", path.display()))?
        {
            let child = child.map_err(|error| error.to_string())?;
            if excluded(&child.file_name()) {
                continue;
            }
            let child_relative = relative.join(child.file_name());
            let metadata = fs::symlink_metadata(child.path()).map_err(|error| error.to_string())?;
            collect(root, &child_relative, &metadata, entries)?;
        }
    } else {
        return Err(format!("unsupported source entry: {}", path.display()));
    }
    entries.push(entry);
    Ok(())
}

fn excluded(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            ".git"
                | ".jj"
                | ".provenance"
                | ".workspaces"
                | ".worktrees"
                | ".codex-prs"
                | "__pycache__"
        )
    )
}

fn file_digest(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("cannot read source {}: {error}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read source {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_symlink(relative: &Path, target: &Path) -> Result<(), String> {
    let mut resolved: Vec<_> = relative
        .parent()
        .unwrap_or(Path::new(""))
        .components()
        .collect();
    for component in target.components() {
        match component {
            Component::Normal(name) if !excluded(name) => resolved.push(component),
            Component::CurDir => {}
            Component::ParentDir if resolved.pop().is_some() => {}
            _ => return Err("source symlink leaves the observed authored tree".into()),
        }
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<String, String> {
    let text = path.to_str().ok_or("source paths must be valid UTF-8")?;
    if text.contains('\\') || text.chars().any(char::is_control) {
        return Err("source path has an ambiguous separator or control character".into());
    }
    Ok(text.into())
}

fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && !label.chars().any(char::is_control)
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(unix)]
fn mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode(metadata: &fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}

/// Inspect the complete selected skill set in a completed provider build.
pub fn selected_skill_bundles(provider_root: &Path) -> Result<BTreeMap<String, String>, String> {
    let skills = provider_root.join("skills");
    match fs::symlink_metadata(&skills) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.to_string()),
        Ok(metadata) if !metadata.is_dir() => {
            return Err("selected skills root is not a directory".into());
        }
        Ok(_) => {}
    }
    let mut selected = BTreeMap::new();
    selected_below(provider_root, &skills, &mut selected)?;
    Ok(selected)
}

fn selected_below(
    provider_root: &Path,
    root: &Path,
    selected: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    if root.join("SKILL.md").is_file() {
        let bundle = super::bundle::inspect_bundle(root);
        if !bundle.problems.is_empty() {
            return Err(format!("invalid selected skill bundle: {}", root.display()));
        }
        let digest = bundle
            .digest
            .ok_or("selected skill bundle has no complete digest")?;
        let key = path_text(
            root.strip_prefix(provider_root)
                .map_err(|error| error.to_string())?,
        )?;
        selected.insert(key, digest);
        return Ok(());
    }
    for child in fs::read_dir(root).map_err(|error| error.to_string())? {
        let child = child.map_err(|error| error.to_string())?;
        if child.file_name() == super::PROVENANCE_DIRECTORY {
            continue;
        }
        let metadata = fs::symlink_metadata(child.path()).map_err(|error| error.to_string())?;
        if !metadata.is_dir() {
            return Err(format!(
                "selected entry has no skill bundle: {}",
                child.path().display()
            ));
        }
        selected_below(provider_root, &child.path(), selected)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_snapshot_tests.rs"]
mod tests;
