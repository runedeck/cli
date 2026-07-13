//! Scan deployment targets and build a `DashboardView`.
//!
//! Rune artifacts can be deployed anywhere: `~/.claude/` (user-scope),
//! `./.claude/` (project-scope), or a custom `--target` path. The scanner
//! reads `.manifest` files from known provider directories at each target
//! and groups artifacts by their source module (via provenance `source_uri`).

mod adr;
pub mod builders;
mod discovery;
pub mod files;
mod history;
mod provenance;
mod references;
mod sidecar;
mod source;
mod target;
mod vcs;

pub use adr::build_adr_artifact;
pub use discovery::discover_local_repos;
pub use history::{
    DEFAULT_HISTORY_BATCH_SIZE, DEFAULT_HISTORY_METADATA_WINDOW, HistoryEntry, HistoryOptions,
    HistoryScope, HistoryUpdate, HistoryWalker, extract_frontmatter_field, git_log_for_artifact,
    read_source_adoption, read_source_sidecar, source_at_deploy,
};
pub use source::{parse_frontmatter, strip_frontmatter};
pub use target::git_log_in_repo;

use crate::error::{Error, ErrorKind};
use crate::manifest::FileStatus;
use crate::provider::ContentKind;
use crate::view::{
    CastView, DashboardView, DeckEntryValidationView, DeckEntryView, DeckTargetArtifactView,
    DeckTargetView, DeckView, ModuleView, StatusSummary, Variant,
};
use adr::discover_adrs;
use discovery::{discover_targets, scan_deck_source_module, scan_source_module};
use provenance::collect_provenance;
use references::artifact_staleness;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use target::{PendingCompanion, attach_companions, scan_target};

/// Content kinds scanned, sourced from the shared `ContentKind` enum rather
/// than a local hardcoded list.
pub(super) fn content_kinds() -> [&'static str; 3] {
    [
        ContentKind::Agents.as_str(),
        ContentKind::Rules.as_str(),
        ContentKind::Skills.as_str(),
    ]
}

pub fn build_view(
    root: &Path,
    providers: &[(String, String)],
    watched_locations: &[PathBuf],
) -> Result<DashboardView, Error> {
    if crate::deck::is_deck(root) {
        return build_deck_view(root, providers, watched_locations);
    }
    let targets = discover_targets(root, providers, watched_locations);
    if targets.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no deployed provider directories found at ~ or {}",
                root.display()
            ),
        ));
    }

    let local_repos = discover_local_repos(root, watched_locations);
    let mut modules_by_source: BTreeMap<String, ModuleView> = BTreeMap::new();
    let mut summary = StatusSummary::default();
    let mut pending_companions: Vec<PendingCompanion> = Vec::new();

    for target_base in &targets {
        scan_target(
            target_base,
            &mut modules_by_source,
            &mut summary,
            &local_repos,
            &mut pending_companions,
            providers,
        );
    }

    attach_companions(&mut modules_by_source, pending_companions);

    let mut modules: Vec<ModuleView> = modules_by_source.into_values().collect();

    for location in watched_locations {
        let module_root = fs::canonicalize(location).unwrap_or_else(|_| location.clone());
        if let Some(mut watched_module) = scan_source_module(&module_root) {
            watched_module.is_target = true;
            modules.push(watched_module);
        }
    }

    if modules.is_empty() {
        modules.push(ModuleView {
            name: "(no manifest)".to_string(),
            version: String::new(),
            description: "No .manifest files found at scanned targets".to_string(),
            source_uri: String::new(),
            is_target: false,
            artifacts: Vec::new(),
            local_path: None,
            vcs: None,
            git_log: Vec::new(),
        });
    }

    let mut provenance = Vec::new();
    for target_base in &targets {
        collect_provenance(target_base, providers, &mut provenance);
    }

    let provider_names: Vec<String> = providers.iter().map(|(name, _)| name.clone()).collect();
    // Several modules can share one repo; scan each repo's VCS state once.
    let mut repo_state: BTreeMap<PathBuf, (Option<vcs::RepoVcs>, Vec<crate::view::GitCommit>)> =
        BTreeMap::new();
    for (module_index, module) in modules.iter_mut().enumerate() {
        module.artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        let repo = local_repos.get(module.source_uri.trim_end_matches(".git"));
        if let Some(repo) = repo
            && !repo_state.contains_key(repo.as_path())
        {
            repo_state.insert(repo.clone(), (vcs::repo_vcs(repo), vcs::repo_log(repo)));
        }
        let cached = repo.and_then(|repo| repo_state.get(repo.as_path()));
        let repo_vcs = cached.and_then(|(state, _)| state.as_ref());
        module.local_path = repo.cloned();
        module.vcs = repo_vcs.map(vcs::RepoVcs::module_state);
        module.git_log = cached.map(|(_, log)| log.clone()).unwrap_or_default();
        let tint = module_index % 8;
        for artifact in &mut module.artifacts {
            artifact.module.clone_from(&module.name);
            artifact.module_tint = tint;
            let vcs_path = if artifact.source_path.is_empty() {
                artifact.relative_path.clone()
            } else {
                artifact.source_path.clone()
            };
            artifact.vcs = repo_vcs.map(|state| state.state_for(&vcs_path));
            let (broken, age) = artifact_staleness(
                repo,
                &vcs_path,
                &artifact.raw_source,
                artifact.latest_commit_date(),
            );
            artifact.broken_refs = broken;
            artifact.age_days = age;
            artifact.variants = repo
                .map(|repo| collect_variants(repo, &artifact.relative_path, &provider_names))
                .unwrap_or_default();
        }
    }

    let adrs = discover_adrs(&local_repos, &active_repo_names(&modules, root));

    Ok(DashboardView {
        modules,
        summary,
        provenance,
        adrs,
        deck: None,
    })
}

