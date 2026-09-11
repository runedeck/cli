//! Preflight the staged Codex move without changing ordinary install conflicts.

use super::{
    collect_files_recursive, deployed_bytes, load_deployed_manifest, load_manifest_or_recover,
    parse_repo, resolve_target_base, same_entry_semantics, write_manifest,
};
use rune::error::{Error, ErrorKind};
use rune::manifest::{self, ManifestEntry};
use rune::provider::{ContentKind, ProviderConfig};
use rune::result::{ActionResult, PrunedFile};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

fn conflict(path: &Path, message: &str) -> Error {
    Error::new(
        ErrorKind::Validate,
        format!("Codex skill migration: {}: {message}", path.display()),
    )
    .with_code("CSI005_MIGRATION_CONFLICT")
}

/// Older build trees use ambiguous adjacent metadata and require reassembly.
pub(super) fn check_build_layout(
    module: &Path,
    providers: &HashMap<String, ProviderConfig>,
) -> Result<(), Error> {
    for name in providers.keys() {
        let build = module.join("build").join(name);
        if !build.is_dir() {
            continue;
        }
        for file in collect_files_recursive(&build)? {
            let relative = file.strip_prefix(&build).unwrap_or(&file);
            if manifest::bundle::is_build_sidecar(relative)
                || relative.starts_with("hooks")
                || manifest::sidecar_path(&file).is_file()
                || file.extension().is_none_or(|extension| extension != "yaml")
            {
                continue;
            }
            if !file
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_file())
            {
                continue;
            }
            let Ok(sidecar) = manifest::provenance::read(&file) else {
                continue;
            };
            let statement = sidecar.provenance;
            if statement.statement_type == manifest::provenance::STATEMENT_TYPE
                && statement
                    .predicate
                    .build_definition
                    .build_type
                    .ends_with("/assemble/v1")
                && statement.subject.len() == 1
                && Path::new(&statement.subject[0].name).file_name() == file.file_stem()
                && file
                    .with_file_name(file.file_stem().unwrap_or_default())
                    .symlink_metadata()
                    .is_ok()
            {
                return Err(Error::new(ErrorKind::Validate,
                    format!("legacy build provenance at {}; run rune assemble or rune install before deployment", file.display()))
                    .with_code("deploy.legacy_build_layout"));
            }
        }
    }
    Ok(())
}

pub(super) fn canonical_target(path: &Path) -> Result<PathBuf, Error> {
    let absolute =
        std::path::absolute(path).map_err(|error| Error::new(ErrorKind::Io, error.to_string()))?;
    let ancestor = absolute
        .ancestors()
        .find(|entry| entry.exists())
        .ok_or_else(|| conflict(path, "cannot resolve the destination"))?;
    let mut resolved = ancestor
        .canonicalize()
        .map_err(|error| Error::new(ErrorKind::Io, error.to_string()))?;
    for component in absolute
        .strip_prefix(ancestor)
        .unwrap_or(Path::new(""))
        .components()
    {
        match component {
            Component::Normal(value) => resolved.push(value),
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => (),
            _ => return Err(conflict(path, "invalid destination path")),
        }
    }
    Ok(resolved)
}

/// Refuse competing manifests before any provider writes its first file.
pub(super) fn check_writers(
    module: &Path,
    target: Option<&str>,
    providers: &HashMap<String, ProviderConfig>,
    known_providers: &HashSet<String>,
    only: Option<&str>,
    force: bool,
) -> Result<(), Error> {
    let mut roots: Vec<(PathBuf, &str)> = Vec::new();
    let mut names: Vec<_> = providers.keys().collect();
    names.sort();
    for name in names {
        if !module.join("build").join(name).is_dir() {
            continue;
        }
        for root in providers[name].target_roots() {
            let root = resolve_target_base(root, target);
            let physical = canonical_target(&root)?;
            if let Some((other, owner)) = roots.iter().find(|(other, owner)| {
                *owner != name && (physical.starts_with(other) || other.starts_with(&physical))
            }) {
                return Err(Error::new(ErrorKind::Config, format!(
                    "providers {owner} and {name} write overlapping roots {} and {}; select one writer or distinct targets",
                    other.display(), physical.display()
                )).with_code("deploy.competing_writers"));
            }
            for entry in load_manifest_or_recover(&root, only, force)?.values() {
                let Some(relative) = &entry.provenance else {
                    continue;
                };
                if !confined(Path::new(relative)) {
                    return Err(conflict(&root, "unconfined provenance claim"));
                }
                if !canonical_target(&root.join(relative))?.starts_with(&physical) {
                    return Err(conflict(
                        &root,
                        "provenance claim escapes the deployment root",
                    ));
                }
                let Ok(sidecar) = manifest::provenance::read(&root.join(relative)) else {
                    continue;
                };
                for subject in sidecar.provenance.subject {
                    let Some(owner) = Path::new(&subject.name)
                        .components()
                        .next()
                        .and_then(|component| component.as_os_str().to_str())
                    else {
                        continue;
                    };
                    if known_providers.contains(owner) && owner != name {
                        return Err(Error::new(ErrorKind::Config, format!(
                            "provider {name} cannot write {} while its manifest records provider {owner}", root.display()
                        )).with_code("deploy.competing_writers"));
                    }
                }
            }
            roots.push((physical, name));
        }
    }
    Ok(())
}

