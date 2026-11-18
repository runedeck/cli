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

/// The volume serial number and the file index, read from an open handle.
/// Stable Rust gives neither from `fs::Metadata` on Windows, and an
/// identity with no value in it would make every file equal every other.
///
/// The index is the 64-bit one. NTFS keeps it unique on a volume. `ReFS`
/// names a file with a 128-bit identifier and gives the 64-bit index only
/// where it can map the long identifier to it without loss. Everywhere
/// else it reports all bits set, and a file system with no index reports
/// zero. Neither value names a file, so `windows_identity` refuses both
/// instead of letting two files compare equal.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity {
    volume: u64,
    index: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity;

/// `FILE_FLAG_BACKUP_SEMANTICS`: lets a directory open, so a directory is
/// reported as not a regular file instead of as an access error.
#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// `FILE_FLAG_OPEN_REPARSE_POINT`: opens a symlink itself, not its target.
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

#[cfg(any(unix, windows))]
fn not_a_regular_file(path: &Path) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("transaction path is not a regular file: {}", path.display()),
    )
}

/// The identity of an open regular file. The caller keeps the handle, so
/// the identity names the file the caller reads, not whatever the path
/// names a moment later.
#[cfg(any(unix, windows))]
pub(super) fn open_file_identity(file: &fs::File, path: &Path) -> Result<FileIdentity, Error> {
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect", path, error))?;
    if !metadata.is_file() {
        return Err(not_a_regular_file(path));
    }
    #[cfg(unix)]
    {
        Ok(unix_identity(&metadata))
    }
    #[cfg(windows)]
    {
        windows_identity(file, path)
    }
}

#[cfg(unix)]
fn unix_identity(metadata: &fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

/// Whether a 64-bit file index names one file. Zero means the file system
/// keeps no index. All bits set means it could not give this file a
/// unique one, which `ReFS` does for a file whose 128-bit identifier does
/// not fit.
#[cfg(windows)]
pub(super) const fn index_names_one_file(index: u64) -> bool {
    index != 0 && index != u64::MAX
}

/// The identity behind an open handle to a regular file. Every Windows
/// identity comes through here, so a path and an open file are held to
/// the same rules: the handle is not a console, a pipe, or another
/// device, and its index names one file.
#[cfg(windows)]
fn windows_identity(file: &fs::File, path: &Path) -> Result<FileIdentity, Error> {
    let kind = winapi_util::file::typ(file).map_err(|error| io_error("inspect", path, error))?;
    if kind.is_char() || kind.is_pipe() || kind.is_unknown() {
        return Err(not_a_regular_file(path));
    }
    let information =
        winapi_util::file::information(file).map_err(|error| io_error("inspect", path, error))?;
    let index = information.file_index();
    if !index_names_one_file(index) {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "cannot prove file identity because the file system gives no unique file index: {}",
                path.display()
            ),
        ));
    }
    Ok(FileIdentity {
        volume: information.volume_serial_number(),
        index,
    })
}

/// Opens `path` without following a symlink and without any access right:
/// reading the file information needs none. This is the handle form of
/// `fs::symlink_metadata`.
#[cfg(windows)]
fn open_without_following(path: &Path) -> Result<fs::File, Error> {
    use std::os::windows::fs::OpenOptionsExt as _;
    fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|error| io_error("inspect", path, error))
}

/// The identity behind a handle from `open_without_following`. The
/// metadata of that handle classifies the path as `fs::symlink_metadata`
/// does: a symlink or a junction is refused, and so is a directory. A
/// deduplicated file or a cloud file also carries a reparse point, but it
/// is a regular file with its own index, and a followed open of the same
/// path reads that same index.
#[cfg(windows)]
fn unfollowed_identity(file: &fs::File, path: &Path) -> Result<FileIdentity, Error> {
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect", path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(not_a_regular_file(path));
    }
    windows_identity(file, path)
}

pub(super) fn file_identity(path: &Path) -> Result<FileIdentity, Error> {
    #[cfg(unix)]
    {
        let metadata =
            fs::symlink_metadata(path).map_err(|error| io_error("inspect", path, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(not_a_regular_file(path));
        }
        Ok(unix_identity(&metadata))
    }
    #[cfg(windows)]
    {
        unfollowed_identity(&open_without_following(path)?, path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Error::new(
            ErrorKind::Io,
            "cannot verify file identity on this platform",
        ))
    }
}

pub(super) fn is_same_file(left: &Path, right: &Path) -> Result<bool, Error> {
    #[cfg(windows)]
    {
        // Both handles stay open until the comparison. A file index is
        // unique only while the file exists, and an open handle keeps it
        // from being given to another file.
        let left_file = open_without_following(left)?;
        let right_file = open_without_following(right)?;
        Ok(unfollowed_identity(&left_file, left)? == unfollowed_identity(&right_file, right)?)
    }
    #[cfg(not(windows))]
    {
        Ok(file_identity(left)? == file_identity(right)?)
    }
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
