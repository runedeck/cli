//! Path-boundary confinement: the single home for the canonicalize-then-
//! `starts_with` check that keeps every write and read inside its allowed
//! root. Raw `..` components and symlinks are resolved before comparison,
//! so neither can slip a path outside the boundary.

use std::path::{Component, Path, PathBuf};

/// Resolve an EXISTING `candidate` and require it inside `base`.
/// Returns the canonical candidate path.
pub fn confine_existing(base: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let resolved_base = base
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", base.display()))?;
    let resolved = candidate
        .canonicalize()
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

/// Require a `candidate` that may NOT exist yet to land inside `base`.
/// The nearest real ancestor is canonicalized (resolving symlinks in the
/// part that exists), then the not-yet-created suffix is resolved lexically:
/// each `..` must not climb above `base`, and a symlinked terminal component
/// is refused because its target cannot be proven to stay inside.
pub fn confine_for_write(base: &Path, candidate: &Path) -> Result<(), String> {
    let resolved_base = base
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", base.display()))?;

    let mut existing = candidate;
    while existing.symlink_metadata().is_err() {
        existing = existing
            .parent()
            .ok_or_else(|| format!("cannot resolve any ancestor of {}", candidate.display()))?;
    }
    if existing
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!(
            "{} escapes {}",
            candidate.display(),
            base.display()
        ));
    }
    let resolved_existing = existing
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", existing.display()))?;

    let suffix = candidate
        .strip_prefix(existing)
        .map_err(|_| format!("cannot resolve {}", candidate.display()))?;
    let mut projected = resolved_existing;
    for component in suffix.components() {
        match component {
            Component::Normal(part) => projected.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                projected.pop();
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "{} escapes {}",
                    candidate.display(),
                    base.display()
                ));
            }
        }
    }
    if projected.starts_with(&resolved_base) {
        Ok(())
    } else {
        Err(format!(
            "{} escapes {}",
            candidate.display(),
            base.display()
        ))
    }
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
        assert!(error.contains("escapes"), "{error}");
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
        assert!(error.contains("escapes"), "{error}");
    }

    #[test]
    fn write_target_not_yet_existing_is_confined_by_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let future = root.path().join("deep/nested/new.txt");
        confine_for_write(root.path(), &future).unwrap();

        let outside = tempfile::tempdir().unwrap();
        let escape = outside.path().join("new.txt");
        let error = confine_for_write(root.path(), &escape).unwrap_err();
        assert!(error.contains("escapes"), "{error}");
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
        assert!(error.contains("escapes"), "{error}");
    }

    #[test]
    fn dangling_symlink_destination_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("not-created-yet.txt");
        let link = root.path().join("dest.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = confine_for_write(root.path(), &link).unwrap_err();
        assert!(error.contains("escapes"), "{error}");
    }
}