fn build_deck_view(
    root: &Path,
    providers: &[(String, String)],
    watched_locations: &[PathBuf],
) -> Result<DashboardView, Error> {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let deck =
        crate::deck::load(&root).map_err(|message| Error::new(ErrorKind::Config, message))?;
    let repo_vcs = vcs::repo_vcs(&root);
    let repo_log = vcs::repo_log(&root);
    let provider_names = providers
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let mut modules = Vec::new();
    let mut deck_entries = Vec::new();
    let mut rune_ids = Vec::new();

    for (index, deck_entry) in deck.entries.iter().enumerate() {
        let (module, deck_entry_view, ids) = build_deck_entry(
            &root,
            &deck,
            deck_entry,
            index,
            repo_vcs.as_ref(),
            &repo_log,
            &provider_names,
        );
        deck_entries.push(deck_entry_view);
        rune_ids.extend(ids);
        modules.push(module);
    }

    let casts = deck
        .casts()
        .map(
            |cast| match deck.resolve_cast(&cast.name, rune_ids.iter().map(String::as_str)) {
                Ok(resolved_runes) => CastView {
                    name: cast.name.clone(),
                    description: cast.description.clone(),
                    extends: cast.extends.clone(),
                    runes: cast.runes.clone(),
                    exclude: cast.exclude.clone(),
                    resolved_runes,
                    resolution_error: None,
                },
                Err(message) => CastView {
                    name: cast.name.clone(),
                    description: cast.description.clone(),
                    extends: cast.extends.clone(),
                    runes: cast.runes.clone(),
                    exclude: cast.exclude.clone(),
                    resolved_runes: Vec::new(),
                    resolution_error: Some(message),
                },
            },
        )
        .collect();
    let targets = discover_deck_targets(&deck, providers, watched_locations);
    let mut summary = StatusSummary::default();
    for target in &targets {
        summary.unchanged += target.summary.unchanged;
        summary.stale += target.summary.stale;
        summary.modified += target.summary.modified;
        summary.new += target.summary.new;
    }
    let local_repos = discover_local_repos(&root, watched_locations);
    let adrs = discover_adrs(&local_repos, &active_repo_names(&modules, &root));

    Ok(DashboardView {
        modules,
        summary,
        provenance: Vec::new(),
        adrs,
        deck: Some(DeckView {
            root,
            name: deck.manifest.name,
            version: deck.manifest.version,
            description: deck.manifest.description,
            entries: deck_entries,
            casts,
            targets,
        }),
    })
}

