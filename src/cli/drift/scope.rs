//! Manifest-scoped drift verification: compare a module's assembled `build/`
//! against where it was deployed, limited to this module's own files.
//!
//! `rune drift --upstream <DIR>` compares two module trees by name and, run
//! against a multi-module deployment like `~/.claude`, floods the report with
//! every other module's files as "upstream only". This mode instead walks
//! `build/<provider>` (this module's output) and compares each file to its
//! deployed counterpart, then consults the deployed `.manifest` + provenance
//! attribution to flag this module's deployed files that are no longer built.

use rune::error::{Error, ErrorKind};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{DriftEntry, DriftResult, DriftStatus, compare_file_content, print_drift_result};
use crate::cli::config;
use crate::cli::deploy::{is_owned_by_module, load_deployed_manifest};
use rune::provider::{ContentKind, ProviderConfig};

/// Verify each provider's `build/<provider>` against `<target_base>/<provider
/// target>` (mirroring `rune install --target`), scoped to this module.
pub fn execute(
    source: &str,
    target_base: &str,
    ignore: &[String],
    show_all: bool,
    json_output: bool,
) -> Result<i32, Error> {
    let module_root = Path::new(source);
    if !module_root.join("module.yaml").is_file() && !crate::cli::dotrune::exists(module_root) {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "{source} is not a module or consumer root (no module.yaml or .rune); --target verify compares its build/ against a deployment"
            ),
        ));
    }

    let source_uri = config::load_source_uri(module_root);
    let module_name = (!source_uri.is_empty()).then_some(source_uri.as_str());

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;

    let ignored: HashSet<&str> = ignore.iter().map(String::as_str).collect();
    let base = Path::new(target_base);
    let mut result = DriftResult::default();
    let mut compared_any = false;

    for (provider_name, provider_config) in &providers {
        // Deploy skips opt-in providers unless named explicitly, so their
        // build output has no deployed counterpart to verify.
        if !provider_config.enabled {
            continue;
        }
        let build_dir = module_root.join("build").join(provider_name);
        if !build_dir.is_dir() {
            continue;
        }
        compared_any = true;
        compare_provider(
            &mut result,
            &build_dir,
            base,
            provider_config,
            provider_name,
            module_name,
            &ignored,
        );
    }

    if !compared_any {
        return Err(Error::new(
            ErrorKind::Io,
            format!("no build/ output under {source}; run `rune assemble` or `rune install` first"),
        ));
    }

    if json_output {
        match serde_json::to_string_pretty(&result) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize drift result: {error}"),
        }
    } else {
        print_drift_result(&result, show_all);
    }

    // LocalOnly counts as drift in target mode: a built file missing from the
    // deployment is exactly what verification exists to catch.
    let has_drift = result.entries.iter().any(|entry| {
        matches!(
            entry.status,
            DriftStatus::FrontmatterOnly
                | DriftStatus::BodyOnly
                | DriftStatus::Both
                | DriftStatus::UpstreamOnly
                | DriftStatus::LocalOnly
                | DriftStatus::Unreadable
        )
    });
    Ok(i32::from(has_drift))
}

/// Verify manifest-tracked files in provider targets discovered in the current
/// working directory. This mode needs neither source material nor `build/`.
pub fn execute_discovered(
    target_base: &Path,
    discovered_targets: &[PathBuf],
    show_all: bool,
    json_output: bool,
) -> Result<i32, Error> {
    let mut result = DriftResult::default();

    for target in discovered_targets {
        let deployed_base = target_base.join(target);
        let category = target
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .trim_start_matches('.')
            .to_string();
        let mut entries = load_deployed_manifest(&deployed_base)?
            .into_iter()
            .collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));

        for (relative, manifest_entry) in entries {
            // The rule-wiring key is a virtual entry, not a deployed file.
            if relative == crate::cli::deploy::wiring::WIRING_MANIFEST_KEY {
                continue;
            }
            let status = match fs::read_to_string(deployed_base.join(&relative)) {
                Ok(content)
                    if rune::manifest::content_sha256(&content) == manifest_entry.fingerprint =>
                {
                    DriftStatus::Identical
                }
                Ok(_) => DriftStatus::BodyOnly,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    DriftStatus::UpstreamOnly
                }
                Err(_) => DriftStatus::Unreadable,
            };
            result
                .entries
                .push(only_entry(&relative, status, &category));
        }
    }

    if json_output {
        match serde_json::to_string_pretty(&result) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize drift result: {error}"),
        }
    } else {
        print_drift_result(&result, show_all);
    }

    let has_drift = result.entries.iter().any(|entry| {
        matches!(
            entry.status,
            DriftStatus::FrontmatterOnly
                | DriftStatus::BodyOnly
                | DriftStatus::Both
                | DriftStatus::UpstreamOnly
                | DriftStatus::Unreadable
        )
    });
    Ok(i32::from(has_drift))
}

