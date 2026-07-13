use commands::error::{Error, ErrorKind};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::cli::dotrune::{DotRune, SCHEMA_VERSION, Source};

pub fn execute(
    artifact: Option<&str>,
    cast: Option<&str>,
    source: Option<&str>,
    reference: Option<&str>,
) -> Result<i32, Error> {
    let repo_root = std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read current directory: {error}"),
        )
    })?;
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
    let entry = manifest.artifacts.entry(source_label).or_default();
    let changed = if let Some(cast) = cast {
        match entry.cast.as_deref() {
            Some(existing) if existing == cast => false,
            Some(existing) => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("source entry already references cast '{existing}'"),
                ));
            }
            None => {
                entry.cast = Some(cast.to_string());
                true
            }
        }
    } else {
        let selection = normalize_artifact(artifact.unwrap_or_default())?;
        if entry.include.contains(&selection) {
            false
        } else {
            entry.include.push(selection);
            true
        }
    };

    if changed || !manifest_path.is_file() {
        let content = serde_yaml::to_string(&manifest).map_err(|error| {
            Error::new(ErrorKind::Parse, format!("cannot serialize .rune: {error}"))
        })?;
        atomic_write(&manifest_path, content.as_bytes())?;
    }
    println!("rune install --source {}", repo_root.display());
    Ok(0)
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
        artifacts: BTreeMap::new(),
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

fn normalize_artifact(artifact: &str) -> Result<String, Error> {
    let parts = artifact.split('/').collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) || !matches!(parts.len(), 1 | 2) {
        return Err(Error::new(
            ErrorKind::Config,
            format!("artifact must be <domain> or <domain>/<Name>, got '{artifact}'"),
        ));
    }
    if parts.len() == 1 {
        Ok(format!("{artifact}/**"))
    } else {
        Ok(artifact.to_string())
    }
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), Error> {
    atomic_write_with(path, content, |from, to| std::fs::rename(from, to))
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
            let _ = std::fs::remove_file(&temp);
        }
        return Err(Error::new(
            ErrorKind::Io,
            format!("cannot atomically rewrite {}: {error}", path.display()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_cleans_temp_and_preserves_destination_on_rename_failure() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join(".rune");
        std::fs::write(&destination, "original\n").unwrap();

        let error = atomic_write_with(&destination, b"replacement\n", |_, _| {
            Err(std::io::Error::other("simulated rename failure"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("simulated rename failure"));
        assert_eq!(std::fs::read_to_string(destination).unwrap(), "original\n");
        let leftovers = std::fs::read_dir(root.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".rune.tmp-"))
            .collect::<Vec<_>>();
        assert_eq!(leftovers, Vec::<String>::new());
    }
}