fn build_deck_entry(
    root: &Path,
    deck: &crate::deck::Deck,
    deck_entry: &crate::deck::DeckEntry,
    index: usize,
    repo_vcs: Option<&vcs::RepoVcs>,
    repo_log: &[crate::view::GitCommit],
    provider_names: &[String],
) -> (ModuleView, DeckEntryView, Vec<String>) {
    let root_path = root.to_path_buf();
    let mut module = scan_deck_source_module(&deck_entry.root).unwrap_or_else(|| ModuleView {
        name: deck_entry.name.clone(),
        version: deck_entry.manifest.version.clone(),
        description: deck_entry.manifest.description.clone(),
        source_uri: deck_entry.manifest.source_uri().to_string(),
        is_target: false,
        artifacts: Vec::new(),
        local_path: Some(root_path.clone()),
        vcs: None,
        git_log: Vec::new(),
    });
    module.name.clone_from(&deck_entry.name);
    module.version.clone_from(&deck_entry.manifest.version);
    module
        .description
        .clone_from(&deck_entry.manifest.description);
    module.source_uri = deck_entry.manifest.source_uri().to_string();
    module.is_target = false;
    module.local_path = Some(root_path.clone());
    module.vcs = repo_vcs.map(vcs::RepoVcs::module_state);
    module.git_log = repo_log.to_vec();

    for artifact in &mut module.artifacts {
        artifact.module.clone_from(&deck_entry.name);
        artifact.module_tint = index % 8;
        artifact.source_path = format!("runes/{}/{}", deck_entry.name, artifact.relative_path);
        artifact.vcs = repo_vcs.map(|state| state.state_for(&artifact.source_path));
        let (broken, age) = artifact_staleness(
            Some(&root_path),
            &artifact.source_path,
            &artifact.raw_source,
            artifact.latest_commit_date(),
        );
        artifact.broken_refs = broken;
        artifact.age_days = age;
        artifact.variants =
            collect_variants(&deck_entry.root, &artifact.relative_path, provider_names);
    }
    module.artifacts.sort_by(|left, right| {
        kind_order(&left.kind)
            .cmp(&kind_order(&right.kind))
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut rune_counts = BTreeMap::new();
    let rune_ids = module
        .artifacts
        .iter()
        .map(|artifact| {
            *rune_counts.entry(artifact.kind.clone()).or_insert(0) += 1;
            format!("{}/{}/{}", deck_entry.name, artifact.kind, artifact.name)
        })
        .collect();
    let deck_entry_view = DeckEntryView {
        name: deck_entry.name.clone(),
        version: deck_entry.manifest.version.clone(),
        description: deck_entry.manifest.description.clone(),
        source_uri: deck_entry.manifest.source_uri().to_string(),
        providers: deck.providers_for(deck_entry).unwrap_or_default().to_vec(),
        rune_counts,
        validation: validate_deck_entry_inventory(&module),
    };
    (module, deck_entry_view, rune_ids)
}

fn validate_deck_entry_inventory(module: &ModuleView) -> DeckEntryValidationView {
    let mut errors = Vec::new();
    for artifact in &module.artifacts {
        for broken in &artifact.broken_refs {
            errors.push(format!(
                "{}: broken reference {broken}",
                artifact.relative_path
            ));
        }
        let extension = Path::new(&artifact.relative_path)
            .extension()
            .and_then(std::ffi::OsStr::to_str);
        let syntax_error = if extension.is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
            serde_json::from_str::<serde_json::Value>(&artifact.raw_source)
                .err()
                .map(|error| format!("invalid JSON: {error}"))
        } else if extension
            .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        {
            serde_yaml::from_str::<serde_yaml::Value>(&artifact.raw_source)
                .err()
                .map(|error| format!("invalid YAML: {error}"))
        } else {
            None
        };
        if let Some(error) = syntax_error {
            errors.push(format!("{}: {error}", artifact.relative_path));
        }
    }
    DeckEntryValidationView {
        valid: errors.is_empty(),
        errors,
    }
}

fn kind_order(kind: &str) -> usize {
    crate::view::KIND_ORDER
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(crate::view::KIND_ORDER.len())
}

fn discover_deck_targets(
    deck: &crate::deck::Deck,
    providers: &[(String, String)],
    watched_locations: &[PathBuf],
) -> Vec<DeckTargetView> {
    let deck_root = fs::canonicalize(&deck.root).unwrap_or_else(|_| deck.root.clone());
    let deck_remote = discovery::git_remote(&deck_root);
    let mut source_roots = std::collections::HashMap::new();
    for deck_entry in &deck.entries {
        source_roots.insert(
            deck_entry
                .manifest
                .source_uri()
                .trim_end_matches(".git")
                .to_string(),
            deck_entry.root.clone(),
        );
    }
    if let Some(remote) = deck_remote.as_ref() {
        source_roots.insert(
            remote.trim_end_matches(".git").to_string(),
            deck_root.clone(),
        );
    }

    let mut locations = watched_locations
        .iter()
        .map(|path| fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
        .filter(|path| path != &deck_root)
        .filter(|path| target_points_at_deck(path, &deck_root, deck_remote.as_deref()))
        .collect::<Vec<_>>();
    locations.sort();
    locations.dedup();

    locations
        .into_iter()
        .map(|target_root| {
            let mut artifacts: BTreeMap<String, DeckTargetArtifactView> = BTreeMap::new();
            for (provider_name, provider_dir) in providers {
                let provider_path = target_root.join(provider_dir);
                for (deployed_path, entry) in source::load_manifest(&provider_path) {
                    let Some(kind) = deployed_path
                        .split('/')
                        .next()
                        .filter(|kind| matches!(*kind, "skills" | "agents" | "rules" | "hooks"))
                    else {
                        continue;
                    };
                    let source_uri = source::resolve_source(&provider_path, &deployed_path, &entry);
                    let source_path = source::resolve_source_path(&provider_path, &entry);
                    let Some(deck_entry) =
                        deck_entry_for_source(deck, &source_uri, source_path.as_deref())
                            .or_else(|| deck_entry_for_deployed_path(deck, kind, &deployed_path))
                    else {
                        continue;
                    };
                    let Some(rune_id) = canonical_deck_rune_id(
                        &deck_entry.name,
                        kind,
                        source_path.as_deref(),
                        &deployed_path,
                    ) else {
                        continue;
                    };
                    let status = target::deployed_status(
                        &provider_path,
                        &deployed_path,
                        &entry,
                        &source_uri,
                        source_path.as_deref(),
                        &source_roots,
                    );
                    let artifact =
                        artifacts
                            .entry(rune_id)
                            .or_insert_with(|| DeckTargetArtifactView {
                                status,
                                providers: BTreeMap::new(),
                            });
                    artifact.providers.insert(provider_name.clone(), status);
                    artifact.status = worse_status(artifact.status, status);
                }
            }
            let mut summary = StatusSummary::default();
            for artifact in artifacts.values() {
                target::tally_status(&mut summary, artifact.status);
            }
            let name = target_root.file_name().map_or_else(
                || target_root.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            DeckTargetView {
                name,
                root: target_root,
                artifacts,
                summary,
            }
        })
        .collect()
}

fn target_points_at_deck(target: &Path, deck_root: &Path, deck_remote: Option<&str>) -> bool {
    let manifest_path = target.join(".rune");
    if !manifest_path.is_file() {
        return false;
    }
    let Ok(content) = fs::read_to_string(manifest_path) else {
        return false;
    };
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return false;
    };
    let Some(sources) = value.get("sources").and_then(serde_yaml::Value::as_mapping) else {
        return false;
    };
    sources.values().any(|source| {
        if let Some(local) = source
            .get("local")
            .and_then(serde_yaml::Value::as_str)
            .or_else(|| {
                source
                    .get("path")
                    .and_then(serde_yaml::Value::as_str)
                    .filter(|_| source.get("git").is_none() && source.get("local").is_none())
            })
        {
            let local = PathBuf::from(local);
            let resolved = if local.is_absolute() {
                local
            } else {
                target.join(local)
            };
            return fs::canonicalize(&resolved).unwrap_or(resolved) == deck_root;
        }
        source
            .get("git")
            .and_then(serde_yaml::Value::as_str)
            .zip(deck_remote)
            .is_some_and(|(source, remote)| {
                source.trim_end_matches(".git") == remote.trim_end_matches(".git")
            })
    })
}

fn deck_entry_for_source<'a>(
    deck: &'a crate::deck::Deck,
    source_uri: &str,
    source_path: Option<&str>,
) -> Option<&'a crate::deck::DeckEntry> {
    if let Some(path) = source_path
        && let Some(deck_name) = path
            .strip_prefix("runes/")
            .and_then(|path| path.split('/').next())
        && let Some(deck_entry) = deck
            .entries
            .iter()
            .find(|deck_entry| deck_entry.name == deck_name)
    {
        return Some(deck_entry);
    }
    let normalized = source_uri.trim_end_matches(".git");
    deck.entries.iter().find(|deck_entry| {
        deck_entry.name == normalized
            || deck_entry.manifest.source_uri().trim_end_matches(".git") == normalized
    })
}

