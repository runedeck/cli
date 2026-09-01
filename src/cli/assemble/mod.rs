mod output;
mod pipeline;
mod provenance;
pub mod sources;

use rune::error::Error;
use rune::result::{ActionResult, DeployedFile};
use std::path::Path;

use crate::cli::config;

/// Assemble rune content into the build/ directory.
///
/// Given a rune source at `path`:
///
/// ```text
/// module/
///   defaults.yaml
///   config.yaml                  (optional override)
///   config/remap-tools.yaml      (optional tool mappings)
///   agents/SecurityArchitect.md
///   rules/MyRule.md
///   skills/Explain/SKILL.md
/// ```
///
/// Produces:
///
/// ```text
/// module/build/
///   claude/agents/SecurityArchitect.md
///   claude/agents/SecurityArchitect.yaml
///   claude/rules/MyRule.md
///   claude/rules/MyRule.yaml
///   gemini/agents/security-architect.md  (with remapped tools)
///   gemini/agents/security-architect.yaml
/// ```
/// Assemble, selecting model variants with `model_override` (the `--model`
/// flag) in place of each provider's configured default model.
pub fn execute_with_model(path: &str, model_override: Option<&str>) -> Result<ActionResult, Error> {
    execute_with_options(path, &[], model_override)
}

pub fn execute_with_options(
    path: &str,
    requested_providers: &[String],
    model_override: Option<&str>,
) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    if !module_root.is_dir() {
        return Err(Error::new(
            rune::error::ErrorKind::Io,
            format!("rune source directory not found: {}", module_root.display()),
        ));
    }
    let module_manifest = module_root.join("module.yaml");
    if !module_manifest.is_file() && !crate::cli::dotrune::exists(module_root) {
        return Err(Error::new(
            rune::error::ErrorKind::Config,
            format!(
                "no module.yaml or .rune at {}; --source must point to a rune source or consumer target",
                module_root.display()
            ),
        ));
    }
    let mut result = ActionResult::new();

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let remap_content = config::load_remap_tools(module_root)?;
    let models = config::load_models(module_root);
    let source_uri = config::load_source_uri(module_root);
    let provider_names: Vec<String> = providers.keys().cloned().collect();
    let valid_qualifiers = sources::build_valid_qualifiers(&provider_names, &models);
    let mut provider_toggles = crate::cli::dotrune::toggle::ToggleMap::default();
    let source_files = if let Some(manifest) = crate::cli::dotrune::load(module_root)? {
        provider_toggles = crate::cli::dotrune::toggle::toggle_map(&manifest);
        crate::cli::dotrune::resolve_sources(&manifest, module_root, &valid_qualifiers)?
    } else {
        sources::collect(module_root, &valid_qualifiers)?
    };

    let build_dir = module_root.join("build");

    // Assembly writes into a staging tree and swaps it in only after every
    // provider succeeds, so a failed run cannot destroy the last good build/
    // or leave a partial one for deploy to consume.
    let staging_dir = module_root.join(format!(".build-staging-{}", std::process::id()));
    if staging_dir.exists() {
        std::fs::remove_dir_all(&staging_dir).map_err(|e| {
            rune::error::Error::new(
                rune::error::ErrorKind::Io,
                format!("cannot clean staging directory: {e}"),
            )
        })?;
    }

    let assembled: Result<(), Error> = assemble_providers(
        &providers,
        &provider_toggles,
        requested_providers,
        &staging_dir,
        module_root,
        model_override,
        remap_content.as_ref(),
        &models,
        &source_uri,
        &source_files,
        &mut result,
    );
    if let Err(error) = assembled {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(error);
    }

    // Nothing assembled (empty module, or every provider filtered out): keep
    // the start-clean contract by clearing the previous build and stop.
    if !staging_dir.exists() {
        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir).map_err(|e| {
                rune::error::Error::new(
                    rune::error::ErrorKind::Io,
                    format!("cannot clean build directory: {e}"),
                )
            })?;
        }
        return Ok(result);
    }

    let retired_dir = module_root.join(format!(".build-retired-{}", std::process::id()));
    if build_dir.exists() {
        std::fs::rename(&build_dir, &retired_dir).map_err(|e| {
            rune::error::Error::new(
                rune::error::ErrorKind::Io,
                format!("cannot retire previous build directory: {e}"),
            )
        })?;
    }
    if let Err(e) = std::fs::rename(&staging_dir, &build_dir) {
        // Put the previous build back so a failed swap never leaves no build.
        if retired_dir.exists() {
            let _ = std::fs::rename(&retired_dir, &build_dir);
        }
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(rune::error::Error::new(
            rune::error::ErrorKind::Io,
            format!("cannot activate new build directory: {e}"),
        ));
    }
    let _ = std::fs::remove_dir_all(&retired_dir);

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn assemble_providers(
    providers: &std::collections::HashMap<String, rune::provider::ProviderConfig>,
    provider_toggles: &crate::cli::dotrune::toggle::ToggleMap,
    requested_providers: &[String],
    build_dir: &Path,
    module_root: &Path,
    model_override: Option<&str>,
    remap_content: Option<&String>,
    models: &std::collections::HashMap<String, Vec<String>>,
    source_uri: &str,
    source_files: &[sources::SourceFile],
    result: &mut ActionResult,
) -> Result<(), Error> {
    for (provider_name, provider_config) in providers {
        if !requested_providers.is_empty()
            && !requested_providers
                .iter()
                .any(|requested| provider_config.matches_target(requested, provider_name))
        {
            continue;
        }
        let provider_build_dir = build_dir.join(provider_name);
        let tool_mappings = config::load_tool_mappings(remap_content, provider_name)?;
        let mut deploy_paths = std::collections::HashMap::new();

        // Parse assembly rules for this provider. A typo must fail loudly:
        // silently dropping a rule assembles without the transformation.
        let assembly_rules: Vec<rune::provider::AssemblyRule> = provider_config
            .assembly
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|name| {
                rune::provider::AssemblyRule::from_name(name).map_err(|error| {
                    Error::new(
                        rune::error::ErrorKind::Config,
                        format!("provider {provider_name}: {error}"),
                    )
                })
            })
            .collect::<Result<_, _>>()?;

        let model_tiers = provider_config.models.clone().unwrap_or_default();
        let effort_tiers = provider_config.effort.clone().unwrap_or_default();
        let active_model =
            resolve_active_model(model_override, provider_config, models, provider_name);

        for source in source_files {
            if crate::cli::dotrune::toggle::toggled_off(provider_toggles, provider_name, source) {
                continue;
            }
            if let Some(deployed) = assemble_source_for_provider(
                source,
                module_root,
                provider_name,
                active_model.as_deref(),
                provider_config,
                &provider_build_dir,
                &tool_mappings,
                &assembly_rules,
                &model_tiers,
                &effort_tiers,
                models,
                source_uri,
                !requested_providers.is_empty(),
                &mut deploy_paths,
            )? {
                result.installed.push(deployed);
            }
        }
    }

    Ok(())
}

