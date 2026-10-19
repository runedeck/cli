//! Consumer-side `.rune` manifest support.
//!
//! A consumer repo (a non-module project that wants to use rune artifacts)
//! drops a `.rune` YAML file at its root listing which skills, agents, and
//! rules it needs and from which producer modules. `rune install` run from
//! that repo reads `.rune`, locates each requested rune in the named
//! producer module on disk, and runs them through the regular assemble +
//! deploy pipeline scoped to the consumer's own `.claude/`, `.gemini/`,
//! `.codex/`, `.opencode/` directories.
//!
//! Sources can be `Local` (a sibling checkout on disk) or `Git` (a remote
//! HTTPS repository pinned to a 40-hex commit SHA). Git sources clone via
//! `gix` into `~/.cache/rune/git/<host>/<owner>/<repo>/`, materialize the
//! pinned tree into a per-SHA worktree dir, then plug into the regular
//! pipeline.

mod filter;
mod git;
mod parse;
mod resolve;
pub mod toggle;

#[cfg(test)]
mod tests;

use rune::error::{Error, ErrorKind};
use std::fs;
use std::path::Path;

pub use git::cached_worktree;
pub use parse::{DotRune, RuneList, SCHEMA_VERSION, Source, validate_commit_sha, validate_git_url};
pub use resolve::{enumerate_ids, materialize_source, resolve_sources};
pub use rune::consumer::manifest_path;

const MAX_BYTES: usize = 64 * 1024;

/// Return whether `repo_root` has a consumer manifest, as the file `.rune`
/// or as `.rune/manifest.yaml` inside a `.rune` directory.
pub fn exists(repo_root: &Path) -> bool {
    rune::consumer::manifest_exists(repo_root)
}

/// Load the consumer manifest of `repo_root`, from the file `.rune` or from
/// `.rune/manifest.yaml` when `.rune` is a directory.
///
/// Returns `Ok(None)` when no manifest exists at the given root (the normal
/// `module.yaml`-driven path takes over), a `.rune` directory without one
/// included. Returns `Ok(Some(...))` after a successful parse and `Err` on
/// size-cap violation, malformed YAML, or schema mismatch. The size cap
/// (64 KiB) is checked before YAML parsing to defend against memory-bomb /
/// billion-laughs shapes.
pub fn load(repo_root: &Path) -> Result<Option<DotRune>, Error> {
    if rune::consumer::is_linked_directory(repo_root) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{} is a symbolic link to a directory; the manifest must be a real file or a real directory",
                repo_root.join(".rune").display()
            ),
        ));
    }
    let path = manifest_path(repo_root);
    if !path.is_file() {
        return Ok(None);
    }
    // The form that was read, relative to the root: `.rune` or `.rune/manifest.yaml`.
    let shown = path.strip_prefix(repo_root).map_or_else(
        |_| path.display().to_string(),
        |relative| relative.display().to_string(),
    );

    let bytes = fs::read(&path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;

    if bytes.len() > MAX_BYTES {
        return Err(Error::new(
            ErrorKind::Parse,
            format!(
                "{shown}: file is {} bytes; limit is {} bytes (the manifest is a pointer file, not a payload)",
                bytes.len(),
                MAX_BYTES
            ),
        ));
    }

    let content = std::str::from_utf8(&bytes).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("{shown}: not valid UTF-8: {error}"),
        )
    })?;

    parse::parse(content).map(Some)
}

/// Atomically replace `.rune` with pre-rendered content. The toggle writer
/// uses this path so a surgical edit keeps every unrelated byte.
pub(crate) fn write_atomic_content(repo_root: &Path, content: &str) -> Result<(), Error> {
    crate::cli::config::write_atomic(&manifest_path(repo_root), content)
}

/// Serialize and atomically replace a consumer manifest.
///
/// Both `rune add` and interactive editors use this durability path so a
/// failed rename never leaves a partially written `.rune` file.
pub fn write_atomic(repo_root: &Path, manifest: &DotRune) -> Result<(), Error> {
    let content = serde_yaml::to_string(manifest).map_err(|error| {
        Error::new(ErrorKind::Parse, format!("cannot serialize .rune: {error}"))
    })?;
    crate::cli::config::write_atomic(&manifest_path(repo_root), content)
}
