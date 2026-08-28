//! `rune provider`: inspect and toggle deploy providers for the current
//! source. Listing reads the merged config; enable/disable write the
//! `providers.<name>.enabled` key into the local `config.yaml`.

use rune::error::{Error, ErrorKind};
use rune::provider::detection::{
    CONFIG_SOURCE, DeploymentState, DetectionEvidence, ProviderDetection, RecommendedAction,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ProviderAction {
    /// Enable a provider for the default deploy set.
    Enable { name: String },
    /// Disable a provider; `--provider <name>` still selects it explicitly.
    Disable { name: String },
    /// Report provider deployment states.
    Status { name: Option<String> },
    /// Explain one provider classification.
    Explain { name: String },
}

pub fn execute(action: Option<ProviderAction>, json: bool) -> Result<i32, Error> {
    match action {
        None => list(json),
        Some(ProviderAction::Enable { name }) => set_enabled(&name, true, json),
        Some(ProviderAction::Disable { name }) => set_enabled(&name, false, json),
        Some(ProviderAction::Status { name }) => status(name.as_deref(), json),
        Some(ProviderAction::Explain { name }) => explain(&name, json),
    }
}

#[derive(Debug, Clone, Serialize)]
struct ProviderReport {
    provider: String,
    config_source: &'static str,
    target: String,
    evidence: Vec<DetectionEvidence>,
    deployment_state: DeploymentState,
    fix_command: Option<String>,
    #[serde(skip)]
    recommended_action: RecommendedAction,
}

fn status(name: Option<&str>, json: bool) -> Result<i32, Error> {
    let root = current_root()?;
    let mut reports = reports_at(&root)?;
    if let Some(name) = name {
        reports = vec![take_named_report(reports, name, "rune provider status")?];
    }

    if json {
        let rendered = serde_json::to_string_pretty(&serde_json::json!({
            "providers": reports,
        }))
        .map_err(|error| output_error(&error, "rune provider status --json"))?;
        println!("{rendered}");
    } else {
        print_status(&reports);
    }
    Ok(0)
}

fn explain(name: &str, json: bool) -> Result<i32, Error> {
    let root = current_root()?;
    let report = take_named_report(reports_at(&root)?, name, "rune provider status")?;
    if json {
        let rendered = serde_json::to_string_pretty(&report).map_err(|error| {
            output_error(&error, &format!("rune provider explain {name} --json"))
        })?;
        println!("{rendered}");
    } else {
        print_explanation(&report);
    }
    Ok(0)
}

fn current_root() -> Result<std::path::PathBuf, Error> {
    std::env::current_dir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("Rune cannot read the current directory: {error}"),
        )
        .with_code("provider.current_directory_unavailable")
        .with_fix_command("pwd")
    })
}

fn reports_at(root: &Path) -> Result<Vec<ProviderReport>, Error> {
    crate::cli::config::detect_registered_providers(root, root).map(|detections| {
        detections
            .into_iter()
            .map(|detection| report_from_detection(root, detection))
            .collect()
    })
}

fn report_from_detection(root: &Path, detection: ProviderDetection) -> ProviderReport {
    let fix_command = fix_command(root, &detection);
    ProviderReport {
        provider: detection.provider,
        config_source: CONFIG_SOURCE,
        target: detection.target.to_string_lossy().into_owned(),
        evidence: detection.evidence,
        deployment_state: detection.deployment_state,
        fix_command,
        recommended_action: detection.recommended_action,
    }
}

