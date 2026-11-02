//! The readable copy of a Claude Code session through `SpecStory`, when it
//! is installed. A readable copy is a projection of the native copy: its
//! failure never records debt, and its absence stops nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Find an executable on the PATH, as a shell would.
#[must_use]
pub fn find_in_path(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let path = path?;
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Run `specstory sync claude` in the session's working folder, for one
/// session when known, into `output`. A project bound to `SpecStory` also
/// gets its own `.specstory/history`, as the script gave it.
pub fn specstory(
    binary: &Path,
    session: Option<&str>,
    cwd: &Path,
    output: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(output)
        .map_err(|error| format!("cannot create {}: {error}", output.display()))?;
    sync(binary, session, cwd, output)?;
    if cwd.join(".specstory/.project.json").is_file() {
        sync(binary, session, cwd, &cwd.join(".specstory/history"))?;
    }
    Ok(())
}

fn sync(binary: &Path, session: Option<&str>, cwd: &Path, output: &Path) -> Result<(), String> {
    let mut command = Command::new(binary);
    command
        .args([
            "sync",
            "claude",
            "--silent",
            "--no-cloud-sync",
            "--no-usage-analytics",
            "--no-version-check",
        ])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(id) = session {
        command.args(["-s", id]);
    }
    command.arg("--output-dir").arg(output);
    let result = command
        .output()
        .map_err(|error| format!("cannot start {}: {error}", binary.display()))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "specstory sync claude{} failed in {}: {}",
            session.map(|id| format!(" -s {id}")).unwrap_or_default(),
            cwd.display(),
            String::from_utf8_lossy(&result.stderr).trim()
        ))
    }
}
