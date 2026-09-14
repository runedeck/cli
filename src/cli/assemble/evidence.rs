//! Bind a complete provider build to the source selection that produced it.

use rune::error::{Error, ErrorKind};
use rune::manifest::source_snapshot::{
    self, SOURCE_SNAPSHOT_PATH, SourceSnapshot, SourceSnapshotRecord,
};
use rune::provider::ProviderConfig;
use rune::result::ActionResult;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

type CollectedSources = (
    SourceSnapshot,
    Vec<super::sources::SourceFile>,
    crate::cli::dotrune::toggle::ToggleMap,
);

pub(super) fn collect_sources(
    root: &Path,
    qualifiers: &HashSet<String>,
    model_override: Option<&str>,
) -> Result<CollectedSources, Error> {
    let consumer = crate::cli::dotrune::load(root)?;
    if let Some(manifest) = &consumer {
        // Resolve authorized Git sources before the read-only snapshot.
        crate::cli::dotrune::resolve_sources(manifest, root, qualifiers)?;
    }
    let snapshot = capture(root, model_override)?;
    if serde_json::to_value(crate::cli::dotrune::load(root)?).ok()
        != serde_json::to_value(&consumer).ok()
    {
        return Err(source_changed());
    }
    let mut toggles = crate::cli::dotrune::toggle::ToggleMap::default();
    let sources = if let Some(manifest) = &consumer {
        toggles = crate::cli::dotrune::toggle::toggle_map(manifest);
        crate::cli::dotrune::resolve_sources(manifest, root, qualifiers)?
    } else {
        super::sources::collect(root, qualifiers)?
    };
    Ok((snapshot, sources, toggles))
}

pub(super) fn capture(root: &Path, model_override: Option<&str>) -> Result<SourceSnapshot, Error> {
    source_snapshot::inspect(&super::snapshot::source_inputs_with_model(
        root,
        model_override,
    )?)
    .map_err(|message| {
        Error::new(ErrorKind::Validate, message).with_code("CSI002_INCOMPLETE_IDENTITY")
    })
}

pub(super) fn source_changed() -> Error {
    Error::new(
        ErrorKind::Validate,
        "source files or selection changed during assembly; repeat the assembly",
    )
    .with_code("CSI002_INCOMPLETE_IDENTITY")
}

pub(super) fn verify_configuration(
    root: &Path,
    merged: &str,
    remap: Option<&String>,
    models: &HashMap<String, Vec<String>>,
    source_uri: &str,
) -> Result<(), Error> {
    use crate::cli::config;
    if config::load_merged_config(root)? != merged
        || config::load_remap_tools(root)?.as_ref() != remap
        || config::load_models(root) != *models
        || config::load_source_uri(root) != source_uri
    {
        return Err(source_changed());
    }
    Ok(())
}

pub(super) fn record(
    staging: &Path,
    snapshot: &SourceSnapshot,
    providers: &HashMap<String, ProviderConfig>,
    requested: &[String],
    model_override: Option<&str>,
    result: &mut ActionResult,
) -> Result<(), Error> {
    for name in providers.keys() {
        if !requested.is_empty() && !requested.iter().any(|value| value == name) {
            continue;
        }
        let root = staging.join(name);
        // An empty Codex selection still needs evidence and a complete prune pass.
        if !root.exists() && name != "codex" {
            continue;
        }
        let selected = match source_snapshot::selected_skill_bundles(&root) {
            Ok(selected) => selected,
            Err(error) => {
                result.warnings.push(format!(
                    "provider {name} has no complete source snapshot: {error}"
                ));
                continue;
            }
        };
        let mut record = SourceSnapshotRecord::new(snapshot.clone(), selected);
        record.model_override = model_override.map(str::to_owned);
        let output = root.join(SOURCE_SNAPSHOT_PATH);
        fs::create_dir_all(output.parent().unwrap_or(&root))
            .map_err(|error| Error::new(ErrorKind::Io, error.to_string()))?;
        let content = serde_json::to_string_pretty(&record)
            .map_err(|error| Error::new(ErrorKind::Io, error.to_string()))?;
        crate::cli::config::write_atomic(&output, &content)?;
    }
    Ok(())
}