fn deck_entry_for_deployed_path<'a>(
    deck: &'a crate::deck::Deck,
    kind: &str,
    deployed_path: &str,
) -> Option<&'a crate::deck::DeckEntry> {
    let matches = deck
        .entries
        .iter()
        .filter(|deck_entry| {
            scan_deck_source_module(&deck_entry.root).is_some_and(|module| {
                module.artifacts.iter().any(|artifact| {
                    artifact.kind == kind && artifact.relative_path == deployed_path
                })
            })
        })
        .collect::<Vec<_>>();
    (matches.len() == 1).then(|| matches[0])
}

fn canonical_deck_rune_id(
    deck: &str,
    kind: &str,
    source_path: Option<&str>,
    deployed_path: &str,
) -> Option<String> {
    let path = source_path.unwrap_or(deployed_path);
    let deck_prefix = format!("runes/{deck}/");
    let path = path.strip_prefix(&deck_prefix).unwrap_or(path);
    let within_kind = path.strip_prefix(&format!("{kind}/"))?;
    let name = match kind {
        "skills" => within_kind.split('/').next()?.to_string(),
        "hooks" => Path::new(within_kind)
            .with_extension("")
            .to_string_lossy()
            .into_owned(),
        "agents" | "rules" => Path::new(within_kind)
            .file_stem()?
            .to_string_lossy()
            .into_owned(),
        _ => return None,
    };
    (!name.is_empty()).then(|| format!("{deck}/{kind}/{name}"))
}

