//! Render the plan into the harness hook tables: one dispatcher entry per
//! subscribed event in Claude's `settings.json` and Codex's `hooks.json`.
//! rune owns only the entries it wrote (known by their command), keeps
//! every other entry, adopts the entries of the tools its adapters
//! replace and of the legacy `sd hook` dispatcher, and stops on an owned
//! entry a hand changed. The inventory lives in a manifest under the rune
//! state directory.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use rune::error::{Error, ErrorKind};
use rune::hooks::{Harness, Plan};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// One entry of a harness table as rune records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Recorded {
    pub event: String,
    pub command: String,
    /// The hash of the whole registration rune wrote (owned entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Which adapter or migration adopted it (adopted entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// The file the entry lives in (foreign and adopted entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HarnessManifest {
    #[serde(default)]
    pub owned: Vec<Recorded>,
    #[serde(default)]
    pub foreign: Vec<Recorded>,
    #[serde(default)]
    pub adopted: Vec<Recorded>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub harnesses: BTreeMap<Harness, HarnessManifest>,
}

/// What one run of the installer found and did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Report {
    pub written: Vec<(Harness, String)>,
    pub unchanged: Vec<(Harness, String)>,
    pub adopted: Vec<(Harness, String, String)>,
    pub foreign: Vec<(Harness, String, String)>,
    pub unsupported: Vec<String>,
    pub drift: Vec<String>,
    pub trust: Vec<String>,
    pub legacy_toml: Vec<String>,
}

impl Report {
    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        for (harness, event) in &self.written {
            let _ = writeln!(out, "{harness}: {event} registered");
        }
        for (harness, event, command) in &self.adopted {
            let _ = writeln!(out, "{harness}: {event} adopted `{command}`");
        }
        for (harness, event, command) in &self.foreign {
            let _ = writeln!(out, "{harness}: {event} keeps foreign `{command}`");
        }
        for line in &self.unsupported {
            let _ = writeln!(out, "unsupported: {line}");
        }
        for line in &self.legacy_toml {
            let _ = writeln!(out, "legacy: {line}");
        }
        for line in &self.drift {
            let _ = writeln!(out, "drift: {line}");
        }
        for line in &self.trust {
            let _ = writeln!(out, "trust: {line}");
        }
        out
    }
}

/// The dispatcher command an entry must start with to be rune's.
fn dispatcher(rune: &str, harness: Harness, event: &str) -> String {
    format!("{rune} hook run --harness {harness} --native-event {event}")
}

fn is_owned(command: &str, rune_names: &[&str]) -> bool {
    rune_names
        .iter()
        .any(|name| command.starts_with(&format!("{name} hook run --harness ")))
}

/// The registration patterns install adopts: the legacy dispatcher always,
/// a tool's own entry when the plan carries the adapter that replaces it.
fn adoption(command: &str, plan_ids: &[String]) -> Option<&'static str> {
    let trimmed = command.trim();
    if trimmed.contains("/.sd/hook ") || trimmed.starts_with("sd hook ") {
        return Some("legacy sd hook");
    }
    let tail = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if plan_ids.iter().any(|id| id == "dcg")
        && (trimmed == "dcg" || tail == "dcg" || tail.starts_with("dcg "))
    {
        return Some("dcg");
    }
    if plan_ids.iter().any(|id| id == "git-ai") && trimmed.contains("git-ai checkpoint") {
        return Some("git-ai");
    }
    if plan_ids.iter().any(|id| id == "author-identity")
        && tail.starts_with("claude-agent-identity")
    {
        return Some("author-identity");
    }
    if plan_ids.iter().any(|id| id == "tmux-status") && tail.starts_with("claude-tmux-status") {
        return Some("tmux-status");
    }
    if plan_ids.iter().any(|id| id == "rtk")
        && (trimmed.starts_with("rtk hook") || tail.starts_with("rtk hook"))
    {
        return Some("rtk");
    }
    None
}

