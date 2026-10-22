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

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A checked result file: absolute, inside a permitted root, not a
/// symlink, with its parent folder held open from the check to the write.
#[derive(Debug)]
pub(crate) struct Destination {
    target: PathBuf,
    file_name: OsString,
    parent: Folder,
}

impl Destination {
    pub(crate) fn path(&self) -> &Path {
        &self.target
    }
}

/// The parent folder of the result file. On unix it is an open directory
/// handle, and every create, rename and remove is relative to it, so a
/// folder swapped for a symlink while the harness runs cannot move the
/// write: the handle still names the folder the check accepted, or a
/// folder that no longer exists, and never the symlink's target.
#[derive(Debug)]
struct Folder {
    #[cfg(unix)]
    handle: std::os::fd::OwnedFd,
    #[cfg(not(unix))]
    path: PathBuf,
}

impl Folder {
    /// Open the checked folder itself. On unix the open follows no symlink
    /// (`O_NOFOLLOW`), and the handle must be the folder the check judged
    /// by device and inode, so a folder swapped between the check and the
    /// open, for a symlink or for another folder, is refused.
    fn open(path: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            use nix::fcntl::OFlag;
            let checked = std::fs::metadata(path)?;
            let handle = nix::fcntl::open(
                path,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                nix::sys::stat::Mode::empty(),
            )?;
            let opened = std::fs::File::from(handle);
            if !same_file(&opened.metadata()?, &checked) {
                return Err(std::io::Error::other(
                    "the folder changed between the check and the open",
                ));
            }
            Ok(Self {
                handle: opened.into(),
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                path: path.to_path_buf(),
            })
        }
    }

    /// Create `name` in the folder exclusively, readable and writable by
    /// the owner only.
    fn create_private(&self, name: &OsStr) -> std::io::Result<std::fs::File> {
        #[cfg(unix)]
        {
            use nix::fcntl::OFlag;
            use nix::sys::stat::Mode;
            let fd = nix::fcntl::openat(
                &self.handle,
                Path::new(name),
                OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_CLOEXEC,
                Mode::S_IRUSR | Mode::S_IWUSR,
            )?;
            Ok(std::fs::File::from(fd))
        }
        #[cfg(not(unix))]
        {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.path.join(name))
        }
    }

    /// Rename `from` to `to`, both inside the folder, in one step.
    fn rename(&self, from: &OsStr, to: &OsStr) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            nix::fcntl::renameat(&self.handle, Path::new(from), &self.handle, Path::new(to))
                .map_err(std::io::Error::from)
        }
        #[cfg(not(unix))]
        {
            std::fs::rename(self.path.join(from), self.path.join(to))
        }
    }

    /// Remove the file `name` inside the folder.
    fn remove(&self, name: &OsStr) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            nix::unistd::unlinkat(
                &self.handle,
                Path::new(name),
                nix::unistd::UnlinkatFlags::NoRemoveDir,
            )
            .map_err(std::io::Error::from)
        }
        #[cfg(not(unix))]
        {
            std::fs::remove_file(self.path.join(name))
        }
    }
}

/// Whether two metadata records name one file: the same device and inode
/// on unix. Elsewhere there is no identity to compare, so they match.
fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (left, right);
        true
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
    // The folder is held from here on: the probe and the final write go
    // through the same handle.
    let folder = Folder::open(&parent).map_err(|error| {
        format!(
            "cannot open the parent folder of output file {}: {error}",
            target.display()
        )
    })?;
    let probe = temporary_name(file_name, "probe");
    folder.create_private(&probe).map_err(|error| {
        format!(
            "the parent folder of output file {} cannot take a new file: {error}",
            target.display()
        )
    })?;
    let _ = folder.remove(&probe);
    Ok(Destination {
        target,
        file_name: file_name.to_os_string(),
        parent: folder,
    })
}

/// A name beside the target that no other process takes: hidden, with
/// the process id and the time in it.
fn temporary_name(file_name: &OsStr, kind: &str) -> OsString {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut name = OsString::from(".");
    name.push(file_name);
    name.push(format!(".{}.{nanos}.{kind}", std::process::id()));
    name
}

/// Write `text` to the checked file in one step: an exclusive 0600
/// temporary file in the held folder, written, synced, and renamed over
/// the target relative to that folder. A failure leaves the old file as it
/// was and no temporary file behind.
pub(crate) fn write(destination: &Destination, text: &str) -> Result<(), String> {
    let folder = &destination.parent;
    let temporary = temporary_name(&destination.file_name, "tmp");
    let written = (|| -> std::io::Result<()> {
        let mut file = folder.create_private(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        folder.rename(&temporary, &destination.file_name)
    })();
    if let Err(error) = written {
        let _ = folder.remove(&temporary);
        return Err(format!(
            "cannot write result file {}: {error}",
            destination.target.display()
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
    fn the_folder_handle_follows_no_symlink() {
        let repository = repository();
        let real = repository.path().join("real");
        std::fs::create_dir_all(&real).expect("real");
        let link = repository.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        assert!(Folder::open(&link).is_err(), "a symlink was opened");
        assert!(Folder::open(&real).is_ok());
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
    fn a_folder_swapped_after_the_check_cannot_move_the_write() {
        let repository = repository();
        let out = repository.path().join("out");
        std::fs::create_dir_all(&out).expect("out");
        let destination =
            accept(&out.join("seat.json"), Some(repository.path())).expect("accepted");

        // The accepted folder moves aside and a symlink takes its path: the
        // write follows the folder, and the symlink's target gets nothing.
        let moved = repository.path().join("out-moved");
        std::fs::rename(&out, &moved).expect("move");
        let elsewhere = repository.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("elsewhere");
        std::os::unix::fs::symlink(&elsewhere, &out).expect("symlink");
        write(&destination, "result").expect("written");
        assert_eq!(
            std::fs::read_to_string(moved.join("seat.json")).expect("moved"),
            "result"
        );
        assert!(!elsewhere.join("seat.json").exists());

        // The accepted folder is removed and a symlink takes its path: the
        // write fails, and the symlink's target gets nothing.
        let gone = repository.path().join("gone");
        std::fs::create_dir_all(&gone).expect("gone");
        let destination =
            accept(&gone.join("seat.json"), Some(repository.path())).expect("accepted");
        std::fs::remove_dir_all(&gone).expect("remove");
        std::os::unix::fs::symlink(&elsewhere, &gone).expect("symlink");
        let error = write(&destination, "result").expect_err("write fails");
        assert!(error.contains("cannot write result file"), "{error}");
        assert!(!elsewhere.join("seat.json").exists());
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
