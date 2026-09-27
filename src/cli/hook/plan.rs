//! The compiled plan on disk: `<state>/hooks/plan.json`, written by
//! `rune install` (and by `rune hook list --write` for a machine that
//! installs nothing), read by `rune hook run`. The dispatcher reads only
//! this file, so a config edit changes nothing until the next install.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use rune::error::{Error, ErrorKind};
use rune::hooks::Plan;

use crate::cli::state;

pub(crate) fn path() -> Result<PathBuf, Error> {
    Ok(state::dir()?.join("hooks").join("plan.json"))
}

pub(crate) fn write(plan: &Plan) -> Result<PathBuf, Error> {
    let path = path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| Error::io(error.to_string()))?;
    }
    let text = serde_json::to_string_pretty(plan).map_err(|error| Error::io(error.to_string()))?;
    let staged = path.with_extension("json.new");
    fs::write(&staged, text).map_err(|error| Error::io(error.to_string()))?;
    fs::rename(&staged, &path).map_err(|error| Error::io(error.to_string()))?;
    Ok(path)
}

/// The plan, and a warning when the user config is newer than it.
pub(crate) fn read() -> Result<(Plan, Option<String>), Error> {
    let path = path()?;
    let text = fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                "no compiled hook plan at {}: run `rune install` ({error})",
                path.display()
            ),
        )
        .with_code("hooks.no_plan")
    })?;
    let plan: Plan = serde_json::from_str(&text).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("{} is not a hook plan: {error}", path.display()),
        )
    })?;
    let warning = newer_config(&path).map(|config| {
        format!(
            "{} is newer than the compiled plan; run `rune install` to apply it",
            config.display()
        )
    });
    Ok((plan, warning))
}

fn newer_config(plan: &PathBuf) -> Option<PathBuf> {
    let config = rune::ontology::config_dir().ok()?.join("config.yaml");
    let plan_time = fs::metadata(plan).and_then(|m| m.modified()).ok()?;
    let config_time = fs::metadata(&config)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    (config_time > plan_time).then_some(config)
}
