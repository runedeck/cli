//! Walk each declared source module on disk and feed its content through
//! the rune filter. The output flat `Vec<SourceFile>` plugs straight
//! into the existing per-provider assemble loop.

use commands::error::{Error, ErrorKind};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::assemble::sources::{self, SourceFile};
use crate::cli::dotrune::filter::{filter_deck_to_requested, filter_to_requested};
use crate::cli::dotrune::parse::{DotRune, Source};

enum CanonicalSource {
    Module(PathBuf),
    Deck(commands::deck::Deck),
}

pub fn resolve_sources(
    manifest: &DotRune,
    repo_root: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<Vec<SourceFile>, Error> {
    let mut collected: Vec<SourceFile> = Vec::new();

    for (source_label, source) in &manifest.sources {
        let canonical = canonicalize_source(source, source_label, repo_root)?;
        let Some(rune_list) = manifest.runes.get(source_label) else {
            continue;
        };
        if rune_list.is_empty() {
            continue;
        }

        let filtered = match &canonical {
            CanonicalSource::Module(module_root) => {
                if let Some(cast) = rune_list.casts.first() {
                    return Err(Error::new(
                        ErrorKind::Config,
                        format!(
                            ".rune: cast '{cast}' requires a deck-root source; source '{source_label}' resolves to a single module"
                        ),
                    ));
                }
                if !rune_list.include.is_empty() || !rune_list.exclude.is_empty() {
                    return Err(Error::new(
                        ErrorKind::Config,
                        format!(
                            ".rune: include/exclude requires a deck-root source; source '{source_label}' resolves to a single module"
                        ),
                    ));
                }
                filter_to_requested(
                    sources::collect(module_root, valid_qualifiers)?,
                    rune_list,
                    source_label,
                    module_root,
                )?
            }
            CanonicalSource::Deck(deck) => {
                let mut files = Vec::new();
                for deck_entry in &deck.entries {
                    let providers = deck.providers_for(deck_entry).map(<[String]>::to_vec);
                    for mut file in sources::collect_deck(&deck_entry.root, valid_qualifiers)? {
                        file.rune_id = Some(canonical_rune_id(&deck_entry.name, &file)?);
                        file.providers.clone_from(&providers);
                        file.source_uri = Some(deck_entry.manifest.source_uri().to_string());
                        files.push(file);
                    }
                }
                filter_deck_to_requested(files, rune_list, source_label, deck)?
            }
        };
        collected.extend(filtered);
    }

    Ok(collected)
}

/// Enumerate every canonical rune id (`<domain>/<kind>/<name>`) one manifest
/// source offers. Requires a deck-root source; single modules have no
/// domain component to qualify against.
pub fn enumerate_ids(
    source: &Source,
    source_label: &str,
    repo_root: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<Vec<String>, Error> {
    match canonicalize_source(source, source_label, repo_root)? {
        CanonicalSource::Module(_) => Err(Error::new(
            ErrorKind::Config,
            format!(
                "source '{source_label}' resolves to a single module; kind-scoped add requires a deck-root source"
            ),
        )),
        CanonicalSource::Deck(deck) => {
            let mut ids = Vec::new();
            for deck_entry in &deck.entries {
                for file in sources::collect_deck(&deck_entry.root, valid_qualifiers)? {
                    ids.push(canonical_rune_id(&deck_entry.name, &file)?);
                }
            }
            ids.sort();
            ids.dedup();
            Ok(ids)
        }
    }
}

/// Materialize one manifest source and return its canonical root.
///
/// Editors use this to inspect the same local or pinned-git source that the
/// install resolver will consume.
pub fn materialize_source(
    source: &Source,
    source_label: &str,
    repo_root: &Path,
) -> Result<PathBuf, Error> {
    let (materialized, subpath) = match source {
        Source::Local { local, path } => (
            canonicalize_local(local, source_label, repo_root)?,
            path.as_deref(),
        ),
        Source::Git { git, commit, path } => (
            crate::cli::dotrune::git::ensure_cached(git, commit, source_label)?,
            path.as_deref(),
        ),
    };
    canonicalize_subpath(&materialized, subpath, source_label)
}

fn canonical_rune_id(deck: &str, file: &SourceFile) -> Result<String, Error> {
    let name = if file.kind == commands::provider::ContentKind::Skills {
        file.relative_path
            .strip_prefix("skills/")
            .and_then(|path| path.split('/').next())
            .map(str::to_string)
    } else if file.kind == commands::provider::ContentKind::Hooks {
        file.relative_path
            .strip_prefix("hooks/")
            .map(Path::new)
            .map(|path| path.with_extension(""))
            .map(|path| path.to_string_lossy().into_owned())
    } else {
        Path::new(&file.relative_path)
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_string)
    }
    .filter(|name| !name.is_empty())
    .ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!(
                "cannot derive rune id for {} in deck '{deck}'",
                file.relative_path
            ),
        )
    })?;
    Ok(format!("{deck}/{}/{name}", file.kind))
}

fn canonicalize_source(
    source: &Source,
    source_label: &str,
    repo_root: &Path,
) -> Result<CanonicalSource, Error> {
    let canonical = materialize_source(source, source_label, repo_root)?;
    let subpath = match source {
        Source::Local { path, .. } | Source::Git { path, .. } => path,
    };

    if subpath.is_some() {
        return require_module(canonical, source_label);
    }
    if commands::deck::is_deck(&canonical) {
        let deck = commands::deck::load(&canonical)
            .map_err(|message| Error::new(ErrorKind::Config, format!(".rune: {message}")))?;
        return Ok(CanonicalSource::Deck(deck));
    }
    require_module(canonical, source_label)
}

fn require_module(path: PathBuf, source_label: &str) -> Result<CanonicalSource, Error> {
    if !path.join("module.yaml").is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                ".rune: source '{source_label}' at {} has no module.yaml",
                path.display()
            ),
        ));
    }
    Ok(CanonicalSource::Module(path))
}

fn canonicalize_local(path: &Path, source_label: &str, repo_root: &Path) -> Result<PathBuf, Error> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };
    fs::canonicalize(&resolved).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                ".rune: source '{source_label}' path {} does not exist: {error}",
                resolved.display()
            ),
        )
    })
}

fn canonicalize_subpath(
    materialized: &Path,
    subpath: Option<&Path>,
    source_label: &str,
) -> Result<PathBuf, Error> {
    let materialized = fs::canonicalize(materialized).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                ".rune: source '{source_label}' at {} does not exist: {error}",
                materialized.display()
            ),
        )
    })?;
    let Some(subpath) = subpath else {
        return Ok(materialized);
    };
    let joined = materialized.join(subpath);
    let canonical = fs::canonicalize(&joined).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                ".rune: source '{source_label}' path {} does not exist: {error}",
                joined.display()
            ),
        )
    })?;
    if !canonical.starts_with(&materialized) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                ".rune: source '{source_label}' path {} escapes the materialized source",
                subpath.display()
            ),
        ));
    }
    Ok(canonical)
}
