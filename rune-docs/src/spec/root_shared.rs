// Root detection and confinement compiled verbatim into both resolvers:
// the library's (`spec::root`) and the CLI's no-spec fallback include this
// file, so the two builds cannot drift.
//
// Resolution order, identical in every build:
//
// 1. A configured `spec.root` wins after validation (repository-relative,
//    no parent or absolute components).
// 2. Otherwise autodetect: a directory is live when it holds a `changes/`
//    or `specs/` tree, or an interrupted transaction journal at
//    `.rune-transaction/journal.yaml`. The journal counts so a root whose
//    live directories were already moved away (an interrupted export)
//    still resolves to the directory holding the unfinished work.
// 3. When `docs/` and `openspec/` are both live, the side holding the
//    transaction journal wins so recovery reacquires the same lock. Two
//    journals, or none, stay ambiguous and require `spec.root`.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

pub(crate) fn validate_configured_root(configured: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(configured);
    if candidate.as_os_str().is_empty()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "spec.root must be a relative path inside the repository: {configured}"
        ));
    }
    Ok(candidate.to_path_buf())
}

pub(crate) fn autodetect_relative_root(repository: &Path) -> Result<PathBuf, String> {
    let native = has_live_tree(&repository.join("docs"));
    let openspec = has_live_tree(&repository.join("openspec"));
    match (native, openspec) {
        (true, true) => transaction_root(repository),
        (false, true) => Ok(PathBuf::from("openspec")),
        (true | false, false) => Ok(PathBuf::from("docs")),
    }
}

fn transaction_root(repository: &Path) -> Result<PathBuf, String> {
    let native_has_journal = has_transaction_journal(&repository.join("docs"));
    let openspec_has_journal = has_transaction_journal(&repository.join("openspec"));
    match (native_has_journal, openspec_has_journal) {
        (true, false) => Ok(PathBuf::from("docs")),
        (false, true) => Ok(PathBuf::from("openspec")),
        _ => Err(
            "both docs/ and openspec/ contain live spec trees; set spec.root explicitly"
                .to_string(),
        ),
    }
}

pub(crate) fn has_live_tree(base: &Path) -> bool {
    base.join("changes").is_dir() || base.join("specs").is_dir() || has_transaction_journal(base)
}

fn has_transaction_journal(base: &Path) -> bool {
    base.join(".rune-transaction/journal.yaml").is_file()
}

pub(crate) fn resolve_confined_destination(
    repository: &Path,
    candidate: &Path,
) -> Result<PathBuf, String> {
    if candidate
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!("spec path is a symlink: {}", candidate.display()));
    }

    let mut existing = candidate;
    let mut missing_segments = Vec::<OsString>::new();
    while !existing.exists() {
        let segment = existing
            .file_name()
            .ok_or_else(|| format!("cannot resolve spec path: {}", candidate.display()))?;
        missing_segments.push(segment.to_os_string());
        existing = existing
            .parent()
            .ok_or_else(|| format!("cannot resolve spec path: {}", candidate.display()))?;
    }

    let resolved_existing = existing.canonicalize().map_err(|error| {
        format!("cannot resolve spec path {}: {error}", existing.display())
    })?;
    if !resolved_existing.starts_with(repository) {
        return Err(format!(
            "spec path escapes repository: {}",
            candidate.display()
        ));
    }

    let mut resolved = resolved_existing;
    for segment in missing_segments.iter().rev() {
        resolved.push(segment);
    }
    Ok(resolved)
}
