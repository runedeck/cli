//! Staged filesystem primitives: exclusive-creation copies of files and
//! whole trees, atomic replacement through a same-directory temporary, and
//! the cleanup that keeps a failed step from leaving residue behind.

use super::confine::{create_confined_directories, reject_symlink};
use super::io_error;
use crate::error::{Error, ErrorKind};
use crate::support::temporary_sibling;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub(super) fn copy_tree_exclusive(source: &Path, destination: &Path) -> Result<(), Error> {
    let metadata =
        fs::symlink_metadata(source).map_err(|error| io_error("inspect", source, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "archive source is not a regular directory: {}",
                source.display()
            ),
        ));
    }
    let destination_parent = destination.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("tree destination has no parent: {}", destination.display()),
        )
    })?;
    fs::create_dir_all(destination_parent)
        .map_err(|error| io_error("create", destination_parent, error))?;
    fs::create_dir(destination).map_err(|error| io_error("create", destination, error))?;
    match copy_tree_contents(source, destination) {
        Ok(()) => Ok(()),
        Err(error) => Err(cleanup_tree_after_error(destination, error)),
    }
}

fn copy_tree_contents(source: &Path, destination: &Path) -> Result<(), Error> {
    let mut entries = fs::read_dir(source)
        .map_err(|error| io_error("read", source, error))?
        .map(|entry| entry.map_err(|error| io_error("read", source, error)))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| io_error("inspect", &source_path, error))?;
        if file_type.is_symlink() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "archive trees cannot contain symlinks: {}",
                    source_path.display()
                ),
            ));
        }
        if file_type.is_dir() {
            fs::create_dir(&destination_path)
                .map_err(|error| io_error("create", &destination_path, error))?;
            copy_tree_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            copy_file_exclusive(&source_path, &destination_path)?;
        } else {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "archive trees require regular files: {}",
                    source_path.display()
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn copy_file_exclusive(source: &Path, destination: &Path) -> Result<(), Error> {
    let content = fs::read(source).map_err(|error| io_error("read", source, error))?;
    write_file_exclusive(destination, &content)
}

pub(super) fn write_file_exclusive(path: &Path, content: &[u8]) -> Result<(), Error> {
    let parent = path.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("file path has no parent: {}", path.display()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| io_error("create", parent, error))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create", path, error))?;
    file.write_all(content)
        .map_err(|error| io_error("write", path, error))?;
    file.sync_all()
        .map_err(|error| io_error("sync", path, error))
}

pub(super) fn write_file_atomic(path: &Path, content: &[u8]) -> Result<(), Error> {
    let parent = path.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("file path has no parent: {}", path.display()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| io_error("create", parent, error))?;
    let temporary = temporary_sibling(path);
    if let Err(error) = write_file_exclusive(&temporary, content) {
        return Err(cleanup_file_after_error(&temporary, error));
    }
    if let Err(error) = fs::rename(&temporary, path) {
        return Err(cleanup_file_after_error(
            &temporary,
            io_error("replace", path, error),
        ));
    }
    let directory = File::open(parent).map_err(|error| io_error("open", parent, error))?;
    directory
        .sync_all()
        .map_err(|error| io_error("sync", parent, error))
}

pub(super) fn replace_file_from_backup(
    root: &Path,
    backup: &Path,
    destination: &Path,
) -> Result<(), Error> {
    let parent = destination.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("file path has no parent: {}", destination.display()),
        )
    })?;
    create_confined_directories(root, parent)?;
    let temporary = temporary_sibling(destination);
    if let Err(error) = copy_file_exclusive(backup, &temporary) {
        return Err(cleanup_file_after_error(&temporary, error));
    }
    fs::rename(&temporary, destination).map_err(|error| {
        cleanup_file_after_error(&temporary, io_error("restore", destination, error))
    })
}

pub(super) fn cleanup_file_after_error(path: &Path, primary: Error) -> Error {
    match fs::remove_file(path) {
        Ok(()) => primary,
        Err(error) if error.kind() == io::ErrorKind::NotFound => primary,
        Err(error) => Error::new(
            ErrorKind::Io,
            format!(
                "{}; cannot clean temporary file {}: {error}",
                primary.message(),
                path.display()
            ),
        ),
    }
}

pub(super) fn cleanup_tree_after_error(path: &Path, primary: Error) -> Error {
    match fs::remove_dir_all(path) {
        Ok(()) => primary,
        Err(error) if error.kind() == io::ErrorKind::NotFound => primary,
        Err(error) => Error::new(
            ErrorKind::Io,
            format!(
                "{}; cannot clean staged tree {}: {error}",
                primary.message(),
                path.display()
            ),
        ),
    }
}

pub(super) fn remove_verified_tree(path: &Path) -> Result<(), Error> {
    reject_symlink(path)?;
    fs::remove_dir_all(path).map_err(|error| io_error("remove", path, error))
}

pub(super) fn archive_staging_path(archive_path: &Path) -> Result<PathBuf, Error> {
    let parent = archive_path.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("archive path has no parent: {}", archive_path.display()),
        )
    })?;
    let name = archive_path.file_name().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("archive path has no name: {}", archive_path.display()),
        )
    })?;
    Ok(parent.join(format!(".{}.rune-stage", name.to_string_lossy())))
}
