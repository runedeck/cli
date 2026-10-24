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
use rune::hooks::{Harness, Mode, OnFailure, Plan, PlannedEvent};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::publish::Staged;
use super::table::Table;

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
    pub warnings: Vec<String>,
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
        for line in &self.warnings {
            let _ = writeln!(out, "warning: {line}");
        }
        out
    }
}

/// The dispatcher command of one event. It names the plan install wrote,
/// so a harness that starts it with another state directory reads the
/// same plan, and it carries the event's strongest failure policy, so a
/// plan that does not load still denies where a guard would have.
fn dispatcher(rune: &str, harness: Harness, event: &str, plan: &str, guarded: bool) -> String {
    let mut command = format!(
        "{} hook run --harness {harness} --native-event {event} --plan {}",
        crate::cli::shell_quote(rune),
        crate::cli::shell_quote(plan)
    );
    if guarded {
        command.push_str(" --on-failure deny");
    }
    command
}

/// Whether any handler of the event denies on failure where the harness
/// can block it.
fn guarded(harness: Harness, event: &PlannedEvent) -> bool {
    event.mode == Mode::Gate
        && event.canonical.is_some_and(|c| c.can_block(harness))
        && event
            .handlers
            .iter()
            .any(|h| h.on_failure == OnFailure::Deny)
}

/// The words of a command line with their quotes removed: enough of the
/// shell to read back what `shell_quote` wrote.
fn words(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(open) if c == open => quote = None,
            Some('"') if c == '\\' => word.extend(chars.next()),
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                started = true;
            }
            None if c == '\\' => {
                word.extend(chars.next());
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    out.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(word);
    }
    out
}

/// The harness and native event a rune dispatcher command serves. Any
/// path to a binary named `rune` counts, so an install from a second
/// build path still finds the first one's entry.
fn dispatcher_of(command: &str) -> Option<(String, String)> {
    let words = words(command);
    let program = Path::new(words.first()?);
    if program.file_name()? != "rune" || words.get(1)? != "hook" || words.get(2)? != "run" {
        return None;
    }
    let value = |flag: &str| {
        words
            .iter()
            .position(|w| w == flag)
            .and_then(|at| words.get(at + 1))
            .cloned()
    };
    Some((value("--harness")?, value("--native-event")?))
}

/// Whether a command is rune's dispatcher for this harness's table.
fn is_owned(command: &str, harness: Harness) -> bool {
    dispatcher_of(command).is_some_and(|(of, _)| of == harness.as_str())
}

/// What adopts the legacy `sd hook` dispatcher.
const LEGACY: &str = "legacy sd hook";

