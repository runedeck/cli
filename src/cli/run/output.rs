//! The result file `rune run --output` writes. Any session that calls rune
//! can name the file, so the path is checked before the harness starts: it
//! must sit inside the repository the caller works in, the system temporary
//! directory, or the rune state directory, with no hidden folder between
//! that root and the file, no hidden file name, and no symlink target. The
//! repository root is the version-control root of the caller's working
//! directory, never a folder `--repo` names: `--repo ~/.claude` must not
//! open `~/.claude/settings.json` for writing. The write is one exclusive
//! temporary file in the target's directory, synced and renamed over the
//! target, so a reader sees the old result or the new one.

use std::io::Write;
use std::path::{Path, PathBuf};

/// A checked result file: absolute, inside a permitted root, not a symlink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Destination {
    target: PathBuf,
}

impl Destination {
    pub(crate) fn path(&self) -> &Path {
        &self.target
    }
}

/// The repository the caller works in: the jj or git root above the
/// working directory, when there is one.
pub(crate) fn caller_repository_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    super::workspace::detect(&cwd)
        .ok()
        .map(|vcs| vcs.root().to_path_buf())
}

/// The roots a result file may sit under, canonical, longest first so the
/// hidden-folder rule is judged from the closest one.
fn permitted_roots(repository: Option<&Path>) -> Vec<(&'static str, PathBuf)> {
    let mut roots = Vec::new();
    if let Some(root) = repository.and_then(|path| std::fs::canonicalize(path).ok()) {
        roots.push(("the repository", root));
    }
    for (label, path) in [
        ("the temporary directory", std::env::temp_dir()),
        ("the temporary directory", PathBuf::from("/tmp")),
    ] {
        if let Ok(root) = std::fs::canonicalize(&path)
            && !roots.iter().any(|(_, known)| *known == root)
        {
            roots.push((label, root));
        }
    }
    if let Ok(state) = crate::cli::sign::state_dir()
        && let Ok(root) = std::fs::canonicalize(&state)
        && !roots.iter().any(|(_, known)| *known == root)
    {
        roots.push(("the rune state directory", root));
    }
    roots.sort_by_key(|(_, root)| std::cmp::Reverse(root.as_os_str().len()));
    roots
}

fn is_hidden(component: &std::ffi::OsStr) -> bool {
    component.to_string_lossy().starts_with('.')
}

/// Check `path` against the rules above, with `repository` the root of the
/// caller's own repository (see [`caller_repository_root`]), if any. The
/// parent is canonicalized, so a symlinked folder is judged by where it
/// leads. A probe file proves that the parent takes a new file now, so a
/// refusal comes before the harness starts, not after it answered.
pub(crate) fn accept(path: &Path, repository: Option<&Path>) -> Result<Destination, String> {
    let absolute = std::path::absolute(path)
        .map_err(|error| format!("cannot resolve output path {}: {error}", path.display()))?;
    let Some(file_name) = absolute.file_name().filter(|name| *name != "..") else {
        return Err(format!("output path {} names no file", path.display()));
    };
    if is_hidden(file_name) {
        return Err(format!(
            "output file {} has a hidden name, and the runner writes no hidden file",
            path.display()
        ));
    }
    let parent = absolute.parent().unwrap_or(Path::new("/"));
    let parent = std::fs::canonicalize(parent).map_err(|error| {
        format!(
            "the parent folder of output file {} cannot take a file: {error}",
            path.display()
        )
    })?;
    let target = parent.join(file_name);
    let roots = permitted_roots(repository);
    let Some((_, root)) = roots.iter().find(|(_, root)| target.starts_with(root)) else {
        let names: Vec<String> = roots
            .iter()
            .map(|(label, root)| format!("{label} ({})", root.display()))
            .collect();
        return Err(format!(
            "output file {} is outside every permitted place: {}",
            target.display(),
            names.join(", ")
        ));
    };
    if let Some(hidden) = parent
        .strip_prefix(root)
        .ok()
        .and_then(|inside| inside.components().find(|part| is_hidden(part.as_os_str())))
    {
        return Err(format!(
            "output file {} lies under the hidden folder {} inside {}",
            target.display(),
            hidden.as_os_str().to_string_lossy(),
            root.display()
        ));
    }
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "output file {} is a symlink, and the runner replaces only a regular file",
                target.display()
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(format!(
                "output file {} exists and is not a regular file",
                target.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot inspect output file {}: {error}",
                target.display()
            ));
        }
    }
    let probe = temporary_name(&parent, file_name, "probe");
    create_private(&probe).map_err(|error| {
        format!(
            "the parent folder of output file {} cannot take a new file: {error}",
            target.display()
        )
    })?;
    let _ = std::fs::remove_file(&probe);
    Ok(Destination { target })
}

/// A name beside the target that no other process takes: hidden, with
/// the process id and the time in it.
fn temporary_name(parent: &Path, file_name: &std::ffi::OsStr, kind: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut name = std::ffi::OsString::from(".");
    name.push(file_name);
    name.push(format!(".{}.{nanos}.{kind}", std::process::id()));
    parent.join(name)
}

