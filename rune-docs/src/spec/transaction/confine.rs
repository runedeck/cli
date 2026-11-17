//! Path confinement and file identity for the transaction: symlink
//! rejection, ancestor inspection for paths about to be created, and the
//! device/inode identity used to prove two paths are one file.

use super::io_error;
use crate::error::{Error, ErrorKind};
use std::fs;
use std::io;
use std::path::{Component, Path};

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity {
    volume: u64,
    index: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity;

#[cfg(unix)]
pub(super) fn metadata_identity(metadata: &fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

/// Stable std keeps the volume serial number and file index of
/// `fs::Metadata` behind an unstable feature on Windows, so the identity is
/// read from the open handle.
#[cfg(windows)]
pub(super) fn handle_identity(file: &fs::File) -> io::Result<FileIdentity> {
    let information = winapi_util::file::information(file)?;
    Ok(FileIdentity {
        volume: information.volume_serial_number(),
        index: information.file_index(),
    })
}

/// Metadata and identity of `path` itself, taken from one inspection so the
/// two cannot describe different files. A symlink is never followed.
#[cfg(unix)]
fn inspect_without_following(path: &Path) -> io::Result<(fs::Metadata, FileIdentity)> {
    let metadata = fs::symlink_metadata(path)?;
    let identity = metadata_identity(&metadata);
    Ok((metadata, identity))
}

#[cfg(windows)]
fn inspect_without_following(path: &Path) -> io::Result<(fs::Metadata, FileIdentity)> {
    use std::os::windows::fs::OpenOptionsExt as _;
    // The reparse-point flag opens a symlink itself rather than its target;
    // backup semantics lets a directory open so the caller rejects it by kind
    // instead of by an access error.
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let metadata = file.metadata()?;
    let identity = handle_identity(&file)?;
    Ok((metadata, identity))
}

pub(super) fn file_identity(path: &Path) -> Result<FileIdentity, Error> {
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        return Err(Error::new(
            ErrorKind::Io,
            "cannot verify file identity on this platform",
        ));
    }
    #[cfg(any(unix, windows))]
    {
        let (metadata, identity) =
            inspect_without_following(path).map_err(|error| io_error("inspect", path, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::new(
                ErrorKind::Io,
                format!("transaction path is not a regular file: {}", path.display()),
            ));
        }
        Ok(identity)
    }
}

pub(super) fn is_same_file(left: &Path, right: &Path) -> Result<bool, Error> {
    Ok(file_identity(left)? == file_identity(right)?)
}

pub(super) fn path_exists_without_following(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error("inspect", path, error)),
    }
}

/// Confinement for a path about to be created: walks the existing ancestors
/// of `candidate`, rejecting symlinks and escapes before anything is
/// written. The counterpart for paths that already exist is
/// `support::confine_existing`; the two forms are deliberately separate and
/// must not be collapsed.
pub(super) fn inspect_existing_ancestors(root: &Path, candidate: &Path) -> Result<(), Error> {
    let relative = candidate.strip_prefix(root).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                "path {} is outside spec root {}: {error}",
                candidate.display(),
                root.display()
            ),
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("invalid path inside spec root: {}", relative.display()),
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("transaction path contains a symlink: {}", current.display()),
                ));
            }
            Ok(_) => {
                let resolved = current
                    .canonicalize()
                    .map_err(|error| io_error("resolve", &current, error))?;
                if !resolved.starts_with(root) {
                    return Err(Error::new(
                        ErrorKind::Config,
                        format!("transaction path escapes spec root: {}", current.display()),
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(io_error("inspect", &current, error)),
        }
    }
    Ok(())
}

pub(super) fn create_confined_directories(root: &Path, directory: &Path) -> Result<(), Error> {
    inspect_existing_ancestors(root, directory)?;
    fs::create_dir_all(directory).map_err(|error| io_error("create", directory, error))?;
    inspect_existing_ancestors(root, directory)
}

/// Twin of `interop::reject_symlink`, kept separate so errors name
/// transaction paths instead of the `OpenSpec` conversion.
pub(super) fn reject_symlink(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::new(
            ErrorKind::Config,
            format!("transaction path is a symlink: {}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect", path, error)),
    }
}
