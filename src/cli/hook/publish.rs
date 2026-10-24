//! Write a set of files as one step. Every file is staged beside its
//! target under a name no other run uses. Before a staged file replaces
//! its target, the old target is kept as a backup beside it, linked or
//! copied. When a replace fails, every target already replaced gets its
//! backup back by rename, a target that did not exist is removed again,
//! and a restore that fails is reported. A target keeps its permission
//! bits. `Lock` serializes the installs that share a target directory.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rune::error::Error;

/// One file of the set: where it goes, what it holds, and the permission
/// bits it gets when it does not exist yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Staged {
    pub target: PathBuf,
    pub text: String,
    pub new_mode: Option<u32>,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A path beside `target` that no other run uses: `.NAME.rune-KIND-PID-NANOS-N`.
fn beside(target: &Path, kind: &str) -> PathBuf {
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    target.with_file_name(format!(
        ".{name}.rune-{kind}-{}-{nanos:x}-{count}",
        std::process::id()
    ))
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

/// Stage one file beside its target in a file this run creates, with the
/// target's mode when it has one, else the file's own.
fn stage(file: &Staged) -> Result<PathBuf, Error> {
    stage_with(file, |out, staged| {
        use std::io::Write as _;
        out.write_all(file.text.as_bytes())?;
        if let Some(mode) = mode_of(&file.target).or(file.new_mode) {
            set_mode(staged, mode)?;
        }
        Ok(())
    })
}

/// Create the staged file and let `write` fill it. A file `write` fails
/// to finish is removed again, so no partial file stays beside a target.
fn stage_with(
    file: &Staged,
    write: impl FnOnce(&mut File, &Path) -> std::io::Result<()>,
) -> Result<PathBuf, Error> {
    if let Some(parent) = file.target.parent() {
        fs::create_dir_all(parent).map_err(|e| io(parent, &e))?;
    }
    let staged = beside(&file.target, "new");
    let mut out = File::options()
        .write(true)
        .create_new(true)
        .open(&staged)
        .map_err(|e| io(&staged, &e))?;
    if let Err(error) = write(&mut out, &staged) {
        drop(out);
        let _ = fs::remove_file(&staged);
        return Err(io(&staged, &error));
    }
    Ok(staged)
}

/// Keep an existing target as a backup beside it: a hard link to the same
/// file, or a copy where the file system has no links. `None` when the
/// target does not exist.
fn back_up(target: &Path) -> Result<Option<PathBuf>, Error> {
    if !target.exists() {
        return Ok(None);
    }
    let backup = beside(target, "old");
    if fs::hard_link(target, &backup).is_err() {
        fs::copy(target, &backup).map_err(|error| {
            Error::io(format!(
                "{}: cannot keep the old file to put it back: {error}",
                target.display()
            ))
        })?;
    }
    Ok(Some(backup))
}

/// Replace one target with its staged file, keeping the old one beside it.
fn replace(file: &Staged, staged: &Path) -> Result<Option<PathBuf>, Error> {
    let backup = back_up(&file.target)?;
    fs::rename(staged, &file.target)
        .map(|()| backup.clone())
        .map_err(|error| {
            if let Some(backup) = &backup {
                let _ = fs::remove_file(backup);
            }
            io(&file.target, &error)
        })
}

/// Put every replaced target back, newest first: its backup by rename, or
/// no file when there was none. Returns the restores that failed.
fn restore_all(done: &[(PathBuf, Option<PathBuf>)]) -> Vec<String> {
    let mut failed = Vec::new();
    for (target, backup) in done.iter().rev() {
        let result = match backup {
            Some(backup) => fs::rename(backup, target),
            None => fs::remove_file(target),
        };
        if let Err(error) = result {
            failed.push(format!("{}: {error}", target.display()));
        }
    }
    failed
}

/// Write every file, or none of them: stage each one, then replace each
/// target in order. Every old file stays beside its target until the last
/// replace, and a failure puts back every target already replaced.
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
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for (index, file) in files.iter().enumerate() {
        match replace(file, &staged[index]) {
            Ok(backup) => done.push((file.target.clone(), backup)),
            Err(error) => {
                for path in &staged[index..] {
                    let _ = fs::remove_file(path);
                }
                return Err(abort(&done, error));
            }
        }
    }
    for (_, backup) in done {
        if let Some(backup) = backup {
            let _ = fs::remove_file(backup);
        }
    }
    Ok(())
}

/// Put back every replaced target, and name the ones that could not be
/// put back in the error.
fn abort(done: &[(PathBuf, Option<PathBuf>)], error: Error) -> Error {
    let failed = restore_all(done);
    if failed.is_empty() {
        return error;
    }
    Error::io(format!(
        "{error}; the earlier files could not be put back: {}",
        failed.join(", ")
    ))
}