/// Create `path` exclusively, readable and writable by the owner only.
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Write `text` to the checked file in one step: an exclusive 0600
/// temporary file beside the target, written, synced, and renamed over it.
/// A failure leaves the old file as it was and no temporary file behind.
pub(crate) fn write(destination: &Destination, text: &str) -> Result<(), String> {
    let target = &destination.target;
    let parent = target.parent().unwrap_or(Path::new("/"));
    let file_name = target.file_name().unwrap_or_default();
    let temporary = temporary_name(parent, file_name, "tmp");
    let written = (|| -> std::io::Result<()> {
        let mut file = create_private(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, target)
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "cannot write result file {}: {error}",
            target.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository folder whose name is not hidden, under the temporary
    /// directory, so the temporary root alone would also accept it.
    fn repository() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("rune-output-repo-")
            .tempdir()
            .expect("repository")
    }

    #[test]
    fn a_file_inside_the_repository_is_accepted_as_an_absolute_path() {
        let repository = repository();
        let nested = repository.path().join("council");
        std::fs::create_dir_all(&nested).expect("nested");
        let accepted =
            accept(&nested.join("seat.json"), Some(repository.path())).expect("accepted");
        assert!(accepted.path().is_absolute());
        assert!(accepted.path().ends_with("council/seat.json"));
        assert_eq!(
            std::fs::read_dir(&nested).expect("nested").count(),
            0,
            "the probe file stays behind"
        );
    }

    #[test]
    fn a_hidden_folder_between_the_root_and_the_file_is_refused() {
        let repository = repository();
        let git = repository.path().join(".git");
        std::fs::create_dir_all(&git).expect(".git");
        let error = accept(&git.join("hook.json"), Some(repository.path())).expect_err("refused");
        assert!(error.contains("hidden folder .git"), "{error}");
    }

    #[test]
    fn a_hidden_file_name_is_refused() {
        let repository = repository();
        let error = accept(&repository.path().join(".zshrc"), Some(repository.path()))
            .expect_err("refused");
        assert!(error.contains("hidden name"), "{error}");
        let error = accept(&repository.path().join(".env"), None).expect_err("refused");
        assert!(error.contains("hidden name"), "{error}");
    }

    #[test]
    fn a_path_outside_every_root_is_refused() {
        let repository = repository();
        let elsewhere = tempfile::Builder::new()
            .prefix("rune-output-elsewhere-")
            .tempdir()
            .expect("elsewhere");
        // A folder under the temporary directory is permitted; one under a
        // fake home is not.
        assert!(accept(&elsewhere.path().join("x.json"), Some(repository.path())).is_ok());
        let home = elsewhere.path().join("home/.claude");
        std::fs::create_dir_all(&home).expect("home");
        let error =
            accept(&home.join("settings.json"), Some(repository.path())).expect_err("refused");
        assert!(error.contains("hidden folder .claude"), "{error}");
        let error = accept(
            Path::new("/nonexistent-root-xyz/x.json"),
            Some(repository.path()),
        )
        .expect_err("refused");
        assert!(error.contains("cannot take a file"), "{error}");
        let error = accept(Path::new("/etc/rune-output.json"), None).expect_err("refused");
        assert!(
            error.contains("outside every permitted place") || error.contains("cannot take"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_and_a_directory_target_are_refused() {
        let repository = repository();
        let real = repository.path().join("real.json");
        std::fs::write(&real, "{}").expect("real");
        let link = repository.path().join("link.json");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        let error = accept(&link, Some(repository.path())).expect_err("refused");
        assert!(error.contains("is a symlink"), "{error}");
        let error = accept(repository.path(), Some(repository.path())).expect_err("refused");
        assert!(error.contains("not a regular file"), "{error}");
        let error =
            accept(&repository.path().join("x/.."), Some(repository.path())).expect_err("refused");
        assert!(error.contains("names no file"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_parent_is_refused_before_the_run() {
        use std::os::unix::fs::PermissionsExt;
        let repository = repository();
        let locked = repository.path().join("locked");
        std::fs::create_dir_all(&locked).expect("locked");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).expect("chmod");
        let result = accept(&locked.join("seat.json"), Some(repository.path()));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let error = result.expect_err("refused");
        assert!(error.contains("cannot take a new file"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn the_write_replaces_the_file_in_one_step_and_keeps_it_private() {
        use std::os::unix::fs::PermissionsExt;
        let repository = repository();
        let target = repository.path().join("seat.json");
        std::fs::write(&target, "old").expect("old");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let destination = accept(&target, Some(repository.path())).expect("accepted");
        write(&destination, "{\"ok\":true}\n").expect("written");
        assert_eq!(
            std::fs::read_to_string(&target).expect("new"),
            "{\"ok\":true}\n"
        );
        let mode = std::fs::metadata(&target)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let names: Vec<String> = std::fs::read_dir(repository.path())
            .expect("dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["seat.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_write_that_fails_keeps_the_old_file_and_names_the_error() {
        use std::os::unix::fs::PermissionsExt;
        let repository = repository();
        let folder = repository.path().join("out");
        std::fs::create_dir_all(&folder).expect("out");
        let target = folder.join("seat.json");
        std::fs::write(&target, "old").expect("old");
        let destination = accept(&target, Some(repository.path())).expect("accepted");
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).expect("chmod");
        let result = write(&destination, "new");
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let error = result.expect_err("write fails");
        assert!(error.contains("cannot write result file"), "{error}");
        assert_eq!(std::fs::read_to_string(&target).expect("old"), "old");
    }
}