/// Verify only the subset recorded in the deployment manifests. A cast does
/// not materialize unselected deck artifacts, so the target manifest is the
/// authoritative comparison scope.
pub fn execute_deck(
    deck: &rune::deck::Deck,
    target_base: &str,
    _ignore: &[String],
    show_all: bool,
    json_output: bool,
) -> Result<i32, Error> {
    let merged_config = config::load_merged_config(&deck.root)?;
    let providers = config::load_providers(&merged_config)?;
    let base = Path::new(target_base);
    let mut result = DriftResult::default();
    let mut providers = providers.iter().collect::<Vec<_>>();
    providers.sort_by_key(|(name, _)| *name);

    for deck_entry in &deck.entries {
        println!("== {} ==", deck_entry.name);
        let source_uri = deck_entry.manifest.source_uri();
        for (provider_name, provider_config) in &providers {
            for target_root in provider_config.target_roots() {
                let deployed_base = base.join(target_root);
                for (key, entry) in load_deployed_manifest(&deployed_base).unwrap_or_else(|error| {
                    eprintln!("warning: {error}; treating manifest as empty for drift");
                    std::collections::HashMap::new()
                }) {
                    let Some(kind) = kind_for_relative(&key) else {
                        continue;
                    };
                    if base.join(provider_config.target_for_kind(kind)) != deployed_base
                        || !entry_belongs_to_deck(
                            &key,
                            &entry,
                            &deployed_base,
                            deck_entry,
                            source_uri,
                        )
                    {
                        continue;
                    }
                    let status = match fs::read_to_string(deployed_base.join(&key)) {
                        Ok(content)
                            if rune::manifest::content_sha256(&content) == entry.fingerprint =>
                        {
                            DriftStatus::Identical
                        }
                        Ok(_) => DriftStatus::BodyOnly,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            DriftStatus::UpstreamOnly
                        }
                        Err(_) => DriftStatus::Unreadable,
                    };
                    result.entries.push(only_entry(
                        &key,
                        status,
                        &format!("{}/{}", deck_entry.name, provider_name),
                    ));
                }
            }
        }
    }

    if json_output {
        match serde_json::to_string_pretty(&result) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize drift result: {error}"),
        }
    } else {
        print_drift_result(&result, show_all);
    }
    let has_drift = result.entries.iter().any(|entry| {
        matches!(
            entry.status,
            DriftStatus::FrontmatterOnly
                | DriftStatus::BodyOnly
                | DriftStatus::Both
                | DriftStatus::UpstreamOnly
                | DriftStatus::Unreadable
        )
    });
    Ok(i32::from(has_drift))
}

fn entry_belongs_to_deck(
    key: &str,
    entry: &rune::manifest::ManifestEntry,
    deployed_base: &Path,
    deck_entry: &rune::deck::DeckEntry,
    source_uri: &str,
) -> bool {
    if key.starts_with(&format!("hooks/{}/", deck_entry.name)) {
        return true;
    }
    if entry.provenance.is_some() && is_owned_by_module(entry, deployed_base, Some(source_uri)) {
        return true;
    }
    let Some(provenance) = &entry.provenance else {
        return false;
    };
    let Ok(sidecar) = rune::manifest::provenance::read(&deployed_base.join(provenance)) else {
        return false;
    };
    sidecar
        .provenance
        .predicate
        .build_definition
        .resolved_dependencies
        .iter()
        .any(|dependency| deck_entry.root.join(&dependency.uri).is_file())
}

