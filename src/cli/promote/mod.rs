//! `rune promote`: move a draft from the consumer into the deck and create
//! the change stub, so the lifecycle starts the moment the draft is worth
//! keeping.

use crate::cli::dotrune;
use crate::cli::draft::{DraftKind, validate_identifier};
use rune::error::{Error, ErrorKind};
use rune::manifest::drafts::Register;
use std::fs;
use std::path::{Path, PathBuf};

/// A change id has at least three hyphen-separated words.
pub fn validate_change_id(change: &str) -> Result<(), Error> {
    let parts: Vec<&str> = change.split('-').collect();
    let clean = parts.iter().all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    });
    if parts.len() >= 3 && clean {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Config,
            format!(
                "change id '{change}' must have at least three hyphen-separated lowercase words"
            ),
        ))
    }
}

/// The deck root: the `--deck` argument, else the first local source in
/// the consumer's `.rune`.
pub fn resolve_deck(consumer_root: &Path, deck: Option<&str>) -> Result<PathBuf, Error> {
    if let Some(path) = deck {
        let root = PathBuf::from(path);
        return if root.join("deck.yaml").is_file() {
            Ok(root)
        } else {
            Err(Error::new(
                ErrorKind::Config,
                format!("{} holds no deck.yaml", root.display()),
            ))
        };
    }
    let manifest = dotrune::load(consumer_root)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!(
                "no .rune under {}: pass --deck <DIR>",
                consumer_root.display()
            ),
        )
    })?;
    for source in manifest.sources.values() {
        if let dotrune::Source::Local { local, .. } = source {
            let root = if local.is_absolute() {
                local.clone()
            } else {
                consumer_root.join(local)
            };
            if root.join("deck.yaml").is_file() {
                return Ok(root);
            }
        }
    }
    Err(Error::new(
        ErrorKind::Config,
        "no local deck source in .rune: pass --deck <DIR>".to_string(),
    ))
}

/// What promote did, for the report.
#[derive(Debug)]
pub struct Promotion {
    pub rune_path: PathBuf,
    pub change_dir: PathBuf,
    pub removed: Vec<String>,
}

pub fn execute(
    consumer_root: &Path,
    name: &str,
    domain: &str,
    change: &str,
    deck: Option<&str>,
) -> Result<Promotion, Error> {
    validate_change_id(change)?;
    validate_identifier("draft name", name)?;
    validate_identifier("domain", domain)?;
    let deck_root = resolve_deck(consumer_root, deck)?;
    let register = Register::load(consumer_root).map_err(|m| Error::new(ErrorKind::Parse, m))?;
    let drafts = register.by_name(name);
    let first = drafts
        .first()
        .ok_or_else(|| Error::new(ErrorKind::Config, format!("no draft named '{name}'")))?;
    if drafts.iter().any(|draft| draft.kind != first.kind) {
        return Err(Error::new(
            ErrorKind::Config,
            format!("draft '{name}' is registered with more than one kind; drop one first"),
        ));
    }
    let kind = DraftKind::parse(&first.kind)?;
    let source_file = consumer_root.join(&first.path);
    if !source_file.is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "draft file {} is missing; drop the draft or restore the file",
                first.path
            ),
        ));
    }

    let change_dir = deck_root.join("docs").join("changes").join(change);
    if change_dir.exists() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} already exists", change_dir.display()),
        ));
    }
    let rune_path = deck_root
        .join("runes")
        .join(domain)
        .join(kind.relative_path(name));
    if rune_path.exists() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} already exists in the deck", rune_path.display()),
        ));
    }

    // Deck first, drafts last: copy the rune, write the stub, and only then
    // drop the provider copies. A failure before the drop leaves the draft
    // intact and removes the deck copy, so a retry starts from the same place.
    if let Some(parent) = rune_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", parent.display()),
            )
        })?;
    }
    if let Err(error) = fs::copy(&source_file, &rune_path) {
        let _ = fs::remove_file(&rune_path);
        return Err(Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", rune_path.display()),
        ));
    }
    if let Err(error) = write_change_stub(&deck_root, change, name, kind, domain) {
        // Both outputs are this call's: the change directory did not exist
        // before the check above, and the rune copy is ours.
        let _ = fs::remove_file(&rune_path);
        let _ = fs::remove_dir_all(&change_dir);
        return Err(error);
    }
    let removed = crate::cli::draft::drop(consumer_root, name)?;
    Ok(Promotion {
        rune_path,
        change_dir,
        removed,
    })
}

/// Scaffold the change with `rune spec propose`, then name the rune in the
/// proposal so the stub is not empty. Nothing here duplicates the templates:
/// a repository override of `templates/spec/proposal.md` still applies.
fn write_change_stub(
    deck_root: &Path,
    change: &str,
    name: &str,
    kind: DraftKind,
    domain: &str,
) -> Result<(), Error> {
    let source = deck_root.to_string_lossy();
    rune_docs::spec::propose_output(&source, change, &[], false)
        .map_err(|error| crate::cli::docs_boundary::convert(&error))?;
    let proposal_path = deck_root
        .join("docs")
        .join("changes")
        .join(change)
        .join("proposal.md");
    let proposal = fs::read_to_string(&proposal_path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", proposal_path.display()),
        )
    })?;
    let rune_rel = format!("runes/{domain}/{}", kind.relative_path(name));
    let kind_name = kind.name();
    let proposal = proposal
        .replace(
            "- Describe the observable change.",
            &format!("- Add the {name} {kind_name} at `{rune_rel}`, promoted from a `rune draft`."),
        )
        .replace(
            "- List the affected runes, casts, commands, or documentation.",
            &format!("- `{rune_rel}` (new)."),
        );
    fs::write(&proposal_path, proposal).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", proposal_path.display()),
        )
    })
}

/// The command entry: promote and print the outcome.
pub fn run(
    target: &str,
    name: &str,
    domain: &str,
    change: &str,
    deck: Option<&str>,
    json: bool,
) -> Result<i32, Error> {
    let done = execute(Path::new(target), name, domain, change, deck)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "promoted": name,
                "rune": done.rune_path.display().to_string(),
                "change": done.change_dir.display().to_string(),
                "removed": done.removed,
            })
        );
    } else {
        println!("promoted {name} to {}", done.rune_path.display());
        for path in &done.removed {
            println!("removed {path}");
        }
        println!("change stub at {}", done.change_dir.display());
    }
    Ok(0)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
