//! Walk each declared source module on disk and feed its content through
//! the artifact filter. The output flat `Vec<SourceFile>` plugs straight
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
        let Some(artifact_list) = manifest.artifacts.get(source_label) else {
            continue;
        };
        if artifact_list.is_empty() {
            continue;
        }

        let filtered = match &canonical {
            CanonicalSource::Module(module_root) => filter_to_requested(
                sources::collect(module_root, valid_qualifiers)?,
                artifact_list,
                source_label,
                module_root,
            )?,
            CanonicalSource::Deck(deck) => {
                let mut files = Vec::new();
                for domain in &deck.domains {
                    let providers = deck.providers_for(domain).map(<[String]>::to_vec);
                    for mut file in sources::collect_deck(&domain.root, valid_qualifiers)? {
                        file.artifact_id = Some(canonical_artifact_id(&domain.name, &file)?);
                        file.providers.clone_from(&providers);
                        files.push(file);
                    }
                }
                filter_deck_to_requested(files, artifact_list, source_label, &deck.root)?
            }
        };
        collected.extend(filtered);
    }

    Ok(collected)
}

fn canonical_artifact_id(domain: &str, file: &SourceFile) -> Result<String, Error> {
    let name = if file.kind == commands::provider::ContentKind::Skills {
        file.relative_path
            .strip_prefix("skills/")
            .and_then(|path| path.split('/').next())
    } else {
        Path::new(&file.relative_path)
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
    }
    .filter(|name| !name.is_empty())
    .ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!(
                "cannot derive artifact id for {} in domain {domain}",
                file.relative_path
            ),
        )
    })?;
    Ok(format!("{domain}/{}/{name}", file.kind))
}

fn canonicalize_source(
    source: &Source,
    source_label: &str,
    repo_root: &Path,
) -> Result<CanonicalSource, Error> {
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
    let canonical = canonicalize_subpath(&materialized, subpath, source_label)?;

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