fn compare_provider(
    result: &mut DriftResult,
    build_dir: &Path,
    target_base: &Path,
    provider_config: &ProviderConfig,
    provider_name: &str,
    module_name: Option<&str>,
    ignored: &HashSet<&str>,
) {
    let build_files = collect_content_files(build_dir);

    // A home-scope install wires codex rules into AGENTS.md instead of
    // deploying them, recording the wiring key in the manifest. When that key
    // is present, codex rules have no on-disk counterpart to compare; a
    // project-scope install has no key and deploys rules normally.
    let rules_wired = provider_config.default_target() == ".codex"
        && load_deployed_manifest(&target_base.join(provider_config.default_target())).is_ok_and(
            |manifest| manifest.contains_key(crate::cli::deploy::wiring::WIRING_MANIFEST_KEY),
        );

    for (relative, build_content) in &build_files {
        let Some(kind) = kind_for_relative(relative) else {
            continue;
        };
        // Build sidecars (`Foo.yaml` beside `Foo.md`) deploy under
        // `.provenance/`, not at their build-relative path; deploy skips them
        // as content and so does this comparison.
        if kind != ContentKind::Hooks
            && Path::new(relative).extension().unwrap_or_default() == "yaml"
        {
            continue;
        }
        if kind == ContentKind::Rules && rules_wired {
            continue;
        }
        let target_root = provider_config.target_for_kind(kind);
        let deployed_base = target_base.join(target_root);
        let deployed_path = deployed_base.join(relative);
        match fs::read(&deployed_path) {
            Ok(deployed_bytes) => {
                // Text pairs get the frontmatter/body comparison; anything
                // binary on either side compares by bytes.
                let text_pair = (
                    std::str::from_utf8(build_content),
                    std::str::from_utf8(&deployed_bytes),
                );
                let entry = if let (Ok(build_text), Ok(deployed_text)) = text_pair {
                    compare_file_content(
                        relative,
                        build_text,
                        deployed_text,
                        provider_name,
                        ignored,
                    )
                } else if build_content == &deployed_bytes {
                    only_entry(relative, DriftStatus::Identical, provider_name)
                } else {
                    only_entry(relative, DriftStatus::BodyOnly, provider_name)
                };
                result.entries.push(entry);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                result
                    .entries
                    .push(only_entry(relative, DriftStatus::LocalOnly, provider_name));
            }
            Err(_) => {
                result
                    .entries
                    .push(only_entry(relative, DriftStatus::Unreadable, provider_name));
            }
        }
    }

    // This module's deployed files (per the target manifest + provenance) that
    // are no longer built — stale deployments that should be pruned.
    for target_root in provider_config.target_roots() {
        let deployed_base = target_base.join(target_root);
        for (key, entry) in load_deployed_manifest(&deployed_base).unwrap_or_else(|error| {
            eprintln!("warning: {error}; treating manifest as empty for drift");
            std::collections::HashMap::new()
        }) {
            let Some(kind) = kind_for_relative(&key) else {
                continue;
            };
            let expected_base = target_base.join(provider_config.target_for_kind(kind));
            if expected_base == deployed_base && build_files.contains_key(&key) {
                continue;
            }
            // Deploy generates these inside a plugin root; they have no
            // build counterpart, and the manifest fingerprint covers them.
            if is_generated_plugin_file(&key, &deployed_base) {
                continue;
            }
            if is_owned_by_module(&entry, &deployed_base, module_name) {
                result
                    .entries
                    .push(only_entry(&key, DriftStatus::UpstreamOnly, provider_name));
            }
        }
    }
}

fn is_generated_plugin_file(key: &str, deployed_base: &Path) -> bool {
    matches!(key, "hooks/hooks.json" | ".claude-plugin/plugin.json")
        && deployed_base.join(".claude-plugin/plugin.json").is_file()
}

fn only_entry(name: &str, status: DriftStatus, category: &str) -> DriftEntry {
    DriftEntry {
        name: name.to_string(),
        status,
        category: category.to_string(),
        changed_keys: Vec::new(),
        renamed_from: None,
        source_uri: None,
    }
}

fn kind_for_relative(relative: &str) -> Option<ContentKind> {
    match relative.split_once('/').map(|(kind, _)| kind) {
        Some("agents") => Some(ContentKind::Agents),
        Some("skills") => Some(ContentKind::Skills),
        Some("rules") => Some(ContentKind::Rules),
        Some("hooks") => Some(ContentKind::Hooks),
        _ => None,
    }
}

/// Collect content files under a provider build directory, keyed by their path
/// relative to it. Sidecar directories, `.manifest`, and dotfiles are skipped.
fn collect_content_files(build_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_recursive(build_dir, build_dir, &mut files);
    files
}

fn collect_recursive(base: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_recursive(base, &path, files);
        } else if let Ok(content) = fs::read(&path) {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            files.insert(relative, content);
        }
    }
}

#[cfg(test)]
mod tests;
