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
/// `\\server\share` for a network path. A path that only the verbatim form
/// can name comes back as it is: one of 260 characters or more, or one
/// with a part that is a device name or ends in a dot or a space.
///
/// On every other platform this is `fs::canonicalize`.
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    fs::canonicalize(path).map(plain)
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
    use super::without_verbatim_prefix;

    #[test]
    fn a_verbatim_drive_or_network_path_loses_its_prefix() {
        assert_eq!(
            without_verbatim_prefix(r"\\?\C:\repo\docs").as_deref(),
            Some(r"C:\repo\docs")
        );
        assert_eq!(
            without_verbatim_prefix(r"\\?\UNC\server\share\repo").as_deref(),
            Some(r"\\server\share\repo")
        );
    }

    #[test]
    fn a_path_only_the_verbatim_form_names_keeps_its_prefix() {
        for path in [
            r"\\?\Volume{b75e2c83-0000-0000-0000-602f00000000}\repo",
            r"\\?\C:\repo\NUL",
            r"\\?\C:\repo\name.",
            r"C:\repo",
        ] {
            assert_eq!(without_verbatim_prefix(path), None, "{path}");
        }
    }
}
