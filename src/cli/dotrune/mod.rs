//! Consumer-side `.rune` manifest support.
//!
//! A consumer repo (a non-module project that wants to use rune artifacts)
//! drops a `.rune` YAML file at its root listing which skills, agents, and
//! rules it needs and from which producer modules. `rune install` run from
//! that repo reads `.rune`, locates each requested artifact in the named
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

#[cfg(test)]
mod tests;

use commands::error::{Error, ErrorKind};
use std::fs;
use std::path::Path;

pub use git::cached_worktree;
pub use parse::{DotRune, SCHEMA_VERSION, Source, validate_commit_sha, validate_git_url};
pub use resolve::resolve_sources;

const MAX_BYTES: usize = 64 * 1024;

/// Return whether `repo_root` has a consumer manifest.
pub fn exists(repo_root: &Path) -> bool {
    repo_root.join(".rune").exists()
}

/// Load `.rune` from `repo_root`.
///
/// Returns `Ok(None)` when no manifest exists at the given root (the normal
/// `module.yaml`-driven path takes over). Returns `Ok(Some(...))` after a
/// successful parse and `Err` on size-cap violation, malformed YAML, or
/// schema mismatch. The size cap (64 KiB) is checked before YAML parsing
/// to defend against memory-bomb / billion-laughs shapes.
pub fn load(repo_root: &Path) -> Result<Option<DotRune>, Error> {
    let path = repo_root.join(".rune");
    if path.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} must be a file, not a directory", path.display()),
        ));
    }
    if !path.is_file() {
        return Ok(None);
    }

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
                ".rune: file is {} bytes; limit is {} bytes (the manifest is a pointer file, not a payload)",
                bytes.len(),
                MAX_BYTES
            ),
        ));
    }

    let content = std::str::from_utf8(&bytes).map_err(|error| {
        Error::new(ErrorKind::Parse, format!(".rune: not valid UTF-8: {error}"))
    })?;

    parse::parse(content).map(Some)
}
