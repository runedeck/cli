use rune::error::{Error, ErrorKind};
use rune::provider;
use rune::yaml;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

mod recovery;
mod source;

pub use recovery::{CheckScope, FileScope, check, defaults, reference};
use source::SourceConfig;

/// Embedded at compile time so the binary works when symlinked away from
/// its source tree (e.g. ~/.local/bin/rune).
const EMBEDDED_DEFAULTS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/defaults.yaml"));
const EMBEDDED_REMAP_TOOLS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/config/remap-tools.yaml"
));
const EMBEDDED_MODELS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/config/models.yaml"));

#[derive(Debug, Clone)]
pub(crate) struct RegisteredProvider {
    pub(crate) name: String,
    pub(crate) config: provider::ProviderConfig,
    pub(crate) detection: provider::detection::DetectionRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredProviderTarget {
    pub(crate) provider: String,
    pub(crate) target: String,
    pub(crate) enabled: bool,
}

/// Advisory per-target lock held while a process mutates a deploy target
/// (deploy, prune, doctor --repair). A second rune process fails fast
/// instead of interleaving manifest and tree writes. The lock file records
/// the holder's pid and is removed on drop.
#[derive(Debug)]
pub struct TargetLock {
    path: std::path::PathBuf,
}

impl Drop for TargetLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn lock_target(base: &Path) -> Result<TargetLock, Error> {
    fs::create_dir_all(base).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", base.display()),
        )
    })?;
    let path = base.join(".rune.lock");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write as _;
            let _ = writeln!(file, "{}", std::process::id());
            Ok(TargetLock { path })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(Error::new(
            ErrorKind::Io,
            format!(
                "another rune process is deploying into this target (lock: {}); wait for it, or remove the file if it is stale",
                path.display()
            ),
        )),
        Err(error) => Err(Error::new(
            ErrorKind::Io,
            format!("cannot create lock {}: {error}", path.display()),
        )),
    }
}

/// Write through an adjacent temporary file plus rename, so interruption
/// never leaves a truncated file. The temporary carries a per-process
/// unique suffix and is opened with `create_new`, so a pre-planted
/// symlink at a predictable name cannot redirect the write; a symlink at
/// the destination itself is refused rather than followed.
static WRITE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn write_atomic(path: &Path, content: &str) -> Result<(), Error> {
    if path.symlink_metadata().is_ok_and(|meta| meta.is_symlink()) {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} is a symlink; refusing to replace it", path.display()),
        ));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let base_name = path.file_name().map_or_else(
        || "rune-write".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let sequence = WRITE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{base_name}.{}.{sequence}.tmp",
        std::process::id()
    ));
    {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", temporary.display()),
                )
            })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", temporary.display()),
            )
        })?;
        let _ = file.sync_all();
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        Error::new(
            ErrorKind::Io,
            format!("cannot replace {}: {error}", path.display()),
        )
    })
}

/// Read a file to string with consistent error handling.
pub fn read_file(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {e}", path.display()),
        )
    })
}

/// Read and deep-merge defaults.yaml with optional config.yaml.
pub fn load_merged_config(module_root: &Path) -> Result<String, Error> {
    let defaults_path = module_root.join("defaults.yaml");
    // defaults.yaml is optional; if missing, we fall back to embedded defaults
    // to allow modules without local configuration to install.
    let defaults_content = if defaults_path.is_file() {
        read_file(&defaults_path).map_err(|error| {
            Error::new(error.kind(), error.message())
                .with_code("config.defaults_unreadable")
                .with_fix_command("rune config check --scope source")
        })?
    } else {
        String::new()
    };

    let config_path = module_root.join("config.yaml");
    if config_path.is_file() {
        let config_content = read_file(&config_path).map_err(|error| {
            Error::new(error.kind(), error.message())
                .with_code("config.override_unreadable")
                .with_fix_command("rune config check --scope source")
        })?;
        yaml::deep_merge(&defaults_content, &config_content).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!("Rune cannot merge the source configuration: {error}"),
            )
            .with_code("config.merge_invalid")
            .with_fix_command("rune config check --scope source")
        })
    } else {
        Ok(defaults_content)
    }
}

/// Load provider configurations, merging module config with embedded defaults.
///
/// The embedded defaults provide the base (`target`, assembly rules, `keep_fields`,
/// `models`). The module's `providers:` section overrides specific fields per provider.
/// If the module has no providers: section, embedded defaults are used entirely.
pub fn load_providers(config: &str) -> Result<HashMap<String, provider::ProviderConfig>, Error> {
    let embedded_config = source::providers(EMBEDDED_DEFAULTS).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("failed to load embedded provider config: {error}"),
        )
        .with_code("config.embedded_providers_invalid")
        .with_fix_command("rune config defaults --scope source")
    })?;
    let embedded_providers = provider::resolve_providers(embedded_config).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("failed to load embedded provider config: {error}"),
        )
        .with_code("config.embedded_providers_invalid")
        .with_fix_command("rune config defaults --scope source")
    })?;

    let Ok(module_config) = yaml::deep_merge(EMBEDDED_DEFAULTS, config) else {
        return Ok(embedded_providers);
    };

    match source::providers(&module_config)
        .map_err(|error| error.to_string())
        .and_then(provider::resolve_providers)
    {
        Ok(providers) => Ok(providers),
        // A semantic conflict the user must resolve; falling back to
        // embedded defaults would silently discard their overrides and
        // deploy to the wrong locations.
        Err(error) if error.contains("cannot combine with a by-kind target map") => {
            Err(Error::new(ErrorKind::Config, error)
                .with_code("config.incompatible")
                .with_fix_command("rune config check --scope source"))
        }
        Err(error) => {
            eprintln!(
                "warning: module config incompatible with provider schema ({error}), using embedded defaults"
            );
            Ok(embedded_providers)
        }
    }
}

