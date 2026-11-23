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
/// `C:\repo`, and `\\server\share` for a network path.
///
/// Every path on a drive or a share comes back in that one form, however
/// long it is, so a folder and a file inside it always compare as prefix
/// and rest. The standard library puts the prefix back for a call that
/// needs it. A path that Windows reads as another file without the prefix
/// is an error: one with a part that is a device name such as `NUL`, or
/// that ends in a dot or a space. rune has no single way to write it.
///
/// On every other platform this is `fs::canonicalize`.
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    canonical_form(path.as_ref())
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
fn canonical_form(path: &Path) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path)
}

/// The text is the path's own UTF-16, not a `str`: a file name on Windows
/// need not be valid Unicode, and such a path gets the same form as any
/// other.
#[cfg(windows)]
fn canonical_form(path: &Path) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

    let verbatim = std::fs::canonicalize(path)?;
    let text: Vec<u16> = verbatim.as_os_str().encode_wide().collect();
    let Some(plain) = without_verbatim_prefix(&text) else {
        return Ok(verbatim);
    };
    let plain = PathBuf::from(std::ffi::OsString::from_wide(&plain));
    // The names Windows reads as devices differ between its versions, so a
    // part that looks like one is put to Windows itself.
    let separator = u16::from(b'\\');
    if text.split(|unit| *unit == separator).all(part_looks_plain)
        || std::fs::canonicalize(&plain).is_ok_and(|read| read == verbatim)
    {
        return Ok(plain);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
            "{} has a part that Windows reads as a device, or without its last dot or space",
            verbatim.display()
        ),
    ))
}

/// A verbatim path without its prefix, when it is on a drive or a share:
/// `\\?\C:\repo` is `C:\repo`, and `\\?\UNC\server\share` is
/// `\\server\share`. Any other verbatim path, such as a volume named by
/// its id, has no shorter form, and neither has a path under it.
#[cfg(any(windows, test))]
fn without_verbatim_prefix(text: &[u16]) -> Option<Vec<u16>> {
    let utf16 = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
    if let Some(rest) = text.strip_prefix(utf16(r"\\?\UNC\").as_slice()) {
        let mut share = utf16(r"\\");
        share.extend_from_slice(rest);
        return Some(share);
    }
    let rest = text.strip_prefix(utf16(r"\\?\").as_slice())?;
    let drive = char::from_u32(u32::from(*rest.first()?))?;
    (drive.is_ascii_alphabetic() && rest.get(1) == Some(&u16::from(b':'))).then(|| rest.to_vec())
}

/// Whether this part of a path is sure to read the same without the
/// verbatim prefix. Windows drops a trailing dot or space, and it can read
/// a device name such as `NUL`, with or without an extension, as the
/// device.
#[cfg(any(windows, test))]
fn part_looks_plain(part: &[u16]) -> bool {
    let (dot, space) = (u16::from(b'.'), u16::from(b' '));
    if part
        .last()
        .is_some_and(|last| *last == dot || *last == space)
    {
        return false;
    }
    let stem = part.split(|unit| *unit == dot).next().unwrap_or(part);
    let stem = String::from_utf16_lossy(stem)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered = |device: &str| {
        stem.strip_prefix(device).is_some_and(|number| {
            let mut digits = number.chars();
            matches!(
                (digits.next(), digits.next()),
                (Some('0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
            )
        })
    };
    !(matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || numbered("COM")
        || numbered("LPT"))
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
    use super::{
        Canonical as _, absolute_home, canonicalize, part_looks_plain, without_verbatim_prefix,
    };
    use std::ffi::OsString;

    fn utf16(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn plain(verbatim: &str) -> Option<String> {
        without_verbatim_prefix(&utf16(verbatim)).map(|plain| String::from_utf16(&plain).unwrap())
    }

    #[test]
    fn a_verbatim_drive_or_network_path_loses_its_prefix() {
        assert_eq!(
            plain(r"\\?\C:\repo\rules").as_deref(),
            Some(r"C:\repo\rules")
        );
        assert_eq!(
            plain(r"\\?\UNC\server\share\repo").as_deref(),
            Some(r"\\server\share\repo")
        );
    }

    #[test]
    fn a_long_path_loses_its_prefix_as_a_short_one_does() {
        let long = format!(r"C:\repo\{}", "a".repeat(300));
        assert_eq!(plain(&format!(r"\\?\{long}")), Some(long));
    }

    #[test]
    fn a_path_that_is_not_on_a_drive_or_a_share_keeps_its_prefix() {
        for path in [
            r"\\?\Volume{b75e2c83-0000-0000-0000-602f00000000}\repo",
            r"\\?\repo",
            r"\\?\",
            r"C:\repo",
            "/home/user",
        ] {
            assert_eq!(plain(path), None, "{path}");
        }
    }

    #[test]
    fn a_device_name_or_a_trailing_dot_or_space_does_not_look_plain() {
        for part in [
            "NUL",
            "nul.txt",
            "com1.txt",
            "COM0",
            "lpt\u{b9}",
            "aux ",
            "name.",
            "name ",
            "CONOUT$",
        ] {
            assert!(!part_looks_plain(&utf16(part)), "{part}");
        }
        for part in [
            "",
            "C:",
            "repo",
            "null",
            "console.md",
            "com10",
            "COM",
            ".git",
            "a.b",
        ] {
            assert!(part_looks_plain(&utf16(part)), "{part}");
        }
    }

    #[test]
    fn a_folder_and_a_long_path_inside_it_share_one_form() {
        let folder = tempfile::tempdir().unwrap();
        let base = canonicalize(folder.path()).unwrap();
        let mut deep = base.clone();
        deep.extend(std::iter::repeat_n("a".repeat(50), 6));
        std::fs::create_dir_all(&deep).unwrap();
        assert!(deep.as_os_str().len() > 300);
        assert!(canonicalize(&deep).unwrap().starts_with(&base));
    }

    #[cfg(windows)]
    #[test]
    fn a_path_only_the_verbatim_form_names_is_an_error() {
        let folder = tempfile::tempdir().unwrap();
        // Only the verbatim form can make a name that ends in a dot.
        let file = std::fs::canonicalize(folder.path()).unwrap().join("name.");
        std::fs::write(&file, "").unwrap();
        let error = canonicalize(&file).unwrap_err();
        std::fs::remove_file(&file).unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("name."), "{error}");
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
