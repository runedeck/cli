//! Small self-contained helpers the crate needs: atomic file replacement and
//! frontmatter splitting. Kept behaviorally identical to the rune CLI's own
//! copies; the crate stays dependency-free by carrying them itself.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

const FENCE: &str = "---";
const MAX_CONTENT_SIZE: usize = 256 * 1024;

static WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Write via a same-directory temporary and rename over the destination.
/// Refuses to replace symlinks so a link can never redirect the write.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    if path.symlink_metadata().is_ok_and(|meta| meta.is_symlink()) {
        return Err(format!(
            "{} is a symlink; refusing to replace it",
            path.display()
        ));
    }
    let temporary = temporary_sibling(path);
    {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
        file.write_all(content.as_bytes()).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            format!("cannot write {}: {error}", temporary.display())
        })?;
        let _ = file.sync_all();
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("cannot replace {}: {error}", path.display())
    })
}

/// Multi-file variant: stage every temporary before renaming any of them,
/// so a write failure aborts with no destination touched. The rename
/// sequence itself remains the residual non-atomic window.
pub fn write_atomic_all(writes: &[(&Path, &str)]) -> Result<(), String> {
    for (index, (path, _)) in writes.iter().enumerate() {
        if writes[..index].iter().any(|(earlier, _)| earlier == path) {
            return Err(format!("{} appears twice in one write set", path.display()));
        }
    }
    let mut staged: Vec<(PathBuf, &Path)> = Vec::new();
    let cleanup = |staged: &[(PathBuf, &Path)]| {
        for (temporary, _) in staged {
            let _ = fs::remove_file(temporary);
        }
    };
    for (path, content) in writes {
        if path.symlink_metadata().is_ok_and(|meta| meta.is_symlink()) {
            cleanup(&staged);
            return Err(format!(
                "{} is a symlink; refusing to replace it",
                path.display()
            ));
        }
        let temporary = temporary_sibling(path);
        let outcome = (|| -> std::io::Result<()> {
            use std::io::Write as _;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(content.as_bytes())?;
            let _ = file.sync_all();
            Ok(())
        })();
        if let Err(error) = outcome {
            let _ = fs::remove_file(&temporary);
            cleanup(&staged);
            return Err(format!("cannot write {}: {error}", temporary.display()));
        }
        staged.push((temporary, path));
    }
    for (index, (temporary, path)) in staged.iter().enumerate() {
        if let Err(error) = fs::rename(temporary, path) {
            for (remaining, _) in &staged[index..] {
                let _ = fs::remove_file(remaining);
            }
            return Err(format!("cannot replace {}: {error}", path.display()));
        }
    }
    Ok(())
}

