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

#[cfg(test)]
mod tests;

use commands::error::{Error, ErrorKind};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

pub use git::cached_worktree;
pub use parse::{DotRune, SCHEMA_VERSION, Source, validate_commit_sha, validate_git_url};
#[cfg(feature = "tui")]
pub use resolve::materialize_source;
pub use resolve::{enumerate_ids, resolve_sources};

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

/// Serialize and atomically replace a consumer manifest.
///
/// Both `rune add` and interactive editors use this durability path so a
/// failed rename never leaves a partially written `.rune` file.
pub fn write_atomic(repo_root: &Path, manifest: &DotRune) -> Result<(), Error> {
    let path = repo_root.join(".rune");
    let content = serde_yaml::to_string(manifest).map_err(|error| {
        Error::new(ErrorKind::Parse, format!("cannot serialize .rune: {error}"))
    })?;
    atomic_write_with(&path, content.as_bytes(), |from, to| fs::rename(from, to))
}

fn atomic_write_with<F>(path: &Path, content: &[u8], rename: F) -> Result<(), Error>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = parent.join(format!(".rune.tmp-{}-{nonce}", std::process::id()));
    let mut created = false;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        created = true;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        rename(&temp, path)
    })();
    if let Err(error) = result {
        if created {
            let _ = fs::remove_file(&temp);
        }
        return Err(Error::new(
            ErrorKind::Io,
            format!("cannot atomically rewrite {}: {error}", path.display()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod write_tests {
    use super::*;

    #[test]
    fn atomic_write_cleans_temp_and_preserves_destination_on_rename_failure() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join(".rune");
        fs::write(&destination, "original\n").unwrap();

        let error = atomic_write_with(&destination, b"replacement\n", |_, _| {
            Err(std::io::Error::other("simulated rename failure"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("simulated rename failure"));
        assert_eq!(fs::read_to_string(destination).unwrap(), "original\n");
        let leftovers = fs::read_dir(root.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".rune.tmp-"))
            .collect::<Vec<_>>();
        assert_eq!(leftovers, Vec::<String>::new());
    }
}
