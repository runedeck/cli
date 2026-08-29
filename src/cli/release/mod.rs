use rune::error::{Error, ErrorKind};
use rune::module;
use rune::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;
use std::process::Command;

use super::install;
use crate::cli::config;

const MAKEFILE_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/make/dist.mk"
));

/// Resolve an optional deck or a single-module rune source.
pub fn execute_source(
    path: &str,
    deck_name: Option<&str>,
    embed: bool,
) -> Result<ActionResult, Error> {
    let root = Path::new(path);
    if !rune::deck::is_deck(root) {
        if let Some(deck) = deck_name {
            return Err(Error::new(
                ErrorKind::Config,
                format!("deck '{deck}' is only valid when --source is a deck root"),
            ));
        }
        return execute(path, embed);
    }

    let deck = rune::deck::load(root).map_err(|message| Error::new(ErrorKind::Config, message))?;
    let deck_name = deck_name.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "release against a deck root requires a deck argument".to_string(),
        )
    })?;
    let deck_entry = deck
        .entries
        .iter()
        .find(|deck_entry| deck_entry.name == deck_name)
        .ok_or_else(|| {
            let available = deck
                .entries
                .iter()
                .map(|deck_entry| deck_entry.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Error::new(
                ErrorKind::Config,
                format!("unknown deck '{deck_name}'; available: {available}"),
            )
        })?;
    execute(&deck_entry.root.to_string_lossy(), embed)
}

/// Assemble, install to a staging directory, then package each provider's
/// output as a self-contained rune release tarball in `dist/`.
///
/// ```text
/// module/
///   build/staging/.claude/...   ← install output (with .manifest)
///   dist/{name}-claude-v{version}.tar.gz
/// ```
///
/// Each tarball wraps `.{provider}/` (with `.manifest` inside, written by
/// install), a generated `Makefile`, and the rune source `README.md`.
#[allow(clippy::too_many_lines)]
pub fn execute(path: &str, embed: bool) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    // A release is publish output: it refuses outright when any adoption
    // review is open, rather than shipping a silently partial package.
    let pending = crate::cli::assemble::sources::pending_review_paths(module_root);
    if !pending.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "adoption review pending for {}; finalize or abandon before releasing",
                pending.join(", ")
            ),
        ));
    }
    let module_manifest = module::load(module_root).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("cannot load module.yaml: {error}"),
        )
    })?;

    // Stage everything via install (assemble + deploy + .manifest)
    let staging_dir = module_root.join("build").join("staging");
    let _ = fs::remove_dir_all(&staging_dir);
    let mut result = install::execute(
        path,
        Some(&staging_dir.to_string_lossy()),
        &[],
        true,
        false,
        false,
        false,
        None,
        None,
        true,
        false,
    )?;
    result.installed.clear();
    result.skipped.clear();

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let readme_path = module_root.join("README.md");
    let dist_dir = module_root.join("dist");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", dist_dir.display()),
        )
    })?;

    for (provider_name, provider_config) in &providers {
        let staged_roots: Vec<&str> = provider_config
            .target_roots()
            .into_iter()
            .filter(|root| staging_dir.join(root).is_dir())
            .collect();
        if staged_roots.is_empty() {
            continue;
        }

        let wrapper_name = format!(
            "{}-{provider_name}-v{}",
            module_manifest.name, module_manifest.version
        );
        let wrapper_dir = module_root.join("build").join(&wrapper_name);
        let _ = fs::remove_dir_all(&wrapper_dir);
        fs::create_dir_all(&wrapper_dir).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", wrapper_dir.display()),
            )
        })?;

        // Move every installed target root (including .manifest files) into
        // the wrapper. Roots can nest in plugin mode (`.claude` and
        // `.claude/skills/rune`), so shallow roots move first and carry
        // their nested roots with them; a root that already moved with a
        // parent is skipped.
        let mut ordered_roots = staged_roots;
        ordered_roots.sort_by_key(|root| Path::new(root).components().count());
        for root in ordered_roots {
            let staged = staging_dir.join(root);
            if !staged.is_dir() {
                continue;
            }
            let destination = wrapper_dir.join(root);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!("cannot create {}: {error}", parent.display()),
                    )
                })?;
            }
            fs::rename(&staged, &destination).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot move {provider_name}: {error}"),
                )
            })?;
        }

        // Add Makefile and README
        let makefile_content = MAKEFILE_TEMPLATE.replace("${PROVIDER}", provider_name);
        fs::write(wrapper_dir.join("Makefile"), makefile_content).map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot write Makefile: {error}"))
        })?;
        if readme_path.is_file() {
            let _ = fs::copy(&readme_path, wrapper_dir.join("README.md"));
        }

        // Tar to dist/ and clean staging
        let tarball_path = dist_dir.join(format!("{wrapper_name}.tar.gz"));
        create_tarball(
            &module_root.join("build"),
            &tarball_path,
            &wrapper_name,
            provider_name,
        )?;
        let _ = fs::remove_dir_all(&wrapper_dir);

        result.installed.push(DeployedFile {
            source: provider_config.default_target().to_string(),
            target: tarball_path.to_string_lossy().to_string(),
            provider: provider_name.clone(),
        });
    }

    let _ = fs::remove_dir_all(&staging_dir);

    if embed {
        eprintln!("warning: --embed is not yet implemented");
    }

    Ok(result)
}

fn create_tarball(
    parent_dir: &Path,
    tarball_path: &Path,
    wrapper_name: &str,
    provider_name: &str,
) -> Result<(), Error> {
    let output = Command::new("tar")
        .args([
            "-czf",
            &tarball_path.to_string_lossy(),
            "-C",
            &parent_dir.to_string_lossy(),
            wrapper_name,
        ])
        .output()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("tar failed for {provider_name}: {error}"),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(
            ErrorKind::Io,
            format!("tar failed for {provider_name}: {stderr}"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests;
