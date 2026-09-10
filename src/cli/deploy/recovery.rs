//! Restore validated migration destinations when the current attempt fails.

use super::{copy_file, load_deployed_manifest, migration::canonical_target, write_manifest};
use rune::error::{Error, ErrorKind};
use rune::manifest::{self, ManifestEntry};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub(super) struct Recovery {
    directory: Option<TempDir>,
    root: PathBuf,
    physical_root: PathBuf,
    bundles: Vec<PathBuf>,
    entries: HashMap<String, ManifestEntry>,
}

impl Recovery {
    /// The caller must validate each complete bundle before this operation.
    pub(super) fn begin(root: &Path, bundles: Vec<PathBuf>) -> Result<Self, Error> {
        let parent = root
            .ancestors()
            .skip(1)
            .find(|path| path.is_dir())
            .ok_or_else(|| io_error("migration recovery has no existing parent"))?;
        let directory = tempfile::Builder::new()
            .prefix(".rune-migration-")
            .tempdir_in(parent)
            .map_err(io_error)?;
        let entries = load_deployed_manifest(root)?;
        let backup = directory.path().join("before");
        fs::create_dir(&backup).map_err(io_error)?;
        fs::write(
            directory.path().join("destination.txt"),
            root.to_string_lossy().as_bytes(),
        )
        .map_err(io_error)?;
        fs::write(
            backup.join("manifest.yaml"),
            manifest::write(&entries).map_err(io_error)?,
        )
        .map_err(io_error)?;
        for relative in &bundles {
            let source = root.join(relative);
            if source.symlink_metadata().is_ok() {
                copy_tree(&source, &backup.join(relative))?;
            }
        }
        Ok(Self {
            directory: Some(directory),
            root: root.to_path_buf(),
            physical_root: canonical_target(root)?,
            bundles,
            entries,
        })
    }

    pub(super) fn commit(mut self) {
        // This directory contains only copies that this guard created.
        self.directory.take();
    }

    fn restore_bundle(&self, recovery: &Path, relative: &Path) -> Result<(), Error> {
        let active = self.root.join(relative);
        let parent = active.parent().ok_or_else(|| io_error("invalid bundle"))?;
        if canonical_target(&self.root)? != self.physical_root
            || !canonical_target(parent)?.starts_with(&self.physical_root)
        {
            return Err(io_error(
                "migration destination changed to an external path",
            ));
        }
        if active.symlink_metadata().is_ok() {
            let failed = recovery.join("failed").join(relative);
            fs::create_dir_all(failed.parent().unwrap_or(recovery)).map_err(io_error)?;
            fs::rename(&active, failed).map_err(io_error)?;
        }
        let backup = recovery.join("before").join(relative);
        if backup.symlink_metadata().is_ok() {
            fs::create_dir_all(parent).map_err(io_error)?;
            fs::rename(backup, active).map_err(io_error)?;
        }
        Ok(())
    }

    fn restore_manifest(&self, recovery: &Path, restored: &[&Path]) -> Result<(), Error> {
        if canonical_target(&self.root)? != self.physical_root {
            return Err(io_error(
                "migration manifest root changed to an external path",
            ));
        }
        let mut current = load_deployed_manifest(&self.root)?;
        let before = manifest::write(&current).map_err(io_error)?;
        // Unrelated successful writes keep their current claims.
        current.retain(|key, _| !self.bundles.iter().any(|path| owns(path, key)));
        for (key, entry) in &self.entries {
            if restored.iter().any(|path| owns(path, key)) {
                current.insert(key.clone(), entry.clone());
            }
        }
        if before == manifest::write(&current).map_err(io_error)? {
            return Ok(());
        }
        let manifest_file = self.root.join(".manifest");
        if manifest_file.symlink_metadata().is_ok() {
            copy_file(&manifest_file, &recovery.join("failed-manifest"))?;
        }
        write_manifest(&self.root, &current)
    }
}

impl Drop for Recovery {
    fn drop(&mut self) {
        let Some(directory) = self.directory.take() else {
            return;
        };
        let recovery = directory.keep();
        let mut restored = Vec::new();
        for relative in &self.bundles {
            match self.restore_bundle(&recovery, relative) {
                Ok(()) => restored.push(relative.as_path()),
                Err(error) => eprintln!(
                    "rune migration: cannot restore {}: {error}",
                    self.root.join(relative).display()
                ),
            }
        }
        if let Err(error) = self.restore_manifest(&recovery, &restored) {
            eprintln!("rune migration: cannot restore manifest claims: {error}");
        }
        eprintln!(
            "rune migration: preserved recovery files at {}",
            recovery.display()
        );
    }
}

fn owns(bundle: &Path, key: &str) -> bool {
    Path::new(key).starts_with(bundle)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), Error> {
    let metadata = source.symlink_metadata().map_err(io_error)?;
    if !metadata.is_dir() {
        return copy_file(source, destination);
    }
    fs::create_dir_all(destination).map_err(io_error)?;
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
    }
    fs::set_permissions(destination, metadata.permissions()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::Io, error.to_string()).with_code("deploy.migration_recovery")
}