fn fix_command(root: &Path, detection: &ProviderDetection) -> Option<String> {
    let provider = crate::cli::shell_quote(&detection.provider);
    match detection.recommended_action {
        RecommendedAction::None => None,
        RecommendedAction::Enable => Some(format!("rune provider enable {provider}")),
        RecommendedAction::Install if installable_source(root) => {
            let source = crate::cli::resolved_path(root);
            let target = crate::cli::resolved_path(root);
            Some(format!(
                "rune install --source {} --target {} --provider {provider}",
                crate::cli::shell_quote(&source.to_string_lossy()),
                crate::cli::shell_quote(&target.to_string_lossy())
            ))
        }
        RecommendedAction::Install => Some("rune context".to_string()),
        RecommendedAction::Repair => Some(format!(
            "rune doctor --target {} --repair",
            crate::cli::shell_quote(&detection.target.to_string_lossy())
        )),
        RecommendedAction::Review => Some(format!(
            "rune doctor --target {}",
            crate::cli::shell_quote(&detection.target.to_string_lossy())
        )),
    }
}

fn installable_source(root: &Path) -> bool {
    root.join("module.yaml").is_file() || root.join(".rune").is_file() || rune::deck::is_deck(root)
}

fn take_named_report(
    reports: Vec<ProviderReport>,
    name: &str,
    fix_command: &str,
) -> Result<ProviderReport, Error> {
    let known = reports
        .iter()
        .map(|report| report.provider.clone())
        .collect::<Vec<_>>();
    reports
        .into_iter()
        .find(|report| report.provider == name)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "Rune does not recognize provider '{name}'. Known providers: {}.",
                    known.join(", ")
                ),
            )
            .with_code("provider.unknown")
            .with_fix_command(fix_command)
        })
}

fn output_error(error: &serde_json::Error, fix_command: &str) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("Rune cannot serialize the provider report: {error}"),
    )
    .with_code("provider.output_invalid")
    .with_fix_command(fix_command)
}

fn print_status(reports: &[ProviderReport]) {
    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading("providers"));
    for report in reports {
        let state = styled_state(&sheet, report.deployment_state);
        let command = report
            .fix_command
            .as_ref()
            .map_or_else(String::new, |command| {
                format!("  {} {command}", sheet.cyan(command_label(report)))
            });
        println!(
            "   {:<12} {state:<16} {}{command}",
            sheet.bold(&report.provider),
            sheet.dim(&report.target),
        );
    }
}

fn print_explanation(report: &ProviderReport) {
    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading(&report.provider));
    println!("{}", sheet.row("config", report.config_source));
    println!("{}", sheet.row("target", &report.target));
    println!("{}", sheet.row("state", report.deployment_state.label()));
    println!("\n{}", sheet.heading("evidence"));
    for evidence in &report.evidence {
        println!(
            "   {:<20} {}  {}",
            evidence.kind.label(),
            evidence.value,
            sheet.dim(evidence.result.label())
        );
    }
    if let Some(command) = &report.fix_command {
        println!("\n{} {command}", sheet.cyan(command_label(report)));
    }
}

fn command_label(report: &ProviderReport) -> &'static str {
    if report.recommended_action == RecommendedAction::Review
        || (report.recommended_action == RecommendedAction::Install
            && report.fix_command.as_deref() == Some("rune context"))
    {
        "review:"
    } else {
        "fix:"
    }
}

fn styled_state(sheet: &crate::cli::style::Sheet, state: DeploymentState) -> String {
    match state {
        DeploymentState::Current => sheet.green(state.label()),
        DeploymentState::Modified => sheet.yellow(state.label()),
        DeploymentState::NeedsRepair => sheet.red(state.label()),
        DeploymentState::Disabled | DeploymentState::NotInstalled | DeploymentState::Outdated => {
            sheet.dim(state.label())
        }
    }
}

