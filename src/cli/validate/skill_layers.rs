//! Strict source lint without deployment or external checkers.

use rune::error::Error;
use rune::provider::ProviderConfig;
use rune::skill_readiness::layers::{
    SourceLayerFinding, SourceLayerReport, inspect_source_with_providers,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn execute(source: &str, json: bool) -> Result<i32, Error> {
    let requested = Path::new(source);
    let mut report = SourceLayerReport {
        version: "rune-skill-source-layers/v1".into(),
        root: requested.display().to_string(),
        checked: 0,
        findings: Vec::new(),
        valid: false,
    };
    match requested.canonicalize() {
        Ok(root) => {
            report.root = root.display().to_string();
            inspect_selected(&root, &mut report);
        }
        Err(error) => report
            .findings
            .push(input_failure(requested, &error.to_string())),
    }
    if report.checked == 0 && report.findings.is_empty() {
        report
            .findings
            .push(input_failure(requested, "no skill files were checked"));
    }
    report.findings.sort_by(|left, right| {
        (
            &left.layer,
            &left.path,
            left.line,
            &left.code,
            &left.token,
            &left.message,
        )
            .cmp(&(
                &right.layer,
                &right.path,
                right.line,
                &right.code,
                &right.token,
                &right.message,
            ))
    });
    report.valid = report.checked > 0 && report.findings.is_empty();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| Error::io(error.to_string()))?
        );
    } else {
        for finding in &report.findings {
            println!(
                "{}:{}: {} [{}] {}",
                finding.path, finding.line, finding.code, finding.layer, finding.message
            );
        }
        println!("Checked {} skill layer files.", report.checked);
    }
    Ok(i32::from(!report.valid))
}

fn inspect_selected(root: &Path, report: &mut SourceLayerReport) {
    match selected_skills(root) {
        Ok(skills) => {
            for skill in skills {
                match source_configuration(&skill, root) {
                    Ok(config) => {
                        let mut inspected = inspect_source_with_providers(
                            &skill,
                            &config.models,
                            &config.providers,
                        );
                        for finding in &mut inspected.findings {
                            finding.path = skill.join(&finding.path).display().to_string();
                        }
                        report.checked += inspected.checked;
                        report.findings.append(&mut inspected.findings);
                    }
                    Err(error) => report.findings.push(input_failure(&skill, &error)),
                }
            }
        }
        Err(error) => report.findings.push(input_failure(root, &error)),
    }
}

fn input_failure(path: &Path, message: &str) -> SourceLayerFinding {
    SourceLayerFinding {
        code: "SL000_INVALID_INPUT".into(),
        layer: "input".into(),
        path: path.display().to_string(),
        line: 1,
        token: String::new(),
        message: message.into(),
    }
}

fn selected_skills(root: &Path) -> Result<Vec<PathBuf>, String> {
    if root.join("SKILL.md").is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    let modules = if rune::deck::is_deck(root) {
        rune::deck::load(root)?
            .entries
            .into_iter()
            .map(|entry| entry.root)
            .collect()
    } else if root.join("module.yaml").is_file() {
        vec![root.to_path_buf()]
    } else {
        return Err("select a skill folder, module, or deck".into());
    };
    let mut skills = Vec::new();
    for module in modules {
        let directory = module.join("skills");
        let metadata = match fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.to_string()),
        };
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "skills directory is a symlink: {}",
                directory.display()
            ));
        }
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink() {
                return Err(format!(
                    "skill selection contains a symlink: {}",
                    entry.path().display()
                ));
            }
            if kind.is_dir() {
                skills.push(entry.path());
            }
        }
    }
    skills.sort();
    skills.dedup();
    Ok(skills)
}

struct SourceConfiguration {
    models: HashMap<String, Vec<String>>,
    providers: HashMap<String, ProviderConfig>,
}

fn source_configuration(skill: &Path, selected: &Path) -> Result<SourceConfiguration, String> {
    let root = configuration_root(skill, selected);
    let authored =
        super::super::config::load_merged_config(root).map_err(|error| error.to_string())?;
    // Strict inspection cannot use the ordinary loader's compatibility fallback.
    let merged = rune::yaml::deep_merge(include_str!("../../../defaults.yaml"), &authored)?;
    let providers = rune::provider::load_providers(&merged)?;
    Ok(SourceConfiguration {
        models: models_for(root)?,
        providers,
    })
}

fn configuration_root<'a>(skill: &'a Path, selected: &'a Path) -> &'a Path {
    if rune::deck::is_deck(selected) || selected.join("module.yaml").is_file() {
        selected
    } else {
        skill
            .ancestors()
            .find(|directory| {
                directory.join("module.yaml").is_file() || rune::deck::is_deck(directory)
            })
            .unwrap_or(skill)
    }
}

fn models_for(module: &Path) -> Result<HashMap<String, Vec<String>>, String> {
    let config = module.join("config/models.yaml");
    // A malformed authored registry must not fall back to embedded defaults.
    let models = if config.try_exists().map_err(|error| error.to_string())? {
        let text = fs::read_to_string(&config).map_err(|error| error.to_string())?;
        rune::provider::load_models(&text)?
    } else {
        super::super::config::load_models(module)
    };
    if models.is_empty()
        || models.values().any(Vec::is_empty)
        || models
            .values()
            .flatten()
            .any(|model| model.trim().is_empty())
    {
        return Err("model registry is empty or contains an empty identifier".into());
    }
    Ok(models)
}