/// The registration patterns install adopts: the legacy dispatcher on an
/// event rune registers, a tool's own entry when the plan carries the
/// adapter that replaces it.
fn adoption(command: &str, plan_ids: &[String]) -> Option<&'static str> {
    let trimmed = command.trim();
    if trimmed.contains("/.sd/hook ") || trimmed.starts_with("sd hook ") {
        return Some(LEGACY);
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

/// The hash of a registration over its keys in sorted order, so the key
/// order a table keeps never reads as a hand edit.
fn hash_value(value: &Value) -> String {
    let mut sorted = value.clone();
    sorted.sort_all_objects();
    let text = serde_json::to_string(&sorted).unwrap_or_default();
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// The entry rune writes for one event. On a guarded Claude event the
/// entry also asks Claude to block when the dispatcher itself fails or
/// overruns its timeout (`onFailure`, Claude Code 2.1.295 and later).
fn rendered(
    rune: &str,
    harness: Harness,
    event: &str,
    planned: &PlannedEvent,
    plan: &str,
) -> Value {
    let timeout = planned.budget_ms.div_ceil(1_000).max(1);
    let guarded = guarded(harness, planned);
    let mut hook = json!({
        "type": "command",
        "command": dispatcher(rune, harness, event, plan, guarded),
        "timeout": timeout,
    });
    if guarded && harness == Harness::Claude {
        hook["onFailure"] = json!("block");
    }
    json!({ "hooks": [hook] })
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

/// The file a table path names: the target of a symbolic link, so the
/// write lands in the owner's file and the link stays a link.
fn resolved(path: &Path) -> Result<PathBuf, Error> {
    if path.is_symlink() {
        fs::canonicalize(path).map_err(|error| {
            Error::io(format!(
                "{} is a link rune cannot follow: {error}",
                path.display()
            ))
        })
    } else {
        Ok(path.to_path_buf())
    }
}

/// What an install found, and the files it would write: the changed
/// tables, then the manifest. Nothing is written while it is computed.
#[derive(Debug)]
pub(crate) struct Prepared {
    pub report: Report,
    pub files: Vec<Staged>,
}

/// Install or check the tables of every registered harness under `home`.
/// Every table is read and every conflict found before anything is
/// written, then the tables and the manifest are written as one step.
/// The command adds the plan to the same step; the tests call this.
#[cfg(test)]
pub(crate) fn apply(
    plan: &Plan,
    home: &Path,
    rune: &str,
    plan_path: &str,
    manifest_path: &Path,
    check: bool,
    dry_run: bool,
) -> Result<Report, Error> {
    let prepared = prepare(plan, home, rune, plan_path, manifest_path, check)?;
    if !check && !dry_run {
        super::publish::publish(&prepared.files)?;
    }
    Ok(prepared.report)
}

/// Read every table, find every conflict, and compute what install would
/// write. `rune` is the dispatcher's own path as the tables will call it.
/// One pass over every event of every registered harness: long, and
/// linear.
#[allow(clippy::too_many_lines)]
pub(crate) fn prepare(
    plan: &Plan,
    home: &Path,
    rune: &str,
    plan_path: &str,
    manifest_path: &Path,
    check: bool,
) -> Result<Prepared, Error> {
    let mut files: Vec<Staged> = Vec::new();
    let mut report = Report::default();
    let mut manifest = read_manifest(manifest_path);
    manifest.version = 1;
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
        let file = resolved(&path)?;
        let table = Table::read(&file)?;
        let previous = manifest
            .harnesses
            .get(&harness)
            .cloned()
            .unwrap_or_default();
        let mut next = HarnessManifest::default();
        let mut changed = false;
        let mut rewritten: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let wanted: BTreeMap<String, Value> = plan
            .harnesses
            .get(&harness)
            .map(|events| {
                events
                    .iter()
                    .map(|(name, event)| {
                        (
                            name.clone(),
                            rendered(rune, harness, name, event, plan_path),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Every event the table already has, in its order, plus every
        // event rune wants.
        let mut names: Vec<String> = table.event_names();
        for name in wanted.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        for name in &names {
            let entries = table.groups(name);
            let event = Event {
                harness,
                name,
                path: &path,
                want: wanted.get(name),
                previous: &previous,
                plan_ids: &plan_ids,
            };
            let kept = event.reconcile(entries, &mut next, &mut report)?;
            if kept.changed {
                changed = true;
                rewritten.insert(name.clone(), kept.groups);
            }
        }
        if harness == Harness::Codex {
            legacy_codex_toml(home, &plan_ids, &wanted, &mut report);
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
        if changed {
            files.push(Staged {
                target: file.clone(),
                text: table.render(&names, &rewritten),
                new_mode: Some(0o600),
            });
        }
        manifest.harnesses.insert(harness, next);
    }
    if !check {
        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|error| Error::io(error.to_string()))?;
        files.push(Staged {
            target: manifest_path.to_path_buf(),
            text: format!("{text}\n"),
            new_mode: Some(0o600),
        });
    }
    Ok(Prepared { report, files })
}

/// One native event of one harness table, as install reconciles it.
struct Event<'a> {
    harness: Harness,
    name: &'a str,
    /// The table path as the owner names it, for messages.
    path: &'a Path,
    /// The entry the plan wants here, or `None` when it wants none.
    want: Option<&'a Value>,
    previous: &'a HarnessManifest,
    plan_ids: &'a [String],
}

/// An event's groups after install, and whether they differ from before.
struct Reconciled {
    groups: Vec<Value>,
    changed: bool,
}

impl Event<'_> {
    fn record(&self, command: &str, by: Option<&str>) -> Recorded {
        Recorded {
            event: self.name.to_string(),
            command: command.to_string(),
            hash: None,
            by: by.map(str::to_string),
            source: Some(self.path.display().to_string()),
        }
    }

    fn conflict(&self) -> Error {
        Error::new(
            ErrorKind::Config,
            format!(
                "{}: the rune entry for {} was changed by hand; restore it or remove it, then install again",
                self.path.display(),
                self.name
            ),
        )
        .with_code("hooks.conflict")
    }

    /// Every group of the event, hook by hook. Rune's own command is
    /// checked against its hash and kept, replaced, or removed alone, so
    /// a hook the owner added to the same group stays. Every other hook
    /// is adopted when a handler replaces it, else kept as foreign.
    #[allow(clippy::too_many_lines)]
    fn reconcile(
        &self,
        entries: Vec<Value>,
        next: &mut HarnessManifest,
        report: &mut Report,
    ) -> Result<Reconciled, Error> {
        let (harness, name) = (self.harness, self.name);
        let want_hash = self.want.map(hash_value);
        let mut groups: Vec<Value> = Vec::new();
        let mut changed = false;
        let mut placed = false;
        for group in entries {
            let list: Vec<Value> = group
                .get("hooks")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut remaining: Vec<Value> = Vec::new();
            for hook in list {
                let command = hook
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !is_owned(&command, harness) {
                    let by = adoption(&command, self.plan_ids);
                    if by == Some(LEGACY) && self.want.is_none() {
                        // The legacy dispatcher still serves this event:
                        // no rune entry takes its place, so it stays.
                        report.warnings.push(format!(
                            "{harness}: {name} keeps the legacy `{command}`, because no handler subscribes to {name}; move what it runs into a handler, then remove it by hand"
                        ));
                        next.foreign.push(self.record(&command, None));
                        report.foreign.push((harness, name.to_string(), command));
                        remaining.push(hook);
                    } else if let Some(by) = by {
                        next.adopted.push(self.record(&command, Some(by)));
                        report.adopted.push((harness, name.to_string(), command));
                        changed = true;
                    } else {
                        next.foreign.push(self.record(&command, None));
                        report.foreign.push((harness, name.to_string(), command));
                        remaining.push(hook);
                    }
                    continue;
                }
                // Rune's registration: the group's own fields with this
                // hook alone, so a sibling the owner added is no edit.
                let mut registration = group.clone();
                if let Some(map) = registration.as_object_mut() {
                    map.insert("hooks".into(), json!([hook.clone()]));
                }
                let hash = hash_value(&registration);
                let known = self
                    .previous
                    .owned
                    .iter()
                    .any(|r| r.event == name && r.hash.as_deref() == Some(hash.as_str()));
                let current = want_hash.as_deref() == Some(hash.as_str());
                match self.want {
                    Some(_) if placed && (known || current) => {
                        // A second rune entry, such as one an install from
                        // another build path left: one stays.
                        changed = true;
                        report
                            .written
                            .push((harness, format!("{name} duplicate removed")));
                    }
                    Some(_) if current => {
                        placed = true;
                        remaining.push(hook);
                        report.unchanged.push((harness, name.to_string()));
                    }
                    Some(want) if known => {
                        placed = true;
                        remaining.push(want["hooks"][0].clone());
                        changed = true;
                        report.written.push((harness, name.to_string()));
                    }
                    None if known => {
                        // Rune wrote it and wants it no more: its command
                        // goes, the rest of the group stays.
                        changed = true;
                        report.written.push((harness, format!("{name} removed")));
                    }
                    _ => return Err(self.conflict()),
                }
            }
            if remaining.is_empty() {
                continue;
            }
            let mut group = group;
            if let Some(map) = group.as_object_mut() {
                map.insert("hooks".into(), Value::Array(remaining));
            }
            groups.push(group);
        }
        if let Some(want) = self.want {
            if !placed {
                groups.push(want.clone());
                changed = true;
                report.written.push((harness, name.to_string()));
            }
            next.owned.push(Recorded {
                event: name.to_string(),
                command: want["hooks"][0]["command"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                hash: want_hash,
                by: None,
                source: None,
            });
        }
        Ok(Reconciled { groups, changed })
    }
}

/// Codex also reads `[[hooks.<Event>]]` from `config.toml`. That file is
/// rendered by the dotfiles and carries Codex's trust hashes, so rune
/// names the legacy lines instead of rewriting the file.
fn legacy_codex_toml(
    home: &Path,
    plan_ids: &[String],
    registered: &BTreeMap<String, Value>,
    report: &mut Report,
) {
    let path = home.join(".codex").join("config.toml");
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    // The event a line belongs to: the last `[[hooks.<Event>…]]` header.
    let mut event = String::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(header) = trimmed.strip_prefix("[[hooks.") {
            event = header
                .split(['.', ']'])
                .next()
                .unwrap_or_default()
                .to_string();
            continue;
        }
        if !trimmed.starts_with("command") {
            continue;
        }
        let Some(command) = trimmed
            .split_once('=')
            .map(|(_, v)| v.trim().trim_matches('"'))
        else {
            continue;
        };
        let by = adoption(command, plan_ids);
        if by == Some(LEGACY) && !registered.contains_key(&event) {
            continue;
        }
        if let Some(by) = by {
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

    const PLAN: &str = "/state/hooks/plan.json";

    const LIVE: &str = r#"{"model":"opus","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/Users/x/.local/bin/dcg"}]},{"hooks":[{"type":"command","command":"sh /Users/x/.sd/hook PreToolUse"}]},{"matcher":"*","hooks":[{"type":"command","command":"/Users/x/.git-ai/bin/git-ai checkpoint claude --hook-input stdin"}]}],"Notification":[{"hooks":[{"type":"command","command":"cmux hooks claude Notification"}]}]}}"#;

    #[test]
    fn foreign_entries_survive_predecessors_are_adopted_and_one_rune_entry_per_event() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), LIVE);
        let manifest = home.path().join("state").join("manifest.json");
        let report = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
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
            [
                "/opt/rune hook run --harness claude --native-event PreToolUse --plan /state/hooks/plan.json --on-failure deny"
            ],
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
        apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let first = fs::read_to_string(&path).unwrap();
        let report = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        assert!(report.written.is_empty(), "{report:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        let edited = first.replace("\"timeout\": 3", "\"timeout\": 99");
        assert_ne!(edited, first);
        fs::write(&path, &edited).unwrap();
        let error = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("changed by hand"), "{error}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            edited,
            "nothing written"
        );
        let report = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            true,
            false,
        );
        assert!(report.is_err(), "check reports the same conflict");
    }

    #[test]
    fn a_plan_change_replaces_a_known_entry_and_check_reports_drift() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), "{}");
        let manifest = home.path().join("state").join("manifest.json");
        apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let bigger: Plan = serde_yaml::from_str::<Hooks>(
            "events:\n  session.end: {budget_ms: 1400}\nhandlers:\n  - id: capture\n    exec: [x]\n    events: [session.end]\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let report = apply(
            &bigger,
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            true,
            false,
        )
        .unwrap();
        assert!(
            report.drift.iter().any(|d| d.contains("settings.json")),
            "{report:?}"
        );
        let report = apply(
            &bigger,
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
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
    fn any_rune_binary_on_any_path_is_rune_s_dispatcher() {
        let of = |c: &str| dispatcher_of(c).map(|(h, e)| format!("{h}:{e}"));
        assert_eq!(
            of("/old/build/rune hook run --harness claude --native-event PreToolUse"),
            Some("claude:PreToolUse".to_string())
        );
        assert_eq!(
            of(
                "'/opt/my rune/rune' hook run --harness codex --native-event Stop --plan '/s p/plan.json' --on-failure deny"
            ),
            Some("codex:Stop".to_string())
        );
        assert_eq!(
            of("rune hook run --harness claude --native-event SessionEnd"),
            Some("claude:SessionEnd".to_string())
        );
        assert_eq!(
            of("/x/rune-dev hook run --harness claude --native-event Stop"),
            None
        );
        assert_eq!(of("/x/rune hook list"), None);
        assert!(!is_owned(
            "/x/rune hook run --harness codex --native-event Stop",
            Harness::Claude
        ));
    }

    #[test]
    fn an_install_from_another_build_path_replaces_the_first_one_s_entry() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), "{}");
        let manifest = home.path().join("state").join("manifest.json");
        apply(
            &plan(),
            home.path(),
            "/a/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        apply(
            &plan(),
            home.path(),
            "/b/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let commands: Vec<&str> = table["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert_eq!(commands.len(), 1, "{commands:?}");
        assert!(commands[0].starts_with("/b/rune hook run"), "{commands:?}");
        // Two rune entries on one event, as the earlier code left them,
        // shrink to one.
        let doubled = fs::read_to_string(&path).unwrap();
        let mut value: Value = serde_json::from_str(&doubled).unwrap();
        let group = value["hooks"]["PreToolUse"][0].clone();
        value["hooks"]["PreToolUse"]
            .as_array_mut()
            .unwrap()
            .push(group);
        fs::write(&path, value.to_string()).unwrap();
        apply(
            &plan(),
            home.path(),
            "/b/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(table["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_dropped_event_takes_rune_s_command_and_leaves_the_owner_s_beside_it() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), "{}");
        let manifest = home.path().join("state").join("manifest.json");
        apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let mut table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        table["hooks"]["PreToolUse"][0]["hooks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "command", "command": "/owner/my-guard"}));
        fs::write(&path, table.to_string()).unwrap();
        // The owner's sibling is no hand edit of rune's entry.
        apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let capture_only: Plan = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: capture\n    exec: [rune-hook-capture]\n    events: [session.end]\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        apply(
            &capture_only,
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let commands: Vec<&str> = table["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert_eq!(commands, ["/owner/my-guard"]);
    }

    #[test]
    fn a_dropped_event_whose_rune_entry_was_edited_is_a_conflict() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), "{}");
        let manifest = home.path().join("state").join("manifest.json");
        apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let mut table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        table["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"] = json!(99);
        fs::write(&path, table.to_string()).unwrap();
        let capture_only: Plan = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: capture\n    exec: [rune-hook-capture]\n    events: [session.end]\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let error = apply(
            &capture_only,
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("changed by hand"), "{error}");
    }

    #[test]
    fn the_legacy_dispatcher_stays_on_an_event_no_handler_takes() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(
            home.path(),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"sh /Users/x/.sd/hook Stop"}]},{"hooks":[{"type":"command","command":"/Users/x/bin/my-stop.sh"}]}],"SessionEnd":[{"hooks":[{"type":"command","command":"sh /Users/x/.sd/hook SessionEnd"}]}],"PreToolUse":[{"hooks":[{"type":"command","command":"sh /Users/x/.sd/hook PreToolUse"}]}]}}"#,
        );
        let manifest = home.path().join("state").join("manifest.json");
        let only_dcg: Plan = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: dcg\n    exec: [rune-hook-dcg]\n    events: [tool.before]\n    on_failure: deny\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let report = apply(
            &only_dcg,
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let commands = |event: &str| -> Vec<String> {
            table["hooks"][event]
                .as_array()
                .map(|groups| {
                    groups
                        .iter()
                        .flat_map(|g| g["hooks"].as_array().unwrap().clone())
                        .map(|h| h["command"].as_str().unwrap().to_string())
                        .collect()
                })
                .unwrap_or_default()
        };
        assert_eq!(
            commands("Stop"),
            ["sh /Users/x/.sd/hook Stop", "/Users/x/bin/my-stop.sh"]
        );
        assert_eq!(commands("SessionEnd"), ["sh /Users/x/.sd/hook SessionEnd"]);
        assert!(
            !commands("PreToolUse")
                .iter()
                .any(|c| c.contains(".sd/hook")),
            "the event rune registers adopts it"
        );
        assert_eq!(report.warnings.len(), 2, "{:?}", report.warnings);
        assert!(report.warnings[0].contains("legacy"));
    }

    #[test]
    fn a_guarded_claude_entry_blocks_when_the_dispatcher_fails() {
        let home = tempfile::tempdir().unwrap();
        let path = settings(home.path(), "{}");
        let manifest = home.path().join("state").join("manifest.json");
        apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            table["hooks"]["PreToolUse"][0]["hooks"][0]["onFailure"],
            "block"
        );
        assert!(
            table["hooks"]["SessionEnd"][0]["hooks"][0]
                .get("onFailure")
                .is_none(),
            "no guard on a passive event"
        );
        let codex: Value = serde_json::from_str(
            &fs::read_to_string(home.path().join(".codex/hooks.json")).unwrap(),
        )
        .unwrap();
        assert!(
            codex["hooks"]["PreToolUse"][0]["hooks"][0]
                .get("onFailure")
                .is_none(),
            "codex has no such field"
        );
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
        let report = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        assert!(
            report.legacy_toml[0].contains("config.toml:3"),
            "{report:?}"
        );
        assert!(report.trust[0].contains("/hooks"));
        let hooks: Value =
            serde_json::from_str(&fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
        assert_eq!(
            hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "/opt/rune hook run --harness codex --native-event PreToolUse --plan /state/hooks/plan.json --on-failure deny"
        );
        assert!(
            hooks["hooks"].get("Notification").is_none(),
            "codex has no notification event"
        );
    }
}
