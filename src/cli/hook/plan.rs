//! The compiled plan on disk. `rune install` writes one plan generation
//! per registration: the plan narrowed to that registration's harness and
//! native event, in `<state>/hooks/plans/<digest>.json`, named by the
//! digest of its text. The registration names its generation, and no
//! install rewrites one, so a registration reads the plan it was installed
//! with until the tables change. `rune hook list --write` writes the whole
//! plan to `<state>/hooks/plan.json` for a machine that installs nothing,
//! and `rune hook run` reads that file when no `--plan` names another. The
//! dispatcher reads only these files, so a config edit changes nothing
//! until the next install.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::error::{Error, ErrorKind};
use rune::hooks::{Harness, Plan};
use sha2::{Digest, Sha256};

use crate::cli::state;

/// The whole plan's path, made absolute against the current directory:
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

/// The directory of plan generations, absolute: a registration carries a
/// generation's path to a harness that runs elsewhere.
pub(crate) fn generations_dir() -> Result<PathBuf, Error> {
    let dir = state::dir()?.join("hooks").join("plans");
    std::path::absolute(&dir).map_err(|error| Error::io(format!("{}: {error}", dir.display())))
}

/// Whether an earlier install left a generation in `dir`.
pub(crate) fn has_generations(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// One registration's plan: the compiled plan narrowed to one harness and
/// native event, and the file it lives in, named by the digest of its
/// text. An event whose plan did not change keeps its file, so its
/// registration keeps its command and Codex keeps trusting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Generation {
    pub path: PathBuf,
    pub text: String,
}

/// The generation of the registration of `event` on `harness`, or `None`
/// when the plan does not subscribe to it.
pub(crate) fn generation(
    dir: &Path,
    plan: &Plan,
    harness: Harness,
    event: &str,
) -> Option<Generation> {
    let planned = plan.event(harness, event)?.clone();
    let narrowed = Plan {
        version: plan.version,
        harnesses: BTreeMap::from([(harness, BTreeMap::from([(event.to_string(), planned)]))]),
        ..Plan::default()
    };
    let text = serde_json::to_string_pretty(&narrowed).ok()?;
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    Some(Generation {
        path: dir.join(format!("{digest}.json")),
        text,
    })
}

/// Write a generation unless it is there already. An existing generation
/// keeps its bytes and only gets a fresh modification time, so the
/// warning that the config is newer than the plan still means an edit no
/// install has applied. A file under the name that holds other bytes is
/// damaged, and is replaced.
pub(crate) fn write_generation(generation: &Generation) -> Result<(), Error> {
    if fs::read_to_string(&generation.path).is_ok_and(|text| text == generation.text) {
        return fs::File::options()
            .write(true)
            .open(&generation.path)
            .and_then(|file| file.set_modified(SystemTime::now()))
            .map_err(|error| Error::io(format!("{}: {error}", generation.path.display())));
    }
    super::publish::publish(&[super::publish::Staged {
        target: generation.path.clone(),
        text: generation.text.clone(),
        new_mode: Some(0o600),
    }])
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
    use rune::hooks::Hooks;

    fn plan(yaml: &str) -> Plan {
        serde_yaml::from_str::<Hooks>(yaml)
            .unwrap()
            .compile()
            .unwrap()
    }

    #[test]
    fn a_generation_holds_one_event_and_is_named_by_its_content() {
        let dir = Path::new("/state/hooks/plans");
        let one = plan(
            "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    on_failure: deny\n  - id: cap\n    exec: [cap]\n    events: [session.end]\n",
        );
        let first = generation(dir, &one, Harness::Claude, "PreToolUse").unwrap();
        let read: Plan = serde_json::from_str(&first.text).unwrap();
        assert!(read.event(Harness::Claude, "PreToolUse").is_some());
        assert!(read.event(Harness::Claude, "SessionEnd").is_none());
        assert!(generation(dir, &one, Harness::Claude, "Stop").is_none());
        // Another event changes; this one's generation, and so its
        // registration's command, stays the same.
        let two = plan(
            "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    on_failure: deny\n  - id: cap\n    exec: [cap, --more]\n    events: [session.end]\n",
        );
        assert_eq!(
            generation(dir, &two, Harness::Claude, "PreToolUse"),
            Some(first)
        );
        assert_ne!(
            generation(dir, &one, Harness::Claude, "SessionEnd"),
            generation(dir, &two, Harness::Claude, "SessionEnd")
        );
    }

    #[test]
    fn writing_a_generation_never_changes_another() {
        let dir = tempfile::tempdir().unwrap();
        let old = plan(
            "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    on_failure: deny\n",
        );
        let new = plan("handlers:\n  - id: dcg\n    exec: [watch]\n    events: [tool.before]\n");
        let first = generation(dir.path(), &old, Harness::Claude, "PreToolUse").unwrap();
        let second = generation(dir.path(), &new, Harness::Claude, "PreToolUse").unwrap();
        write_generation(&first).unwrap();
        write_generation(&second).unwrap();
        write_generation(&first).unwrap();
        assert_ne!(first.path, second.path);
        assert_eq!(fs::read_to_string(&first.path).unwrap(), first.text);
        assert_eq!(fs::read_to_string(&second.path).unwrap(), second.text);
    }
}
