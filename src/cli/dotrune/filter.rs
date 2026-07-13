//! Filter a producer's full `Vec<SourceFile>` down to the subset requested
//! by a single source's `ArtifactList`. Records which requested names matched
//! and errors if any requested artifact failed to resolve.

use commands::error::{Error, ErrorKind};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::cli::assemble::sources::SourceFile;
use crate::cli::dotrune::parse::ArtifactList;

pub fn filter_to_requested(
    all_files: Vec<SourceFile>,
    list: &ArtifactList,
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
    list: &ArtifactList,
    source_label: &str,
    source_path: &Path,
) -> Result<Vec<SourceFile>, Error> {
    let canonical_ids: BTreeSet<String> = all_files
        .iter()
        .filter_map(|file| file.artifact_id.clone())
        .collect();
    let mut selected = BTreeSet::new();

    for requested in list.ids() {
        let candidates: Vec<&String> = canonical_ids
            .iter()
            .filter(|candidate| id_matches(requested, candidate))
            .collect();
        match candidates.as_slice() {
            [] => {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        ".rune: artifact '{requested}' requested from source '{source_label}' not found at {}",
                        source_path.display()
                    ),
                ));
            }
            [candidate] => {
                selected.insert((*candidate).clone());
            }
            _ => {
                let listed = candidates
                    .iter()
                    .map(|candidate| candidate.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(".rune: artifact '{requested}' is ambiguous; candidates: {listed}"),
                ));
            }
        }
    }

    all_files.retain(|file| {
        file.artifact_id
            .as_ref()
            .is_some_and(|id| selected.contains(id))
    });
    all_files.sort_by_key(deck_output_key);
    Ok(all_files)
}

fn id_matches(requested: &str, canonical: &str) -> bool {
    let requested_parts: Vec<&str> = requested.split('/').collect();
    let canonical_parts: Vec<&str> = canonical.split('/').collect();
    match (requested_parts.as_slice(), canonical_parts.as_slice()) {
        ([domain, kind, name], [candidate_domain, candidate_kind, candidate_name]) => {
            domain == candidate_domain && kind == candidate_kind && name == candidate_name
        }
        ([domain, name], [candidate_domain, _, candidate_name]) => {
            domain == candidate_domain && name == candidate_name
        }
        ([name], [_, _, candidate_name]) => name == candidate_name,
        _ => false,
    }
}

fn deck_output_key(file: &SourceFile) -> (String, u8, String, String) {
    let id = file.artifact_id.as_deref().unwrap_or_default();
    let mut parts = id.split('/');
    let domain = parts.next().unwrap_or_default().to_string();
    let kind = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default().to_string();
    let kind_order = match kind {
        "skills" => 0,
        "agents" => 1,
        "rules" => 2,
        "hooks" => 3,
        _ => 4,
    };
    (domain, kind_order, name, file.relative_path.clone())
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
