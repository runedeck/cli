//! Deployed-target scanning: manifest walk, artifact construction, companion
//! attachment, deploy status, and per-artifact git history.

use super::discovery::module_name_from_source;
use super::history::{
    GIT_LOG_FORMAT, enrich_commits_with_entire, git_log_for_artifact, parse_git_log,
    read_source_adoption, read_source_sidecar, recorded_input_sha,
};
use super::source::{
    load_manifest, parse_artifact_key, read_artifact_content, read_source_content, resolve_source,
    resolve_source_name, resolve_source_path,
};
use crate::manifest::{self, FileStatus, ManifestEntry};
use crate::view::{ArtifactView, Companion, GitCommit, ModuleView, ProviderStatus, StatusSummary};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns a warning when the resolved sidecar uses a non-canonical filename.
/// Canonical is `{file_stem}.yaml` (e.g. `SKILL.yaml` for a skill's `SKILL.md`).
/// Empty when the sidecar is canonical or absent.
pub(super) fn sidecar_name_warning(relative_path: &str, sidecar_path: &Path) -> String {
    if !sidecar_path.is_file() {
        return String::new();
    }
    let Some(actual) = sidecar_path.file_name().and_then(|name| name.to_str()) else {
        return String::new();
    };
    let stem = Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let canonical = format!("{stem}.yaml");
    if actual == canonical {
        String::new()
    } else {
        format!("non-canonical sidecar name '{actual}' (canonical is '{canonical}')")
    }
}

pub fn git_log_in_repo(repo: &Path, file_rel: &str) -> Vec<GitCommit> {
    let output = Command::new("git")
        .args(["log", "--follow", "-n", "5", GIT_LOG_FORMAT, "--", file_rel])
        .current_dir(repo)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut commits = parse_git_log(&String::from_utf8_lossy(&output.stdout));
    enrich_commits_with_entire(repo, &mut commits);
    commits
}

pub(super) struct PendingCompanion {
    pub(super) source_uri: String,
    pub(super) parent: String,
    pub(super) companion: Companion,
}

