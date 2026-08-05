//! Conversion planning: every source, destination, collision, and ancestor
//! is checked and the complete write/removal plan is built before the
//! transaction touches a single live file.

use super::manifest::{
    MANIFEST_VERSION, Manifest, ManifestEntry, load_manifest, manifest_path, serialize_manifest,
};
use super::verify::{
    ensure_import_has_no_unowned_source, verify_imported_manifest, verify_owned_file,
};
use super::walk::{
    classify_path, deepest_first, is_reserved_state_path, native_path, openspec_root,
    reject_symlink, walk_regular_files,
};
use super::{Classification, INTEROP_DIRECTORY, LOCK_FILE, MIRROR_DIRECTORY, io_error};
use crate::error::{Error, ErrorKind};
use crate::spec::SpecRoot;
use crate::spec::transaction::{self, FileRemoval, FileWrite};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(super) struct ConversionPlan {
    pub(super) manifest: Manifest,
    pub(super) writes: Vec<FileWrite>,
    pub(super) removals: Vec<FileRemoval>,
    pub(super) removable_directories: Vec<PathBuf>,
    pub(super) destination: PathBuf,
}

pub(super) fn preflight_import(spec_root: &SpecRoot) -> Result<ConversionPlan, Error> {
    let manifest_path = manifest_path(spec_root);
    if manifest_path.exists() {
        let manifest = load_manifest(spec_root)?;
        verify_imported_manifest(spec_root, &manifest)?;
        ensure_import_has_no_unowned_source(spec_root, &manifest)?;
        return Ok(ConversionPlan {
            manifest,
            writes: Vec::new(),
            removals: Vec::new(),
            removable_directories: Vec::new(),
            destination: spec_root.base().to_path_buf(),
        });
    }

    let openspec = openspec_root(spec_root);
    if !openspec.exists() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("nothing to import from {}", openspec.display()),
        ));
    }
    let walked = walk_regular_files(&openspec)?;
    let mut entries = Vec::new();
    let mut writes = Vec::new();
    let mut removals = Vec::new();
    let mut destinations = BTreeSet::new();

    for (relative, source) in walked.files {
        if relative == LOCK_FILE {
            continue;
        }
        if is_reserved_state_path(&relative) {
            return Err(Error::new(
                ErrorKind::Config,
                format!("OpenSpec source contains reserved transaction state: {relative}"),
            ));
        }
        let classification = classify_path(&relative);
        let destination = native_path(spec_root, &relative, classification)?;
        let sha256 = transaction::sha256_file(&source)?;
        let content = fs::read(&source).map_err(|error| io_error("read", &source, error))?;
        if !destinations.insert(destination.clone()) {
            return Err(Error::new(
                ErrorKind::Config,
                format!("multiple OpenSpec files map to {}", destination.display()),
            ));
        }
        if source != destination {
            preflight_destination(spec_root.repository(), &destination)?;
            writes.push(FileWrite {
                path: destination,
                content,
            });
            removals.push(FileRemoval {
                path: source,
                sha256: sha256.clone(),
            });
        }
        entries.push(ManifestEntry {
            path: relative,
            classification,
            sha256,
        });
    }
    if entries.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("nothing to import from {}", openspec.display()),
        ));
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        entries,
    };
    let manifest_content = serialize_manifest(&manifest)?;
    preflight_destination(spec_root.repository(), &manifest_path)?;
    writes.push(FileWrite {
        path: manifest_path,
        content: manifest_content,
    });

    let removable_directories = walked
        .directories
        .into_iter()
        .filter(|directory| directory != spec_root.base())
        .collect();
    Ok(ConversionPlan {
        manifest,
        writes,
        removals,
        removable_directories: deepest_first(removable_directories),
        destination: spec_root.base().to_path_buf(),
    })
}