fn hash_value(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// The entry rune writes for one event.
fn rendered(rune: &str, harness: Harness, event: &str, budget_ms: u64) -> Value {
    let timeout = budget_ms.div_ceil(1_000).max(1);
    json!({
        "hooks": [{
            "type": "command",
            "command": dispatcher(rune, harness, event),
            "timeout": timeout,
        }]
    })
}

pub(crate) fn manifest_path() -> Result<PathBuf, Error> {
    Ok(crate::cli::state::dir()?
        .join("hooks")
        .join("manifest.json"))
}

fn read_manifest(path: &Path) -> Manifest {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn table_path(home: &Path, harness: Harness) -> PathBuf {
    match harness {
        Harness::Claude => home.join(".claude").join("settings.json"),
        Harness::Codex => home.join(".codex").join("hooks.json"),
        _ => home.join(format!(".{harness}")).join("hooks.json"),
    }
}

fn read_table(path: &Path) -> Result<Map<String, Value>, Error> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| Error::io(format!("{}: {error}", path.display())))?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("{} is not JSON: {error}", path.display()),
        )
    })?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(Error::new(
            ErrorKind::Parse,
            format!("{} is not a JSON object", path.display()),
        )),
    }
}

fn write_table(path: &Path, table: &Map<String, Value>) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| Error::io(error.to_string()))?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(table.clone()))
        .map_err(|error| Error::io(error.to_string()))?;
    let staged = path.with_extension("json.new");
    fs::write(&staged, format!("{text}\n")).map_err(|error| Error::io(error.to_string()))?;
    fs::rename(&staged, path).map_err(|error| Error::io(error.to_string()))?;
    Ok(())
}

