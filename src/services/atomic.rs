//! The one atomic-replace writer every durable file goes through.
//!
//! A write lands in an adjacent temporary file and is renamed over the
//! destination, so an interruption never leaves a truncated file. The
//! temporary carries a per-process unique suffix and is opened with
//! `create_new`, so a pre-planted symlink at a predictable name cannot
//! redirect the write. A symlink at the destination itself is refused
//! rather than followed or replaced. An existing regular file keeps its
//! permissions, so an executable script stays executable after an edit.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, ErrorKind};

static WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Replace `path` with `content` through a temporary file and a rename.
pub fn write_atomic(path: &Path, content: impl AsRef<[u8]>) -> Result<(), Error> {
    write_atomic_with(path, content.as_ref(), |from, to| fs::rename(from, to))
}

fn write_atomic_with<F>(path: &Path, content: &[u8], rename: F) -> Result<(), Error>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_symlink())
    {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} is a symlink; refusing to replace it", path.display()),
        ));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let base_name = path.file_name().map_or_else(
        || "rune-write".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let sequence = WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{base_name}.{}.{sequence}.tmp",
        std::process::id()
    ));
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", temporary.display()),
                )
            })?;
        file.write_all(content).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", temporary.display()),
            )
        })?;
        let _ = file.sync_all();
    }
    if let Ok(metadata) = fs::metadata(path)
        && metadata.is_file()
    {
        let _ = fs::set_permissions(&temporary, metadata.permissions());
    }
    rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        Error::new(
            ErrorKind::Io,
            format!("cannot replace {}: {error}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leftovers(directory: &Path) -> Vec<String> {
        fs::read_dir(directory)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| {
                Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension == "tmp")
            })
            .collect()
    }

    #[test]
    fn replaces_content_and_leaves_no_temporary_behind() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("record.yaml");
        fs::write(&path, "before\n").unwrap();

        write_atomic(&path, "after\n").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "after\n");
        assert_eq!(leftovers(root.path()), Vec::<String>::new());
    }

    #[test]
    fn accepts_bytes_and_creates_a_missing_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("asset.bin");

        write_atomic(&path, b"\x00\xff binary").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"\x00\xff binary");
    }

    #[test]
    fn refuses_a_symlinked_destination() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("real.txt");
        fs::write(&target, "real\n").unwrap();
        let link = root.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = write_atomic(&link, "replacement\n").unwrap_err();

        assert!(error.message().contains("symlink"), "{error}");
        assert_eq!(fs::read_to_string(&target).unwrap(), "real\n");
    }

    #[cfg(unix)]
    #[test]
    fn keeps_the_permissions_of_the_replaced_file() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("script.sh");
        fs::write(&path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();

        write_atomic(&path, "#!/bin/sh\necho edited\n").unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    fn cleans_temp_and_preserves_destination_on_rename_failure() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join(".rune");
        fs::write(&destination, "original\n").unwrap();

        let error = write_atomic_with(&destination, b"replacement\n", |_, _| {
            Err(std::io::Error::other("simulated rename failure"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("simulated rename failure"));
        assert_eq!(fs::read_to_string(destination).unwrap(), "original\n");
        assert_eq!(leftovers(root.path()), Vec::<String>::new());
    }
}