pub(super) fn preflight_export(spec_root: &SpecRoot) -> Result<ConversionPlan, Error> {
    let manifest_path = manifest_path(spec_root);
    let manifest = if manifest_path.exists() {
        let manifest = load_manifest(spec_root)?;
        verify_imported_manifest(spec_root, &manifest)?;
        manifest
    } else {
        manifest_from_native_trees(spec_root)?
    };
    let destination_root = openspec_root(spec_root);
    reject_symlink(&destination_root)?;
    let mut writes = Vec::new();
    let mut removals = Vec::new();
    let mut destinations = BTreeSet::new();
    let mut removable_directories = BTreeSet::new();

    for entry in &manifest.entries {
        let source = native_path(spec_root, &entry.path, entry.classification)?;
        verify_owned_file(&source, &entry.sha256)?;
        let destination = destination_root.join(Path::new(&entry.path));
        if !destinations.insert(destination.clone()) {
            return Err(Error::new(
                ErrorKind::Config,
                format!("manifest entries collide at {}", destination.display()),
            ));
        }
        if source == destination {
            continue;
        }
        preflight_destination(spec_root.repository(), &destination)?;
        let content = fs::read(&source).map_err(|error| io_error("read", &source, error))?;
        writes.push(FileWrite {
            path: destination,
            content,
        });
        removals.push(FileRemoval {
            path: source.clone(),
            sha256: entry.sha256.clone(),
        });
        collect_owned_ancestors(
            spec_root,
            entry.classification,
            &source,
            &mut removable_directories,
        );
    }
    if manifest_path.exists() {
        let manifest_sha256 = transaction::sha256_file(&manifest_path)?;
        removals.push(FileRemoval {
            path: manifest_path.clone(),
            sha256: manifest_sha256,
        });
        // The mirror directory only exists when the manifest recorded opaque
        // files; the transaction refuses removable paths that are not
        // directories, so schedule only what is actually on disk.
        for reserved in [INTEROP_DIRECTORY, MIRROR_DIRECTORY] {
            let directory = spec_root.base().join(reserved);
            if directory.is_dir() {
                removable_directories.insert(directory);
            }
        }
    }

    Ok(ConversionPlan {
        manifest,
        writes,
        removals,
        removable_directories: deepest_first(removable_directories.into_iter().collect()),
        destination: destination_root,
    })
}

fn manifest_from_native_trees(spec_root: &SpecRoot) -> Result<Manifest, Error> {
    let mut entries = Vec::new();
    collect_native_entries(
        spec_root.changes(),
        "changes",
        Classification::Change,
        &mut entries,
    )?;
    collect_native_entries(
        spec_root.specifications(),
        "specs",
        Classification::Specification,
        &mut entries,
    )?;
    if entries.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("nothing to export from {}", spec_root.base().display()),
        ));
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Manifest {
        version: MANIFEST_VERSION,
        entries,
    })
}

fn collect_native_entries(
    root: &Path,
    prefix: &str,
    classification: Classification,
    entries: &mut Vec<ManifestEntry>,
) -> Result<(), Error> {
    if !root.exists() {
        return Ok(());
    }
    for (relative, source) in walk_regular_files(root)?.files {
        entries.push(ManifestEntry {
            path: format!("{prefix}/{relative}"),
            classification,
            sha256: transaction::sha256_file(&source)?,
        });
    }
    Ok(())
}

fn collect_owned_ancestors(
    spec_root: &SpecRoot,
    classification: Classification,
    source: &Path,
    directories: &mut BTreeSet<PathBuf>,
) {
    let boundary = match classification {
        Classification::Change => spec_root.changes().to_path_buf(),
        Classification::Specification => spec_root.specifications().to_path_buf(),
        Classification::File => spec_root.base().join(MIRROR_DIRECTORY),
    };
    let mut current = source.parent();
    while let Some(directory) = current {
        if !directory.starts_with(&boundary) {
            break;
        }
        directories.insert(directory.to_path_buf());
        if directory == boundary {
            break;
        }
        current = directory.parent();
    }
}

fn preflight_destination(repository: &Path, destination: &Path) -> Result<(), Error> {
    if !destination.starts_with(repository) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "conversion destination escapes repository: {}",
                destination.display()
            ),
        ));
    }
    let relative = destination.strip_prefix(repository).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("cannot confine {}: {error}", destination.display()),
        )
    })?;
    let mut current = repository.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("invalid conversion destination: {}", destination.display()),
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "conversion destination contains a symlink: {}",
                        current.display()
                    ),
                ));
            }
            Ok(_) if current == destination => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "{} already exists; conversion refuses to overwrite (nothing was written)",
                        destination.display()
                    ),
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "conversion destination parent is not a directory: {}",
                        current.display()
                    ),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(io_error("inspect", &current, error)),
        }
    }
    Ok(())
}