fn worse_status(left: FileStatus, right: FileStatus) -> FileStatus {
    fn rank(status: FileStatus) -> u8 {
        match status {
            FileStatus::Unchanged => 0,
            FileStatus::New => 1,
            FileStatus::Stale => 2,
            FileStatus::Modified => 3,
        }
    }
    if rank(right) > rank(left) {
        right
    } else {
        left
    }
}

/// A confirmation-ready cast mutation. Persist it only after user approval.
#[derive(Debug, Clone)]
pub struct CastEdit {
    pub cast_name: String,
    pub rune_id: String,
    pub include: bool,
    pub before: Vec<String>,
    pub after: Vec<String>,
    path: PathBuf,
    original: Vec<u8>,
    replacement: Vec<u8>,
}

impl CastEdit {
    /// Serialized YAML that will replace the cast manifest.
    #[must_use]
    pub fn yaml(&self) -> &str {
        std::str::from_utf8(&self.replacement).unwrap_or_default()
    }
}

/// Prepares a cast toggle without writing to disk.
pub fn prepare_cast_toggle(
    deck_root: &Path,
    cast_name: &str,
    rune_id: &str,
    include: bool,
) -> Result<CastEdit, Error> {
    let deck =
        crate::deck::load(deck_root).map_err(|message| Error::new(ErrorKind::Config, message))?;
    let rune_ids = deck_rune_ids(&deck);
    if !rune_ids.iter().any(|candidate| candidate == rune_id) {
        return Err(Error::new(
            ErrorKind::Config,
            format!("unknown deck rune '{rune_id}'"),
        ));
    }
    let cast = deck
        .cast(cast_name)
        .ok_or_else(|| Error::new(ErrorKind::Config, format!("unknown cast '{cast_name}'")))?;
    let before = deck
        .resolve_cast(cast_name, rune_ids.iter().map(String::as_str))
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    let mut edited = cast.clone();
    if include {
        edited.exclude.retain(|pattern| pattern != rune_id);
        if !before.iter().any(|selected| selected == rune_id)
            && !edited.runes.iter().any(|rune| rune == rune_id)
        {
            edited.runes.push(rune_id.to_string());
        }
    } else {
        edited.runes.retain(|rune| rune != rune_id);
        if !edited.exclude.iter().any(|excluded| excluded == rune_id) {
            edited.exclude.push(rune_id.to_string());
        }
    }
    let after = deck
        .resolve_cast_with_override(
            cast_name,
            rune_ids.iter().map(String::as_str),
            Some(&edited),
        )
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    if include && !after.iter().any(|selected| selected == rune_id) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "cannot include '{rune_id}' while a wildcard or inherited cast exclusion still matches it"
            ),
        ));
    }
    let original = fs::read(&cast.path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", cast.path.display()),
        )
    })?;
    let replacement = serde_yaml::to_string(&edited)
        .map_err(|error| Error::new(ErrorKind::Parse, format!("cannot serialize cast: {error}")))?
        .into_bytes();
    Ok(CastEdit {
        cast_name: cast_name.to_string(),
        rune_id: rune_id.to_string(),
        include,
        before,
        after,
        path: cast.path.clone(),
        original,
        replacement,
    })
}

