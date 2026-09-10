//! Derive snapshot inputs from current configuration without fetching sources.

use crate::cli::config;
use crate::cli::dotrune::{self, Source};
use rune::error::{Error, ErrorKind};
use rune::manifest::source_snapshot::SourceInput;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const AUTHORED_PATHS: &[&str] = &[
    ".rune",
    "config.yaml",
    "defaults.yaml",
    "module.yaml",
    "deck.yaml",
    "config",
    "casts",
    "runes",
    "skills",
    "agents",
    "rules",
    "hooks",
];

/// Both assembly and doctor use this authoritative, read-only input selection.
/// Every configured source is observed, including sources outside the selected cast.
#[cfg(test)]
pub(crate) fn source_inputs(root: &Path) -> Result<Vec<SourceInput>, Error> {
    source_inputs_with_model(root, None)
}

pub(crate) fn source_inputs_with_model(
    root: &Path,
    model_override: Option<&str>,
) -> Result<Vec<SourceInput>, Error> {
    source_inputs_with_cache(root, model_override, &dotrune::cached_worktree)
}

fn source_inputs_with_cache(
    root: &Path,
    model_override: Option<&str>,
    cached: &impl Fn(&str, &str, &str) -> Option<PathBuf>,
) -> Result<Vec<SourceInput>, Error> {
    let root = canonical_directory(root)?;
    if !root.join("module.yaml").is_file()
        && !root.join("deck.yaml").is_file()
        && !root.join(".rune").is_file()
    {
        return Err(incomplete(
            "source root has no module.yaml, deck.yaml, or .rune",
        ));
    }
    let mut inputs = Vec::new();
    append_inputs(&mut inputs, "root", &root);
    let executable = std::env::current_exe().map_err(|error| incomplete(error.to_string()))?;
    let executable = fs::canonicalize(executable).map_err(|error| incomplete(error.to_string()))?;
    inputs.push(SourceInput::required("builder:executable", executable));
    let merged = config::load_merged_config(&root)?;
    let providers: BTreeMap<_, _> = config::load_providers(&merged)?.into_iter().collect();
    let models: BTreeMap<_, _> = config::load_models(&root).into_iter().collect();
    let effective = (
        &merged,
        providers,
        models,
        config::load_remap_tools(&root)?,
        model_override,
    );
    let value = serde_json::to_value(&effective).map_err(|error| incomplete(error.to_string()))?;
    let bytes =
        serde_json::to_vec(&stable_value(value)).map_err(|error| incomplete(error.to_string()))?;
    inputs.push(SourceInput::value("builder:effective_configuration", bytes));
    if let Some(manifest) = dotrune::load(&root)? {
        for (label, source) in manifest.sources {
            let (materialized, subpath) = match source {
                Source::Local { local, path } => {
                    let local = if local.is_absolute() {
                        local
                    } else {
                        root.join(local)
                    };
                    (canonical_directory(&local)?, path)
                }
                Source::Git { git, commit, path } => {
                    let worktree = cached(&git, &commit, &label).ok_or_else(|| {
                        incomplete(format!("source '{label}' has no available pinned cache"))
                    })?;
                    (canonical_directory(&worktree)?, path)
                }
            };
            let source_root = if let Some(path) = &subpath {
                let selected = canonical_directory(&materialized.join(path))?;
                if !selected.starts_with(&materialized) {
                    return Err(incomplete(format!(
                        "source '{label}' path escapes its configured root"
                    )));
                }
                selected
            } else {
                materialized
            };
            if !source_root.join("module.yaml").is_file()
                && (subpath.is_some() || !source_root.join("deck.yaml").is_file())
            {
                return Err(incomplete(format!(
                    "source '{label}' has no required module or deck marker"
                )));
            }
            append_inputs(&mut inputs, &format!("source:{label}"), &source_root);
        }
    }
    Ok(inputs)
}

fn stable_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let sorted: BTreeMap<_, _> = object.into_iter().collect();
            serde_json::Value::Object(
                sorted
                    .into_iter()
                    .map(|(key, value)| (key, stable_value(value)))
                    .collect(),
            )
        }
        serde_json::Value::Array(array) => {
            serde_json::Value::Array(array.into_iter().map(stable_value).collect())
        }
        scalar => scalar,
    }
}

fn append_inputs(inputs: &mut Vec<SourceInput>, label: &str, root: &Path) {
    for relative in AUTHORED_PATHS {
        inputs.push(SourceInput::optional(
            format!("{label}:{relative}"),
            root.join(relative),
        ));
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf, Error> {
    let path = fs::canonicalize(path).map_err(|error| {
        incomplete(format!(
            "source root {} is unavailable: {error}",
            path.display()
        ))
    })?;
    if !path.is_dir() {
        return Err(incomplete("configured source root is not a directory"));
    }
    Ok(path)
}

fn incomplete(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Config, message).with_code("CSI002_INCOMPLETE_IDENTITY")
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
