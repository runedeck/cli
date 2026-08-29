use rune::error::Error;
use rune::error::ErrorKind;
use rune::result::ActionResult;
use std::path::Path;

use super::assemble;
use super::config;
use super::deploy;

struct StaleSource {
    trunk: String,
    commits_behind: usize,
}

/// Assemble and deploy module content to provider directories.
///
/// ```text
/// 1. assemble(path)    → build/ populated
/// 2. deploy(path)      → build/ → provider targets
/// ```
///
/// Returns only the deployment result — assembly is an internal step.
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
pub fn execute(
    path: &str,
    target: Option<&str>,
    requested_providers: &[String],
    force: bool,
    prune: bool,
    interactive: bool,
    dry_run: bool,
    only: Option<&str>,
    model: Option<&str>,
    allow_stale: bool,
    fire_events: bool,
) -> Result<ActionResult, Error> {
    let retry_command = install_command(
        path,
        target,
        requested_providers,
        force,
        prune,
        interactive,
        dry_run,
        only,
        model,
        true,
    );
    let warnings = warn_or_block_stale_source(Path::new(path), allow_stale, &retry_command)?;
    assemble::execute_with_options(path, requested_providers, model)?;
    let mut result = deploy::execute(
        path,
        target,
        requested_providers,
        force,
        prune,
        interactive,
        dry_run,
        only,
    )?;
    result.warnings.extend(warnings);
    // Only a user install fires plugin events: internal staging
    // callers (release) pass fire_events false.
    if fire_events && !dry_run {
        crate::cli::plugin::fire(
            crate::cli::plugin::POST_INSTALL,
            &serde_json::json!({
                "event": crate::cli::plugin::POST_INSTALL,
                "source": crate::cli::resolved_path(Path::new(path)).display().to_string(),
                "target": target,
                "providers": requested_providers,
                "deployed": result.installed.len(),
            }),
        );
    }
    Ok(result)
}

fn warn_or_block_stale_source(
    module_root: &Path,
    allow_stale: bool,
    fix_command: &str,
) -> Result<Vec<String>, Error> {
    let module_label = module_label(module_root);
    let stale = match detect_stale_source(module_root) {
        Ok(Some(stale)) => stale,
        Ok(None) => return Ok(Vec::new()),
        Err(error) => {
            return Ok(vec![format!(
                "cannot determine git freshness for {module_label}: {error}; continuing"
            )]);
        }
    };

    let count = match stale.commits_behind {
        1 => "1 commit".to_string(),
        count => format!("{count} commits"),
    };
    let warning = format!(
        "WARNING: source module {module_label} is {count} behind {}; deploying it may resurrect stale content",
        stale.trunk
    );

    if allow_stale {
        return Ok(vec![warning]);
    }

    Err(Error::new(
        ErrorKind::Config,
        format!("{warning}. Rune requires --allow-stale to continue."),
    )
    .with_code("install.source_stale")
    .with_fix_command(fix_command))
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn install_command(
    path: &str,
    target: Option<&str>,
    requested_providers: &[String],
    force: bool,
    prune: bool,
    interactive: bool,
    dry_run: bool,
    only: Option<&str>,
    model: Option<&str>,
    allow_stale: bool,
) -> String {
    let source = crate::cli::resolved_path(Path::new(path));
    let mut arguments = vec![
        "rune".to_string(),
        "install".to_string(),
        "--source".to_string(),
        crate::cli::shell_quote(&source.to_string_lossy()),
    ];
    if let Some(target) = target {
        let target = crate::cli::resolved_path(Path::new(target));
        arguments.push("--target".to_string());
        arguments.push(crate::cli::shell_quote(&target.to_string_lossy()));
    }
    for provider in requested_providers {
        arguments.push("--provider".to_string());
        arguments.push(crate::cli::shell_quote(provider));
    }
    if force {
        arguments.push("--force".to_string());
    }
    if !prune {
        arguments.push("--no-prune".to_string());
    }
    if interactive {
        arguments.push("--interactive".to_string());
    }
    if dry_run {
        arguments.push("--dry-run".to_string());
    }
    if let Some(only) = only {
        arguments.push("--only".to_string());
        arguments.push(crate::cli::shell_quote(only));
    }
    if let Some(model) = model {
        arguments.push("--model".to_string());
        arguments.push(crate::cli::shell_quote(model));
    }
    if allow_stale {
        arguments.push("--allow-stale".to_string());
    }
    let command = arguments.join(" ");
    if target.is_some() {
        return command;
    }

    let current_directory = crate::cli::resolved_path(Path::new("."));
    format!(
        "cd {} && {command}",
        crate::cli::shell_quote(&current_directory.to_string_lossy())
    )
}

fn module_label(module_root: &Path) -> String {
    let source_uri = config::load_source_uri(module_root);
    if source_uri.is_empty() {
        module_root.display().to_string()
    } else {
        source_uri
    }
}

fn detect_stale_source(module_root: &Path) -> Result<Option<StaleSource>, String> {
    if !module_root.join(".git").exists() {
        return Ok(None);
    }

    let repo = gix::open(module_root).map_err(|error| error.to_string())?;
    let head = repo.head().map_err(|error| error.to_string())?;
    if head.is_unborn() {
        return Ok(None);
    }
    let head_id = repo.head_id().map_err(|error| error.to_string())?.detach();

    // Deliberately do not fetch here. `rune install` compares against the
    // last-known origin trunk ref so the check stays cheap and offline-safe.
    let Some((trunk_name, trunk_ref)) = trunk_reference(&repo)? else {
        return Ok(None);
    };
    let trunk_id = trunk_ref
        .into_fully_peeled_id()
        .map_err(|error| error.to_string())?;

    if trunk_id == head_id {
        return Ok(None);
    }

    for (commits_behind, item) in trunk_id
        .ancestors()
        .all()
        .map_err(|error| error.to_string())?
        .enumerate()
    {
        let info = item.map_err(|error| error.to_string())?;
        if info.id == head_id {
            return Ok(Some(StaleSource {
                trunk: trunk_name.to_string(),
                commits_behind,
            }));
        }
    }

    Ok(None)
}

fn trunk_reference(
    repo: &gix::Repository,
) -> Result<Option<(&'static str, gix::Reference<'_>)>, String> {
    for name in ["refs/remotes/origin/main", "refs/remotes/origin/master"] {
        if let Some(reference) = repo
            .try_find_reference(name)
            .map_err(|error| error.to_string())?
        {
            return Ok(Some((name, reference)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
