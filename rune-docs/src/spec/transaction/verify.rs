//! Hash verification for files and whole trees: SHA-256 over content, and
//! a deterministic tree digest (sorted entries, kind- and length-prefixed)
//! that proves a copied archive matches its source byte for byte.

use super::confine::{FileIdentity, metadata_identity, reject_symlink};
use super::io_error;
use crate::error::{Error, ErrorKind};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read as _;
use std::path::Path;

pub(super) fn hash_bytes(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, Error> {
    hash_file(path)
}

pub(super) fn hash_file(path: &Path) -> Result<String, Error> {
    let mut file = File::open(path).map_err(|error| io_error("open", path, error))?;
    hash_open_file(&mut file, path)
}

pub(super) fn hash_file_with_identity(path: &Path) -> Result<(String, FileIdentity), Error> {
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
        reject_symlink(path)?;
        let mut file = File::open(path).map_err(|error| io_error("open", path, error))?;
        let metadata = file
            .metadata()
            .map_err(|error| io_error("inspect", path, error))?;
        if !metadata.is_file() {
            return Err(Error::new(
                ErrorKind::Io,
                format!("transaction path is not a regular file: {}", path.display()),
            ));
        }
        let identity = metadata_identity(&metadata);
        Ok((hash_open_file(&mut file, path)?, identity))
    }
}

fn hash_open_file(file: &mut File, path: &Path) -> Result<String, Error> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error("read", path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn unexpected_hash_error(path: &Path, expected: &str, actual: &str) -> Error {
    Error::new(
        ErrorKind::Io,
        format!(
            "SHA-256 verification failed for {}: expected {expected}, found {actual}",
            path.display()
        ),
    )
}

pub(super) fn verify_file_hash(path: &Path, expected: &str) -> Result<(), Error> {
    let actual = hash_file(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(unexpected_hash_error(path, expected, &actual))
    }
}

#[derive(Clone)]
struct TreeEntry {
    kind: u8,
    relative: String,
    content: Vec<u8>,
}

pub(super) fn hash_tree(root: &Path) -> Result<String, Error> {
    hash_tree_with_override(root, None)
}

pub(super) fn hash_tree_with_file_override(
    root: &Path,
    overridden_path: &Path,
    overridden_content: &[u8],
) -> Result<String, Error> {
    let relative = overridden_path.strip_prefix(root).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                "active update {} is outside {}: {error}",
                overridden_path.display(),
                root.display()
            ),
        )
    })?;
    let override_name = normalized_relative(relative)?;
    hash_tree_with_override(root, Some((&override_name, overridden_content)))
}

fn hash_tree_with_override(
    root: &Path,
    file_override: Option<(&str, &[u8])>,
) -> Result<String, Error> {
    let metadata = fs::symlink_metadata(root).map_err(|error| io_error("inspect", root, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("tree root is not a regular directory: {}", root.display()),
        ));
    }
    let mut entries = Vec::new();
    collect_tree_entries(root, root, file_override, &mut entries)?;
    entries.sort_by(|left, right| {
        left.relative
            .cmp(&right.relative)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update([entry.kind]);
        update_length_prefixed(&mut hasher, entry.relative.as_bytes());
        update_length_prefixed(&mut hasher, &entry.content);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_tree_entries(
    root: &Path,
    directory: &Path,
    file_override: Option<(&str, &[u8])>,
    entries: &mut Vec<TreeEntry>,
) -> Result<(), Error> {
    let mut children = fs::read_dir(directory)
        .map_err(|error| io_error("read", directory, error))?
        .map(|entry| entry.map_err(|error| io_error("read", directory, error)))
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let path = child.path();
        let file_type = child
            .file_type()
            .map_err(|error| io_error("inspect", &path, error))?;
        if file_type.is_symlink() {
            return Err(Error::new(
                ErrorKind::Config,
                format!("archive trees cannot contain symlinks: {}", path.display()),
            ));
        }
        let relative = normalized_relative(path.strip_prefix(root).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!("cannot hash tree path {}: {error}", path.display()),
            )
        })?)?;
        if file_type.is_dir() {
            entries.push(TreeEntry {
                kind: b'd',
                relative,
                content: Vec::new(),
            });
            collect_tree_entries(root, &path, file_override, entries)?;
        } else if file_type.is_file() {
            let content = match file_override {
                Some((override_name, override_content)) if relative == override_name => {
                    override_content.to_vec()
                }
                Some(_) | None => {
                    fs::read(&path).map_err(|error| io_error("read", &path, error))?
                }
            };
            entries.push(TreeEntry {
                kind: b'f',
                relative,
                content,
            });
        } else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("archive trees require regular files: {}", path.display()),
            ));
        }
    }
    Ok(())
}

/// Twin of `interop::normalized_relative`, kept separate so errors name
/// archive trees instead of the `OpenSpec` conversion.
fn normalized_relative(path: &Path) -> Result<String, Error> {
    let mut segments = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("invalid relative tree path: {}", path.display()),
            ));
        };
        let segment = segment.to_str().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("tree path is not UTF-8: {}", path.display()),
            )
        })?;
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(Error::new(ErrorKind::Config, "tree path is empty"));
    }
    Ok(segments.join("/"))
}

fn update_length_prefixed(hasher: &mut Sha256, content: &[u8]) {
    hasher.update(
        u64::try_from(content.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(content);
}

pub(super) fn verify_tree_hash(path: &Path, expected: &str) -> Result<(), Error> {
    let actual = hash_tree(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Io,
            format!(
                "tree SHA-256 verification failed for {}: expected {expected}, found {actual}",
                path.display()
            ),
        ))
    }
}
