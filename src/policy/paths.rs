//! Paths as the policy compares them. A scope path and a subject path both
//! pass through `normalize`, so a symbolic link, a `~`, or another letter
//! case never hides a match.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// A leading `~` as the home directory.
#[must_use]
pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    if path == "~" {
        return home.to_path_buf();
    }
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}

/// The path made absolute against `cwd`, with `.` and `..` removed by
/// their text alone, as Node's `path.resolve` does.
fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The real path of the nearest existing ancestor, with the rest appended.
/// A file about to be written does not exist yet, but its directory
/// usually does.
#[must_use]
pub fn realpath_nearest(abs: &Path) -> PathBuf {
    let mut tail: Vec<OsString> = Vec::new();
    let mut head = abs.to_path_buf();
    loop {
        if let Ok(real) = crate::folders::canonicalize(&head) {
            let mut out = real;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (head.parent(), head.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                head = parent.to_path_buf();
            }
            _ => return abs.to_path_buf(),
        }
    }
}

/// The real path of `path`, before any case folding: `~` expanded, made
/// absolute, and resolved through the nearest existing ancestor.
#[must_use]
pub(crate) fn real(path: &str, home: &Path, cwd: &Path) -> PathBuf {
    realpath_nearest(&absolute(&expand_home(path.trim(), home), cwd))
}

/// A real path as the policy compares it.
#[must_use]
pub(crate) fn fold(path: &Path, fold_case: bool) -> String {
    let text = path.to_string_lossy().into_owned();
    if fold_case { text.to_lowercase() } else { text }
}

/// `path` as the policy compares it: real, absolute, and folded to lower
/// case where the file system ignores case.
#[must_use]
pub fn normalize(path: &str, home: &Path, cwd: &Path, fold_case: bool) -> String {
    fold(&real(path, home, cwd), fold_case)
}

/// Whether `subject` is `scope` or lies below it. Both must be normalized.
#[must_use]
pub fn contains(scope: &str, subject: &str) -> bool {
    if scope == subject {
        return true;
    }
    let separator = std::path::MAIN_SEPARATOR;
    if scope.ends_with(separator) {
        return subject.starts_with(scope);
    }
    subject
        .strip_prefix(scope)
        .is_some_and(|rest| rest.starts_with(separator))
}

/// The nearest ancestor of a real path, itself included, that holds `.jj`
/// or `.git`.
#[must_use]
pub fn repository_root(real: &Path) -> Option<PathBuf> {
    real.ancestors()
        .find(|dir| dir.join(".jj").exists() || dir.join(".git").exists())
        .map(Path::to_path_buf)
}