/// An exclusive lock on the directories of a set of targets, held while
/// one install reads and writes them, so two installs never interleave.
/// Each target's directory is created before it is locked, so every
/// install that shares a target locks that directory itself, whether it
/// existed before or not. Locking an ancestor of a missing directory let
/// a second install lock the directory the first one went on to create.
/// Each directory is locked once by its canonical path: a second lock on
/// it through another name would wait for the first forever. The lock is
/// std's file lock on `LOCK_FILE` in the directory, `flock` on Unix and
/// `LockFileEx` on Windows, so it holds on every shipped target. It is
/// advisory: only rune takes it. Only a writing install takes it.
pub(crate) struct Lock {
    /// The locked files, released when the value drops. Only a test reads
    /// them.
    #[cfg_attr(not(test), allow(dead_code))]
    held: Vec<File>,
}

/// The file in each locked directory that holds the lock. It stays empty,
/// and a later install locks it again.
pub(crate) const LOCK_FILE: &str = ".rune-install.lock";

impl Lock {
    /// Lock the directory of every target.
    pub(crate) fn acquire(targets: &[PathBuf]) -> Result<Self, Error> {
        let dirs: Vec<PathBuf> = targets
            .iter()
            .filter_map(|target| target.parent().map(Path::to_path_buf))
            .collect();
        Self::acquire_dirs(&dirs)
    }

    /// Lock every directory, each made first and named by its canonical
    /// path.
    pub(crate) fn acquire_dirs(given: &[PathBuf]) -> Result<Self, Error> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for dir in given {
            let dir = std::path::absolute(dir).map_err(|e| io(dir, &e))?;
            fs::create_dir_all(&dir).map_err(|e| io(&dir, &e))?;
            dirs.push(fs::canonicalize(&dir).map_err(|e| io(&dir, &e))?);
        }
        dirs.sort();
        dirs.dedup();
        let mut held = Vec::new();
        for dir in dirs {
            let path = dir.join(LOCK_FILE);
            let file = File::options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .map_err(|e| io(&path, &e))?;
            file.lock()
                .map_err(|e| Error::io(format!("{}: cannot lock: {e}", path.display())))?;
            held.push(file);
        }
        Ok(Self { held })
    }
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

    fn leftovers(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".rune-") && name != LOCK_FILE)
            .collect()
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
        assert_eq!(leftovers(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_failed_replace_puts_back_every_file_already_replaced() {
        let dir = tempfile::tempdir().unwrap();
        // A read-only file still goes back: the restore is a rename.
        let first = dir.path().join("first.json");
        fs::write(&first, "first-old").unwrap();
        set_mode(&first, 0o444).unwrap();
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
        assert_eq!(leftovers(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_stage_that_fails_to_write_leaves_no_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("settings.json");
        let error = stage_with(&staged(target, "new"), |out, _| {
            use std::io::Write as _;
            out.write_all(b"half")?;
            Err(std::io::Error::other("the disk is full"))
        })
        .unwrap_err();
        assert!(error.to_string().contains("the disk is full"), "{error}");
        assert_eq!(leftovers(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_restore_that_fails_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("plan.json");
        fs::write(&target, "new").unwrap();
        let gone = dir.path().join(".plan.json.rune-old-gone");
        let failed = restore_all(&[(target.clone(), Some(gone))]);
        assert_eq!(failed.len(), 1);
        assert!(failed[0].contains("plan.json"), "{failed:?}");
    }

    #[test]
    fn two_installs_on_one_target_never_interleave() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("settings.json");
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let target = target.clone();
                std::thread::spawn(move || {
                    let _lock = Lock::acquire(std::slice::from_ref(&target)).unwrap();
                    let text = format!("writer {n}");
                    publish(&[staged(target.clone(), &text)]).unwrap();
                    assert_eq!(fs::read_to_string(&target).unwrap(), text);
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert!(fs::read_to_string(&target).unwrap().starts_with("writer "));
        assert_eq!(leftovers(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_second_install_waits_although_the_first_created_the_directories() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("state").join("hooks").join("plan.json");
        let first = Lock::acquire(std::slice::from_ref(&target)).unwrap();
        // The first install's staging makes the missing directories.
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let (sent, got) = std::sync::mpsc::channel();
        let second_target = target.clone();
        let second = std::thread::spawn(move || {
            let lock = Lock::acquire(std::slice::from_ref(&second_target)).unwrap();
            let _ = sent.send(());
            drop(lock);
        });
        assert!(
            got.recv_timeout(std::time::Duration::from_millis(300))
                .is_err(),
            "both installs held the lock at once"
        );
        drop(first);
        got.recv_timeout(std::time::Duration::from_secs(5))
            .expect("the second install runs once the first is done");
        second.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_named_two_ways_is_locked_once() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let targets = vec![real.join("plan.json"), link.join("manifest.json")];
        // A second lock on one directory would wait forever, so the
        // acquire runs on a thread the test gives five seconds.
        let (sent, got) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let lock = Lock::acquire(&targets).map(|lock| lock.held.len());
            let _ = sent.send(lock);
        });
        let held = got
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the lock waited on itself")
            .unwrap();
        assert_eq!(held, 1);
    }
}
