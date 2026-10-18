//! Path-boundary confinement: the single home for the canonicalize-then-
//! `starts_with` check that keeps every write and read inside its allowed
//! root. Raw `..` components and symlinks are resolved before comparison,
//! so neither can slip a path outside the boundary.
//!
//! A refusal is a [`Violation`], so a caller can keep its own wording for
//! each kind while the walk itself lives here once.

use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Why a candidate path failed confinement.
#[derive(Debug)]
pub enum Violation {
    /// The candidate resolves to a location outside the base.
    Escape {
        candidate: PathBuf,
        base: PathBuf,
        resolved: PathBuf,
    },
    /// The deepest existing component of a write target is a symlink, so
    /// the not-yet-created suffix cannot be proven to land inside the base.
    Symlink {
        candidate: PathBuf,
        base: PathBuf,
        link: PathBuf,
    },
    /// A path could not be resolved on the filesystem.
    Unresolvable {
        path: PathBuf,
        error: std::io::Error,
    },
    /// No ancestor of the candidate exists, so there is nothing to resolve.
    NoAncestor { candidate: PathBuf },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Escape {
                candidate, base, ..
            }
            | Self::Symlink {
                candidate, base, ..
            } => write!(
                formatter,
                "{} escapes {}",
                candidate.display(),
                base.display()
            ),
            Self::Unresolvable { path, error } => {
                write!(formatter, "cannot resolve {}: {error}", path.display())
            }
            Self::NoAncestor { candidate } => write!(
                formatter,
                "cannot resolve any ancestor of {}",
                candidate.display()
            ),
        }
    }
}

impl std::error::Error for Violation {}

impl From<Violation> for String {
    fn from(violation: Violation) -> Self {
        violation.to_string()
    }
}

/// True when `path` is non-empty and every component is a plain name: no
/// root, no prefix, no `.` or `..`. The lexical check for a manifest key or
/// a relative path read from a record before it is joined onto a root.
#[must_use]
pub fn is_plain_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Resolve an EXISTING `candidate` and require it inside `base`.
/// Returns the canonical candidate path.
pub fn confine_existing(base: &Path, candidate: &Path) -> Result<PathBuf, Violation> {
    let resolved_base = canonicalize(base)?;
    let resolved = canonicalize(candidate)?;
    if resolved.starts_with(&resolved_base) {
        Ok(resolved)
    } else {
        Err(Violation::Escape {
            candidate: candidate.to_path_buf(),
            base: base.to_path_buf(),
            resolved,
        })
    }
}

