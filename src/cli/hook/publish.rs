//! Write a set of files as one step. Every file is staged beside its
//! target first. Then each staged file replaces its target, in the order
//! given. When a replace fails, every target already replaced gets its
//! old content back and the remaining staged files are removed, so the
//! set is never left half new. A target keeps its permission bits.

use std::fs;
use std::path::{Path, PathBuf};

use rune::error::Error;

/// One file of the set: where it goes, what it holds, and the permission
/// bits it gets when it does not exist yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Staged {
    pub target: PathBuf,
    pub text: String,
    pub new_mode: Option<u32>,
}

/// The staging path beside a target: `.NAME.rune-new` in its directory.
fn staging_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    target.with_file_name(format!(".{name}.rune-new"))
}

fn io(path: &Path, error: &std::io::Error) -> Error {
    Error::io(format!("{}: {error}", path.display()))
}

/// The permission bits of an existing file.
#[cfg(unix)]
fn mode_of(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn mode_of(_path: &Path) -> Option<u32> {
    None
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

/// Stage one file beside its target, with the target's mode when it has
/// one, else the file's own.
fn stage(file: &Staged) -> Result<PathBuf, Error> {
    if let Some(parent) = file.target.parent() {
        fs::create_dir_all(parent).map_err(|e| io(parent, &e))?;
    }
    let staged = staging_path(&file.target);
    fs::write(&staged, &file.text).map_err(|e| io(&staged, &e))?;
    if let Some(mode) = mode_of(&file.target).or(file.new_mode) {
        set_mode(&staged, mode).map_err(|e| io(&staged, &e))?;
    }
    Ok(staged)
}

/// Put a target back as it was: its old bytes, or no file at all.
fn restore(target: &Path, old: Option<&[u8]>) {
    match old {
        Some(bytes) => {
            let _ = fs::write(target, bytes);
        }
        None => {
            let _ = fs::remove_file(target);
        }
    }
}

/// Write every file, or none of them.
pub(crate) fn publish(files: &[Staged]) -> Result<(), Error> {
    let mut staged: Vec<PathBuf> = Vec::new();
    for file in files {
        match stage(file) {
            Ok(path) => staged.push(path),
            Err(error) => {
                for path in &staged {
                    let _ = fs::remove_file(path);
                }
                return Err(error);
            }
        }
    }
    let old: Vec<Option<Vec<u8>>> = files.iter().map(|f| fs::read(&f.target).ok()).collect();
    for (index, file) in files.iter().enumerate() {
        if let Err(error) = fs::rename(&staged[index], &file.target) {
            for done in (0..index).rev() {
                restore(&files[done].target, old[done].as_deref());
            }
            for path in &staged[index..] {
                let _ = fs::remove_file(path);
            }
            return Err(io(&file.target, &error));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(target: PathBuf, text: &str) -> Staged {
        Staged {
            target,
            text: text.to_string(),
            new_mode: Some(0o600),
        }
    }

    #[test]
    fn every_file_lands_and_an_existing_one_keeps_its_mode() {
        let dir = tempfile::tempdir().unwrap();
        let kept = dir.path().join("kept.json");
        fs::write(&kept, "old").unwrap();
        set_mode(&kept, 0o640).unwrap();
        let fresh = dir.path().join("sub/fresh.json");
        publish(&[staged(kept.clone(), "new"), staged(fresh.clone(), "made")]).unwrap();
        assert_eq!(fs::read_to_string(&kept).unwrap(), "new");
        assert_eq!(fs::read_to_string(&fresh).unwrap(), "made");
        #[cfg(unix)]
        {
            assert_eq!(mode_of(&kept), Some(0o640));
            assert_eq!(mode_of(&fresh), Some(0o600));
        }
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".rune-new"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn a_failed_replace_puts_back_every_file_already_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.json");
        fs::write(&first, "first-old").unwrap();
        let second = dir.path().join("second.json");
        // A directory with an entry cannot be replaced by a file.
        let blocked = dir.path().join("blocked");
        fs::create_dir_all(blocked.join("inside")).unwrap();
        let error = publish(&[
            staged(first.clone(), "first-new"),
            staged(second.clone(), "second-new"),
            staged(blocked.clone(), "never"),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("blocked"), "{error}");
        assert_eq!(fs::read_to_string(&first).unwrap(), "first-old");
        assert!(!second.exists(), "a file that did not exist is gone again");
        assert!(blocked.join("inside").is_dir());
        assert!(!staging_path(&blocked).exists());
    }
}
