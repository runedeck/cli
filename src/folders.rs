//! The user's folders: home, cache, data, and state. And the canonical
//! form of a path, which every folder comparison in rune starts from.
//!
//! Every lookup of one of these folders goes through this module, so one
//! rule decides where rune keeps its files.
//!
//! On Unix the rule is the one `dirs` applies: HOME and the XDG variables,
//! then the account database.
//!
//! On Windows `dirs` asks the shell for its known folders and reads no
//! environment variable. Nothing could then point rune at another home:
//! not a test, not a sandbox, and not a shell such as Git Bash, which sets
//! HOME for the commands it starts. So on Windows a HOME that holds an
//! absolute path is the home, as it is for git there, and the cache and
//! data folders sit under it where Windows keeps them. With no such HOME
//! the shell's known folders answer.

use std::path::{Path, PathBuf};

/// `fs::canonicalize`, in the form a person writes a path.
///
/// On Windows `fs::canonicalize` answers in the verbatim form,
/// `\\?\C:\repo`. In that form `/` is not a separator, so
/// `root.join("rules/Style.md")` names no file, and the path does not
/// compare equal to the same folder written `C:\repo`. This answers
/// `C:\repo`, and `\\server\share` for a network path. A path that only
/// the verbatim form can name comes back as it is: one of 260 characters
/// or more, or one with a part that is a device name or ends in a dot or
/// a space.
///
/// On every other platform this is `fs::canonicalize`.
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path).map(plain)
}

/// [`canonicalize`] as a method, for a path at the end of a chain.
pub trait Canonical {
    /// See [`canonicalize`].
    fn canonical(&self) -> std::io::Result<PathBuf>;
}

impl Canonical for Path {
    fn canonical(&self) -> std::io::Result<PathBuf> {
        canonicalize(self)
    }
}

/// `base` joined with a path written with `/`, one part at a time.
///
/// `base.join(".config/rune")` keeps the `/` inside the text of the path.
/// Windows reads it as a separator, so the path names the right folder,
/// but a command that prints it shows `C:\Users\owner\.config/rune`. This
/// gives `C:\Users\owner\.config\rune`, the form a person reads and the
/// form a test compares. On Unix the two are the same path and text.
#[must_use]
pub fn join(base: &Path, relative: &str) -> PathBuf {
    let mut path = base.to_path_buf();
    path.extend(relative.split('/').filter(|part| !part.is_empty()));
    path
}

#[cfg(not(windows))]
fn plain(path: PathBuf) -> PathBuf {
    path
}

#[cfg(windows)]
fn plain(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(without_verbatim_prefix) {
        Some(text) => PathBuf::from(text),
        None => path,
    }
}

/// A verbatim path without its prefix, when the shorter text names the
/// same file: `\\?\C:\repo` is `C:\repo`, and `\\?\UNC\server\share` is
/// `\\server\share`.
#[cfg(any(windows, test))]
fn without_verbatim_prefix(text: &str) -> Option<String> {
    let plain = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else {
        let rest = text.strip_prefix(r"\\?\")?;
        let mut characters = rest.chars();
        let drive = characters.next()?;
        if !drive.is_ascii_alphabetic() || characters.next() != Some(':') {
            return None;
        }
        rest.to_owned()
    };
    (plain.len() < 260 && plain.split('\\').all(part_reads_as_written)).then_some(plain)
}

/// Whether Windows reads this part of a plain path as the name written.
/// It drops a trailing dot or space, and it reads a device name such as
/// `NUL`, with or without an extension, as the device.
#[cfg(any(windows, test))]
fn part_reads_as_written(part: &str) -> bool {
    if part.ends_with('.') || part.ends_with(' ') {
        return false;
    }
    let stem = part
        .split('.')
        .next()
        .unwrap_or(part)
        .trim_end()
        .to_ascii_uppercase();
    let numbered = |device: &str| {
        stem.strip_prefix(device).is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
    };
    !(matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT"))
}

/// The user's home folder.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    if let Some(home) = home_from_environment() {
        return Some(home);
    }
    dirs::home_dir()
}

/// The folder for files rune can make again, such as a clone or a download.
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    if let Some(home) = home_from_environment() {
        return Some(home.join("AppData").join("Local"));
    }
    dirs::cache_dir()
}

/// The folder for files rune keeps for the user, such as shell completions.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    if let Some(home) = home_from_environment() {
        return Some(home.join("AppData").join("Roaming"));
    }
    dirs::data_dir()
}

/// The folder for state that outlives a run. Only Linux has one, so a
/// caller names its own fallback.
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    dirs::state_dir()
}

#[cfg(windows)]
fn home_from_environment() -> Option<PathBuf> {
    absolute_home(std::env::var_os("HOME"))
}

/// HOME as a home folder. Only an absolute path names one: an empty or a
/// relative HOME would move with the working directory.
#[cfg(any(windows, test))]
fn absolute_home(home: Option<std::ffi::OsString>) -> Option<PathBuf> {
    home.map(PathBuf::from).filter(|home| home.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::{Canonical as _, absolute_home, canonicalize, without_verbatim_prefix};
    use std::ffi::OsString;

    #[test]
    fn a_verbatim_drive_or_network_path_loses_its_prefix() {
        assert_eq!(
            without_verbatim_prefix(r"\\?\C:\repo\rules").as_deref(),
            Some(r"C:\repo\rules")
        );
        assert_eq!(
            without_verbatim_prefix(r"\\?\UNC\server\share\repo").as_deref(),
            Some(r"\\server\share\repo")
        );
    }

    #[test]
    fn a_path_only_the_verbatim_form_names_keeps_its_prefix() {
        let long = format!(r"\\?\C:\{}", "a".repeat(260));
        for path in [
            r"\\?\Volume{b75e2c83-0000-0000-0000-602f00000000}\repo",
            r"\\?\C:\repo\NUL",
            r"\\?\C:\repo\com1.txt",
            r"\\?\C:\repo\name.",
            r"\\?\C:\repo\name \file",
            long.as_str(),
        ] {
            assert_eq!(without_verbatim_prefix(path), None, "{path}");
        }
    }

    #[test]
    fn a_path_without_the_prefix_is_not_a_verbatim_path() {
        assert_eq!(without_verbatim_prefix(r"C:\repo"), None);
        assert_eq!(without_verbatim_prefix("/home/user"), None);
    }

    #[test]
    fn a_canonical_folder_joins_a_path_written_with_slashes() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(folder.path().join("rules")).unwrap();
        std::fs::write(folder.path().join("rules").join("Style.md"), "").unwrap();
        let canonical = canonicalize(folder.path()).unwrap();
        assert_eq!(canonical, folder.path().canonical().unwrap());
        assert!(canonical.join("rules/Style.md").is_file());
        assert!(!canonical.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn an_absolute_home_is_the_home() {
        let home = std::env::temp_dir();
        assert!(home.is_absolute(), "{}", home.display());
        assert_eq!(
            absolute_home(Some(home.clone().into_os_string())),
            Some(home)
        );
    }

    #[test]
    fn an_unset_an_empty_or_a_relative_home_is_ignored() {
        assert_eq!(absolute_home(None), None);
        assert_eq!(absolute_home(Some(OsString::new())), None);
        assert_eq!(absolute_home(Some(OsString::from("home/user"))), None);
    }
}
