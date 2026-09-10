//! Store source evidence only after the complete selected skill set is installed.

use super::{canonical_metadata_path, deployed_bytes, load_deployed_manifest, resolve_target_base};
use rune::error::{Error, ErrorKind};
use rune::manifest::{
    self,
    source_snapshot::{self, SOURCE_SNAPSHOT_PATH, SourceSnapshotRecord},
};
use rune::provider::{ContentKind, ProviderConfig};
use rune::result::ActionResult;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn incomplete(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Validate, message).with_code("CSI002_INCOMPLETE_IDENTITY")
}

pub(crate) fn read_snapshot(root: &Path) -> Result<Option<SourceSnapshotRecord>, Error> {
    let file = root.join(SOURCE_SNAPSHOT_PATH);
    match file.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(incomplete(error.to_string())),
        Ok(metadata) if !metadata.is_file() => {
            return Err(incomplete("source snapshot must be a regular file"));
        }
        Ok(_) => (),
    }
    canonical_metadata_path(&file, root)?;
    let record: SourceSnapshotRecord =
        serde_json::from_slice(&fs::read(&file).map_err(|error| incomplete(error.to_string()))?)
            .map_err(|error| incomplete(error.to_string()))?;
    record.validate().map_err(incomplete)?;
    Ok(Some(record))
}

/// Doctor and deploy use the same complete bundle and manifest checks.
pub(crate) fn verify_installed_snapshot(
    root: &Path,
    provider: &str,
    record: &SourceSnapshotRecord,
) -> Result<(), Error> {
    record.validate().map_err(incomplete)?;
    let entries = load_deployed_manifest(root)?;
    for (relative, expected) in &record.selected_skills {
        let bundle = root.join(relative);
        canonical_metadata_path(&bundle, root)?;
        let inspection = manifest::bundle::inspect_bundle(&bundle);
        if !inspection.problems.is_empty() || inspection.digest.as_ref() != Some(expected) {
            return Err(incomplete(format!(
                "selected skill {relative} differs from the complete source snapshot"
            )));
        }
        for entry in inspection.entries {
            if entry.kind == manifest::bundle::BundleEntryKind::Directory {
                continue;
            }
            let key = format!("{relative}/{}", entry.path);
            let claim = entries.get(&key).ok_or_else(|| {
                incomplete(format!("selected skill file {key} has no manifest claim"))
            })?;
            let fingerprint = manifest::content_sha256_bytes(
                &deployed_bytes(&root.join(&key)).map_err(|error| incomplete(error.to_string()))?,
            );
            if claim.fingerprint != fingerprint
                || claim.provenance.as_deref() != Some(&manifest::provenance_path(&key))
            {
                return Err(incomplete(format!(
                    "selected skill file {key} has incomplete manifest evidence"
                )));
            }
            let provenance = root.join(manifest::provenance_path(&key));
            canonical_metadata_path(&provenance, root)?;
            if !provenance
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_file())
            {
                return Err(incomplete(format!(
                    "selected skill file {key} has no regular provenance file"
                )));
            }
            let statement = manifest::provenance::read(&provenance)
                .map_err(incomplete)?
                .provenance;
            if statement.statement_type != manifest::provenance::STATEMENT_TYPE
                || statement.predicate_type != manifest::provenance::PREDICATE_TYPE
                || statement.subject.len() != 1
                || statement.subject[0].name != format!("{provider}/{key}")
                || statement.subject[0].digest.sha256 != fingerprint
                || statement
                    .predicate
                    .build_definition
                    .resolved_source()
                    .is_empty()
            {
                return Err(incomplete(format!(
                    "selected skill file {key} has invalid provenance evidence"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn record_completed_install(
    module: &Path,
    target: Option<&str>,
    providers: &HashMap<String, ProviderConfig>,
    complete: bool,
    result: &mut ActionResult,
) -> Result<(), Error> {
    for (name, provider) in providers {
        let build = module.join("build").join(name);
        if !build.is_dir() {
            continue;
        }
        let root = resolve_target_base(provider.target_for_kind(ContentKind::Skills), target);
        let record = read_snapshot(&build)?;
        if !complete || record.is_none() {
            invalidate(&root)?;
            if record.is_some() {
                result.warnings.push(format!("provider {name} source snapshot remains unverified after a partial or no-prune install"));
            }
            continue;
        }
        let record = record.expect("record presence checked above");
        let inputs = crate::cli::assemble::snapshot::source_inputs_with_model(
            module,
            record.model_override.as_deref(),
        )?;
        let current = source_snapshot::inspect(&inputs).map_err(incomplete)?;
        if current != record.source
            || source_snapshot::selected_skill_bundles(&build).map_err(incomplete)?
                != record.selected_skills
        {
            invalidate(&root)?;
            return Err(incomplete(
                "source selection or build changed before deployment completed",
            ));
        }
        let verified = verify_installed_snapshot(&root, name, &record).and_then(|()| {
            if let Some(previous) = read_snapshot(&root)? {
                for retired in previous
                    .selected_skills
                    .keys()
                    .filter(|key| !record.selected_skills.contains_key(*key))
                {
                    if root.join(retired).symlink_metadata().is_ok() {
                        return Err(incomplete(format!(
                            "previously selected skill {retired} remains after deployment"
                        )));
                    }
                }
            }
            Ok(())
        });
        if let Err(error) = verified {
            invalidate(&root)?;
            result.warnings.push(format!(
                "provider {name} source snapshot remains unverified: {error}"
            ));
            continue;
        }
        let file = root.join(SOURCE_SNAPSHOT_PATH);
        canonical_metadata_path(&file, &root)?;
        fs::create_dir_all(file.parent().unwrap_or(&root))
            .map_err(|error| incomplete(error.to_string()))?;
        let content =
            serde_json::to_string_pretty(&record).map_err(|error| incomplete(error.to_string()))?;
        if !fs::read_to_string(&file).is_ok_and(|current| current == content) {
            crate::cli::config::write_atomic(&file, &content)?;
        }
    }
    Ok(())
}

fn invalidate(root: &Path) -> Result<(), Error> {
    let file = root.join(SOURCE_SNAPSHOT_PATH);
    if file.symlink_metadata().is_err() {
        return Ok(());
    }
    canonical_metadata_path(&file, root)?;
    fs::remove_file(&file).map_err(|error| incomplete(error.to_string()))
}