/// Load only providers that have trusted, bundled detection rules.
pub(crate) fn load_registered_providers(root: &Path) -> Result<Vec<RegisteredProvider>, Error> {
    let merged = load_merged_config(root)?;
    let mut configured = load_providers(&merged)?;
    let registry = provider::detection::bundled_registry().map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("Rune cannot load the bundled provider registry: {error}"),
        )
        .with_code("provider.registry_invalid")
        .with_fix_command("rune --version")
    })?;

    registry
        .into_iter()
        .map(|(name, detection)| {
            let config = configured.remove(&name).ok_or_else(|| {
                Error::new(
                    ErrorKind::Config,
                    format!("The bundled provider registry has no configuration for '{name}'."),
                )
                .with_code("provider.registry_mismatch")
                .with_fix_command("rune --version")
            })?;
            Ok(RegisteredProvider {
                name,
                config,
                detection,
            })
        })
        .collect()
}

pub(crate) fn detect_registered_providers(
    source_root: &Path,
    target_base: &Path,
) -> Result<Vec<provider::detection::ProviderDetection>, Error> {
    let search_path = std::env::var_os("PATH");
    load_registered_providers(source_root).map(|providers| {
        providers
            .into_iter()
            .map(|registered| {
                provider::detection::detect_provider(
                    &registered.name,
                    &registered.detection,
                    &registered.config,
                    source_root,
                    target_base,
                    search_path.as_deref(),
                )
            })
            .collect()
    })
}

pub(crate) fn registered_provider_targets(root: &Path) -> Result<Vec<(String, String)>, Error> {
    registered_provider_target_records(root).map(|targets| {
        targets
            .into_iter()
            .map(|target| (target.provider, target.target))
            .collect()
    })
}

pub(crate) fn registered_provider_target_records(
    root: &Path,
) -> Result<Vec<RegisteredProviderTarget>, Error> {
    let mut targets = load_registered_providers(root)?
        .into_iter()
        .flat_map(|provider| {
            let name = provider.name;
            let enabled = provider.config.enabled;
            provider
                .config
                .target_roots()
                .into_iter()
                .map(|target| RegisteredProviderTarget {
                    provider: name.clone(),
                    target: target.to_string(),
                    enabled,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        left.provider
            .cmp(&right.provider)
            .then_with(|| left.target.cmp(&right.target))
    });
    targets.dedup();
    Ok(targets)
}

/// Load the allowlist of settings filenames the dashboard surfaces per harness,
/// from `dashboard.settings_files` in the merged config. Falls back to the
/// embedded defaults when the module config omits the section.
#[cfg_attr(not(feature = "dashboard"), allow(dead_code))]
pub fn load_settings_filenames(module_root: &Path) -> Vec<String> {
    let merged = load_merged_config(module_root).unwrap_or_default();
    let from_module = match source::dashboard(&merged) {
        Ok(config) => config.settings_files,
        Err(error) => {
            eprintln!(
                "warning: failed to parse dashboard.settings_files ({error}), using embedded defaults"
            );
            Vec::new()
        }
    };
    if from_module.is_empty() {
        SourceConfig::installed_defaults()
            .map(|config| config.dashboard.settings_files)
            .unwrap_or_default()
    } else {
        from_module
    }
}

pub(crate) fn source_spec_root(config: &str) -> Option<String> {
    source::spec(config).ok()?.root?.joined()
}

pub(crate) fn source_adr_prefixes(config: &str) -> Option<String> {
    source::adr(config).ok()?.prefixes?.joined()
}

pub(crate) fn source_validate_excludes(config: &str) -> Vec<String> {
    source::validate(config)
        .map(|config| {
            config
                .exclude
                .map(|exclude| exclude.items())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Load remap-tools.yaml from the module, falling back to embedded defaults.
pub fn load_remap_tools(module_root: &Path) -> Result<Option<String>, Error> {
    let module_remap = module_root.join("config/remap-tools.yaml");
    if module_remap.is_file() {
        return Ok(Some(read_file(&module_remap)?));
    }
    Ok(Some(EMBEDDED_REMAP_TOOLS.to_string()))
}

pub fn load_tool_mappings(
    remap_content: Option<&String>,
    provider_name: &str,
) -> Result<HashMap<String, String>, Error> {
    match remap_content {
        Some(content) => provider::load_tool_mappings(content, provider_name).map_err(|e| {
            Error::new(
                ErrorKind::Config,
                format!("failed to load tool mappings: {e}"),
            )
        }),
        None => Ok(HashMap::new()),
    }
}

/// Load model definitions from models.yaml, falling back to embedded defaults.
///
/// Returns an empty map if neither the module file nor the embedded file
/// can be parsed (all model-tier qualifiers become unresolvable).
pub fn load_models(module_root: &Path) -> HashMap<String, Vec<String>> {
    let models_path = module_root.join("config/models.yaml");
    let content = if models_path.is_file() {
        read_file(&models_path).ok()
    } else {
        Some(EMBEDDED_MODELS.to_string())
    };

    match content {
        Some(yaml) => match provider::load_models(&yaml) {
            Ok(models) => models,
            Err(error) => {
                eprintln!(
                    "warning: failed to parse models config ({error}), using embedded defaults"
                );
                provider::load_models(EMBEDDED_MODELS).unwrap_or_default()
            }
        },
        None => HashMap::new(),
    }
}

/// Load the source URI for provenance from module.yaml.
///
/// Checks for a `repository` field first (full URL), then falls back
/// to the `name` field as a plain identifier.
pub fn load_source_uri(module_root: &Path) -> String {
    match rune::module::load(module_root) {
        Ok(manifest) => manifest.source_uri().to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests;
