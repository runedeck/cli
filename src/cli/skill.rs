//! Ship rune's own agent skill: a reference document that teaches AI coding
//! agents how to drive the CLI. `show` prints it; `install` places it in a
//! harness skills directory.

use rune::error::{Error, ErrorKind};
use std::fs;
use std::path::{Path, PathBuf};

const SKILL_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/skill/SKILL.md"
));

fn rendered() -> String {
    SKILL_TEMPLATE.replace("${VERSION}", env!("CARGO_PKG_VERSION"))
}

pub fn show() -> i32 {
    let sheet = crate::cli::style::Sheet::detect(false);
    let content = rendered();
    if let Some((frontmatter, body)) = rune::parse::split_frontmatter(&content) {
        println!("{}", sheet.heading("rune skill"));
        for line in frontmatter.lines() {
            match line.split_once(':') {
                Some((key, value)) => {
                    println!("{}", sheet.row(key.trim(), value.trim()));
                }
                None => println!("   {}", sheet.dim(line)),
            }
        }
        println!();
        print!("{body}");
    } else {
        print!("{content}");
    }
    0
}

pub fn install(directory: Option<&str>, json: bool) -> Result<i32, Error> {
    let root = match directory {
        Some(directory) => PathBuf::from(directory),
        None => dirs::home_dir()
            .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))?,
    };
    let content = rendered();
    let mut reports = Vec::new();
    for (provider, target) in enabled_skill_targets(&root)? {
        let base = root.join(&target).join("skills");
        let report = install_into(&base, &content)?;
        reports.push((provider, report));
    }
    if json {
        let rows: Vec<serde_json::Value> = reports
            .iter()
            .map(|(provider, report)| {
                serde_json::json!({
                    "provider": provider,
                    "installed": report.path.display().to_string(),
                    "status": report.status,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "skills": rows }));
        return Ok(0);
    }
    for (provider, report) in &reports {
        println!("{provider}: {} → {}", report.status, report.path.display());
    }
    println!("agents pick the skill up on their next session");
    Ok(0)
}

struct InstallReport {
    path: PathBuf,
    status: &'static str,
}

/// Write the skill into one skills directory. A user-modified file stays
/// protected: the write happens only when the target is absent or carries
/// rune's own previous content shape (the rune frontmatter name).
fn install_into(base: &Path, content: &str) -> Result<InstallReport, Error> {
    let destination = base.join("rune");
    let skill_path = destination.join("SKILL.md");
    fs::create_dir_all(base).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", base.display()),
        )
    })?;
    rune::services::confine::confine_for_write(base, &skill_path)
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    let previous = fs::read_to_string(&skill_path).ok();
    let status = match &previous {
        None => "installed",
        Some(existing) if existing == content => "unchanged",
        Some(existing) if existing.contains("name: rune") => "updated",
        Some(_) => {
            return Ok(InstallReport {
                path: skill_path,
                status: "kept (modified by the user; remove the file to reinstall)",
            });
        }
    };
    if status != "unchanged" {
        fs::create_dir_all(&destination).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", destination.display()),
            )
        })?;
        fs::write(&skill_path, content).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", skill_path.display()),
            )
        })?;
    }
    Ok(InstallReport {
        path: skill_path,
        status,
    })
}

/// The enabled providers and their default target roots, for skill installs
/// under a home base.
fn enabled_skill_targets(root: &Path) -> Result<Vec<(String, String)>, Error> {
    let merged = crate::cli::config::load_merged_config(root)?;
    let providers = crate::cli::config::load_providers(&merged)?;
    let mut targets: Vec<(String, String)> = providers
        .iter()
        .filter(|(_, config)| config.enabled)
        .map(|(name, config)| (name.clone(), config.default_target().to_string()))
        .collect();
    targets.sort();
    Ok(targets)
}

#[cfg(test)]
mod tests;
