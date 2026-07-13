use commands::error::{Error, ErrorKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cli::dotrune::{DotRune, SCHEMA_VERSION, Source};

pub fn execute(
    rune: Option<&str>,
    cast: Option<&str>,
    source: Option<&str>,
    reference: Option<&str>,
) -> Result<i32, Error> {
    let current_dir = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read current directory: {error}"),
        )
    })?;
    let repo_root = if current_dir.join(".rune").is_file() {
        current_dir
    } else if let Some(quest) = crate::cli::quest::bound_quest() {
        println!("using bound quest {}", quest.display());
        quest
    } else {
        current_dir
    };
    let manifest_path = repo_root.join(".rune");
    let mut manifest = if manifest_path.is_file() {
        crate::cli::dotrune::load(&repo_root)?.ok_or_else(|| {
            Error::new(ErrorKind::Config, "cannot load existing .rune".to_string())
        })?
    } else {
        let configured_source;
        let source = if let Some(source) = source {
            source
        } else {
            configured_source = configured_deck_source()?;
            &configured_source
        };
        minimal_manifest(source, reference)?
    };

    let configured_source;
    let selected_source =
        if source.is_some() || (manifest_path.is_file() && !manifest.sources.is_empty()) {
            source
        } else {
            configured_source = configured_deck_source()?;
            Some(configured_source.as_str())
        };
    let source_label = select_source(&mut manifest, selected_source, reference)?;
    let entry = manifest.runes.entry(source_label).or_default();
    let mut changed = false;
    if let Some(cast) = cast {
        for cast in split_comma_list(cast, "cast")? {
            if !entry.casts.contains(&cast) {
                entry.casts.push(cast);
                changed = true;
            }
        }
    } else {
        for selection in split_comma_list(rune.unwrap_or_default(), "rune")? {
            let selection = normalize_rune_id(&selection)?;
            if !entry.include.contains(&selection) {
                entry.include.push(selection);
                changed = true;
            }
        }
    }

    match validate_selection(&manifest, &repo_root) {
        Ok(()) => {}
        Err(Deferred(note)) => println!("note: {note}"),
        Err(Invalid(error)) => return Err(error),
    }

    if changed || !manifest_path.is_file() {
        crate::cli::dotrune::write_atomic(&repo_root, &manifest)?;
        println!("updated {}", manifest_path.display());
    }
    println!("rune install --source {}", repo_root.display());
    Ok(0)
}

fn split_comma_list(raw: &str, what: &str) -> Result<Vec<String>, Error> {
    let items: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect();
    if items.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("expected at least one {what}, got '{raw}'"),
        ));
    }
    Ok(items)
}

enum ValidationError {
    Deferred(String),
    Invalid(Error),
}
use ValidationError::{Deferred, Invalid};

/// Resolve the whole manifest against its sources so unknown ids, unknown
/// casts, and ambiguous short forms fail at add time instead of install
/// time. Git sources may need a network fetch, so validation defers to
/// install rather than cloning during an edit command.
fn validate_selection(manifest: &DotRune, repo_root: &Path) -> Result<(), ValidationError> {
    if manifest
        .sources
        .values()
        .any(|source| matches!(source, Source::Git { .. }))
    {
        return Err(Deferred(
            "selection uses a git source; ids are verified at install".to_string(),
        ));
    }
    let merged_config = crate::cli::config::load_merged_config(repo_root)
        .map_err(|error| Invalid(Error::new(ErrorKind::Config, error.to_string())))?;
    let providers = crate::cli::config::load_providers(&merged_config)
        .map_err(|error| Invalid(Error::new(ErrorKind::Config, error.to_string())))?;
    let models = crate::cli::config::load_models(repo_root);
    let provider_names: Vec<String> = providers.keys().cloned().collect();
    let qualifiers =
        crate::cli::assemble::sources::build_valid_qualifiers(&provider_names, &models);
    crate::cli::dotrune::resolve_sources(manifest, repo_root, &qualifiers)
        .map(|_| ())
        .map_err(Invalid)
}

fn configured_deck_source() -> Result<String, Error> {
    commands::ontology::load()?
        .deck
        .map(|value| value.value)
        .ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "no deck source configured; pass --source <path-or-url>, set RUNE_DECK, or set `deck` in ~/.config/rune/config.yaml with `rune config set deck <path-or-url>`"
                .to_string(),
        )
    })
}

fn minimal_manifest(source: &str, reference: Option<&str>) -> Result<DotRune, Error> {
    let mut sources = BTreeMap::new();
    sources.insert("deck".to_string(), parse_source(source, reference)?);
    Ok(DotRune {
        version: SCHEMA_VERSION,
        sources,
        runes: BTreeMap::new(),
    })
}

fn select_source(
    manifest: &mut DotRune,
    source: Option<&str>,
    reference: Option<&str>,
) -> Result<String, Error> {
    if let Some(source) = source {
        if let Some((label, _)) = manifest
            .sources
            .iter()
            .find(|(_, candidate)| source_matches(candidate, source, reference))
        {
            return Ok(label.clone());
        }
        let label = next_source_label(&manifest.sources);
        manifest
            .sources
            .insert(label.clone(), parse_source(source, reference)?);
        return Ok(label);
    }
    match manifest.sources.keys().next() {
        Some(label) if manifest.sources.len() == 1 => Ok(label.clone()),
        Some(_) => Err(Error::new(
            ErrorKind::Config,
            "multiple sources are configured; pass --source to select one".to_string(),
        )),
        None => Err(Error::new(
            ErrorKind::Config,
            ".rune has no sources; pass --source to add one".to_string(),
        )),
    }
}

fn next_source_label(sources: &BTreeMap<String, Source>) -> String {
    if !sources.contains_key("deck") {
        return "deck".to_string();
    }
    (2..=sources.len() + 2)
        .map(|index| format!("deck-{index}"))
        .find(|label| !sources.contains_key(label))
        .expect("a finite map with n keys has a free label among n + 1 candidates")
}

fn parse_source(source: &str, reference: Option<&str>) -> Result<Source, Error> {
    if source.starts_with("https://") {
        let reference = reference.ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                "--ref <SHA> is required for an HTTPS source".to_string(),
            )
        })?;
        crate::cli::dotrune::validate_git_url(source)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
        crate::cli::dotrune::validate_commit_sha(reference)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
        Ok(Source::Git {
            git: source.to_string(),
            commit: reference.to_string(),
            path: None,
        })
    } else {
        if reference.is_some() {
            return Err(Error::new(
                ErrorKind::Config,
                "--ref is only valid with an HTTPS source".to_string(),
            ));
        }
        Ok(Source::Local {
            local: PathBuf::from(source),
            path: None,
        })
    }
}

fn source_matches(source: &Source, requested: &str, reference: Option<&str>) -> bool {
    match source {
        Source::Local { local, path: None } => local == Path::new(requested) && reference.is_none(),
        Source::Git {
            git,
            commit,
            path: None,
        } => git == requested && reference.is_none_or(|reference| reference == commit),
        _ => false,
    }
}

fn normalize_rune_id(rune_id: &str) -> Result<String, Error> {
    let parts = rune_id.split('/').collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) || !matches!(parts.len(), 1..=3) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "rune id must be <deck>, <Name>, <deck>/<Name>, or <deck>/<kind>/<Name>, got '{rune_id}'"
            ),
        ));
    }
    Ok(rune_id.to_string())
}
