pub mod bundle;
pub mod drafts;
pub(crate) mod extract;
pub mod provenance;
mod read;
pub mod source_snapshot;
mod staleness;
mod statement;
mod status;
mod write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use read::read;
pub use staleness::check_sources;
pub use statement::{
    generate_adopt_statement, generate_adopt_statement_with_transforms, generate_statement,
};
pub use status::{status, status_bytes};
pub use write::write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FileStatus {
    New,
    Unchanged,
    Stale,
    Modified,
}

impl FileStatus {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            FileStatus::New => "new",
            FileStatus::Unchanged => "unchanged",
            FileStatus::Stale => "stale",
            FileStatus::Modified => "modified",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub fingerprint: String,
    pub provenance: Option<String>,
}

pub const SIDECAR_EXTENSION: &str = "yaml";
pub const PROVENANCE_DIRECTORY: &str = ".provenance";

/// Compute the provenance sidecar path relative to the provider target.
///
/// The full filename is encoded so same-stem files cannot collide on one
/// sidecar (`logo.png` and `logo.svg` get distinct sidecars):
///
/// `rules/CurrencyFormatting.md` → `rules/.provenance/CurrencyFormatting.md.yaml`
/// `skills/SessionPrep/SKILL.md` → `skills/SessionPrep/.provenance/SKILL.md.yaml`
pub fn provenance_path(manifest_key: &str) -> String {
    let path = std::path::Path::new(manifest_key);
    let parent = path.parent().unwrap_or(std::path::Path::new(""));
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();

    let provenance_dir = parent.join(PROVENANCE_DIRECTORY);
    slash_lossy(&provenance_dir.join(format!("{file_name}.{SIDECAR_EXTENSION}")))
}

/// The stem-named sidecar location written by earlier versions
/// (`CurrencyFormatting.md` → `.provenance/CurrencyFormatting.yaml`).
/// Read-side fallback only; nothing writes this shape anymore.
pub fn legacy_provenance_path(manifest_key: &str) -> String {
    let path = std::path::Path::new(manifest_key);
    let parent = path.parent().unwrap_or(std::path::Path::new(""));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();

    let provenance_dir = parent.join(PROVENANCE_DIRECTORY);
    slash_lossy(&provenance_dir.join(format!("{stem}.{SIDECAR_EXTENSION}")))
}

/// Sidecar path beside a file on disk, full-filename encoded.
pub fn sidecar_for(file_path: &std::path::Path) -> std::path::PathBuf {
    let parent = file_path.parent().unwrap_or(std::path::Path::new("."));
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
    parent
        .join(PROVENANCE_DIRECTORY)
        .join(format!("{file_name}.{SIDECAR_EXTENSION}"))
}

/// Read-side sidecar resolution: the full-filename sidecar when present,
/// else the legacy stem-named sidecar. `None` when neither exists.
pub fn existing_sidecar_for(file_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let current = sidecar_for(file_path);
    if current.is_file() {
        return Some(current);
    }
    let parent = file_path.parent().unwrap_or(std::path::Path::new("."));
    let stem = file_path.file_stem().unwrap_or_default().to_string_lossy();
    let legacy = parent
        .join(PROVENANCE_DIRECTORY)
        .join(format!("{stem}.{SIDECAR_EXTENSION}"));
    legacy.is_file().then_some(legacy)
}

/// Keep generated build provenance in the same reserved namespace as deployment.
/// Authored companions such as `data` and `data.yaml` never share a metadata path.
pub fn sidecar_path(content_path: &std::path::Path) -> std::path::PathBuf {
    sidecar_for(content_path)
}

/// A path as text with `/` between its parts on every platform, so a
/// record and its digest are the same wherever they are made.
///
/// Windows writes `\` between the parts, and no file name can hold one
/// there, so each `\` is a separator and becomes `/`. On Unix a `\` is a
/// character of a name. It stays, and the caller refuses the path as
/// ambiguous: a record with it could not be told from a Windows path.
/// A path that is not valid UTF-8 has no text.
#[must_use]
pub fn slash_text(path: &std::path::Path) -> Option<String> {
    let text = path.to_str()?;
    if cfg!(windows) {
        Some(text.replace('\\', "/"))
    } else {
        Some(text.to_owned())
    }
}

/// [`slash_text`] for a path that is a key or is shown, where a name that
/// is not valid UTF-8 is written with replacement characters instead of
/// refused. A relative path under a module, a build, or a target is such a
/// path: `rules/nested/Style.md` on every platform.
#[must_use]
pub fn slash_lossy(path: &std::path::Path) -> String {
    slash_text(path).unwrap_or_else(|| {
        let text = path.to_string_lossy();
        if cfg!(windows) {
            text.replace('\\', "/")
        } else {
            text.into_owned()
        }
    })
}

pub fn content_sha256(content: &str) -> String {
    content_sha256_bytes(content.as_bytes())
}

pub fn content_sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests;
