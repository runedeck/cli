//! The ownership manifest: the typed, versioned record of every path the
//! conversion owns, with SHA-256 hashes so tampering and drift are caught
//! before any file moves.

use super::io_error;
use super::walk::{classify_path, is_reserved_state_path, reject_symlink};
use crate::error::{Error, ErrorKind};
use crate::spec::SpecRoot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) const MANIFEST_VERSION: u32 = 1;
pub(super) const MANIFEST_PATH: &str = ".interop/openspec/manifest.yaml";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestEntry {
    pub(super) path: String,
    pub(super) classification: super::Classification,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub(super) version: u32,
    pub(super) entries: Vec<ManifestEntry>,
}

pub(super) fn manifest_path(spec_root: &SpecRoot) -> PathBuf {
    spec_root.base().join(MANIFEST_PATH)
}

pub(super) fn load_manifest(spec_root: &SpecRoot) -> Result<Manifest, Error> {
    let path = manifest_path(spec_root);
    reject_symlink(&path)?;
    let content = fs::read_to_string(&path).map_err(|error| io_error("read", &path, error))?;
    let manifest: Manifest = serde_yaml::from_str(&content).map_err(|error| {
        Error::new(
            ErrorKind::Validate,
            format!(
                "cannot parse OpenSpec interop manifest {}: {error}",
                path.display()
            ),
        )
    })?;
    validate_manifest(&manifest, &path)?;
    Ok(manifest)
}

pub(super) fn validate_manifest(manifest: &Manifest, path: &Path) -> Result<(), Error> {
    if manifest.version != MANIFEST_VERSION {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "unsupported OpenSpec interop manifest version {} at {}",
                manifest.version,
                path.display()
            ),
        ));
    }
    let mut ownership = BTreeSet::new();
    for entry in &manifest.entries {
        validate_manifest_entry(entry)?;
        if !ownership.insert(entry.path.clone()) {
            return Err(Error::new(
                ErrorKind::Validate,
                format!(
                    "duplicate OpenSpec ownership for '{}' in {}",
                    entry.path,
                    path.display()
                ),
            ));
        }
    }
    if manifest.entries.is_empty() {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "OpenSpec interop manifest has no entries: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn validate_manifest_entry(entry: &ManifestEntry) -> Result<(), Error> {
    let relative = validated_relative(&entry.path)?;
    if is_reserved_state_path(&entry.path) {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "manifest path is reserved for transaction state: {}",
                entry.path
            ),
        ));
    }
    let expected = classify_path(&entry.path);
    if expected != entry.classification {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "manifest classification {:?} does not own {}",
                entry.classification,
                relative.display()
            ),
        ));
    }
    if entry.sha256.len() != 64
        || !entry
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new(
            ErrorKind::Validate,
            format!("manifest SHA-256 is invalid for {}", entry.path),
        ));
    }
    Ok(())
}

pub(super) fn serialize_manifest(manifest: &Manifest) -> Result<Vec<u8>, Error> {
    let mut content = serde_yaml::to_string(manifest).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot serialize OpenSpec interop manifest: {error}"),
        )
    })?;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    Ok(content.into_bytes())
}

pub(super) fn validated_relative(relative: &str) -> Result<PathBuf, Error> {
    let path = Path::new(relative);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::new(
            ErrorKind::Validate,
            format!("OpenSpec manifest path must be relative: {relative}"),
        ));
    }
    Ok(path.to_path_buf())
}

pub(super) fn manifest_path_error(path: &Path, error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Validate,
        format!("invalid OpenSpec manifest path {}: {error}", path.display()),
    )
}
