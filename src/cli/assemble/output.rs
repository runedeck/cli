use rune::error::{Error, ErrorKind};
use std::fs;
use std::path::Path;

/// Activate a complete staging tree and preserve the last build on failure.
pub(super) fn activate_build(module: &Path, staging: &Path, build: &Path) -> Result<(), Error> {
    if !staging.exists() {
        if build.exists() {
            fs::remove_dir_all(build).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot clean build directory: {error}"),
                )
            })?;
        }
        return Ok(());
    }
    let retired = module.join(format!(".build-retired-{}", std::process::id()));
    if build.exists() {
        fs::rename(build, &retired).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot retire previous build directory: {error}"),
            )
        })?;
    }
    if let Err(error) = fs::rename(staging, build) {
        if retired.exists() {
            let _ = fs::rename(&retired, build);
        }
        let _ = fs::remove_dir_all(staging);
        return Err(Error::new(
            ErrorKind::Io,
            format!("cannot activate new build directory: {error}"),
        ));
    }
    let _ = fs::remove_dir_all(retired);
    Ok(())
}

/// Write a binary passthrough asset byte-for-byte, creating parent dirs.
pub fn write_file_bytes(output_path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    fs::write(output_path, bytes).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", output_path.display()),
        )
    })
}

/// Write assembled content to the build directory, creating parent dirs.
/// Always ensures a trailing newline (POSIX text file convention).
pub fn write_file(output_path: &Path, content: &str) -> Result<(), Error> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    let mut bytes = content.as_bytes().to_vec();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    fs::write(output_path, bytes).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", output_path.display()),
        )
    })
}