/// Install or check the tables of every registered harness under `home`.
/// `rune` is the dispatcher's own path as the tables will call it. One
/// pass over every event of every registered harness: long, and linear.
#[allow(clippy::too_many_lines)]
pub(crate) fn apply(
    plan: &Plan,
    home: &Path,
    rune: &str,
    manifest_path: &Path,
    check: bool,
    dry_run: bool,
) -> Result<Report, Error> {
    let mut report = Report::default();
    let mut manifest = read_manifest(manifest_path);
    manifest.version = 1;
    let rune_names: Vec<&str> = vec![rune, "rune"];
    let plan_ids: Vec<String> = plan
        .harnesses
        .values()
        .flat_map(|events| events.values())
        .flat_map(|event| event.handlers.iter().map(|h| h.id.clone()))
        .collect();
    for gap in &plan.unsupported {
        if Harness::REGISTERED.contains(&gap.harness) {
            report.unsupported.push(format!(
                "{} has no {} for {}",
                gap.harness,
                gap.event,
                gap.handlers.join(", ")
            ));
        }
    }
    for harness in Harness::REGISTERED {
        let path = table_path(home, harness);
        let mut table = read_table(&path)?;
        let previous = manifest
            .harnesses
            .get(&harness)
            .cloned()
            .unwrap_or_default();
        let mut next = HarnessManifest::default();
        let mut changed = false;
        let mut hooks: Map<String, Value> = table
            .get("hooks")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let wanted: BTreeMap<String, Value> = plan
            .harnesses
            .get(&harness)
            .map(|events| {
                events
                    .iter()
                    .map(|(name, event)| {
                        (name.clone(), rendered(rune, harness, name, event.budget_ms))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Every event the table already has, plus every event rune wants.
        let mut names: Vec<String> = hooks.keys().cloned().collect();
        for name in wanted.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        for name in names {
            let entries: Vec<Value> = hooks
                .get(&name)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut kept: Vec<Value> = Vec::new();
            let mut owned_seen = false;
            for group in entries {
                let commands: Vec<String> = group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(|h| {
                                h.get("command").and_then(Value::as_str).map(str::to_string)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mine = commands.iter().any(|c| is_owned(c, &rune_names));
                if mine {
                    owned_seen = true;
                    let Some(want) = wanted.get(&name) else {
                        // rune wrote it, rune no longer wants it.
                        changed = true;
                        report.written.push((harness, format!("{name} removed")));
                        continue;
                    };
                    let hash = hash_value(&group);
                    let want_hash = hash_value(want);
                    let known = previous
                        .owned
                        .iter()
                        .any(|r| r.event == name && r.hash.as_deref() == Some(hash.as_str()));
                    if hash == want_hash {
                        kept.push(group);
                        report.unchanged.push((harness, name.clone()));
                    } else if known {
                        kept.push(want.clone());
                        changed = true;
                        report.written.push((harness, name.clone()));
                    } else {
                        return Err(Error::new(
                            ErrorKind::Config,
                            format!(
                                "{}: the rune entry for {name} was changed by hand; restore it or remove it, then install again",
                                path.display()
                            ),
                        )
                        .with_code("hooks.conflict"));
                    }
                    continue;
                }
                // A foreign group: adopt the commands rune replaces, keep the rest.
                let mut remaining = group.clone();
                let mut list: Vec<Value> = remaining
                    .get("hooks")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let before = list.len();
                list.retain(|h| {
                    let command = h.get("command").and_then(Value::as_str).unwrap_or_default();
                    if let Some(by) = adoption(command, &plan_ids) {
                        next.adopted.push(Recorded {
                            event: name.clone(),
                            command: command.to_string(),
                            hash: None,
                            by: Some(by.to_string()),
                            source: Some(path.display().to_string()),
                        });
                        report
                            .adopted
                            .push((harness, name.clone(), command.to_string()));
                        return false;
                    }
                    {
                        {
                            next.foreign.push(Recorded {
                                event: name.clone(),
                                command: command.to_string(),
                                hash: None,
                                by: None,
                                source: Some(path.display().to_string()),
                            });
                            report
                                .foreign
                                .push((harness, name.clone(), command.to_string()));
                            true
                        }
                    }
                });
                if list.len() != before {
                    changed = true;
                }
                if list.is_empty() {
                    continue;
                }
                if let Some(map) = remaining.as_object_mut() {
                    map.insert("hooks".into(), Value::Array(list));
                }
                kept.push(remaining);
            }
            if let Some(want) = wanted.get(&name) {
                if !owned_seen {
                    kept.push(want.clone());
                    changed = true;
                    report.written.push((harness, name.clone()));
                }
                next.owned.push(Recorded {
                    event: name.clone(),
                    command: dispatcher(rune, harness, &name),
                    hash: Some(hash_value(want)),
                    by: None,
                    source: None,
                });
            }
            if kept.is_empty() {
                hooks.remove(&name);
            } else {
                hooks.insert(name, Value::Array(kept));
            }
        }
        if harness == Harness::Codex {
            legacy_codex_toml(home, &plan_ids, &mut report);
            if !wanted.is_empty() {
                report.trust.push(
                    "codex binds trust to each hook's hash: run `/hooks` in codex and accept the rune entries, or they stay inactive"
                        .to_string(),
                );
            }
        }
        if check {
            if changed {
                report
                    .drift
                    .push(format!("{}: differs from the plan", path.display()));
            }
            continue;
        }
        if changed && !dry_run {
            table.insert("hooks".into(), Value::Object(hooks));
            write_table(&path, &table)?;
        }
        manifest.harnesses.insert(harness, next);
    }
    if !check && !dry_run {
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent).map_err(|error| Error::io(error.to_string()))?;
        }
        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|error| Error::io(error.to_string()))?;
        fs::write(manifest_path, format!("{text}\n"))
            .map_err(|error| Error::io(error.to_string()))?;
    }
    Ok(report)
}

/// Codex also reads `[[hooks.<Event>]]` from `config.toml`. That file is
/// rendered by the dotfiles and carries Codex's trust hashes, so rune
/// names the legacy lines instead of rewriting the file.
fn legacy_codex_toml(home: &Path, plan_ids: &[String], report: &mut Report) {
    let path = home.join(".codex").join("config.toml");
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("command") {
            continue;
        }
        let Some(command) = trimmed
            .split_once('=')
            .map(|(_, v)| v.trim().trim_matches('"'))
        else {
            continue;
        };
        if let Some(by) = adoption(command, plan_ids) {
            report.legacy_toml.push(format!(
                "{}:{}: `{command}` is now the {by} handler; remove it from config.toml by hand, rune does not rewrite that file",
                path.display(),
                index + 1
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::hooks::Hooks;

    fn plan() -> Plan {
        serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: dcg\n    exec: [rune-hook-dcg]\n    events: [tool.before]\n    on_failure: deny\n  - id: git-ai\n    exec: [rune-hook-git-ai]\n    events: [tool.before, tool.after]\n  - id: capture\n    exec: [rune-hook-capture]\n    events: [session.end]\n",
        )
        .unwrap()
        .compile()
        .unwrap()
    }

    fn settings(home: &Path, text: &str) -> PathBuf {
        let path = home.join(".claude").join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    const LIVE: &str = r#"{"model":"opus","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/Users/x/.local/bin/dcg"}]},{"hooks":[{"type":"command","command":"sh /Users/x/.sd/hook PreToolUse"}]},{"matcher":"*","hooks":[{"type":"command","command":"/Users/x/.git-ai/bin/git-ai checkpoint claude --hook-input stdin"}]}],"Notification":[{"hooks":[{"type":"command","command":"cmux hooks claude Notification"}]}]}}"#;

    #[test]
    fn foreign_entries_survive_predecessors_are_adopted_and_one_rune_entry_per_event() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), LIVE);
        let manifest = home.path().join("state").join("manifest.json");
        let report = apply(&plan(), home.path(), "/opt/rune", &manifest, false, false).unwrap();
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(table["model"], "opus", "the rest of the file is untouched");
        let pre = table["hooks"]["PreToolUse"].as_array().unwrap();
        let commands: Vec<&str> = pre
            .iter()
            .flat_map(|g| g["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert_eq!(
            commands,
            ["/opt/rune hook run --harness claude --native-event PreToolUse"],
            "dcg, the legacy dispatcher, and git-ai were adopted"
        );
        assert_eq!(
            table["hooks"]["Notification"][0]["hooks"][0]["command"],
            "cmux hooks claude Notification",
            "a foreign entry on an event rune does not want stays"
        );
        assert_eq!(table["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 1);
        assert_eq!(report.adopted.len(), 3);
        assert!(
            report
                .foreign
                .iter()
                .any(|(_, e, c)| e == "Notification" && c.starts_with("cmux"))
        );
        let manifest: Manifest =
            serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
        let claude = &manifest.harnesses[&Harness::Claude];
        assert_eq!(claude.owned.len(), 3, "PreToolUse, PostToolUse, SessionEnd");
        assert_eq!(claude.foreign.len(), 1);
        assert_eq!(claude.adopted.len(), 3);
    }

    #[test]
    fn a_second_install_changes_nothing_and_a_hand_edit_stops_it() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), LIVE);
        let manifest = home.path().join("state").join("manifest.json");
        apply(&plan(), home.path(), "/opt/rune", &manifest, false, false).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        let report = apply(&plan(), home.path(), "/opt/rune", &manifest, false, false).unwrap();
        assert!(report.written.is_empty(), "{report:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        let edited = first.replace("\"timeout\": 3", "\"timeout\": 99");
        assert_ne!(edited, first);
        fs::write(&path, &edited).unwrap();
        let error = apply(&plan(), home.path(), "/opt/rune", &manifest, false, false).unwrap_err();
        assert!(error.to_string().contains("changed by hand"), "{error}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            edited,
            "nothing written"
        );
        let report = apply(&plan(), home.path(), "/opt/rune", &manifest, true, false);
        assert!(report.is_err(), "check reports the same conflict");
    }

    #[test]
    fn a_plan_change_replaces_a_known_entry_and_check_reports_drift() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), "{}");
        let manifest = home.path().join("state").join("manifest.json");
        apply(&plan(), home.path(), "/opt/rune", &manifest, false, false).unwrap();
        let bigger: Plan = serde_yaml::from_str::<Hooks>(
            "events:\n  session.end: {budget_ms: 1400}\nhandlers:\n  - id: capture\n    exec: [x]\n    events: [session.end]\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let report = apply(&bigger, home.path(), "/opt/rune", &manifest, true, false).unwrap();
        assert!(
            report.drift.iter().any(|d| d.contains("settings.json")),
            "{report:?}"
        );
        let report = apply(&bigger, home.path(), "/opt/rune", &manifest, false, false).unwrap();
        assert!(report.written.iter().any(|(_, e)| e == "SessionEnd"));
        assert!(
            report
                .written
                .iter()
                .any(|(_, e)| e == "PreToolUse removed")
        );
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(table["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 2);
        assert!(table["hooks"].get("PreToolUse").is_none());
    }

    #[test]
    fn codex_legacy_toml_is_named_not_rewritten_and_trust_is_printed() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex");
        fs::create_dir_all(&codex).unwrap();
        fs::write(
            codex.join("config.toml"),
            "[[hooks.PreToolUse]]\n[[hooks.PreToolUse.hooks]]\ncommand = \"sh /Users/x/.sd/hook PreToolUse\"\ntype = \"command\"\n",
        )
        .unwrap();
        let manifest = home.path().join("state").join("manifest.json");
        let report = apply(&plan(), home.path(), "/opt/rune", &manifest, false, false).unwrap();
        assert!(
            report.legacy_toml[0].contains("config.toml:3"),
            "{report:?}"
        );
        assert!(report.trust[0].contains("/hooks"));
        let hooks: Value =
            serde_json::from_str(&fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
        assert_eq!(
            hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "/opt/rune hook run --harness codex --native-event PreToolUse"
        );
        assert!(
            hooks["hooks"].get("Notification").is_none(),
            "codex has no notification event"
        );
    }
}