fn confined(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

struct LegacySkill {
    relative: PathBuf,
    digest: String,
    keys: Vec<String>,
    expected_digest: String,
    source: String,
    build: PathBuf,
}

pub(super) struct MigrationPlan {
    old_root: PathBuf,
    new_root: PathBuf,
    skills: Vec<LegacySkill>,
}

impl MigrationPlan {
    pub(super) fn begin_recovery(&self) -> Result<Option<super::recovery::Recovery>, Error> {
        if self.skills.is_empty() {
            return Ok(None);
        }
        let entries = load_deployed_manifest(&self.new_root)?;
        for skill in &self.skills {
            if self
                .new_root
                .join(&skill.relative)
                .symlink_metadata()
                .is_ok()
            {
                inspect_legacy(
                    &self.new_root,
                    &skill.relative,
                    &entries,
                    &skill.source,
                    &skill.build,
                )?;
            }
        }
        super::recovery::Recovery::begin(
            &self.new_root,
            self.skills
                .iter()
                .map(|skill| skill.relative.clone())
                .collect(),
        )
        .map(Some)
    }

    pub(super) fn prepare(
        module: &Path,
        target: Option<&str>,
        codex: Option<&ProviderConfig>,
        only: Option<&str>,
    ) -> Result<Option<Self>, Error> {
        let Some(codex) = codex else {
            return Ok(None);
        };
        let old_root = resolve_target_base(".codex", target);
        let new_root = resolve_target_base(codex.target_for_kind(ContentKind::Skills), target);
        if canonical_target(&old_root)? == canonical_target(&new_root)? {
            return Ok(None);
        }
        let base = canonical_target(Path::new(target.unwrap_or(".")))?;
        if !canonical_target(&old_root)?.starts_with(&base)
            || !canonical_target(&new_root)?.starts_with(&base)
        {
            return Err(conflict(&old_root, "migration roots escape the target"));
        }
        let build = module.join("build/codex/skills");
        let mut plan = Self {
            old_root,
            new_root,
            skills: Vec::new(),
        };
        if !build.is_dir() {
            return Ok(Some(plan));
        }
        let mut bundles: Vec<_> = fs::read_dir(&build)
            .map_err(|error| Error::new(ErrorKind::Io, error.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| Error::new(ErrorKind::Io, error.to_string()))?;
        bundles.sort_by_key(std::fs::DirEntry::file_name);
        for bundle in bundles {
            let relative = Path::new("skills").join(bundle.file_name());
            if !bundle.path().join("SKILL.md").is_file() {
                continue;
            }
            if let Some(prefix) = only {
                let selected = collect_files_recursive(&bundle.path())?.iter().any(|file| {
                    file.strip_prefix(&build).is_ok_and(|path| {
                        super::only_matches(&format!("skills/{}", path.display()), prefix)
                    })
                });
                if !selected {
                    continue;
                }
            }
            let expected = manifest::bundle::inspect_bundle(&bundle.path());
            if !expected.problems.is_empty() || expected.digest.is_none() {
                return Err(conflict(
                    &bundle.path(),
                    "selected bundle has invalid content or references",
                ));
            }
            let old = plan.old_root.join(&relative);
            if fs::symlink_metadata(&old).is_err() {
                continue;
            }
            if let Some(prefix) = only {
                let complete = expected
                    .entries
                    .iter()
                    .filter(|entry| entry.kind != manifest::bundle::BundleEntryKind::Directory)
                    .all(|entry| {
                        super::only_matches(
                            &format!("{}/{}", relative.display(), entry.path),
                            prefix,
                        )
                    });
                if !complete {
                    return Err(conflict(
                        &old,
                        "a partial selection cannot migrate a complete skill bundle",
                    ));
                }
            }
            if fs::symlink_metadata(&old).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Err(conflict(&old, "legacy skill directory is a symbolic link"));
            }
            let old_manifest = load_deployed_manifest(&plan.old_root)?;
            let new_manifest = load_deployed_manifest(&plan.new_root)?;
            let (source, expected_digest) =
                inspect_selected_build(&bundle.path(), &relative, &expected)?;
            let mut skill = inspect_legacy(
                &plan.old_root,
                &relative,
                &old_manifest,
                &source,
                &bundle.path(),
            )?;
            skill.expected_digest = expected_digest;
            let new = plan.new_root.join(&relative);
            if fs::symlink_metadata(&new).is_ok() {
                // A previously completed replacement is safe to resume only with its own evidence.
                inspect_legacy(
                    &plan.new_root,
                    &relative,
                    &new_manifest,
                    &source,
                    &bundle.path(),
                )?;
            }
            plan.skills.push(skill);
        }
        Ok(Some(plan))
    }

    pub(super) fn protect_legacy(
        &self,
        root: &Path,
        entries: &HashMap<String, ManifestEntry>,
        deployed: &mut HashSet<String>,
    ) {
        if root == self.old_root {
            deployed.extend(
                entries
                    .keys()
                    .filter(|key| key.starts_with("skills/"))
                    .cloned(),
            );
        }
    }

    pub(super) fn finish(self, result: &mut ActionResult) -> Result<(), Error> {
        if self.skills.is_empty() {
            return Ok(());
        }
        let mut entries = load_deployed_manifest(&self.old_root)?;
        let new_entries = load_deployed_manifest(&self.new_root)?;
        // Recheck bytes after deployment and before moving the recoverable old copy.
        for skill in &self.skills {
            let old = self.old_root.join(&skill.relative);
            if manifest::bundle::inspect_bundle(&old).digest.as_deref() != Some(&skill.digest) {
                return Err(conflict(&old, "legacy content changed during deployment"));
            }
            inspect_legacy(
                &self.old_root,
                &skill.relative,
                &entries,
                &skill.source,
                &skill.build,
            )?;
            let replacement = inspect_legacy(
                &self.new_root,
                &skill.relative,
                &new_entries,
                &skill.source,
                &skill.build,
            )?;
            if replacement.digest != skill.expected_digest {
                return Err(conflict(
                    &self.new_root.join(&skill.relative),
                    "replacement differs from the selected complete bundle",
                ));
            }
        }
        let stamp = chrono::Utc::now()
            .format("%Y-%m-%d-%H%M%S%.9fZ")
            .to_string();
        let trash = self.old_root.join(".trash").join(stamp);
        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        for skill in &self.skills {
            let source = self.old_root.join(&skill.relative);
            let destination = trash.join(&skill.relative);
            let operation = fs::create_dir_all(destination.parent().unwrap_or(&trash))
                .and_then(|()| fs::rename(&source, &destination));
            if let Err(error) = operation {
                rollback(&moved);
                return Err(conflict(&source, &format!("quarantine failed: {error}")));
            }
            moved.push((source, destination));
            for key in &skill.keys {
                entries.remove(key);
            }
        }
        if let Err(error) = write_manifest(&self.old_root, &entries) {
            rollback(&moved);
            return Err(error);
        }
        for (source, _) in moved {
            result.pruned.push(PrunedFile {
                target: source.to_string_lossy().into_owned(),
                provider: "codex".into(),
            });
        }
        Ok(())
    }
}

fn rollback(moved: &[(PathBuf, PathBuf)]) {
    for (source, destination) in moved.iter().rev() {
        if let Err(error) = fs::rename(destination, source) {
            eprintln!(
                "rune migration: recover {} to {}: {error}",
                destination.display(),
                source.display()
            );
        }
    }
}

fn build_source(bundle: &Path, key: &str) -> Result<String, Error> {
    let path = bundle.join("SKILL.md");
    let bytes = deployed_bytes(&path).map_err(|error| conflict(&path, &error.to_string()))?;
    let fingerprint = manifest::content_sha256_bytes(&bytes);
    let sidecar = manifest::sidecar_path(&path);
    provenance_source(&sidecar, key, &fingerprint)
}

fn inspect_selected_build(
    bundle: &Path,
    relative: &Path,
    expected: &manifest::bundle::BundleInspection,
) -> Result<(String, String), Error> {
    let source = build_source(bundle, &relative.join("SKILL.md").to_string_lossy())?;
    for entry in &expected.entries {
        if entry.kind == manifest::bundle::BundleEntryKind::Directory {
            continue;
        }
        let file = bundle.join(&entry.path);
        let fingerprint = manifest::content_sha256_bytes(
            &deployed_bytes(&file).map_err(|error| conflict(&file, &error.to_string()))?,
        );
        let selected_source = provenance_source(
            &manifest::sidecar_path(&file),
            &format!("{}/{}", relative.display(), entry.path),
            &fingerprint,
        )?;
        if !same_source(&source, &selected_source) {
            return Err(conflict(
                &file,
                "selected companion source differs from its entrypoint",
            ));
        }
    }
    let digest = expected
        .digest
        .clone()
        .ok_or_else(|| conflict(bundle, "selected bundle cannot be read completely"))?;
    Ok((source, digest))
}

fn provenance_source(path: &Path, key: &str, fingerprint: &str) -> Result<String, Error> {
    if !path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_file())
    {
        return Err(conflict(path, "provenance must be a regular file"));
    }
    let sidecar = manifest::provenance::read(path).map_err(|error| conflict(path, &error))?;
    let statement = sidecar.provenance;
    if statement.statement_type != manifest::provenance::STATEMENT_TYPE
        || statement.predicate_type != manifest::provenance::PREDICATE_TYPE
        || statement.subject.len() != 1
        || statement.subject[0].name != format!("codex/{key}")
        || statement.subject[0].digest.sha256 != fingerprint
    {
        return Err(conflict(
            path,
            "provenance does not identify the recorded Codex file",
        ));
    }
    let source = statement.predicate.build_definition.resolved_source();
    if source.is_empty() {
        return Err(conflict(path, "source identity is missing"));
    }
    Ok(source.to_owned())
}

