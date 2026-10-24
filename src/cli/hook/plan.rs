//! The compiled plan on disk: `<state>/hooks/plan.json`, written by
//! `rune install` (and by `rune hook list --write` for a machine that
//! installs nothing), read by `rune hook run` at the path its registration
//! names. The dispatcher reads only this file, so a config edit changes
//! nothing until the next install.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::error::{Error, ErrorKind};
use rune::hooks::Plan;

use crate::cli::state;

/// The plan's path, made absolute against the current directory: the
/// registration carries it to a harness that runs elsewhere, and
/// `RUNE_STATE_DIR` may be relative.
pub(crate) fn path() -> Result<PathBuf, Error> {
    let path = state::dir()?.join("hooks").join("plan.json");
    std::path::absolute(&path).map_err(|error| Error::io(format!("{}: {error}", path.display())))
}

pub(crate) fn write(plan: &Plan) -> Result<PathBuf, Error> {
    let path = path()?;
    let text = serde_json::to_string_pretty(plan).map_err(|error| Error::io(error.to_string()))?;
    let _lock = super::publish::Lock::acquire(std::slice::from_ref(&path))?;
    super::publish::publish(&[super::publish::Staged {
        target: path.clone(),
        text,
        new_mode: None,
    }])?;
    Ok(path)
}

/// The plan for the moment between publishing the plan and the tables:
/// the new plan plus every event the old plan has and the new one drops.
/// A registration of either install finds its event in it, so no guarded
/// event falls to its registration's fallback while the tables change.
pub(crate) fn bridge(old: &Plan, new: &Plan) -> Plan {
    let mut bridge = new.clone();
    for (harness, events) in &old.harnesses {
        let target = bridge.harnesses.entry(*harness).or_default();
        for (name, event) in events {
            target.entry(name.clone()).or_insert_with(|| event.clone());
        }
    }
    bridge
}

/// The plan at `path`, and a warning when the user config is newer than
/// it. The registration names the path install wrote, so a harness that
/// runs the dispatcher with another state directory reads the same plan.
pub(crate) fn read_at(path: &Path) -> Result<(Plan, Option<String>), Error> {
    let path = path.to_path_buf();
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

#[cfg(test)]
mod tests {
    use super::*;
    use rune::hooks::{Harness, Hooks};

    fn plan(yaml: &str) -> Plan {
        serde_yaml::from_str::<Hooks>(yaml)
            .unwrap()
            .compile()
            .unwrap()
    }

    #[test]
    fn the_bridge_keeps_every_event_of_both_plans() {
        let old = plan(
            "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    on_failure: deny\n",
        );
        let new = plan("handlers:\n  - id: cap\n    exec: [cap]\n    events: [session.end]\n");
        let both = bridge(&old, &new);
        assert!(both.event(Harness::Claude, "PreToolUse").is_some());
        assert!(both.event(Harness::Claude, "SessionEnd").is_some());
        assert_eq!(bridge(&new, &new), new, "no dropped event, no bridge");
    }
}
