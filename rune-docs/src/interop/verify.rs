//! Ownership verification: every owned file's bytes must match the
//! manifest's hash, mirrored files must match the manifest exactly, and an
//! import over an existing manifest refuses sources the manifest does not
//! own.

use super::manifest::{Manifest, manifest_path, validate_manifest};
use super::walk::{
    is_reserved_state_path, native_path, openspec_root, reject_symlink, walk_regular_files,
};
use super::{Classification, LOCK_FILE, MIRROR_DIRECTORY, io_error};
use crate::error::{Error, ErrorKind};
use crate::spec::SpecRoot;
use crate::spec::transaction;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) fn verify_imported_manifest(
    spec_root: &SpecRoot,
    manifest: &Manifest,
) -> Result<(), Error> {
    validate_manifest(manifest, &manifest_path(spec_root))?;
    let mut mirrored_files = BTreeSet::new();
    for entry in &manifest.entries {
        let source = native_path(spec_root, &entry.path, entry.classification)?;
        verify_owned_file(&source, &entry.sha256)?;
        if entry.classification == Classification::File {
            mirrored_files.insert(source);
        }
    }
    let mirror_root = spec_root.base().join(MIRROR_DIRECTORY);
    let actual_mirrors = if mirror_root.exists() {
        walk_regular_files(&mirror_root)?
            .files
            .into_iter()
            .map(|(_, path)| path)
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    if actual_mirrors != mirrored_files {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "mirrored OpenSpec files do not match manifest ownership at {}",
                mirror_root.display()
            ),
        ));
    }
    Ok(())
}

pub(super) fn verify_exported_manifest(
    spec_root: &SpecRoot,
    manifest: &Manifest,
) -> Result<(), Error> {
    let openspec = openspec_root(spec_root);
    for entry in &manifest.entries {
        verify_owned_file(&openspec.join(Path::new(&entry.path)), &entry.sha256)?;
    }
    Ok(())
}

pub(super) fn ensure_import_has_no_unowned_source(
    spec_root: &SpecRoot,
    manifest: &Manifest,
) -> Result<(), Error> {
    let openspec = openspec_root(spec_root);
    if !openspec.exists() {
        return Ok(());
    }
    let mut allowed_aliases = BTreeSet::new();
    for entry in &manifest.entries {
        let native = native_path(spec_root, &entry.path, entry.classification)?;
        let original = openspec.join(Path::new(&entry.path));
        if native == original {
            allowed_aliases.insert(entry.path.clone());
        }
    }
    let remaining = walk_regular_files(&openspec)?
        .files
        .into_iter()
        .map(|(relative, _)| relative)
        .filter(|relative| relative != LOCK_FILE)
        .filter(|relative| !is_reserved_state_path(relative))
        .filter(|relative| !allowed_aliases.contains(relative))
        .collect::<Vec<_>>();
    if remaining.is_empty() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Config,
            format!(
                "OpenSpec source contains files not owned by the existing manifest: {}",
                remaining.join(", ")
            ),
        ))
    }
}

pub(super) fn verify_owned_file(path: &Path, expected_sha256: &str) -> Result<(), Error> {
    reject_symlink(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error("inspect", path, error))?;
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "owned OpenSpec artifact is not a regular file: {}",
                path.display()
            ),
        ));
    }
    let actual = transaction::sha256_file(path)?;
    if actual == expected_sha256 {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Validate,
            format!(
                "OpenSpec ownership hash mismatch for {}: expected {expected_sha256}, found {actual}",
                path.display()
            ),
        ))
    }
}