/// The one temporary-name helper for same-directory staging: a hidden
/// sibling tagged with the process id and a process-wide sequence number,
/// used by atomic writes here and by the spec transaction layer.
pub(crate) fn temporary_sibling(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let base_name = path.file_name().map_or_else(
        || "rune-write".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let sequence = WRITE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    parent.join(format!(
        ".{base_name}.{}.{sequence}.tmp",
        std::process::id()
    ))
}

/// Confinement for a path that already exists: require `candidate` to
/// resolve inside `base` once both are canonicalized, so `..` components
/// and symlinks cannot escape. The counterpart for paths about to be
/// created is the transaction layer's ancestor inspection; the two forms
/// are deliberately separate and must not be collapsed.
pub fn confine_existing(base: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let resolved_base = canonicalize(base)
        .map_err(|error| format!("cannot resolve {}: {error}", base.display()))?;
    let resolved = canonicalize(candidate)
        .map_err(|error| format!("cannot resolve {}: {error}", candidate.display()))?;
    if resolved.starts_with(&resolved_base) {
        Ok(resolved)
    } else {
        Err(format!(
            "{} escapes {}",
            candidate.display(),
            base.display()
        ))
    }
}

/// `fs::canonicalize`, in the form a person writes a path.
///
/// On Windows `fs::canonicalize` answers in the verbatim form,
/// `\\?\C:\repo`. In that form `/` is not a separator, so
/// `root.join("docs/specs")` names no folder, and the path does not compare
/// equal to the same folder written `C:\repo`. This answers `C:\repo`, and
/// `\\server\share` for a network path.
///
/// Every path on a drive or a share comes back in that one form, however
/// long it is, so a folder and a file inside it always compare as prefix
/// and rest. The standard library puts the prefix back for a call that
/// needs it. A path that Windows reads as another file without the prefix
/// is an error: one with a part that is a device name such as `NUL`, or
/// that ends in a dot or a space. This crate has no single way to write it.
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

#[cfg(not(windows))]
fn canonical_form(path: &Path) -> std::io::Result<PathBuf> {
    fs::canonicalize(path)
}

/// The text is the path's own UTF-16, not a `str`: a file name on Windows
/// need not be valid Unicode, and such a path gets the same form as any
/// other.
#[cfg(windows)]
fn canonical_form(path: &Path) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

    let verbatim = fs::canonicalize(path)?;
    let text: Vec<u16> = verbatim.as_os_str().encode_wide().collect();
    let Some(plain) = without_verbatim_prefix(&text) else {
        return Ok(verbatim);
    };
    let plain = PathBuf::from(std::ffi::OsString::from_wide(&plain));
    // The names Windows reads as devices differ between its versions, so a
    // part that looks like one is put to Windows itself.
    let separator = u16::from(b'\\');
    if text.split(|unit| *unit == separator).all(part_looks_plain)
        || fs::canonicalize(&plain).is_ok_and(|read| read == verbatim)
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

/// Split `---`-fenced YAML frontmatter from a markdown body. Returns `None`
/// when there is no opening fence at the start, the opening fence is never
/// closed, or the content exceeds 256 KB.
pub fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    if content.len() > MAX_CONTENT_SIZE || !content.starts_with(FENCE) {
        return None;
    }

    let after_opening = content[FENCE.len()..]
        .strip_prefix('\n')
        .unwrap_or(&content[FENCE.len()..]);

    // Empty frontmatter: two fences back to back.
    if let Some(remainder) = after_opening.strip_prefix(FENCE) {
        let body = remainder.strip_prefix('\n').unwrap_or(remainder);
        return Some(("", body));
    }

    let closing_fence = format!("\n{FENCE}");
    let closing_pos = after_opening.find(&closing_fence)?;
    let yaml_text = &after_opening[..closing_pos];
    let after_closing = &after_opening[closing_pos + closing_fence.len()..];
    let body = after_closing.strip_prefix('\n').unwrap_or(after_closing);

    Some((yaml_text, body))
}

#[cfg(test)]
mod tests {
    use super::{canonicalize, confine_existing, part_looks_plain, without_verbatim_prefix};

    fn utf16(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn plain(verbatim: &str) -> Option<String> {
        without_verbatim_prefix(&utf16(verbatim)).map(|plain| String::from_utf16(&plain).unwrap())
    }

    #[test]
    fn a_verbatim_drive_or_network_path_loses_its_prefix() {
        assert_eq!(plain(r"\\?\C:\repo\docs").as_deref(), Some(r"C:\repo\docs"));
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
            r"C:\repo",
        ] {
            assert_eq!(plain(path), None, "{path}");
        }
    }

    #[test]
    fn a_device_name_or_a_trailing_dot_or_space_does_not_look_plain() {
        for part in ["NUL", "nul.txt", "com1.txt", "COM0", "aux ", "name."] {
            assert!(!part_looks_plain(&utf16(part)), "{part}");
        }
        for part in ["", "C:", "docs", "null", "console.md", "com10", ".git"] {
            assert!(part_looks_plain(&utf16(part)), "{part}");
        }
    }

    #[test]
    fn a_long_path_inside_the_base_is_confined_to_it() {
        let folder = tempfile::tempdir().unwrap();
        let mut deep = canonicalize(folder.path()).unwrap();
        deep.extend(std::iter::repeat_n("a".repeat(50), 6));
        std::fs::create_dir_all(&deep).unwrap();
        assert!(deep.as_os_str().len() > 300);
        assert_eq!(confine_existing(folder.path(), &deep), Ok(deep));
    }
}
