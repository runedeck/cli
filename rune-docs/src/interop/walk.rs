//! Tree walking and path classification for the conversion: which files
//! exist, which side owns them, and where each relative path lands in the
//! native layout. Symlinks and non-regular files are rejected, never
//! followed or recreated.

use super::manifest::{manifest_path_error, validated_relative};
use super::{Classification, INTEROP_DIRECTORY, MIRROR_DIRECTORY, TRANSACTION_DIRECTORY, io_error};
use crate::error::{Error, ErrorKind};
use crate::spec::SpecRoot;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(super) struct WalkedFiles {
    pub(super) files: Vec<(String, PathBuf)>,
    pub(super) directories: Vec<PathBuf>,
}

pub(super) fn classify_path(relative: &str) -> Classification {
    let path = Path::new(relative);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(first)), Some(_)) if first == "changes" => Classification::Change,
        (Some(Component::Normal(first)), Some(_)) if first == "specs" => {
            Classification::Specification
        }
        _ => Classification::File,
    }
}

pub(super) fn native_path(
    spec_root: &SpecRoot,
    relative: &str,
    classification: Classification,
) -> Result<PathBuf, Error> {
    let relative = validated_relative(relative)?;
    match classification {
        Classification::Change => relative
            .strip_prefix("changes")
            .map(|tail| spec_root.changes().join(tail))
            .map_err(|error| manifest_path_error(&relative, error)),
        Classification::Specification => relative
            .strip_prefix("specs")
            .map(|tail| spec_root.specifications().join(tail))
            .map_err(|error| manifest_path_error(&relative, error)),
        Classification::File => Ok(spec_root.base().join(MIRROR_DIRECTORY).join(relative)),
    }
}

pub(super) fn walk_regular_files(root: &Path) -> Result<WalkedFiles, Error> {
    reject_symlink(root)?;
    let metadata = fs::symlink_metadata(root).map_err(|error| io_error("inspect", root, error))?;
    if !metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("conversion source is not a directory: {}", root.display()),
        ));
    }
    let mut walked = WalkedFiles {
        files: Vec::new(),
        directories: vec![root.to_path_buf()],
    };
    collect_regular_files(root, root, &mut walked)?;
    Ok(walked)
}

fn collect_regular_files(
    root: &Path,
    directory: &Path,
    walked: &mut WalkedFiles,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error("read", directory, error))?
        .map(|entry| entry.map_err(|error| io_error("read", directory, error)))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| io_error("inspect", &path, error))?;
        if file_type.is_symlink() {
            return Err(Error::new(
                ErrorKind::Config,
                format!("OpenSpec conversion refuses symlinks: {}", path.display()),
            ));
        }
        if file_type.is_dir() {
            walked.directories.push(path.clone());
            collect_regular_files(root, &path, walked)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| manifest_path_error(&path, error))?;
            walked.files.push((normalized_relative(relative)?, path));
        } else {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "OpenSpec conversion requires regular files: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

/// Twin of `transaction::normalized_relative`, kept separate so errors name
/// the `OpenSpec` conversion instead of archive trees.
fn normalized_relative(path: &Path) -> Result<String, Error> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("invalid OpenSpec relative path: {}", path.display()),
            ));
        };
        let segment = segment.to_str().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("OpenSpec path is not UTF-8: {}", path.display()),
            )
        })?;
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            "OpenSpec relative path is empty",
        ));
    }
    Ok(segments.join("/"))
}

pub(super) fn deepest_first(mut directories: Vec<PathBuf>) -> Vec<PathBuf> {
    directories.sort_by(|left, right| {
        right
            .components()
            .count()
            .cmp(&left.components().count())
            .then_with(|| right.cmp(left))
    });
    directories.dedup();
    directories
}

pub(super) fn is_reserved_state_path(relative: &str) -> bool {
    relative == TRANSACTION_DIRECTORY
        || relative.starts_with(&format!("{TRANSACTION_DIRECTORY}/"))
        || relative == INTEROP_DIRECTORY
        || relative.starts_with(&format!("{INTEROP_DIRECTORY}/"))
}

pub(super) fn openspec_root(spec_root: &SpecRoot) -> PathBuf {
    spec_root.repository().join("openspec")
}

/// Twin of `transaction::reject_symlink`, kept separate so errors name the
/// `OpenSpec` conversion instead of transaction paths.
pub(super) fn reject_symlink(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::new(
            ErrorKind::Config,
            format!("OpenSpec conversion refuses symlinks: {}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect", path, error)),
    }
}
