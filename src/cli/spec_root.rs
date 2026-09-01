//! Spec-tree root resolution and summary types available in every build.

#[cfg(feature = "spec")]
#[allow(unused_imports)]
pub(crate) use rune_docs::spec::{ChangeState, ChangeSummary, SpecificationSummary};

#[cfg(feature = "spec")]
pub(crate) fn changes_root(
    root: &std::path::Path,
) -> Result<std::path::PathBuf, rune::error::Error> {
    rune_docs::spec::changes_root(root).map_err(|error| crate::cli::docs_boundary::convert(&error))
}

#[cfg(feature = "spec")]
pub(crate) fn specs_root(root: &std::path::Path) -> Result<std::path::PathBuf, rune::error::Error> {
    rune_docs::spec::specs_root(root).map_err(|error| crate::cli::docs_boundary::convert(&error))
}

#[cfg(not(feature = "spec"))]
pub(crate) use fallback::{
    ChangeState, ChangeSummary, SpecificationSummary, changes_root, specs_root,
};

#[cfg(not(feature = "spec"))]
mod fallback {
    use rune::error::{Error, ErrorKind};
    use serde::Serialize;
    use std::ffi::OsString;
    use std::path::{Component, Path, PathBuf};

    #[derive(Debug)]
    struct ResolvedRoots {
        changes: PathBuf,
        specifications: PathBuf,
    }

    pub(crate) fn changes_root(root: &Path) -> Result<PathBuf, Error> {
        Ok(resolve_roots(root)?.changes)
    }

    pub(crate) fn specs_root(root: &Path) -> Result<PathBuf, Error> {
        Ok(resolve_roots(root)?.specifications)
    }

    fn resolve_roots(root: &Path) -> Result<ResolvedRoots, Error> {
        let repository = root
            .canonicalize()
            .map_err(|error| root_error("resolve repository", root, error))?;
        let merged = crate::cli::config::load_merged_config(root)?;
        let configured = crate::cli::config::source_spec_root(&merged);
        let relative = match configured.as_deref() {
            Some(configured) => validate_configured_root(configured)?,
            None => autodetect_relative_root(&repository)?,
        };
        let base = resolve_confined_destination(&repository, &repository.join(relative))?;
        if base.exists() && !base.is_dir() {
            return Err(Error::new(
                ErrorKind::Config,
                format!("spec root is not a directory: {}", base.display()),
            ));
        }
        let changes = resolve_confined_destination(&repository, &base.join("changes"))?;
        let specifications = resolve_confined_destination(&repository, &base.join("specs"))?;
        Ok(ResolvedRoots {
            changes,
            specifications,
        })
    }

    fn validate_configured_root(configured: &str) -> Result<PathBuf, Error> {
        let candidate = Path::new(configured);
        if candidate.as_os_str().is_empty()
            || candidate
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Error::new(
                ErrorKind::Config,
                format!("spec.root must be a relative path inside the repository: {configured}"),
            ));
        }
        Ok(candidate.to_path_buf())
    }

    fn autodetect_relative_root(repository: &Path) -> Result<PathBuf, Error> {
        let native = has_live_tree(&repository.join("docs"));
        let openspec = has_live_tree(&repository.join("openspec"));
        match (native, openspec) {
            (true, true) => Err(Error::new(
                ErrorKind::Config,
                "both docs/ and openspec/ contain live spec trees; set spec.root explicitly",
            )),
            (false, true) => Ok(PathBuf::from("openspec")),
            (true | false, false) => Ok(PathBuf::from("docs")),
        }
    }

    fn has_live_tree(base: &Path) -> bool {
        base.join("changes").is_dir() || base.join("specs").is_dir()
    }

    fn resolve_confined_destination(repository: &Path, candidate: &Path) -> Result<PathBuf, Error> {
        if candidate
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(Error::new(
                ErrorKind::Config,
                format!("spec path is a symlink: {}", candidate.display()),
            ));
        }

        let mut existing = candidate;
        let mut missing_segments = Vec::<OsString>::new();
        while !existing.exists() {
            let segment = existing.file_name().ok_or_else(|| {
                Error::new(
                    ErrorKind::Config,
                    format!("cannot resolve spec path: {}", candidate.display()),
                )
            })?;
            missing_segments.push(segment.to_os_string());
            existing = existing.parent().ok_or_else(|| {
                Error::new(
                    ErrorKind::Config,
                    format!("cannot resolve spec path: {}", candidate.display()),
                )
            })?;
        }

        let resolved_existing = existing
            .canonicalize()
            .map_err(|error| root_error("resolve spec path", existing, error))?;
        if !resolved_existing.starts_with(repository) {
            return Err(Error::new(
                ErrorKind::Config,
                format!("spec path escapes repository: {}", candidate.display()),
            ));
        }

        let mut resolved = resolved_existing;
        for segment in missing_segments.iter().rev() {
            resolved.push(segment);
        }
        Ok(resolved)
    }

    fn root_error(action: &str, path: &Path, error: impl std::fmt::Display) -> Error {
        Error::new(
            ErrorKind::Config,
            format!("cannot {action} {}: {error}", path.display()),
        )
    }

    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "lowercase")]
    pub(crate) enum ChangeState {
        Draft,
        Active,
        Complete,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
    pub(crate) struct ChangeSummary {
        pub(crate) id: String,
        pub(crate) state: ChangeState,
        pub(crate) completed: usize,
        pub(crate) total: usize,
    }

    impl ChangeSummary {
        #[allow(dead_code)]
        pub(crate) fn completion_percent(&self) -> usize {
            self.completed
                .saturating_mul(100)
                .checked_div(self.total)
                .unwrap_or(0)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
    pub(crate) struct SpecificationSummary {
        pub(crate) capability: String,
        pub(crate) requirements: usize,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use tempfile::TempDir;

        #[test]
        fn fallback_rejects_ambiguous_live_trees() {
            let root = TempDir::new().unwrap();
            fs::create_dir_all(root.path().join("docs/specs")).unwrap();
            fs::create_dir_all(root.path().join("openspec/changes")).unwrap();

            let error = resolve_roots(root.path()).unwrap_err();

            assert!(error.message().contains("both docs/ and openspec/"));
        }

        #[cfg(unix)]
        #[test]
        fn fallback_rejects_a_symlink_escape() {
            use std::os::unix::fs::symlink;

            let root = TempDir::new().unwrap();
            let outside = TempDir::new().unwrap();
            symlink(outside.path(), root.path().join("linked")).unwrap();
            fs::write(
                root.path().join("config.yaml"),
                "spec:\n    root: linked/specs\n",
            )
            .unwrap();

            let error = resolve_roots(root.path()).unwrap_err();

            assert!(error.message().contains("escapes repository"));
        }
    }
}