/// Atomically persists a previously prepared cast edit.
pub fn persist_cast_edit(edit: &CastEdit) -> Result<(), Error> {
    let current = fs::read(&edit.path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", edit.path.display()),
        )
    })?;
    if current != edit.original {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "cast '{}' changed after the edit was prepared; refresh before writing",
                edit.cast_name
            ),
        ));
    }
    atomic_write(&edit.path, &edit.replacement)
}

fn deck_rune_ids(deck: &crate::deck::Deck) -> Vec<String> {
    let mut ids = Vec::new();
    for deck_entry in &deck.entries {
        if let Some(module) = scan_deck_source_module(&deck_entry.root) {
            ids.extend(module.artifacts.into_iter().map(|artifact| {
                format!("{}/{}/{}", deck_entry.name, artifact.kind, artifact.name)
            }));
        }
    }
    ids.sort_by(|left, right| {
        let left_parts = left.split('/').collect::<Vec<_>>();
        let right_parts = right.split('/').collect::<Vec<_>>();
        left_parts
            .first()
            .cmp(&right_parts.first())
            .then_with(|| {
                kind_order(left_parts.get(1).copied().unwrap_or_default())
                    .cmp(&kind_order(right_parts.get(1).copied().unwrap_or_default()))
            })
            .then_with(|| left.cmp(right))
    });
    ids
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = parent.join(format!(".cast.tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(Error::new(
            ErrorKind::Io,
            format!("cannot atomically rewrite {}: {error}", path.display()),
        ));
    }
    Ok(())
}

/// Directory-name allowlist for ADRs and schemas: the active modules plus the
/// repo the dashboard runs in. Confines both to the same source set as the
/// rest of the dashboard, dropping ADRs from unrelated sibling repos.
pub fn active_repo_names(modules: &[ModuleView], root: &Path) -> HashSet<String> {
    let mut names: HashSet<String> = modules.iter().map(|module| module.name.clone()).collect();
    if let Some(root_name) = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .file_name()
    {
        names.insert(root_name.to_string_lossy().to_string());
    }
    names
}

/// Discovers harness and model qualifier overrides of a base artifact in the
/// source tree (PROV-0005): `<kind-dir>/<provider>/<file>` for harness-level and
/// `<kind-dir>/<provider>/<model>/<file>` for model-level, plus the `user/`
/// overlay. The base directory is the artifact file's parent, so the same logic
/// serves flat kinds (rules, agents) and skill directories alike.
pub(super) fn collect_variants(
    repo: &Path,
    relative_path: &str,
    provider_names: &[String],
) -> Vec<Variant> {
    let base_file = repo.join(relative_path);
    let Some(base_dir) = base_file.parent() else {
        return Vec::new();
    };
    let Some(file_name) = base_file.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let mut qualifiers: Vec<&str> = provider_names.iter().map(String::as_str).collect();
    qualifiers.push("user");
    let mut variants = Vec::new();
    for provider in qualifiers {
        let provider_dir = base_dir.join(provider);
        if !provider_dir.is_dir() {
            continue;
        }
        let provider_file = provider_dir.join(file_name);
        if provider_file.is_file() {
            variants.push(make_variant(repo, provider, "", &provider_file));
        }
        let Ok(entries) = fs::read_dir(&provider_dir) else {
            continue;
        };
        let mut model_dirs: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .collect();
        model_dirs.sort();
        for model in model_dirs {
            let model_file = provider_dir.join(&model).join(file_name);
            if model_file.is_file() {
                variants.push(make_variant(repo, provider, &model, &model_file));
            }
        }
    }
    variants
}