fn inspect_legacy(
    root: &Path,
    relative: &Path,
    entries: &HashMap<String, ManifestEntry>,
    expected_source: &str,
    build: &Path,
) -> Result<LegacySkill, Error> {
    let bundle = root.join(relative);
    validate_bundle_root(root, &bundle)?;
    let inspection = manifest::bundle::inspect_bundle(&bundle);
    if !inspection.problems.is_empty() {
        return Err(conflict(
            &bundle,
            "bundle contains invalid paths, references, or symbolic links",
        ));
    }
    let Some(digest) = inspection.digest else {
        return Err(conflict(
            &bundle,
            "bundle contains invalid paths, references, or symbolic links",
        ));
    };
    let prefix = format!("{}/", relative.display());
    let mut keys: Vec<_> = entries
        .keys()
        .filter(|key| key.starts_with(&prefix))
        .cloned()
        .collect();
    keys.sort();
    if !keys.contains(&format!("{prefix}SKILL.md")) {
        return Err(conflict(
            &bundle,
            "entrypoint has no manifest ownership evidence",
        ));
    }
    let mut claimed = HashSet::new();
    for key in &keys {
        if !confined(Path::new(key)) {
            return Err(conflict(&bundle, "invalid manifest key"));
        }
        let entry = &entries[key];
        let file = root.join(key);
        let bytes = deployed_bytes(&file).map_err(|error| conflict(&file, &error.to_string()))?;
        if manifest::content_sha256_bytes(&bytes) != entry.fingerprint {
            return Err(conflict(&file, "managed content was modified locally"));
        }
        let Some(provenance) = &entry.provenance else {
            return Err(conflict(&file, "source ownership is unknown"));
        };
        if !confined(Path::new(provenance)) || !Path::new(provenance).starts_with(relative) {
            return Err(conflict(&file, "provenance leaves the skill bundle"));
        }
        let source = provenance_source(&root.join(provenance), key, &entry.fingerprint)?;
        if !same_source(&source, expected_source) {
            return Err(conflict(
                &file,
                "source ownership differs from the selected source",
            ));
        }
        let build_file = build.join(
            Path::new(key)
                .strip_prefix(relative)
                .unwrap_or(Path::new(key)),
        );
        if build_file.exists() && !same_entry_semantics(&file, &build_file) {
            return Err(conflict(
                &file,
                "file type or executable permissions differ from the selected bundle",
            ));
        }
        claimed.insert(file);
        claimed.insert(root.join(provenance));
    }
    for file in collect_files_recursive(&bundle)? {
        if !claimed.contains(&file) {
            return Err(conflict(&file, "untracked content must remain in place"));
        }
    }
    for entry in inspection.entries {
        if entry.kind == manifest::bundle::BundleEntryKind::Directory
            && !claimed
                .iter()
                .any(|file| file.starts_with(bundle.join(&entry.path)))
        {
            return Err(conflict(
                &bundle.join(entry.path),
                "untracked directory must remain in place",
            ));
        }
    }
    Ok(LegacySkill {
        relative: relative.to_owned(),
        digest,
        keys,
        expected_digest: String::new(),
        source: expected_source.to_owned(),
        build: build.to_owned(),
    })
}

fn validate_bundle_root(root: &Path, bundle: &Path) -> Result<(), Error> {
    if bundle
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_symlink())
    {
        return Err(conflict(
            bundle,
            "migration bundle root must not be a symbolic link",
        ));
    }
    if !canonical_target(bundle)?.starts_with(canonical_target(root)?) {
        return Err(conflict(bundle, "bundle leaves its deployment root"));
    }
    Ok(())
}

fn same_source(left: &str, right: &str) -> bool {
    left == right || (parse_repo(left).is_some() && parse_repo(left) == parse_repo(right))
}
