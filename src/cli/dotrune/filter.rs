//! Filter a producer's full `Vec<SourceFile>` down to the subset requested
//! by a single source's `RuneList`. Records which requested names matched
//! and errors if any requested rune failed to resolve.

use commands::error::{Error, ErrorKind};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::cli::assemble::sources::SourceFile;
use crate::cli::dotrune::parse::RuneList;

pub fn filter_to_requested(
    all_files: Vec<SourceFile>,
    list: &RuneList,
    source_label: &str,
    source_path: &Path,
) -> Result<Vec<SourceFile>, Error> {
    let wanted_skills: HashSet<&str> = list.skills.iter().map(String::as_str).collect();
    let wanted_agents: HashSet<&str> = list.agents.iter().map(String::as_str).collect();
    let wanted_rules: HashSet<&str> = list.rules.iter().map(String::as_str).collect();
    let wanted_hooks: HashSet<&str> = list.hooks.iter().map(String::as_str).collect();

    let mut kept: Vec<SourceFile> = Vec::new();
    let mut seen_skills: BTreeSet<String> = BTreeSet::new();
    let mut seen_agents: BTreeSet<String> = BTreeSet::new();
    let mut seen_rules: BTreeSet<String> = BTreeSet::new();
    let mut seen_hooks: BTreeSet<String> = BTreeSet::new();

    for file in all_files {
        if let Some(name) = skill_name(&file.relative_path)
            && wanted_skills.contains(name.as_str())
        {
            seen_skills.insert(name);
            kept.push(file);
        } else if let Some(name) = flat_name(&file.relative_path, "agents/")
            && wanted_agents.contains(name.as_str())
        {
            seen_agents.insert(name);
            kept.push(file);
        } else if let Some(name) = flat_name(&file.relative_path, "rules/")
            && wanted_rules.contains(name.as_str())
        {
            seen_rules.insert(name);
            kept.push(file);
        } else if let Some(name) = flat_name(&file.relative_path, "hooks/")
            && wanted_hooks.contains(name.as_str())
        {
            seen_hooks.insert(name);
            kept.push(file);
        }
    }

    require_matched(
        "skill",
        &list.skills,
        &seen_skills,
        source_label,
        source_path,
    )?;
    require_matched(
        "agent",
        &list.agents,
        &seen_agents,
        source_label,
        source_path,
    )?;
    require_matched("rule", &list.rules, &seen_rules, source_label, source_path)?;
    require_matched("hook", &list.hooks, &seen_hooks, source_label, source_path)?;

    Ok(kept)
}

pub fn filter_deck_to_requested(
    mut all_files: Vec<SourceFile>,
    list: &RuneList,
    source_label: &str,
    deck: &commands::deck::Deck,
) -> Result<Vec<SourceFile>, Error> {
    let canonical_ids: BTreeSet<String> = all_files
        .iter()
        .filter_map(|file| file.rune_id.clone())
        .collect();
    let mut selected = BTreeSet::new();

    for cast in &list.casts {
        selected.extend(
            deck.resolve_cast(cast, canonical_ids.iter().map(String::as_str))
                .map_err(|message| Error::new(ErrorKind::Config, format!(".rune: {message}")))?,
        );
    }

    for requested in list.ids() {
        if let Some(deck_ids) = whole_deck_selection(requested, deck, &canonical_ids) {
            selected.extend(deck_ids);
            continue;
        }
        let candidates: Vec<&String> = canonical_ids
            .iter()
            .filter(|candidate| {
                if requested.contains(['*', '?']) {
                    commands::deck::matches_rune_glob(requested, candidate)
                } else {
                    id_matches(requested, candidate)
                }
            })
            .collect();
        match candidates.as_slice() {
            [] => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        ".rune: rune '{requested}' requested from source '{source_label}' not found at {}",
                        deck.root.display()
                    ),
                ));
            }
            [candidate] => {
                selected.insert((*candidate).clone());
            }
            _ if requested.contains(['*', '?']) => {
                selected.extend(candidates.into_iter().cloned());
            }
            _ => {
                let listed = candidates
                    .iter()
                    .map(|candidate| candidate.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(".rune: rune '{requested}' is ambiguous; candidates: {listed}"),
                ));
            }
        }
    }

    let selected_decks = selected
        .iter()
        .filter_map(|id| {
            let mut parts = id.split('/');
            let deck_entry = parts.next()?;
            let kind = parts.next()?;
            (kind != "hooks").then_some(deck_entry.to_string())
        })
        .collect::<BTreeSet<_>>();
    for id in &canonical_ids {
        let mut parts = id.split('/');
        let deck_entry = parts.next().unwrap_or_default();
        let kind = parts.next().unwrap_or_default();
        if kind == "hooks" && selected_decks.contains(deck_entry) {
            selected.insert(id.clone());
        }
    }
    selected.retain(|id| {
        let mut parts = id.split('/');
        let deck_entry = parts.next().unwrap_or_default();
        let kind = parts.next().unwrap_or_default();
        kind != "hooks" || selected_decks.contains(deck_entry)
    });
    selected.retain(|id| {
        !list
            .exclude
            .iter()
            .any(|pattern| commands::deck::matches_rune_glob(pattern, id))
    });

    all_files.retain(|file| {
        file.rune_id
            .as_ref()
            .is_some_and(|id| selected.contains(id))
    });
    all_files.sort_by_key(deck_output_key);
    Ok(all_files)
}