fn list(json: bool) -> Result<i32, Error> {
    let merged = crate::cli::config::load_merged_config(Path::new("."))?;
    let providers = crate::cli::config::load_providers(&merged)?;
    let mut names: Vec<&String> = providers.keys().collect();
    names.sort();

    if json {
        let rows: Vec<serde_json::Value> = names
            .iter()
            .map(|name| {
                let provider = &providers[*name];
                serde_json::json!({
                    "name": name,
                    "enabled": provider.enabled,
                    "target": provider.default_target(),
                    "plugin": provider.plugin,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "providers": rows }));
        return Ok(0);
    }

    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading("providers"));
    for name in names {
        let provider = &providers[name];
        let state = if provider.enabled {
            sheet.green("enabled")
        } else {
            sheet.dim("disabled")
        };
        let plugin = provider
            .plugin
            .as_ref()
            .map(|plugin| sheet.dim(&format!("  plugin: {plugin}")))
            .unwrap_or_default();
        println!(
            "   {:<12} {state:<10} {}{plugin}",
            sheet.bold(name),
            sheet.dim(provider.default_target()),
        );
    }
    Ok(0)
}

fn set_enabled(name: &str, enabled: bool, json: bool) -> Result<i32, Error> {
    set_enabled_at(Path::new("."), name, enabled, json)
}

fn set_enabled_at(root: &Path, name: &str, enabled: bool, json: bool) -> Result<i32, Error> {
    let merged = crate::cli::config::load_merged_config(root)?;
    let providers = crate::cli::config::load_providers(&merged)?;
    if !providers.contains_key(name) {
        let mut known: Vec<&String> = providers.keys().collect();
        known.sort();
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "Rune does not recognize provider '{name}'. Known providers: {}.",
                known
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with_code("provider.unknown")
        .with_fix_command("rune provider"));
    }

    let config_path = root.join("config.yaml");
    let mut document: serde_yaml::Mapping = if config_path.is_file() {
        let content = crate::cli::config::read_file(&config_path).map_err(|error| {
            Error::new(error.kind(), error.message())
                .with_code("provider.config_unreadable")
                .with_fix_command(config_check_command(root))
        })?;
        serde_yaml::from_str(&content).map_err(|error| {
            Error::new(ErrorKind::Config, format!("config.yaml: {error}"))
                .with_code("provider.config_invalid")
                .with_fix_command(config_check_command(root))
        })?
    } else {
        serde_yaml::Mapping::new()
    };

    let providers_key = serde_yaml::Value::from("providers");
    let provider_entry = document
        .entry(providers_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let Some(provider_map) = provider_entry.as_mapping_mut() else {
        return Err(Error::new(
            ErrorKind::Config,
            "config.yaml providers: is not a map".to_string(),
        )
        .with_code("provider.config_invalid")
        .with_fix_command(config_check_command(root)));
    };
    let entry = provider_map
        .entry(serde_yaml::Value::from(name))
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let Some(entry_map) = entry.as_mapping_mut() else {
        return Err(Error::new(
            ErrorKind::Config,
            format!("config.yaml providers.{name}: is not a map"),
        )
        .with_code("provider.config_invalid")
        .with_fix_command(config_check_command(root)));
    };
    entry_map.insert(
        serde_yaml::Value::from("enabled"),
        serde_yaml::Value::from(enabled),
    );

    let mut rendered = serde_yaml::to_string(&document).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("Rune cannot render config.yaml: {error}"),
        )
        .with_code("provider.config_render_failed")
        .with_fix_command(config_check_command(root))
    })?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    crate::cli::config::write_atomic(&config_path, &rendered).map_err(|error| {
        Error::new(error.kind(), error.message())
            .with_code("provider.config_write_failed")
            .with_fix_command(format!(
                "ls -ld -- {}",
                crate::cli::shell_quote(&crate::cli::resolved_path(root).to_string_lossy())
            ))
    })?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "provider": name, "enabled": enabled })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        let state = if enabled { "enabled" } else { "disabled" };
        println!("{}", sheet.ok(&format!("{name} {state} in ./config.yaml")));
    }
    Ok(0)
}

fn config_check_command(root: &Path) -> String {
    format!(
        "cd {} && rune config check --scope source",
        crate::cli::shell_quote(&crate::cli::resolved_path(root).to_string_lossy())
    )
}

#[cfg(test)]
mod tests;