pub(super) fn make_variant(repo: &Path, provider: &str, model: &str, file: &Path) -> Variant {
    let relative_path = file
        .strip_prefix(repo)
        .unwrap_or(file)
        .to_string_lossy()
        .to_string();
    let content = fs::read_to_string(file).unwrap_or_default();
    let mode = match extract_frontmatter_field(&content, "mode") {
        mode if mode.is_empty() => "replace".to_string(),
        mode => mode,
    };
    let qualifier = if model.is_empty() {
        provider.to_string()
    } else {
        format!("{provider}/{model}")
    };
    Variant {
        qualifier,
        provider: provider.to_string(),
        model: model.to_string(),
        relative_path,
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    use std::fs;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn minimal_deck(root: &Path) {
        write(
            &root.join("deck.yaml"),
            "schema: 1\nname: test-deck\nversion: 1.0.0\ndescription: Test deck.\n",
        );
        write(
            &root.join("runes/science/module.yaml"),
            "name: science\nversion: 0.1.0\ndescription: Science.\nevents: []\n",
        );
        write(
            &root.join("runes/science/rules/Alpha.md"),
            "---\ndescription: Alpha rule.\n---\nAlpha.\n",
        );
        write(
            &root.join("casts/all.yaml"),
            "name: all\ndescription: Everything.\nrunes: [science/**]\n",
        );
    }

    #[test]
    fn build_view_reads_deployed_skill_from_provider_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let provider_dir = temp.path().join(".claude");
        let skill_path = provider_dir.join("skills/Demo/SKILL.md");
        let sidecar_path = provider_dir.join("skills/Demo/.provenance/SKILL.yaml");
        let skill_content = "---\ndescription: Demo skill\n---\nUse the demo skill.\n";
        let fingerprint = manifest::content_sha256(skill_content);

        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, skill_content).unwrap();
        fs::create_dir_all(sidecar_path.parent().unwrap()).unwrap();
        fs::write(
            &sidecar_path,
            format!(
                "source: https://github.com/example/rune-demo.git\n\
                 subject:\n\
                 - name: skills/Demo/SKILL.md\n\
                   digest:\n\
                     sha256: {fingerprint}\n\
                 resolvedDependencies:\n\
                 - uri: skills/Demo/SKILL.md\n\
                   digest:\n\
                     sha256: {fingerprint}\n"
            ),
        )
        .unwrap();
        fs::write(
            provider_dir.join(".manifest"),
            format!(
                "skills:\n  Demo:\n    SKILL.md:\n      fingerprint: {fingerprint}\n      provenance: skills/Demo/.provenance/SKILL.yaml\n"
            ),
        )
        .unwrap();

        let providers = vec![("claude".to_string(), ".claude".to_string())];
        let view = build_view(temp.path(), &providers, &[]).unwrap();
        let module = view
            .modules
            .iter()
            .find(|module| module.name == "rune-demo")
            .unwrap();
        let artifact = module
            .artifacts
            .iter()
            .find(|artifact| artifact.name == "Demo" && artifact.kind == "skills")
            .unwrap();

        assert_eq!(artifact.description, "Demo skill");
        assert_eq!(artifact.content_body, "Use the demo skill.\n");
        assert_eq!(
            artifact.providers.get("claude").unwrap().status,
            manifest::FileStatus::Unchanged
        );
    }

    #[test]
    fn build_view_discovers_deck_entries_kinds_and_casts() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck");

        let view = build_view(&fixture, &[], &[]).unwrap();
        let deck = view.deck.as_ref().unwrap();

        assert_eq!(
            deck.entries
                .iter()
                .map(|deck_entry| deck_entry.name.as_str())
                .collect::<Vec<_>>(),
            ["science", "writing"]
        );
        assert_eq!(
            view.modules
                .iter()
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>(),
            ["science", "writing"]
        );
        let science = &view.modules[0];
        assert_eq!(science.version, "0.1.0");
        assert!(science.artifacts.iter().any(|artifact| {
            artifact.kind == "hooks"
                && artifact.name == "hooks"
                && artifact.source_path == "runes/science/hooks/hooks.json"
        }));
        let science_cast = deck
            .casts
            .iter()
            .find(|cast| cast.name == "science")
            .unwrap();
        assert_eq!(science_cast.resolved_runes.len(), 3);
        assert!(
            deck.casts
                .iter()
                .find(|cast| cast.name == "stale")
                .unwrap()
                .resolution_error
                .is_some()
        );
    }

    #[test]
    fn deck_target_statuses_only_include_watched_consumers_of_deck() {
        let temp = tempfile::tempdir().unwrap();
        let deck_root = temp.path().join("deck");
        let target_root = temp.path().join("target");
        minimal_deck(&deck_root);
        write(
            &target_root.join(".rune"),
            &format!(
                "version: 1\nsources:\n  deck:\n    local: {}\nartifacts: {{}}\n",
                deck_root.display()
            ),
        );
        let deployed = "Alpha.\n";
        let fingerprint = manifest::content_sha256(deployed);
        write(&target_root.join(".claude/rules/Alpha.md"), deployed);
        write(
            &target_root.join(".claude/rules/.provenance/Alpha.yaml"),
            "source: science\nresolvedDependencies:\n  - uri: rules/Alpha.md\n",
        );
        write(
            &target_root.join(".claude/.manifest"),
            &format!(
                "rules:\n  Alpha.md:\n    fingerprint: {fingerprint}\n    provenance: rules/.provenance/Alpha.yaml\n"
            ),
        );

        let providers = [("claude".to_string(), ".claude".to_string())];
        let view = build_view(&deck_root, &providers, std::slice::from_ref(&target_root)).unwrap();
        let target = &view.deck.unwrap().targets[0];

        assert_eq!(target.name, "target");
        assert_eq!(
            target.artifacts["science/rules/Alpha"].status,
            FileStatus::Unchanged
        );
    }

    #[test]
    fn cast_toggle_previews_then_persists_atomically() {
        let temp = tempfile::tempdir().unwrap();
        minimal_deck(temp.path());
        let cast_path = temp.path().join("casts/all.yaml");
        let original = fs::read_to_string(&cast_path).unwrap();

        let edit = prepare_cast_toggle(temp.path(), "all", "science/rules/Alpha", false).unwrap();

        assert_eq!(fs::read_to_string(&cast_path).unwrap(), original);
        assert!(edit.before.contains(&"science/rules/Alpha".to_string()));
        assert!(!edit.after.contains(&"science/rules/Alpha".to_string()));
        persist_cast_edit(&edit).unwrap();
        let deck = crate::deck::load(temp.path()).unwrap();
        assert!(
            deck.resolve_cast("all", ["science/rules/Alpha"])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn cast_edit_refuses_to_overwrite_a_newer_change() {
        let temp = tempfile::tempdir().unwrap();
        minimal_deck(temp.path());
        let edit = prepare_cast_toggle(temp.path(), "all", "science/rules/Alpha", false).unwrap();
        write(
            &temp.path().join("casts/all.yaml"),
            "name: all\ndescription: Concurrent.\nrunes: [science/**]\n",
        );

        let error = persist_cast_edit(&edit).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("changed after the edit was prepared")
        );
    }

    #[test]
    fn cast_toggle_does_not_remove_a_broad_exclusion() {
        let temp = tempfile::tempdir().unwrap();
        minimal_deck(temp.path());
        write(
            &temp.path().join("casts/all.yaml"),
            "name: all\ndescription: Broad exclusion.\nrunes: [science/**]\nexclude: [science/**]\n",
        );

        let error = prepare_cast_toggle(temp.path(), "all", "science/rules/Alpha", true)
            .expect_err("a single toggle must not widen a broad exclusion");

        assert!(error.to_string().contains("wildcard or inherited"));
        assert!(
            fs::read_to_string(temp.path().join("casts/all.yaml"))
                .unwrap()
                .contains("exclude: [science/**]")
        );
    }
}