pub(super) fn scan_target(
    target_base: &Path,
    modules: &mut BTreeMap<String, ModuleView>,
    summary: &mut StatusSummary,
    local_repos: &HashMap<String, PathBuf>,
    pending_companions: &mut Vec<PendingCompanion>,
    providers: &[(String, String)],
) {
    for (provider_name, provider_dir) in providers {
        let provider_path = target_base.join(provider_dir);
        if !provider_path.is_dir() {
            continue;
        }
        let entries = load_manifest(&provider_path);
        for (relative_key, entry) in &entries {
            let Some((kind, deployed_name)) = parse_artifact_key(relative_key) else {
                continue;
            };
            let source = resolve_source(&provider_path, relative_key, entry);

            if let Some(pending) = companion_entry(&provider_path, relative_key, &source) {
                pending_companions.push(pending);
                continue;
            }

            let canonical_name =
                resolve_source_name(&provider_path, entry).unwrap_or(deployed_name);
            let source_path = resolve_source_path(&provider_path, entry);
            let status = deployed_status(
                &provider_path,
                relative_key,
                entry,
                &source,
                source_path.as_deref(),
                local_repos,
            );
            tally_status(summary, status);

            let module_view = modules.entry(source.clone()).or_insert_with(|| ModuleView {
                name: module_name_from_source(&source),
                version: String::new(),
                description: String::new(),
                source_uri: source,
                is_target: false,
                artifacts: Vec::new(),
                local_path: None,
                vcs: None,
                git_log: Vec::new(),
            });

            let provider_status = ProviderStatus {
                status,
                fingerprint: Some(entry.fingerprint.clone()),
            };
            let existing = module_view
                .artifacts
                .iter_mut()
                .find(|artifact| artifact.name == canonical_name && artifact.kind == kind);
            if let Some(artifact) = existing {
                artifact
                    .providers
                    .insert(provider_name.clone(), provider_status);
            } else {
                let artifact = build_deployed_artifact(
                    &provider_path,
                    relative_key,
                    kind,
                    canonical_name,
                    &module_view.source_uri,
                    source_path.as_deref(),
                    (provider_name.as_str(), provider_status),
                    local_repos,
                );
                module_view.artifacts.push(artifact);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_deployed_artifact(
    provider_path: &Path,
    relative_key: &str,
    kind: &str,
    canonical_name: String,
    source_uri: &str,
    source_path: Option<&str>,
    provider: (&str, ProviderStatus),
    local_repos: &HashMap<String, PathBuf>,
) -> ArtifactView {
    let mut providers = BTreeMap::new();
    providers.insert(provider.0.to_string(), provider.1);
    let source_content = read_source_content(source_uri, source_path, local_repos);
    let deployed_content = read_artifact_content(provider_path, relative_key);
    let raw_source = if source_content.raw.is_empty() {
        fs::read_to_string(provider_path.join(relative_key)).unwrap_or_default()
    } else {
        source_content.raw.clone()
    };
    let description = if source_content.description.is_empty() {
        deployed_content.description
    } else {
        source_content.description
    };
    // Prefer the source body: it keeps reference-link definitions (assembly
    // strips them), so the markdown preview resolves reflinks.
    let content_body = if source_content.body.is_empty() {
        deployed_content.body
    } else {
        source_content.body
    };
    let content_preview = if description.is_empty() {
        content_body.lines().take(10).collect::<Vec<_>>().join("\n")
    } else {
        String::new()
    };
    let provenance_raw = read_source_sidecar(source_uri, source_path, local_repos)
        .or_else(|| read_deployed_sidecar(provider_path, relative_key))
        .unwrap_or_default();
    ArtifactView {
        name: canonical_name,
        kind: kind.to_string(),
        module: String::new(),
        relative_path: relative_key.to_string(),
        source_path: source_path.unwrap_or_default().to_string(),
        description,
        content_preview,
        content_body,
        raw_source,
        provenance_raw,
        metadata: source_content.metadata,
        providers,
        git_log: git_log_for_artifact(source_uri, source_path, local_repos),
        adoption: read_source_adoption(source_uri, source_path, local_repos),
        sidecar_warning: String::new(),
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
        vcs: None,
    }
}

/// Reads the canonical sidecar beside a deployed artifact when its source
/// checkout is unavailable. Resolution stays shared with source-side reads so
/// canonical and tolerated legacy names behave identically.
fn read_deployed_sidecar(provider_path: &Path, relative_key: &str) -> Option<String> {
    let relative = Path::new(relative_key);
    let parent = provider_path.join(relative.parent()?);
    let sidecar = super::source::resolve_sidecar(&parent, relative)?;
    fs::read_to_string(sidecar).ok()
}

/// Builds a `PendingCompanion` from a deployed companion manifest entry,
/// or `None` if the entry is not a skill companion file.
pub(super) fn companion_entry(
    provider_path: &Path,
    relative_key: &str,
    source_uri: &str,
) -> Option<PendingCompanion> {
    let (parent, companion_name) = companion_of(relative_key)?;
    let content = read_artifact_content(provider_path, relative_key);
    let raw_source = fs::read_to_string(provider_path.join(relative_key)).unwrap_or_default();
    Some(PendingCompanion {
        source_uri: source_uri.to_string(),
        parent,
        companion: Companion {
            name: companion_name,
            relative_path: relative_key.to_string(),
            description: content.description,
            content_body: content.body,
            raw_source,
        },
    })
}

/// Detects a skill companion file: `skills/<Parent>/<Name>.md` where
/// `<Name>` is not `SKILL`. Returns `(parent, companion_name)`.
pub(super) fn companion_of(relative_key: &str) -> Option<(String, String)> {
    let segments: Vec<&str> = relative_key.split('/').collect();
    if segments.len() != 3 || segments[0] != "skills" {
        return None;
    }
    let stem = segments[2]
        .trim_end_matches(".md")
        .trim_end_matches(".toml");
    if stem == "SKILL" {
        return None;
    }
    Some((segments[1].to_string(), stem.to_string()))
}

/// Attaches collected companion files to their parent skill artifacts,
/// deduplicating across providers by companion name.
pub(super) fn attach_companions(
    modules: &mut BTreeMap<String, ModuleView>,
    pending: Vec<PendingCompanion>,
) {
    for item in pending {
        let Some(module) = modules.get_mut(&item.source_uri) else {
            continue;
        };
        let Some(parent) = module
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == "skills" && artifact.name == item.parent)
        else {
            continue;
        };
        if parent
            .companions
            .iter()
            .any(|existing| existing.name == item.companion.name)
        {
            continue;
        }
        parent.companions.push(item.companion);
    }
    for module in modules.values_mut() {
        for artifact in &mut module.artifacts {
            artifact.companions.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }
}

/// Deploy status with precedence: a deployed file edited since deploy is
/// `Modified`; an unchanged file whose source drifted is `Stale`.
pub(super) fn deployed_status(
    provider_path: &Path,
    relative_key: &str,
    entry: &ManifestEntry,
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> FileStatus {
    let status = compute_deployed_status(provider_path, relative_key, entry);
    if status == FileStatus::Unchanged
        && is_stale(provider_path, entry, source_uri, source_path, local_repos)
    {
        return FileStatus::Stale;
    }
    status
}

pub(super) fn compute_deployed_status(
    target_dir: &Path,
    relative_key: &str,
    entry: &ManifestEntry,
) -> FileStatus {
    let target_path = target_dir.join(relative_key);
    let Ok(content) = fs::read_to_string(&target_path) else {
        return FileStatus::New;
    };
    let current_sha = manifest::content_sha256(&content);
    if current_sha == entry.fingerprint {
        FileStatus::Unchanged
    } else {
        FileStatus::Modified
    }
}

pub(super) fn tally_status(summary: &mut StatusSummary, status: FileStatus) {
    match status {
        FileStatus::Unchanged => summary.unchanged += 1,
        FileStatus::Stale => summary.stale += 1,
        FileStatus::Modified => summary.modified += 1,
        FileStatus::New => summary.new += 1,
    }
}

/// A deployed artifact is stale when its source changed since deploy: the
/// current source file SHA differs from the input SHA recorded in the
/// deployed `assemble/v1` sidecar's `resolvedDependencies`.
pub(super) fn is_stale(
    provider_path: &Path,
    entry: &ManifestEntry,
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> bool {
    let Some(provenance_rel) = entry.provenance.as_ref() else {
        return false;
    };
    let Ok(sidecar) = fs::read_to_string(provider_path.join(provenance_rel)) else {
        return false;
    };
    let Some(recorded_sha) = recorded_input_sha(&sidecar) else {
        return false;
    };
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo) = local_repos.get(normalized) else {
        return false;
    };
    let Some(rel) = source_path else {
        return false;
    };
    let Ok(current) = fs::read_to_string(repo.join(rel)) else {
        return false;
    };
    manifest::content_sha256(&current) != recorded_sha
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_SLSA_SIDECAR: &str = "provenance:\n  _type: https://in-toto.io/Statement/v1\n  predicateType: https://slsa.dev/provenance/v1\n  subject:\n    - name: rules/Demo.md\n      digest:\n        sha256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n  predicate:\n    buildDefinition:\n      buildType: https://github.com/runedeck/rune/assemble/v1\n      externalParameters:\n        invocation:\n          configSource: deck.yaml\n    runDetails:\n      builder:\n        id: https://github.com/runedeck/rune\n      metadata:\n        startedOn: 2026-07-14T10:00:00Z\n        finishedOn: 2026-07-14T10:00:01Z\n";

    #[test]
    fn deployed_artifact_uses_deployed_bytes_when_source_repo_is_unavailable() {
        let target = tempfile::tempdir().unwrap();
        let rules = target.path().join("rules");
        fs::create_dir(&rules).unwrap();
        fs::create_dir(rules.join(".provenance")).unwrap();
        let raw = "---\ndescription: Raw deployed rule\n---\n\nRule body.\n";
        fs::write(rules.join("Demo.md"), raw).unwrap();
        fs::write(rules.join(".provenance/Demo.yaml"), FULL_SLSA_SIDECAR).unwrap();

        let artifact = build_deployed_artifact(
            target.path(),
            "rules/Demo.md",
            "rules",
            "Demo".to_string(),
            "https://example.invalid/missing",
            Some("rules/Demo.md"),
            (
                "claude",
                ProviderStatus {
                    status: FileStatus::Unchanged,
                    fingerprint: None,
                },
            ),
            &HashMap::new(),
        );

        assert_eq!(artifact.raw_source, raw);
        assert_eq!(artifact.provenance_raw, FULL_SLSA_SIDECAR);
        assert!(artifact.provenance_raw.contains("predicateType:"));
        assert!(artifact.provenance_raw.contains("builder:"));
        assert!(artifact.provenance_raw.contains("sha256:"));
    }
}