/// Choose the model ID whose `provider/<model>/` variants win this assembly:
/// `--model` when it is a valid model for the provider, otherwise the
/// provider's configured default. An override that names another provider's
/// model is ignored so a single `--model` flag is safe across all providers.
fn resolve_active_model(
    model_override: Option<&str>,
    provider_config: &rune::provider::ProviderConfig,
    models: &std::collections::HashMap<String, Vec<String>>,
    provider_name: &str,
) -> Option<String> {
    if let Some(override_id) = model_override
        && models
            .get(provider_name)
            .is_some_and(|ids| ids.iter().any(|id| id == override_id))
    {
        return Some(override_id.to_string());
    }
    provider_config.model.clone()
}

fn source_matches_provider_target(
    source: &sources::SourceFile,
    provider_name: &str,
    provider_config: &rune::provider::ProviderConfig,
) -> bool {
    source.targets.as_ref().is_none_or(|file_targets| {
        file_targets
            .iter()
            .any(|target| provider_config.matches_target(target, provider_name))
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn assemble_source_for_provider(
    source: &sources::SourceFile,
    module_root: &Path,
    provider_name: &str,
    active_model: Option<&str>,
    provider_config: &rune::provider::ProviderConfig,
    provider_build_dir: &Path,
    tool_mappings: &std::collections::HashMap<String, String>,
    assembly_rules: &[rune::provider::AssemblyRule],
    model_tiers: &std::collections::HashMap<String, Vec<String>>,
    effort_tiers: &std::collections::HashMap<String, String>,
    models: &std::collections::HashMap<String, Vec<String>>,
    source_uri: &str,
    providers_overridden: bool,
    deploy_paths: &mut std::collections::HashMap<String, String>,
) -> Result<Option<DeployedFile>, Error> {
    if !providers_overridden
        && source
            .providers
            .as_ref()
            .is_some_and(|providers| !providers.iter().any(|provider| provider == provider_name))
    {
        return Ok(None);
    }
    if !source_qualifier_matches_provider(source, provider_name, active_model, models) {
        return Ok(None);
    }

    if !source_matches_provider_target(source, provider_name, provider_config) {
        return Ok(None);
    }

    // Binary passthrough assets copy byte-for-byte: no variants, no text
    // transforms, no newline normalization. The filename still goes through
    // the provider's rename rules so a companion lands beside its (possibly
    // kebab-renamed) skill directory.
    if let Some(bytes) = &source.content_bytes {
        return assemble_binary_passthrough(
            source,
            bytes,
            provider_name,
            provider_build_dir,
            tool_mappings,
            assembly_rules,
            source_uri,
            deploy_paths,
        );
    }

    let kind_keep_fields = provider_config
        .keep_fields
        .as_ref()
        .and_then(|fields_by_kind| fields_by_kind.get(source.kind.as_str()))
        .cloned()
        .unwrap_or_default();

    let is_hook = source.kind == rune::provider::ContentKind::Hooks;
    let mut assembled = if is_hook {
        source.content.clone()
    } else {
        pipeline::assemble_source(
            source,
            module_root,
            provider_name,
            active_model,
            &kind_keep_fields,
            model_tiers,
            effort_tiers,
            assembly_rules.contains(&rune::provider::AssemblyRule::StripLinks),
        )?
    };
    // For skills, preserve the skill directory: skills/SceneReview/SKILL.md
    // For agents/rules, use just the filename: agents/GameMaster.md
    // For qualifier-only files, strip the qualifier directory too:
    //   rules/sonnet/ReviewDiscipline.md → ReviewDiscipline.md
    let stripped_kind = source
        .relative_path
        .strip_prefix(&format!("{}/", source.kind))
        .unwrap_or(&source.relative_path);
    // Qualifier-only files live one or two levels deep (rules/<provider>/file
    // or rules/<provider>/<model>/file); deploy them flat under the kind, so
    // strip every qualifier segment down to the basename.
    let relative_within_kind = if source.qualifier.is_some() {
        stripped_kind
            .rsplit_once('/')
            .map_or(stripped_kind, |(_, filename)| filename)
    } else {
        stripped_kind
    };

    // Apply transformation rules (kebab-case, kebab-case-agents, remap-tools, etc.)
    let (transformed_content, transformed_filename) = if is_hook {
        let deck = hook_deck(source)?;
        let content = if relative_within_kind == "hooks.json" {
            rewrite_hook_commands(
                &assembled,
                provider_config.default_target(),
                deck,
                provider_config.plugin.is_some(),
            )?
        } else {
            assembled.clone()
        };
        (content, format!("{deck}/{relative_within_kind}"))
    } else {
        rune::transform::apply_rules(
            &assembled,
            relative_within_kind,
            assembly_rules,
            tool_mappings,
            source.kind.as_str(),
        )
        .map_err(|e| rune::error::Error::new(rune::error::ErrorKind::Validate, e))?
    };

    assembled = transformed_content;

    // Normalize assembled documents to the POSIX trailing-newline convention.
    // Passthrough skill assets and hooks retain their source bytes verbatim.
    if !is_hook && !source.passthrough && !assembled.is_empty() && !assembled.ends_with('\n') {
        assembled.push('\n');
    }

    let output_path = provider_build_dir
        .join(source.kind.as_str())
        .join(&transformed_filename);
    let deploy_relative = format!("{}/{transformed_filename}", source.kind);
    let rune_id = source.rune_id.as_deref().unwrap_or(&source.relative_path);
    if let Some(existing_id) = deploy_paths.insert(deploy_relative.clone(), rune_id.to_string()) {
        return Err(rune::error::Error::new(
            rune::error::ErrorKind::Config,
            format!(
                "deploy-path collision for provider '{provider_name}' at {deploy_relative}: {existing_id} and {rune_id}"
            ),
        ));
    }
    let manifest_key = format!("{}/{}/{}", provider_name, source.kind, transformed_filename);

    output::write_file(&output_path, &assembled)?;
    if is_hook || source.passthrough {
        preserve_executable_bit(source, &output_path)?;
    }

    if !is_hook {
        let statement = provenance::build_statement(&manifest_key, &assembled, source, source_uri);
        provenance::write_sidecar(&output_path, &statement)?;
    }

    Ok(Some(DeployedFile {
        source: source.relative_path.clone(),
        target: output_path.to_string_lossy().to_string(),
        provider: provider_name.to_string(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn assemble_binary_passthrough(
    source: &sources::SourceFile,
    bytes: &[u8],
    provider_name: &str,
    provider_build_dir: &Path,
    tool_mappings: &std::collections::HashMap<String, String>,
    assembly_rules: &[rune::provider::AssemblyRule],
    source_uri: &str,
    deploy_paths: &mut std::collections::HashMap<String, String>,
) -> Result<Option<DeployedFile>, Error> {
    let stripped_kind = source
        .relative_path
        .strip_prefix(&format!("{}/", source.kind))
        .unwrap_or(&source.relative_path);
    // Only the filename mapping applies to bytes; rules never derive a name
    // from content, so an empty document yields the same rename.
    let (_, transformed_filename) = rune::transform::apply_rules(
        "",
        stripped_kind,
        assembly_rules,
        tool_mappings,
        source.kind.as_str(),
    )
    .map_err(|e| rune::error::Error::new(rune::error::ErrorKind::Validate, e))?;

    let output_path = provider_build_dir
        .join(source.kind.as_str())
        .join(&transformed_filename);
    let deploy_relative = format!("{}/{transformed_filename}", source.kind);
    let rune_id = source.rune_id.as_deref().unwrap_or(&source.relative_path);
    if let Some(existing_id) = deploy_paths.insert(deploy_relative.clone(), rune_id.to_string()) {
        return Err(rune::error::Error::new(
            rune::error::ErrorKind::Config,
            format!(
                "deploy-path collision for provider '{provider_name}' at {deploy_relative}: {existing_id} and {rune_id}"
            ),
        ));
    }
    let manifest_key = format!("{}/{}/{}", provider_name, source.kind, transformed_filename);

    output::write_file_bytes(&output_path, bytes)?;
    preserve_executable_bit(source, &output_path)?;

    let statement = provenance::build_statement_bytes(&manifest_key, bytes, source, source_uri);
    provenance::write_sidecar(&output_path, &statement)?;

    Ok(Some(DeployedFile {
        source: source.relative_path.clone(),
        target: output_path.to_string_lossy().to_string(),
        provider: provider_name.to_string(),
    }))
}

fn hook_deck(source: &sources::SourceFile) -> Result<&str, Error> {
    source
        .rune_id
        .as_deref()
        .and_then(|id| id.split('/').next())
        .filter(|deck| !deck.is_empty())
        .ok_or_else(|| {
            rune::error::Error::new(
                rune::error::ErrorKind::Config,
                format!("hook {} has no deck", source.relative_path),
            )
        })
}

fn rewrite_hook_commands(
    content: &str,
    provider_target: &str,
    deck: &str,
    plugin_mode: bool,
) -> Result<String, Error> {
    let mut manifest: serde_json::Value = serde_json::from_str(content).map_err(|error| {
        rune::error::Error::new(
            rune::error::ErrorKind::Validate,
            format!("invalid hooks/hooks.json: {error}"),
        )
    })?;
    // A plugin deployment keeps ${CLAUDE_PLUGIN_ROOT}: the harness defines
    // it, and scripts live one domain directory below the plugin's hooks/.
    let deployed_root = if plugin_mode {
        format!("${{CLAUDE_PLUGIN_ROOT}}/hooks/{deck}")
    } else {
        format!(
            "${{CLAUDE_PROJECT_DIR}}/{}/hooks/{deck}",
            provider_target.trim_end_matches('/')
        )
    };
    rewrite_command_values(&mut manifest, &deployed_root);
    serde_json::to_string_pretty(&manifest)
        .map(|mut value| {
            value.push('\n');
            value
        })
        .map_err(|error| {
            rune::error::Error::new(
                rune::error::ErrorKind::Validate,
                format!("cannot serialize hooks/hooks.json: {error}"),
            )
        })
}

fn rewrite_command_values(value: &mut serde_json::Value, deployed_root: &str) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if key == "command"
                    && let serde_json::Value::String(command) = child
                {
                    *command = command.replace("${CLAUDE_PLUGIN_ROOT}/hooks", deployed_root);
                } else {
                    rewrite_command_values(child, deployed_root);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                rewrite_command_values(item, deployed_root);
            }
        }
        _ => {}
    }
}

fn preserve_executable_bit(source: &sources::SourceFile, output_path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let source_mode = std::fs::metadata(&source.full_path)
            .map_err(|error| {
                rune::error::Error::new(
                    rune::error::ErrorKind::Io,
                    format!("cannot inspect {}: {error}", source.full_path),
                )
            })?
            .permissions()
            .mode();
        if source_mode & 0o111 != 0 {
            let mut permissions = std::fs::metadata(output_path)
                .map_err(|error| {
                    rune::error::Error::new(
                        rune::error::ErrorKind::Io,
                        format!("cannot inspect {}: {error}", output_path.display()),
                    )
                })?
                .permissions();
            permissions.set_mode(permissions.mode() | (source_mode & 0o111));
            std::fs::set_permissions(output_path, permissions).map_err(|error| {
                rune::error::Error::new(
                    rune::error::ErrorKind::Io,
                    format!(
                        "cannot set permissions on {}: {error}",
                        output_path.display()
                    ),
                )
            })?;
        }
    }
    Ok(())
}

/// Check whether a qualifier-only source applies to a provider/model target.
///
/// Provider-only files such as `rules/claude/Foo.md` apply to that provider
/// for every model. Model-only files such as
/// `rules/claude/claude-sonnet-4-6/Foo.md` apply only when the provider and
/// active model both match, so a non-active model variant cannot keep a stale
/// deployed base file alive during prune.
fn source_qualifier_matches_provider(
    source: &sources::SourceFile,
    provider_name: &str,
    active_model: Option<&str>,
    models: &std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if source.qualifier.is_none() {
        return true;
    }

    let segments = qualifier_segments(source);
    match segments.as_slice() {
        [provider] => {
            provider == provider_name
                || active_model_matches(provider, active_model, provider_name, models)
        }
        [provider, model, ..] => provider == provider_name && active_model == Some(model.as_str()),
        _ => false,
    }
}

fn qualifier_segments(source: &sources::SourceFile) -> Vec<String> {
    let stripped_kind = source
        .relative_path
        .strip_prefix(&format!("{}/", source.kind))
        .unwrap_or(&source.relative_path);
    let mut segments: Vec<String> = stripped_kind.split('/').map(str::to_string).collect();
    let _ = segments.pop();
    segments
}

fn active_model_matches(
    qualifier: &str,
    active_model: Option<&str>,
    provider_name: &str,
    models: &std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if active_model != Some(qualifier) {
        return false;
    }
    if let Some(model_ids) = models.get(provider_name) {
        return model_ids.iter().any(|id| id == qualifier);
    }
    false
}

#[cfg(test)]
fn qualifier_matches_provider(
    qualifier: &str,
    provider_name: &str,
    active_model: Option<&str>,
    models: &std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if qualifier == provider_name {
        return true;
    }
    active_model_matches(qualifier, active_model, provider_name, models)
}

#[cfg(test)]
mod tests;