/// Require a `candidate` that may NOT exist yet to land inside `base`.
/// The nearest real ancestor is canonicalized (resolving symlinks in the
/// part that exists), then the not-yet-created suffix is resolved lexically:
/// each `..` must not climb above `base`, and a symlinked terminal component
/// is refused because its target cannot be proven to stay inside.
pub fn confine_for_write(base: &Path, candidate: &Path) -> Result<(), Violation> {
    let resolved_base = canonicalize(base)?;

    let mut existing = candidate;
    while existing.symlink_metadata().is_err() {
        existing = existing.parent().ok_or_else(|| Violation::NoAncestor {
            candidate: candidate.to_path_buf(),
        })?;
    }
    if existing
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(Violation::Symlink {
            candidate: candidate.to_path_buf(),
            base: base.to_path_buf(),
            link: existing.to_path_buf(),
        });
    }
    let resolved_existing = canonicalize(existing)?;

    let escape = |resolved: PathBuf| Violation::Escape {
        candidate: candidate.to_path_buf(),
        base: base.to_path_buf(),
        resolved,
    };
    let suffix = candidate
        .strip_prefix(existing)
        .map_err(|_| Violation::NoAncestor {
            candidate: candidate.to_path_buf(),
        })?;
    let mut projected = resolved_existing;
    for component in suffix.components() {
        match component {
            Component::Normal(part) => projected.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                projected.pop();
            }
            Component::RootDir | Component::Prefix(_) => return Err(escape(projected)),
        }
    }
    if projected.starts_with(&resolved_base) {
        Ok(())
    } else {
        Err(escape(projected))
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, Violation> {
    path.canonicalize()
        .map_err(|error| Violation::Unresolvable {
            path: path.to_path_buf(),
            error,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_inside_passes_and_returns_canonical() {
        let root = tempfile::tempdir().unwrap();
        let inside = root.path().join("file.txt");
        std::fs::write(&inside, "content for the confinement test\n").unwrap();

        let resolved = confine_existing(root.path(), &inside).unwrap();
        assert_eq!(resolved, inside.canonicalize().unwrap());
    }

    #[test]
    fn dotdot_escape_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let escape = root.path().join("..").join(
            outside
                .path()
                .file_name()
                .map(std::path::PathBuf::from)
                .unwrap(),
        );

        let error = confine_existing(root.path(), &escape).unwrap_err();
        assert!(matches!(error, Violation::Escape { .. }), "{error:?}");
        assert!(error.to_string().contains("escapes"), "{error}");
    }

    #[test]
    fn absolute_path_outside_root_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("elsewhere.txt");
        std::fs::write(&file, "content outside the root\n").unwrap();

        let existing = confine_existing(root.path(), &file).unwrap_err();
        assert!(matches!(existing, Violation::Escape { .. }), "{existing:?}");

        let future = confine_for_write(root.path(), &outside.path().join("new.txt")).unwrap_err();
        assert!(matches!(future, Violation::Escape { .. }), "{future:?}");
    }

    #[test]
    fn symlink_escape_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "outside content the link points at\n").unwrap();
        let link = root.path().join("link.txt");
        std::os::unix::fs::symlink(&secret, &link).unwrap();

        let error = confine_existing(root.path(), &link).unwrap_err();
        assert!(matches!(error, Violation::Escape { .. }), "{error:?}");
        assert!(error.to_string().contains("escapes"), "{error}");
    }

    #[test]
    fn write_target_not_yet_existing_is_confined_by_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let future = root.path().join("deep/nested/new.txt");
        confine_for_write(root.path(), &future).unwrap();

        let outside = tempfile::tempdir().unwrap();
        let escape = outside.path().join("new.txt");
        let error = confine_for_write(root.path(), &escape).unwrap_err();
        assert!(error.to_string().contains("escapes"), "{error}");
    }

    #[test]
    fn dotdot_in_nonexistent_suffix_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let escape = root
            .path()
            .join("missing")
            .join("..")
            .join("..")
            .join("evil.txt");

        let error = confine_for_write(root.path(), &escape).unwrap_err();
        assert!(matches!(error, Violation::Escape { .. }), "{error:?}");
    }

    #[test]
    fn dangling_symlink_destination_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("not-created-yet.txt");
        let link = root.path().join("dest.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = confine_for_write(root.path(), &link).unwrap_err();
        assert!(
            matches!(error, Violation::Symlink { ref link, .. } if link == &root.path().join("dest.txt")),
            "{error:?}"
        );
        assert!(error.to_string().contains("escapes"), "{error}");
    }

    #[test]
    fn unresolvable_base_is_reported_as_such() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("never-created");

        let error = confine_for_write(&missing, &missing.join("file.txt")).unwrap_err();
        assert!(matches!(error, Violation::Unresolvable { .. }), "{error:?}");
        assert!(error.to_string().contains("cannot resolve"), "{error}");
    }

    #[test]
    fn plain_relative_accepts_names_and_rejects_the_rest() {
        assert!(is_plain_relative(Path::new("skills/Demo/SKILL.md")));
        assert!(!is_plain_relative(Path::new("")));
        assert!(!is_plain_relative(Path::new("../escape.md")));
        assert!(!is_plain_relative(Path::new("skills/../rules/x.md")));
        assert!(!is_plain_relative(Path::new("/etc/passwd")));
        assert!(!is_plain_relative(Path::new("./skills/x.md")));
    }
}