/// A bare token that names a deck selects every rune in it. Deck names win
/// over rune names so `add development` never silently narrows to a rune
/// that happens to share the deck's name.
fn whole_deck_selection(
    requested: &str,
    deck: &commands::deck::Deck,
    canonical_ids: &BTreeSet<String>,
) -> Option<Vec<String>> {
    if requested.contains('/') || requested.contains(['*', '?']) {
        return None;
    }
    deck.entries
        .iter()
        .any(|deck_entry| deck_entry.name == requested)
        .then(|| {
            canonical_ids
                .iter()
                .filter(|id| id.split('/').next() == Some(requested))
                .cloned()
                .collect()
        })
}

fn id_matches(requested: &str, canonical: &str) -> bool {
    let requested_parts: Vec<&str> = requested.split('/').collect();
    let canonical_parts: Vec<&str> = canonical.split('/').collect();
    match (requested_parts.as_slice(), canonical_parts.as_slice()) {
        ([deck, kind, name], [candidate_deck, candidate_kind, candidate_name]) => {
            deck == candidate_deck && kind == candidate_kind && name == candidate_name
        }
        ([deck, name], [candidate_deck, _, candidate_name]) => {
            deck == candidate_deck && name == candidate_name
        }
        ([name], [_, _, candidate_name]) => name == candidate_name,
        _ => false,
    }
}

fn deck_output_key(file: &SourceFile) -> (String, u8, String, String) {
    let id = file.rune_id.as_deref().unwrap_or_default();
    let mut parts = id.split('/');
    let deck = parts.next().unwrap_or_default().to_string();
    let kind = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default().to_string();
    let kind_order = match kind {
        "skills" => 0,
        "agents" => 1,
        "rules" => 2,
        "hooks" => 3,
        _ => 4,
    };
    (deck, kind_order, name, file.relative_path.clone())
}

fn skill_name(relative_path: &str) -> Option<String> {
    let stripped = relative_path.strip_prefix("skills/")?;
    let first = stripped.split('/').next()?;
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn flat_name(relative_path: &str, prefix: &str) -> Option<String> {
    let stripped = relative_path.strip_prefix(prefix)?;
    if stripped.contains('/') {
        return None;
    }
    let name = stripped.strip_suffix(".md")?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn require_matched(
    kind: &str,
    requested: &[String],
    matched: &BTreeSet<String>,
    source_label: &str,
    source_path: &Path,
) -> Result<(), Error> {
    for name in requested {
        if !matched.contains(name) {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    ".rune: {kind} '{name}' requested from source '{source_label}' not found at {}",
                    source_path.display()
                ),
            ));
        }
    }
    Ok(())
}
