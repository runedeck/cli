//! Render the plan into the harness hook tables: one dispatcher entry per
//! subscribed event in Claude's `settings.json` and Codex's `hooks.json`.
//! rune owns only the entries it wrote (known by their command), keeps
//! every other entry, adopts the entries of the tools its adapters
//! replace, and stops on an owned entry a hand changed. The legacy
//! dispatcher's entry stays, and every handler stays in its event's
//! generation: where a part of that entry runs a handler's whole command,
//! install records the pair in a cover file (see `cover`), and the
//! dispatcher skips the handler at each firing only while that still
//! holds. The inventory lives in a manifest under the rune state
//! directory.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use rune::error::{Error, ErrorKind};
use rune::hooks::{Harness, Mode, OnFailure, Plan, PlannedEvent};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::codex_trust::{self, Trust};
use super::cover;
use super::legacy;
use super::publish::Staged;
use super::retire::{self, Retired, canonical};
use super::table::Table;

/// One entry of a harness table as rune records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Recorded {
    pub event: String,
    pub command: String,
    /// The hash of the whole registration rune wrote (owned entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Which adapter adopted it (adopted entries).
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
    /// The plan generations the last install's registrations name.
    #[serde(default)]
    pub generations: Vec<String>,
    /// The generations no table names any more, each with the time, the
    /// install, and the days at which installs found it unnamed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired: Vec<Retired>,
    /// How many installs wrote this manifest: a count the clock cannot
    /// move.
    #[serde(default)]
    pub installs: u64,
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
    /// A handler a part of a kept legacy dispatcher entry covers, so the
    /// dispatcher skips it at each firing while the part runs its command.
    pub covered: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    /// The report as `rune install` and `rune hook install --json` carry it.
    pub(crate) fn summary(&self) -> rune::result::HookSummary {
        rune::result::HookSummary {
            skipped: None,
            written: self
                .written
                .iter()
                .map(|(h, e)| format!("{h}:{e}"))
                .collect(),
            adopted: self
                .adopted
                .iter()
                .map(|(h, e, c)| format!("{h}:{e}:{c}"))
                .collect(),
            foreign: self
                .foreign
                .iter()
                .map(|(h, e, c)| format!("{h}:{e}:{c}"))
                .collect(),
            unsupported: self.unsupported.clone(),
            legacy: self.legacy_toml.clone(),
            covered: self.covered.clone(),
            drift: self.drift.clone(),
            trust: self.trust.clone(),
            warnings: self.warnings.clone(),
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        // A written entry is an event registered, or an event with what
        // happened to it, such as `PreToolUse removed`.
        for (harness, event) in &self.written {
            let done = if event.contains(' ') {
                ""
            } else {
                " registered"
            };
            let _ = writeln!(out, "{harness}: {event}{done}");
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
        for line in &self.covered {
            let _ = writeln!(out, "covered: {line}");
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
/// shell to read back what `shell_quote` wrote. Any blank splits words,
/// as main reads a predecessor's command; `legacy::words` reads a part
/// the way `sh` does.
pub(crate) fn words(command: &str) -> Vec<String> {
    split_words(command, char::is_whitespace)
}

/// The words of `command`, split outside quotes wherever `is_blank`.
pub(crate) fn split_words(command: &str, is_blank: impl Fn(char) -> bool) -> Vec<String> {
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
            None if is_blank(c) => {
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

/// Whether two records name the same entry: its event, command, and file.
fn same_entry(a: &Recorded, b: &Recorded) -> bool {
    a.event == b.event && a.command == b.command && a.source == b.source
}

/// The built-in adapter a handler runs: the `<name>` of a resolved
/// executable `rune-hook-<name>`.
fn adapter_of(handler: &rune::hooks::PlannedHandler) -> Option<&str> {
    handler.exec.first()?.strip_prefix("rune-hook-")
}

/// The built-in adapters an event's registration runs, as install
/// registers it: each handler whose resolved executable is
/// `rune-hook-<name>`.
fn adapters_of(planned: &PlannedEvent) -> Vec<String> {
    planned
        .handlers
        .iter()
        .filter_map(adapter_of)
        .map(str::to_string)
        .collect()
}

/// A predecessor install finds in a hook's command, by what replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Predecessor {
    /// The command runs the predecessor and nothing else: install may
    /// remove it.
    Alone(&'static str),
    /// The command runs the predecessor with other commands: it stays,
    /// and the owner splits it by hand.
    Compound(&'static str),
}

/// The characters that join, redirect, or substitute commands. A command
/// line without them runs one program.
const OPERATORS: &[char] = &[';', '&', '|', '<', '>', '(', ')', '$', '`', '\n', '\r'];

/// The predecessor in a hook's command: a tool whose adapter the plan runs
/// on the same harness and native event. Only a command that runs the
/// predecessor alone is install's to remove. The legacy dispatcher is no
/// predecessor: its entry stays, and `legacy` reads its parts.
fn predecessor(command: &str, adapters: &[String]) -> Option<Predecessor> {
    if !command.contains(OPERATORS) {
        return predecessor_run(&words(command), adapters).map(Predecessor::Alone);
    }
    command
        .split(OPERATORS)
        .find_map(|part| predecessor_run(&words(part), adapters))
        .map(Predecessor::Compound)
}

/// What one simple command runs, by its words: the program of a tool
/// whose adapter the plan runs here.
fn predecessor_run(words: &[String], adapters: &[String]) -> Option<&'static str> {
    let name = |word: &String| {
        Path::new(word)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let program = name(words.first()?);
    let second = words.get(1).map(String::as_str);
    let runs = |adapter: &str| adapters.iter().any(|a| a == adapter);
    if runs("dcg") && program == "dcg" {
        return Some("dcg");
    }
    if runs("git-ai") && program == "git-ai" && second == Some("checkpoint") {
        return Some("git-ai");
    }
    if runs("author-identity") && program.starts_with("claude-agent-identity") {
        return Some("author-identity");
    }
    if runs("tmux-status") && program.starts_with("claude-tmux-status") {
        return Some("tmux-status");
    }
    if runs("rtk") && program == "rtk" && second == Some("hook") {
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

/// The manifest's path, made absolute like the plan's, because
/// `RUNE_STATE_DIR` may be relative.
pub(crate) fn manifest_path() -> Result<PathBuf, Error> {
    let path = crate::cli::state::dir()?
        .join("hooks")
        .join("manifest.json");
    std::path::absolute(&path).map_err(|error| Error::io(format!("{}: {error}", path.display())))
}

/// Whether an earlier install recorded an entry rune owns.
pub(crate) fn has_owned(manifest_path: &Path) -> bool {
    read_manifest(manifest_path)
        .harnesses
        .values()
        .any(|harness| !harness.owned.is_empty())
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

/// The table files install may write for every registered harness, links
/// resolved: the set its lock covers with the plan and the manifest.
pub(crate) fn table_files(home: &Path) -> Result<Vec<PathBuf>, Error> {
    Harness::REGISTERED
        .iter()
        .map(|harness| resolved(&table_path(home, *harness)))
        .collect()
}

/// What an install found, and the files it would write: the plan
/// generations its registrations name, then the changed tables and the
/// manifest. Nothing is written while it is computed.
#[derive(Debug)]
pub(crate) struct Prepared {
    pub report: Report,
    pub generations: Vec<super::plan::Generation>,
    /// The cover files that changed: per event, the handlers a part of a
    /// kept legacy dispatcher entry covers. Written beside the generations.
    pub covers: Vec<Staged>,
    /// The old manifest without the retired records of generations the
    /// old or the new tables name, written before the tables when it
    /// differs from the file.
    pub early: Option<Staged>,
    pub files: Vec<Staged>,
}

/// Write what `prepare` computed, in order: the new generations and the
/// cover files, the early manifest, then the tables and the manifest as
/// one step. The cleanup that follows runs only after that step succeeds.
pub(crate) fn write(prepared: &Prepared) -> Result<(), Error> {
    for generation in &prepared.generations {
        super::plan::write_generation(generation)?;
    }
    if !prepared.covers.is_empty() {
        super::publish::publish(&prepared.covers)?;
    }
    if let Some(early) = &prepared.early {
        super::publish::publish(std::slice::from_ref(early))?;
    }
    super::publish::publish(&prepared.files)
}

/// Install or check the tables of every registered harness under `home`.
/// Every table is read and every conflict found before anything is
/// written, then the tables and the manifest are written as one step.
/// The command also writes the generations first and sweeps the ones no
/// table names after; the tests of the tables call this.
#[cfg(test)]
pub(crate) fn apply(
    plan: &Plan,
    home: &Path,
    rune: &str,
    generations: &str,
    manifest_path: &Path,
    check: bool,
    dry_run: bool,
) -> Result<Report, Error> {
    let prepared = prepare(
        plan,
        home,
        rune,
        Path::new(generations),
        manifest_path,
        check,
        chrono::Utc::now(),
    )?;
    if !check && !dry_run {
        super::publish::publish(&prepared.files)?;
    }
    Ok(prepared.report)
}

/// The plan generation a rune registration names after `--plan`.
fn generation_of(command: &str) -> Option<PathBuf> {
    dispatcher_of(command)?;
    let words = words(command);
    let at = words.iter().position(|word| word == "--plan")?;
    words.get(at + 1).map(PathBuf::from)
}

/// Keep every generation in `dir` that a registration in a table under
/// `home` names, from a fresh read of the tables. Record in the manifest
/// when, at which install, and on which days installs found each other
/// one unnamed, and delete it only once `retire::observe` lets it go.
/// Returns the generations deleted.
pub(crate) fn sweep_generations(
    home: &Path,
    dir: &Path,
    manifest_path: &Path,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<PathBuf>, Error> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let named = named_generations(home, &canonical)?;
    let mut manifest = read_manifest(manifest_path);
    let installs = manifest.installs;
    let earlier: BTreeMap<PathBuf, &Retired> = manifest
        .retired
        .iter()
        .map(|record| (canonical(Path::new(&record.generation)), record))
        .collect();
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    let mut retired = Vec::new();
    let mut swept = Vec::new();
    for path in paths {
        let key = canonical(&path);
        if named.contains(&key) {
            continue;
        }
        if let Some(kept) = retire::observe(earlier.get(&key).copied(), &path, now, installs) {
            retired.push(kept);
        } else {
            fs::remove_file(&path)
                .map_err(|error| Error::io(format!("{}: {error}", path.display())))?;
            swept.push(path);
        }
    }
    if retired != manifest.retired {
        manifest.retired = retired;
        super::publish::publish(&[manifest_file(&manifest, manifest_path)?])?;
    }
    Ok(swept)
}

/// The canonical path of every generation a rune registration in a
/// table under `home` names.
fn named_generations(
    home: &Path,
    canonical: &dyn Fn(&Path) -> PathBuf,
) -> Result<std::collections::BTreeSet<PathBuf>, Error> {
    let mut named = std::collections::BTreeSet::new();
    for harness in Harness::REGISTERED {
        let table = Table::read(&resolved(&table_path(home, harness))?)?;
        for event in table.event_names() {
            for plan in generations_in(&table.groups(&event)) {
                named.insert(canonical(&plan));
            }
        }
    }
    Ok(named)
}

/// The generation every rune registration in an event's groups names.
fn generations_in(groups: &[Value]) -> Vec<PathBuf> {
    groups
        .iter()
        .filter_map(|group| group["hooks"].as_array())
        .flatten()
        .filter_map(|hook| generation_of(hook["command"].as_str().unwrap_or_default()))
        .collect()
}

/// Read every table, find every conflict, and compute what install would
/// write. `rune` is the dispatcher's own path as the tables will call it,
/// and `now` the time a generation the new tables stop naming is retired
/// at. One pass over every event of every registered harness: long, and
/// linear.
#[allow(clippy::too_many_lines)]
pub(crate) fn prepare(
    plan: &Plan,
    home: &Path,
    rune: &str,
    generations_dir: &Path,
    manifest_path: &Path,
    check: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Prepared, Error> {
    let mut files: Vec<Staged> = Vec::new();
    let mut generations: Vec<super::plan::Generation> = Vec::new();
    let mut covers: Vec<Staged> = Vec::new();
    // The cover files live beside the generations, under the hooks dir.
    let hooks_dir = generations_dir.parent().unwrap_or(generations_dir);
    // The generations rune's registrations name before and after install.
    let mut named_before = std::collections::BTreeSet::new();
    let mut named_after = std::collections::BTreeSet::new();
    let mut report = Report::default();
    let mut manifest = read_manifest(manifest_path);
    manifest.version = 1;
    // Codex keeps hook trust in the user config.
    let codex_config = home.join(".codex").join("config.toml");
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
        let codex = harness == Harness::Codex;
        // Each registration names its own generation: the event as the
        // plan compiles it, every handler included. A handler that a part
        // of a kept legacy dispatcher entry covers goes in the event's
        // cover file, which the dispatcher reads at each firing.
        let dispatchers = Dispatchers {
            home,
            harness,
            table: &path,
            path_env: std::env::var_os("PATH"),
            shell_variable: cover::shell_variable_set(),
        };
        let toml = if codex {
            codex_toml_hooks(&codex_config)
        } else {
            Vec::new()
        };
        let mut wanted: BTreeMap<String, Value> = BTreeMap::new();
        let mut events: BTreeMap<String, PlannedEvent> = BTreeMap::new();
        for (name, event) in plan.harnesses.get(&harness).into_iter().flatten() {
            let Some(generation) = super::plan::generation_of(
                generations_dir,
                plan.version,
                harness,
                name,
                event.clone(),
            ) else {
                continue;
            };
            let plan_path = generation.path.display().to_string();
            wanted.insert(
                name.clone(),
                rendered(rune, harness, name, event, &plan_path),
            );
            events.insert(name.clone(), event.clone());
            generations.push(generation);
        }
        // Every event the table already has, in its order, plus every
        // event rune wants.
        let mut names: Vec<String> = table.event_names();
        for name in wanted.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        let mut untrusted: Vec<(String, Trust)> = Vec::new();
        let mut moved: Vec<(String, Slot, Slot)> = Vec::new();
        let mut deferred: Vec<String> = Vec::new();
        for name in &names {
            let entries = table.groups(name);
            named_before.extend(generations_in(&entries));
            let mut event = Event {
                harness,
                name,
                path: &path,
                want: wanted.get(name),
                previous: &previous,
                adapters: events.get(name).map(adapters_of).unwrap_or_default(),
                adopt: !codex,
                keep_slots: codex,
                hold: std::collections::BTreeSet::new(),
            };
            if codex && event.want.is_some() {
                // Codex trusts a hook at its position, so the question is
                // whether it trusts the rune entry where the entry lands
                // once the predecessors go. A removed group leaves a slot,
                // but a predecessor before rune's entry in the same group
                // moves the entry, and then every predecessor stays.
                //
                // Rune's own earlier entry is such a predecessor too. A
                // changed plan gives the event a new command, which Codex
                // does not trust yet, so the earlier entry Codex trusts
                // stays, and the new one goes in a group of its own after
                // it, until Codex trusts the new one where it stands.
                let earlier = event.trusted_earlier(&codex_config, &entries);
                event.hold.extend(earlier);
                let mut landing = event.landing(&codex_config, &entries)?;
                if let Some(slot) = earlier
                    && landing.0 == Trust::Trusted
                {
                    event.hold.remove(&slot);
                    let without = event.landing(&codex_config, &entries)?;
                    if without.0 == Trust::Trusted {
                        landing = without;
                    } else {
                        event.hold.insert(slot);
                    }
                }
                let (trust, from, to) = landing;
                event.adopt = trust == Trust::Trusted;
                if let (Some(slot), Some(to)) = (earlier, to)
                    && event.hold.contains(&slot)
                {
                    report.trust.push(format!(
                        "{harness}: {name} keeps rune's earlier entry, which {harness} trusts, until {harness} trusts the new one at {}",
                        slot_word(to)
                    ));
                }
                match (from, to) {
                    _ if event.adopt => {}
                    (Some(from), Some(to)) if from != to => moved.push((name.clone(), from, to)),
                    // The entry stays where it is, so its trust there is
                    // the trust it keeps.
                    _ => untrusted.push((name.clone(), trust)),
                }
            }
            if codex {
                // A removal moves every later hook of its group to a lower
                // handler, and Codex keys trust by position, so a removal
                // that would move a hook Codex trusts waits.
                for (slot, shift) in event.hold_trusted(&codex_config, &entries)? {
                    // A held rune entry is never the one the plan renders.
                    let what = describe(&entries, slot, harness, "rune's earlier entry");
                    let moved_hook = describe(&entries, shift.from, harness, "rune's entry");
                    report.trust.push(format!(
                        "{harness}: {name} keeps {what}, because removing it would move {moved_hook} from {} to {}, where {harness} does not trust it; remove it by hand, trust {moved_hook} in `/hooks` again, then run `rune install`",
                        slot_word(shift.from),
                        slot_word(shift.to)
                    ));
                    deferred.push(format!(
                        "{}: removing {what} from {name} would move {moved_hook}, which {harness} trusts where it stands",
                        path.display()
                    ));
                }
            }
            let kept = event.reconcile(entries, &mut next, &mut report)?;
            if codex && check {
                // Every hook in a group install keeps rune in, or removes a
                // hook from, must still be trusted where it stands.
                touched_trust(&event, &codex_config, &kept, &mut report.drift);
            }
            named_after.extend(generations_in(&kept.groups));
            // The cover names each entry at the slot the table has once
            // this install wrote it, so an entry adopted before the
            // dispatcher in this same install does not shift the slot.
            if let Some(planned) = events.get(name) {
                let handlers = dispatchers.covered(name, planned, &kept.groups, &mut report);
                if !check && let Some(staged) = cover_file(hooks_dir, harness, name, handlers)? {
                    covers.push(staged);
                }
            }
            if kept.changed {
                changed = true;
                rewritten.insert(name.clone(), kept.groups);
            }
        }
        for (name, trust) in &untrusted {
            report.trust.push(format!(
                "{harness}: the rune entry for {name} is {}: run `/hooks` in {harness} and trust it, then run `rune install` again; the entries it replaces stay until then",
                trust.word()
            ));
        }
        for (name, from, to) in &moved {
            report.trust.push(format!(
                "{harness}: removing the entries the rune entry for {name} replaces would move it from {} to {}, where {harness} does not trust it; remove them by hand, trust the rune entry in `/hooks`, then run `rune install` again",
                slot_word(*from),
                slot_word(*to)
            ));
        }
        // An entry adopted by an earlier install is gone from the table,
        // so its record comes forward from the manifest. A legacy
        // dispatcher entry an earlier release adopted is one rune now
        // keeps: install says it is gone, until the owner puts it back,
        // when it is foreign and its record goes.
        for earlier in &previous.adopted {
            if next.adopted.iter().any(|r| same_entry(r, earlier)) {
                continue;
            }
            if earlier.by.as_deref() == Some(LEGACY_SD_HOOK) {
                if next.foreign.iter().any(|r| same_entry(r, earlier)) {
                    continue;
                }
                report.warnings.push(format!(
                    "{harness}: {} no longer has the legacy dispatcher entry `{}`, which an earlier install removed; rune keeps that entry now, so put it back by hand if its parts should run",
                    earlier.event, earlier.command
                ));
            }
            next.adopted.push(earlier.clone());
        }
        // A foreign entry the manifest does not list arrived after the
        // last install.
        let arrived: Vec<String> = next
            .foreign
            .iter()
            .filter(|r| !previous.foreign.iter().any(|p| same_entry(p, r)))
            .map(|r| {
                format!(
                    "{}: foreign `{}` on {} is new since the last install",
                    path.display(),
                    r.command,
                    r.event
                )
            })
            .collect();
        if codex {
            legacy_codex_toml(&codex_config, &toml, &events, &mut report);
        }
        if check {
            if changed {
                report
                    .drift
                    .push(format!("{}: differs from the plan", path.display()));
            }
            report.drift.extend(arrived);
            report.drift.extend(untrusted.iter().map(|(name, trust)| {
                format!(
                    "{}: the rune entry for {name} is {} in {harness}",
                    path.display(),
                    trust.word()
                )
            }));
            report.drift.extend(moved.iter().map(|(name, _, _)| {
                format!(
                    "{}: the rune entry for {name} would move when the entries it replaces go",
                    path.display()
                )
            }));
            report.drift.extend(deferred);
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
    let mut early = None;
    if !check {
        // The manifest goes last in the step, so an install killed after
        // the tables changed leaves the old one. Before the tables change,
        // the old manifest drops the record of every generation the old
        // or the new tables name: a record that survives a kill is then
        // never older than the last time a table named its generation.
        let mut first = read_manifest(manifest_path);
        let unnamed = retire::unnamed_only(&first.retired, &named_before, &named_after);
        if unnamed != first.retired {
            first.retired = unnamed;
            early = Some(manifest_file(&first, manifest_path)?);
        }
        manifest.installs += 1;
        manifest.generations = named_after
            .iter()
            .map(|generation| generation.display().to_string())
            .collect();
        // No generation the new tables name keeps a record, and each one
        // they stop naming starts its window now.
        manifest.retired = retire::reconcile(
            &manifest.retired,
            &named_before,
            &named_after,
            now,
            manifest.installs,
        );
        files.push(manifest_file(&manifest, manifest_path)?);
    }
    Ok(Prepared {
        report,
        generations,
        covers,
        early,
        files,
    })
}

/// The `by` an earlier release recorded when it adopted, and removed, a
/// legacy dispatcher entry.
const LEGACY_SD_HOOK: &str = "legacy sd hook";

/// The cover file of `event` on `harness` as install stages it, or `None`
/// when the file already holds `handlers`, or there is nothing to cover
/// and no file to clear.
fn cover_file(
    hooks_dir: &Path,
    harness: Harness,
    event: &str,
    handlers: BTreeMap<String, super::cover::Coverage>,
) -> Result<Option<Staged>, Error> {
    let target = super::cover::path_for(hooks_dir, harness, event);
    let cover = super::cover::Cover {
        v: 1,
        harness: harness.as_str().to_string(),
        event: event.to_string(),
        handlers,
    };
    let text =
        serde_json::to_string_pretty(&cover).map_err(|error| Error::io(error.to_string()))?;
    let text = format!("{text}\n");
    let current = fs::read_to_string(&target).ok();
    if current.as_deref() == Some(text.as_str()) || (current.is_none() && cover.handlers.is_empty())
    {
        return Ok(None);
    }
    Ok(Some(Staged {
        target,
        text,
        new_mode: Some(0o600),
    }))
}

/// The manifest as the file install stages.
fn manifest_file(manifest: &Manifest, path: &Path) -> Result<Staged, Error> {
    let text =
        serde_json::to_string_pretty(manifest).map_err(|error| Error::io(error.to_string()))?;
    Ok(Staged {
        target: path.to_path_buf(),
        text: format!("{text}\n"),
        new_mode: Some(0o600),
    })
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
    /// The built-in adapters the registration runs on this harness and
    /// event.
    adapters: Vec<String>,
    /// Whether a predecessor may go now. On Codex it waits until Codex
    /// trusts the rune entry that replaces it.
    adopt: bool,
    /// Whether an emptied group stays as `{"hooks": []}`. Codex keys trust
    /// by position, so a removed group would move every later one.
    keep_slots: bool,
    /// The slots, in the table as read, of hooks that stay although
    /// install would remove them: rune's earlier entry while Codex trusts
    /// it and not yet the new one, and any hook whose removal would move a
    /// hook Codex trusts where it stands.
    hold: std::collections::BTreeSet<Slot>,
}

/// An event's groups after install, whether they differ from before, and
/// the slots of the hooks install removed, in the table as read.
struct Reconciled {
    groups: Vec<Value>,
    changed: bool,
    removed: std::collections::BTreeSet<Slot>,
}

/// A hook Codex trusts that a removal before it in its group would move:
/// where it stands, where it would go, and the removals that move it.
struct Shift {
    from: Slot,
    to: Slot,
    causes: Vec<Slot>,
}

/// One hook as its own registration: the group's fields with this hook
/// alone, hashed, so a sibling the owner added to the group is no edit.
fn registration_hash(group: &Value, hook: &Value) -> String {
    let mut registration = group.clone();
    if let Some(map) = registration.as_object_mut() {
        map.insert("hooks".into(), json!([hook.clone()]));
    }
    hash_value(&registration)
}

impl Event<'_> {
    /// Whether an earlier install wrote this registration hash here.
    fn known(&self, hash: &str) -> bool {
        self.previous
            .owned
            .iter()
            .any(|r| r.event == self.name && r.hash.as_deref() == Some(hash))
    }

    /// Rune's earlier entry in `entries` that Codex trusts where it stands,
    /// when the plan now renders another: its group and handler.
    fn trusted_earlier(&self, config: &Path, entries: &[Value]) -> Option<Slot> {
        let want_hash = self.want.map(hash_value)?;
        for (g, group) in entries.iter().enumerate() {
            for (h, hook) in group["hooks"].as_array().into_iter().flatten().enumerate() {
                if !is_owned(hook["command"].as_str().unwrap_or_default(), self.harness) {
                    continue;
                }
                let hash = registration_hash(group, hook);
                if hash == want_hash || !self.known(&hash) {
                    continue;
                }
                let expected = codex_trust::hash(self.name, group, hook);
                if codex_trust::trust(config, self.path, self.name, g, h, &expected)
                    == Trust::Trusted
                {
                    return Some((g, h));
                }
            }
        }
        None
    }

    /// Where the wanted entry lands, with the predecessors kept and with
    /// them gone, and whether Codex trusts it once they are gone.
    fn landing(
        &mut self,
        config: &Path,
        entries: &[Value],
    ) -> Result<(Trust, Option<Slot>, Option<Slot>), Error> {
        self.adopt = false;
        let kept = self.reconcile(
            entries.to_vec(),
            &mut HarnessManifest::default(),
            &mut Report::default(),
        )?;
        self.adopt = true;
        let adopted = self.reconcile(
            entries.to_vec(),
            &mut HarnessManifest::default(),
            &mut Report::default(),
        )?;
        let trust = codex_trust_of(self, config, &adopted.groups);
        Ok((
            trust,
            self.rune_slot(&kept.groups),
            self.rune_slot(&adopted.groups),
        ))
    }

    /// The hooks Codex trusts where they stand in `entries` that the
    /// removals in `removed` would move to a lower handler of their group.
    /// Codex keys trust by position, so such a hook would lose it.
    fn shifts(
        &self,
        config: &Path,
        entries: &[Value],
        removed: &std::collections::BTreeSet<Slot>,
    ) -> Vec<Shift> {
        let mut out = Vec::new();
        for (g, group) in entries.iter().enumerate() {
            for (h, hook) in group["hooks"].as_array().into_iter().flatten().enumerate() {
                if removed.contains(&(g, h)) {
                    continue;
                }
                let causes: Vec<Slot> = removed
                    .iter()
                    .copied()
                    .filter(|&(rg, rh)| rg == g && rh < h)
                    .collect();
                if causes.is_empty() {
                    continue;
                }
                let expected = codex_trust::hash(self.name, group, hook);
                if codex_trust::trust(config, self.path, self.name, g, h, &expected)
                    == Trust::Trusted
                {
                    out.push(Shift {
                        from: (g, h),
                        to: (g, h - causes.len()),
                        causes,
                    });
                }
            }
        }
        out
    }

    /// Hold every removal that would move a hook Codex trusts, until no
    /// removal moves one. Returns each hold with the hook it keeps in
    /// place: the held slot, and the shift it prevents.
    fn hold_trusted(
        &mut self,
        config: &Path,
        entries: &[Value],
    ) -> Result<Vec<(Slot, Shift)>, Error> {
        let mut held = Vec::new();
        loop {
            let trial = self.reconcile(
                entries.to_vec(),
                &mut HarnessManifest::default(),
                &mut Report::default(),
            )?;
            let shifts = self.shifts(config, entries, &trial.removed);
            if shifts.is_empty() {
                return Ok(held);
            }
            for shift in shifts {
                for cause in &shift.causes {
                    if self.hold.insert(*cause) {
                        held.push((
                            *cause,
                            Shift {
                                from: shift.from,
                                to: shift.to,
                                causes: shift.causes.clone(),
                            },
                        ));
                    }
                }
            }
        }
    }

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

    /// One hook that is not rune's: adopted when a handler on this event
    /// replaces it and the command runs nothing else, else kept as
    /// foreign. The legacy dispatcher is foreign too: it stays, whatever
    /// its parts run. Returns whether it stays.
    fn foreign(&self, command: &str, next: &mut HarnessManifest, report: &mut Report) -> bool {
        let (harness, name) = (self.harness, self.name);
        match predecessor(command, &self.adapters) {
            Some(Predecessor::Compound(by)) => {
                report.warnings.push(format!(
                    "{harness}: {name} keeps `{command}`, because it runs {by} with other commands; split it by hand, and the next install takes the {by} part"
                ));
            }
            Some(Predecessor::Alone(_)) if !self.adopt => {
                report.trust.push(format!(
                    "{harness}: {name} keeps `{command}` until {harness} trusts the rune entry that replaces it"
                ));
            }
            Some(Predecessor::Alone(by)) => {
                next.adopted.push(self.record(command, Some(by)));
                report
                    .adopted
                    .push((harness, name.to_string(), command.to_string()));
                return false;
            }
            None => {}
        }
        next.foreign.push(self.record(command, None));
        report
            .foreign
            .push((harness, name.to_string(), command.to_string()));
        true
    }

    /// Every group of the event, hook by hook. Rune's own command is
    /// checked against its hash and kept, replaced, or removed alone, so
    /// a hook the owner added to the same group stays. Every other hook
    /// is adopted when a handler replaces it, else kept as foreign. One
    /// decision per hook, in table order: long, and linear.
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
        let mut removed = std::collections::BTreeSet::new();
        let original = entries.clone();
        // Whether the table already holds the entry the plan renders, so
        // an earlier rune entry goes instead of turning into it.
        let has_current = self.wanted_slot(&original).is_some();
        for (g, group) in entries.into_iter().enumerate() {
            let list: Vec<Value> = group
                .get("hooks")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut remaining: Vec<Value> = Vec::new();
            for (h, hook) in list.into_iter().enumerate() {
                let command = hook
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let held = self.hold.contains(&(g, h));
                if !is_owned(&command, harness) {
                    if held {
                        // Its removal would move a hook Codex trusts.
                        next.foreign.push(self.record(&command, None));
                        report
                            .foreign
                            .push((harness, name.to_string(), command.clone()));
                        remaining.push(hook);
                    } else if self.foreign(&command, next, report) {
                        remaining.push(hook);
                    } else {
                        changed = true;
                        removed.insert((g, h));
                    }
                    continue;
                }
                let hash = registration_hash(&group, &hook);
                let known = self.known(&hash);
                let current = want_hash.as_deref() == Some(hash.as_str());
                if held && (if current { placed } else { known }) {
                    // Codex trusts it, and not yet the new entry, or its
                    // removal would move a hook Codex trusts: it stays
                    // until an install can remove it without either.
                    remaining.push(hook);
                    next.owned.push(Recorded {
                        event: name.to_string(),
                        command,
                        hash: Some(hash),
                        by: None,
                        source: None,
                    });
                    continue;
                }
                match self.want {
                    Some(_) if current && placed => {
                        // A second copy of the entry the plan renders.
                        changed = true;
                        removed.insert((g, h));
                        report
                            .written
                            .push((harness, format!("{name} duplicate removed")));
                    }
                    Some(_) if current => {
                        placed = true;
                        remaining.push(hook);
                        report.unchanged.push((harness, name.to_string()));
                    }
                    Some(_) if known && (placed || has_current) => {
                        // A second rune entry, such as one an install from
                        // another build path left, or the earlier entry the
                        // new one has replaced: one stays.
                        changed = true;
                        removed.insert((g, h));
                        let what = if has_current {
                            "earlier entry removed"
                        } else {
                            "duplicate removed"
                        };
                        report.written.push((harness, format!("{name} {what}")));
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
                        removed.insert((g, h));
                        report.written.push((harness, format!("{name} removed")));
                    }
                    _ => return Err(self.conflict()),
                }
            }
            if remaining.is_empty() {
                if self.keep_slots {
                    groups.push(json!({ "hooks": [] }));
                }
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
        // A slot after the last real group, rune's new one included,
        // moves nothing.
        while groups.last().is_some_and(is_slot) {
            groups.pop();
        }
        changed |= groups != original;
        Ok(Reconciled {
            groups,
            changed,
            removed,
        })
    }

    /// Where rune's own entry sits in `groups`, as Codex counts it: the
    /// group and the handler within it. That is the entry the plan
    /// renders, when the groups hold it beside an earlier one.
    fn rune_slot(&self, groups: &[Value]) -> Option<Slot> {
        self.wanted_slot(groups)
            .or_else(|| self.find_owned(groups, |_, _| true))
    }

    /// Where the entry the plan renders sits in `groups`, if it is there.
    fn wanted_slot(&self, groups: &[Value]) -> Option<Slot> {
        let want_hash = self.want.map(hash_value)?;
        self.find_owned(groups, |group, hook| {
            registration_hash(group, hook) == want_hash
        })
    }

    /// The first rune entry in `groups` that `pick` accepts.
    fn find_owned(&self, groups: &[Value], pick: impl Fn(&Value, &Value) -> bool) -> Option<Slot> {
        groups.iter().enumerate().find_map(|(g, group)| {
            group["hooks"]
                .as_array()?
                .iter()
                .position(|hook| {
                    is_owned(hook["command"].as_str().unwrap_or_default(), self.harness)
                        && pick(group, hook)
                })
                .map(|h| (g, h))
        })
    }
}

/// A hook's position in an event, as Codex keys its trust: the group and
/// the handler within it.
type Slot = (usize, usize);

/// A handler a legacy dispatcher part covers, with the part and the
/// command the handler's adapter runs on this harness and event.
type Pair<'a> = (
    &'a rune::hooks::PlannedHandler,
    &'a legacy::Part,
    Vec<String>,
);

/// A slot as the owner finds it in `/hooks`, counted from one.
fn slot_word((group, handler): Slot) -> String {
    format!("group {} handler {}", group + 1, handler + 1)
}

/// A group that holds no hook: what an emptied Codex group becomes.
fn is_slot(group: &Value) -> bool {
    group
        .as_object()
        .is_some_and(|map| map.len() == 1 && map["hooks"].as_array().is_some_and(Vec::is_empty))
}

/// The hook at `slot` of `entries` as a message names it: its command,
/// or `rune` for one of rune's own entries.
fn describe(entries: &[Value], (g, h): Slot, harness: Harness, rune: &str) -> String {
    let command = entries
        .get(g)
        .and_then(|group| group["hooks"].get(h))
        .and_then(|hook| hook["command"].as_str())
        .unwrap_or_default();
    if is_owned(command, harness) {
        rune.to_string()
    } else {
        format!("`{command}`")
    }
}

/// Drift for every hook that is not rune's and that Codex does not trust
/// where it stands, in each group of `kept` that holds a rune entry or
/// lost a hook. Rune's own trust is reported on its own, and a hook the
/// owner disabled is the owner's choice.
fn touched_trust(event: &Event<'_>, config: &Path, kept: &Reconciled, drift: &mut Vec<String>) {
    let lost: std::collections::BTreeSet<usize> = kept.removed.iter().map(|(g, _)| *g).collect();
    for (g, group) in kept.groups.iter().enumerate() {
        let hooks = group["hooks"].as_array().cloned().unwrap_or_default();
        let owned =
            |hook: &Value| is_owned(hook["command"].as_str().unwrap_or_default(), event.harness);
        if !lost.contains(&g) && !hooks.iter().any(owned) {
            continue;
        }
        for (h, hook) in hooks.iter().enumerate() {
            if owned(hook) {
                continue;
            }
            let expected = codex_trust::hash(event.name, group, hook);
            match codex_trust::trust(config, event.path, event.name, g, h, &expected) {
                Trust::Trusted | Trust::Disabled => {}
                trust => drift.push(format!(
                    "{}: `{}` on {} at {} is {} in {}",
                    event.path.display(),
                    hook["command"].as_str().unwrap_or_default(),
                    event.name,
                    slot_word((g, h)),
                    trust.word(),
                    event.harness
                )),
            }
        }
    }
}

/// What Codex makes of the rune entry in `groups` of one event.
fn codex_trust_of(event: &Event<'_>, config: &Path, groups: &[Value]) -> Trust {
    let Some((g, h)) = event.rune_slot(groups) else {
        return Trust::Untrusted;
    };
    let group = &groups[g];
    let expected = codex_trust::hash(event.name, group, &group["hooks"][h]);
    codex_trust::trust(config, event.path, event.name, g, h, &expected)
}

/// One command hook Codex reads from `config.toml`, with the line that
/// holds its command, for a message.
pub(crate) struct TomlHook {
    pub event: String,
    pub command: String,
    pub line: Option<usize>,
}

/// The command hooks of `[[hooks.<Event>]]` in Codex's `config.toml`, read
/// with the TOML parser, so a comment, a single-quoted string, and an
/// inline table read as Codex reads them. A file that does not parse, or
/// is no regular file under the limit, holds none install can read. The
/// line named for a hook is the line of its `command` value, from the
/// parser's spans: a string elsewhere that quotes the command never wins.
pub(crate) fn codex_toml_hooks(path: &Path) -> Vec<TomlHook> {
    let Some(text) = legacy::read_text_bounded(path, codex_trust::CONFIG_LIMIT) else {
        return Vec::new();
    };
    let Ok(config) = toml::from_str::<toml::Value>(&text) else {
        return Vec::new();
    };
    let Some(events) = config.get("hooks").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let spans = toml::from_str::<SpannedConfig>(&text).ok();
    let mut out = Vec::new();
    for (event, groups) in events {
        // `hooks.state` is a table of trust, not an array of groups.
        let Some(groups) = groups.as_array() else {
            continue;
        };
        for (g, group) in groups.iter().enumerate() {
            let Ok(group) = serde_json::to_value(group) else {
                continue;
            };
            let list = group["hooks"].as_array().cloned().unwrap_or_default();
            for (h, hook) in list.into_iter().enumerate() {
                let Some(command) = hook["command"].as_str().map(str::to_string) else {
                    continue;
                };
                let line = spans
                    .as_ref()
                    .and_then(|spans| spans.start_of(event, g, h))
                    .map(|start| 1 + text[..start].matches('\n').count());
                out.push(TomlHook {
                    event: event.clone(),
                    command,
                    line,
                });
            }
        }
    }
    out
}

/// `config.toml` read for the span of each hook's `command` value only.
#[derive(Deserialize)]
struct SpannedConfig {
    #[serde(default)]
    hooks: BTreeMap<String, SpannedGroups>,
}

impl SpannedConfig {
    /// The byte offset of the `command` value of hook `h` in group `g` of
    /// `event`.
    fn start_of(&self, event: &str, g: usize, h: usize) -> Option<usize> {
        let hook = self.hooks.get(event)?.0.get(g)?.hooks.get(h)?;
        Some(hook.command.as_ref()?.span().start)
    }
}

/// The groups of one event, or none for a member of `hooks` that is a
/// table, such as `hooks.state`.
struct SpannedGroups(Vec<SpannedGroup>);

impl<'de> Deserialize<'de> for SpannedGroups {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArrayOrTable;
        impl<'de> serde::de::Visitor<'de> for ArrayOrTable {
            type Value = SpannedGroups;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an array of hook groups or a table")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<SpannedGroups, A::Error> {
                let mut groups = Vec::new();
                while let Some(group) = seq.next_element::<SpannedGroup>()? {
                    groups.push(group);
                }
                Ok(SpannedGroups(groups))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<SpannedGroups, A::Error> {
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}
                Ok(SpannedGroups(Vec::new()))
            }
        }
        deserializer.deserialize_any(ArrayOrTable)
    }
}

#[derive(Deserialize)]
struct SpannedGroup {
    #[serde(default)]
    hooks: Vec<SpannedHook>,
}

#[derive(Deserialize)]
struct SpannedHook {
    command: Option<toml::Spanned<String>>,
}

/// Codex also reads `[[hooks.<Event>]]` from `config.toml`. That file is
/// rendered by the dotfiles and carries Codex's trust hashes, so rune
/// names the line of a tool's entry whose adapter now runs, instead of
/// rewriting the file. A legacy dispatcher line is never named: it stays.
fn legacy_codex_toml(
    path: &Path,
    hooks: &[TomlHook],
    events: &BTreeMap<String, PlannedEvent>,
    report: &mut Report,
) {
    for hook in hooks {
        let adapters = events.get(&hook.event).map(adapters_of).unwrap_or_default();
        let (by, alone) = match predecessor(&hook.command, &adapters) {
            Some(Predecessor::Alone(by)) => (by, true),
            Some(Predecessor::Compound(by)) => (by, false),
            None => continue,
        };
        let what = if alone {
            format!("is now the {by} handler; remove it from config.toml")
        } else {
            format!("runs {by} with other commands; split it in config.toml")
        };
        let at = hook.line.map(|line| format!(":{line}")).unwrap_or_default();
        report.legacy_toml.push(format!(
            "{}{at}: `{}` {what} by hand, rune does not rewrite that file",
            path.display(),
            hook.command
        ));
    }
}

/// The legacy dispatcher entries of one harness, as the rule reads them
/// in the harness table.
struct Dispatchers<'a> {
    home: &'a Path,
    harness: Harness,
    table: &'a Path,
    /// The `PATH` a bare shell word in a command resolves by.
    path_env: Option<std::ffi::OsString>,
    /// A variable of the install's environment that changes how `sh`
    /// reads a script, when one is set: nothing is covered then.
    shell_variable: Option<String>,
}

/// One hook of an event as the dispatcher scan sees it: in `file`, at
/// `slot`.
struct Found<'a> {
    command: String,
    group: &'a Value,
    hook: &'a Value,
    slot: Slot,
    file: &'a Path,
}

/// Why `found` covers nothing, when its first part would otherwise: the
/// entry has a key rune does not read, or its matcher is not every call.
fn uncovered_because(found: &Found<'_>) -> Option<String> {
    if let Err(why) = cover::plain_entry(found.group, found.hook) {
        return Some(why);
    }
    if !cover::every_tool(found.group) {
        return Some("its matcher does not cover every call".to_string());
    }
    None
}

impl Dispatchers<'_> {
    /// Every hook of the table `groups`.
    fn found<'a>(&'a self, groups: &'a [Value]) -> Vec<Found<'a>> {
        groups
            .iter()
            .enumerate()
            .flat_map(|(g, group)| {
                group["hooks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .map(move |(h, hook)| Found {
                        command: hook["command"].as_str().unwrap_or_default().to_string(),
                        group,
                        hook,
                        slot: (g, h),
                        file: self.table,
                    })
            })
            .collect()
    }

    /// The handlers of `planned` that the first part of a kept legacy
    /// dispatcher entry on `name` covers: the part's command is the
    /// handler's whole command here. Only the first part can cover, since
    /// the dispatcher stops at the first part that exits non-zero. A
    /// dispatcher inside a compound command, one whose script or first
    /// part rune cannot read for sure, one whose folder has a name the
    /// locale may sort first, or one under a matcher that is not every
    /// tool covers nothing, and install says why where the first part
    /// might then run twice. Nothing is covered on Codex: it loads its
    /// hooks and their trust once per session, so a firing cannot read
    /// what the session runs, and a handler skipped there could be lost.
    /// Nothing is covered while a variable that changes how `sh` reads a
    /// script is set.
    ///
    /// `candidates` names the handlers of `planned` a part may cover, each
    /// with the command its adapter runs here. A `deny` handler, one that
    /// rewrites the tool call, a handler with its own arguments, one
    /// `covered` already holds, and one whose adapter reshapes the payload
    /// off Claude are no candidates.
    fn candidates<'a>(
        &self,
        planned: &'a PlannedEvent,
        canonical: &str,
        covered: &BTreeMap<String, cover::Coverage>,
    ) -> Vec<(&'a rune::hooks::PlannedHandler, Vec<String>)> {
        let mut out: Vec<(&'a rune::hooks::PlannedHandler, Vec<String>)> = Vec::new();
        for handler in &planned.handlers {
            if handler.on_failure == OnFailure::Deny
                || handler.rewrites
                || handler.exec.len() != 1
                || covered.contains_key(&handler.id)
                || out.iter().any(|(h, _)| h.id == handler.id)
            {
                continue;
            }
            let Some(adapter) = adapter_of(handler) else {
                continue;
            };
            let Some((runs, shaped)) = cover::runs(adapter, self.harness, canonical, self.home)
            else {
                continue;
            };
            if shaped && self.harness != Harness::Claude {
                continue;
            }
            out.push((handler, runs));
        }
        out
    }

    #[allow(clippy::too_many_lines)]
    fn covered(
        &self,
        name: &str,
        planned: &PlannedEvent,
        groups: &[Value],
        report: &mut Report,
    ) -> BTreeMap<String, cover::Coverage> {
        let (harness, home) = (self.harness, self.home);
        let mut out: BTreeMap<String, cover::Coverage> = BTreeMap::new();
        if harness == Harness::Codex {
            return out;
        }
        let canonical = planned
            .canonical
            .map_or_else(|| name.to_string(), |c| c.as_str().to_string());
        // One install scan reads as many bytes of parts as one firing may.
        let mut budget = legacy::SCAN_LIMIT;
        // The `sh` the dispatcher runs each part with, by the PATH here.
        let inner_sh = legacy::resolve_shell("sh", self.path_env.as_deref());
        let mut shell = |word: &str| legacy::resolve_shell(word, self.path_env.as_deref());
        for found in self.found(groups) {
            let command = &found.command;
            let expanded = legacy::expand_home_in(command, home);
            if expanded.contains(OPERATORS) {
                let joined = expanded
                    .split(OPERATORS)
                    .any(|part| legacy::dispatcher(&legacy::words(part)).is_some());
                if joined {
                    report.warnings.push(format!(
                        "{harness}: {name} reads no part of `{command}`, because it runs the legacy dispatcher with other commands, so a part may run what a handler runs twice; split it by hand, and the next install reads its parts"
                    ));
                }
                continue;
            }
            if legacy::expands(&expanded) {
                continue;
            }
            let Some(dispatcher) = legacy::dispatcher(&legacy::words(&expanded)) else {
                continue;
            };
            let Some(script) = dispatcher.script else {
                continue;
            };
            // The line, the script, the shell in front, and the `sh` the
            // script runs each part with are read only as the shell reads
            // them: one plain line, the known script text, a shell word
            // that resolves to /bin/sh.
            if !legacy::plain_command(command)
                || legacy::script(&script)
                    .check(&script, dispatcher.shell.as_deref())
                    .is_err()
                || dispatcher.shell.as_deref().is_some_and(|word| {
                    legacy::resolve_shell(word, self.path_env.as_deref()).is_none()
                })
                || inner_sh.is_none()
            {
                continue;
            }
            let candidates = self.candidates(planned, &canonical, &out);
            if candidates.is_empty() {
                continue;
            }
            if let Some(variable) = &self.shell_variable {
                report.warnings.push(format!(
                    "{harness}: {name} reads no part of `{command}`, because {variable} is set, which changes how sh reads a script; every handler runs"
                ));
                continue;
            }
            let folder = legacy::folder(home, &dispatcher.event);
            let part = match legacy::first_part(&folder, home, &mut budget) {
                Ok(Some(part)) => part,
                Ok(None) => continue,
                Err(why) => {
                    report.warnings.push(format!(
                        "{harness}: {name} reads no part of `{command}`, because {why}; every handler runs"
                    ));
                    continue;
                }
            };
            if !part.active {
                continue;
            }
            let pairs: Vec<Pair<'_>> = candidates
                .into_iter()
                .filter(|(_, runs)| cover::same_command(&part.exec, runs, &mut shell))
                .map(|(handler, runs)| (handler, &part, runs))
                .collect();
            if pairs.is_empty() {
                continue;
            }
            if let Some(why) = uncovered_because(&found) {
                for (handler, part, _) in &pairs {
                    report.warnings.push(format!(
                        "{harness}: {name} reads no part of `{command}`, because {why}, so the part {} may run what {} runs twice; every handler runs",
                        part.path.display(),
                        handler.id
                    ));
                }
                continue;
            }
            for (handler, part, runs) in pairs {
                report.covered.push(format!(
                    "{harness}: {name} covers {} with the part {} of `{command}`; rune hook run skips {} while that part runs its command",
                    handler.id,
                    part.path.display(),
                    handler.id
                ));
                out.insert(
                    handler.id.clone(),
                    cover::Coverage {
                        home: home.to_path_buf(),
                        handler_exec: handler.exec.clone(),
                        file: found.file.to_path_buf(),
                        slot: found.slot,
                        command: command.clone(),
                        script: script.clone(),
                        shell: dispatcher.shell.clone(),
                        folder: folder.clone(),
                        part: part.path.clone(),
                        exec: part.exec.clone(),
                        runs,
                    },
                );
            }
        }
        out
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

    /// The directory of plan generations the registrations name.
    const PLAN: &str = "/state/hooks/plans";

    /// The entry an install renders for one event of `plan`, naming that
    /// event's generation.
    fn rendered_for(plan: &Plan, harness: Harness, event: &str) -> Value {
        let generation =
            super::super::plan::generation(Path::new(PLAN), plan, harness, event).unwrap();
        rendered(
            "/opt/rune",
            harness,
            event,
            plan.event(harness, event).unwrap(),
            &generation.path.display().to_string(),
        )
    }

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
        let rune = rendered_for(&plan(), Harness::Claude, "PreToolUse");
        assert_eq!(
            commands,
            [
                "sh /Users/x/.sd/hook PreToolUse",
                rune["hooks"][0]["command"].as_str().unwrap()
            ],
            "dcg and git-ai were adopted; the legacy dispatcher stays"
        );
        assert!(
            commands[1].starts_with("/opt/rune hook run --harness claude --native-event PreToolUse --plan /state/hooks/plans/")
                && commands[1].ends_with(".json --on-failure deny"),
            "{commands:?}"
        );
        assert_eq!(
            table["hooks"]["Notification"][0]["hooks"][0]["command"],
            "cmux hooks claude Notification",
            "a foreign entry on an event rune does not want stays"
        );
        assert_eq!(table["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 1);
        assert_eq!(report.adopted.len(), 2, "{report:?}");
        assert!(
            report
                .foreign
                .iter()
                .any(|(_, e, c)| e == "Notification" && c.starts_with("cmux"))
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let manifest: Manifest =
            serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
        let claude = &manifest.harnesses[&Harness::Claude];
        assert_eq!(claude.owned.len(), 3, "PreToolUse, PostToolUse, SessionEnd");
        assert_eq!(claude.foreign.len(), 2, "cmux and the legacy dispatcher");
        assert_eq!(claude.adopted.len(), 2);
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
        let pre = commands("PreToolUse");
        assert_eq!(pre.len(), 2, "{pre:?}");
        assert_eq!(
            pre[0], "sh /Users/x/.sd/hook PreToolUse",
            "the dispatcher stays on the event rune registers too"
        );
        assert!(pre[1].starts_with("/opt/rune hook run"), "{pre:?}");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.adopted.is_empty(), "{:?}", report.adopted);
    }

    /// A home whose Claude table names `named` from a rune registration
    /// and `orphan` from a foreign command, and a generations directory
    /// that holds both and a file that is no generation.
    fn sweep_home() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let home = tempfile::tempdir().unwrap();
        let plans = home.path().join("plans");
        fs::create_dir_all(&plans).unwrap();
        for name in ["named.json", "orphan.json", "notes.txt"] {
            fs::write(plans.join(name), "{}").unwrap();
        }
        let manifest = home.path().join("manifest.json");
        settings(
            home.path(),
            &json!({"hooks": {"PreToolUse": [{"hooks": [
                {"type": "command", "command": format!("/opt/rune hook run --harness claude --native-event PreToolUse --plan {}", plans.join("named.json").display())},
                {"type": "command", "command": format!("/owner/tool --plan {}", plans.join("orphan.json").display())}
            ]}]}})
            .to_string(),
        );
        (home, plans, manifest)
    }

    fn at(text: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn the_sweep_keeps_an_unnamed_generation_and_records_when_it_went() {
        let (home, plans, manifest) = sweep_home();
        let now = at("2026-10-24T12:00:00Z");
        let swept = sweep_generations(home.path(), &plans, &manifest, now).unwrap();
        assert_eq!(swept, Vec::<PathBuf>::new());
        for name in ["named.json", "orphan.json", "notes.txt"] {
            assert!(plans.join(name).exists(), "{name} stays");
        }
        assert_eq!(
            read_manifest(&manifest).retired,
            [retire::fresh(&plans.join("orphan.json"), now, 0)],
            "only rune's commands name a generation"
        );
    }

    #[test]
    fn the_sweep_deletes_only_a_generation_retire_lets_go() {
        let (home, plans, manifest) = sweep_home();
        for name in [
            "recent.json",
            "garbled.json",
            "young.json",
            "uncounted.json",
        ] {
            fs::write(plans.join(name), "{}").unwrap();
        }
        let record = |name: &str, since: &str, install: Option<u64>, days: u32| Retired {
            generation: plans.join(name).display().to_string(),
            since: since.to_string(),
            install,
            days,
            last_day: Some(
                if days == 14 {
                    "2026-10-24"
                } else {
                    "2026-10-23"
                }
                .to_string(),
            ),
        };
        let earlier = Manifest {
            version: 1,
            installs: 7,
            retired: vec![
                // A table names it again, so its record goes.
                record("named.json", "2026-01-01T00:00:00Z", Some(1), 13),
                record("orphan.json", "2026-10-10T12:00:00Z", Some(5), 13),
                record("recent.json", "2026-10-10T12:00:01Z", Some(5), 13),
                // 14 days, but one install since.
                record("young.json", "2026-01-01T00:00:00Z", Some(6), 13),
                record("garbled.json", "last week", Some(1), 13),
                record("uncounted.json", "2026-01-01T00:00:00Z", None, 13),
                // The file is gone, and so is its record.
                record("deleted.json", "2026-10-20T00:00:00Z", Some(6), 13),
            ],
            ..Manifest::default()
        };
        fs::write(&manifest, serde_json::to_string(&earlier).unwrap()).unwrap();
        let now = at("2026-10-24T12:00:00Z");
        let swept = sweep_generations(home.path(), &plans, &manifest, now).unwrap();
        assert_eq!(swept, [plans.join("orphan.json")], "14 days to the second");
        for name in ["named.json", "recent.json", "young.json", "garbled.json"] {
            assert!(plans.join(name).exists(), "{name} stays");
        }
        assert_eq!(
            read_manifest(&manifest).retired,
            [
                retire::fresh(&plans.join("garbled.json"), now, 7),
                record("recent.json", "2026-10-10T12:00:01Z", Some(5), 14),
                retire::fresh(&plans.join("uncounted.json"), now, 7),
                record("young.json", "2026-01-01T00:00:00Z", Some(6), 14),
            ]
        );
    }

    #[test]
    fn a_named_generation_keeps_no_record_and_one_that_stops_starts_afresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = |name: &str| dir.path().join(name);
        let now = at("2026-10-24T12:00:00Z");
        let record = |name: &str, since: &str| Retired {
            since: since.to_string(),
            ..retire::fresh(&path(name), now, 1)
        };
        // `again` was retired, then named again by an install that stopped
        // before its cleanup. `stale` carries a backdated record although
        // the old tables name it. `other` was retired and stays so.
        let records = vec![
            record("again.json", "2026-01-01T00:00:00Z"),
            record("stale.json", "2025-01-01T00:00:00Z"),
            record("other.json", "2026-10-01T00:00:00Z"),
        ];
        let before = [path("stale.json"), path("kept.json")]
            .into_iter()
            .collect();
        let after = [path("again.json"), path("kept.json")]
            .into_iter()
            .collect();
        assert_eq!(
            retire::reconcile(&records, &before, &after, now, 9),
            [
                record("other.json", "2026-10-01T00:00:00Z"),
                retire::fresh(&path("stale.json"), now, 9),
            ]
        );
        // What install writes before the tables keeps only `other`.
        assert_eq!(
            retire::unnamed_only(&records, &before, &after),
            [record("other.json", "2026-10-01T00:00:00Z")]
        );
    }

    /// Run one install of `plan` the way `rune hook install` does, at
    /// `now`, or stop it after the tables changed, before the manifest,
    /// as a killed install would.
    fn install_at(
        plan: &Plan,
        home: &Path,
        plans: &Path,
        manifest: &Path,
        now: chrono::DateTime<chrono::Utc>,
        killed: bool,
    ) {
        let prepared = prepare(plan, home, "/opt/rune", plans, manifest, false, now).unwrap();
        if killed {
            for generation in &prepared.generations {
                super::super::plan::write_generation(generation).unwrap();
            }
            if let Some(early) = &prepared.early {
                super::super::publish::publish(std::slice::from_ref(early)).unwrap();
            }
            let tables = &prepared.files[..prepared.files.len() - 1];
            super::super::publish::publish(tables).unwrap();
            return;
        }
        write(&prepared).unwrap();
        sweep_generations(home, plans, manifest, now).unwrap();
    }

    #[test]
    fn a_killed_install_can_only_delay_a_deletion() {
        let home = tempfile::tempdir().unwrap();
        let plans = home.path().join("plans");
        let manifest = home.path().join("manifest.json");
        let guard = plan();
        let watch = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: dcg\n    exec: [watch]\n    events: [tool.before]\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let generation =
            super::super::plan::generation(&plans, &guard, Harness::Claude, "PreToolUse")
                .unwrap()
                .path;
        let day = |n: u32| at(&format!("2026-10-{n:02}T12:00:00Z"));
        // Day 1 names the guard's generation, day 2 retires it, and daily
        // installs to day 13 count twelve days of its window.
        install_at(&guard, home.path(), &plans, &manifest, day(1), false);
        for n in 2..=13 {
            install_at(&watch, home.path(), &plans, &manifest, day(n), false);
        }
        // Day 14 names it again, and the install dies after the tables,
        // before the manifest.
        install_at(&guard, home.path(), &plans, &manifest, day(14), true);
        // On day 15 the owner removes the entries the manifest does not
        // know, and installs again: the tables stop naming the generation.
        let _ = fs::remove_file(home.path().join(".claude").join("settings.json"));
        let _ = fs::remove_file(home.path().join(".codex").join("hooks.json"));
        // The record from day 2 would end the window on day 16, two days
        // after the tables last named the generation. Its window starts on
        // day 15 instead, and lasts 14 days and 14 install days.
        for n in 15..=28 {
            install_at(&watch, home.path(), &plans, &manifest, day(n), false);
            assert!(
                generation.exists(),
                "the generation is still there on day {n}"
            );
        }
        install_at(&watch, home.path(), &plans, &manifest, day(29), false);
        assert!(!generation.exists(), "the window ends on day 29");
    }

    #[test]
    fn a_removed_entry_prints_as_removed() {
        let report = Report {
            written: vec![
                (Harness::Claude, "PreToolUse".to_string()),
                (Harness::Claude, "Stop removed".to_string()),
            ],
            ..Report::default()
        };
        assert_eq!(
            report.render(),
            "claude: PreToolUse registered\nclaude: Stop removed\n"
        );
    }

    #[test]
    fn only_a_command_that_runs_the_predecessor_alone_is_adopted() {
        let adapters = vec![
            "dcg".to_string(),
            "rtk".to_string(),
            "turn-checkpoint".to_string(),
        ];
        let of = |command: &str| predecessor(command, &adapters);
        // The legacy dispatcher is no predecessor: its entry stays.
        assert_eq!(of("sh ~/.sd/hook PreToolUse"), None);
        assert_eq!(of("/Users/x/.sd/hook Stop"), None);
        assert_eq!(of("sd hook Stop"), None);
        assert_eq!(of("sh ~/.config/just/hook Stop"), None);
        assert_eq!(
            of("/Users/x/.local/bin/dcg"),
            Some(Predecessor::Alone("dcg"))
        );
        assert_eq!(of("~/.local/bin/dcg"), Some(Predecessor::Alone("dcg")));
        // A `$` is an operator: a home spelling is a compound line, which
        // install names for a hand edit and never removes.
        assert_eq!(
            of("$HOME/.local/bin/dcg"),
            Some(Predecessor::Compound("dcg"))
        );
        assert_eq!(
            of("/bin/sh -e $HOME/.local/bin/dcg"),
            Some(Predecessor::Compound("dcg"))
        );
        assert_eq!(
            of("/bin/sh -e ~/.local/bin/dcg"),
            None,
            "a shell in front is the program"
        );
        assert_eq!(
            of("sh $HOME/.config/just/claude/turn/checkpoint begin"),
            None,
            "the checkpoint script is no adapter's name"
        );
        assert_eq!(of("rtk hook claude"), Some(Predecessor::Alone("rtk")));
        // A predecessor's words split at any blank, as main reads them; a
        // part's words split at a space or a tab, as sh reads a line.
        assert_eq!(
            of("rtk\thook\u{a0}claude"),
            Some(Predecessor::Alone("rtk")),
            "a no-break space is a blank to the predecessor reader"
        );
        assert_eq!(
            super::super::legacy::words("rtk\thook\u{a0}claude"),
            ["rtk", "hook\u{a0}claude"]
        );
        assert_eq!(of("sh ~/.sd/hook PreToolUse && /owner/check-policy"), None);
        assert_eq!(
            of("dcg && /owner/check"),
            Some(Predecessor::Compound("dcg"))
        );
        assert_eq!(
            of("/owner/check; rtk hook claude"),
            Some(Predecessor::Compound("rtk"))
        );
        assert_eq!(of("dcg > /tmp/dcg.log"), Some(Predecessor::Compound("dcg")));
        assert_eq!(of("cat ~/.sd/hook"), None);
        assert_eq!(of("/owner/dcg-wrapper"), None);
        assert_eq!(of("/owner/my-guard"), None);
    }

    #[test]
    fn a_compound_legacy_command_stays_and_is_named_for_a_hand_split() {
        let home = tempfile::tempdir().unwrap();
        let compound = "sh ~/.sd/hook PreToolUse && /owner/check-policy";
        let path = settings(
            home.path(),
            &json!({"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": compound}]}]}})
                .to_string(),
        );
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
        assert_eq!(
            table["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            compound
        );
        assert!(report.adopted.is_empty(), "{report:?}");
        assert!(report.covered.is_empty(), "{report:?}");
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains(compound) && w.contains("split it by hand")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_predecessor_is_adopted_only_where_its_adapter_runs() {
        let home = tempfile::tempdir().unwrap();
        let claude = settings(home.path(), "{}");
        let codex = home.path().join(".codex").join("hooks.json");
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        let dcg = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/Users/x/.local/bin/dcg"}]}],"PostToolUse":[{"hooks":[{"type":"command","command":"/Users/x/.local/bin/dcg"}]}]}}"#;
        fs::write(&claude, dcg).unwrap();
        fs::write(&codex, dcg).unwrap();
        // A handler named dcg on Claude only, and another adapter named dcg
        // nowhere: the id decides nothing, the executable on the event does.
        let claude_only: Plan = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: dcg\n    exec: [rune-hook-dcg]\n    events: [claude:PreToolUse]\n  - id: lint\n    exec: [rune-hook-lint-on-write]\n    events: [tool.after]\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let manifest = home.path().join("state").join("manifest.json");
        let report = apply(
            &claude_only,
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            false,
            false,
        )
        .unwrap();
        let commands = |path: &Path, event: &str| -> Vec<String> {
            let table: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
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
        assert!(
            !commands(&claude, "PreToolUse").contains(&"/Users/x/.local/bin/dcg".to_string()),
            "{report:?}"
        );
        assert!(commands(&claude, "PostToolUse").contains(&"/Users/x/.local/bin/dcg".to_string()));
        assert!(commands(&codex, "PreToolUse").contains(&"/Users/x/.local/bin/dcg".to_string()));
        assert_eq!(report.adopted.len(), 1, "{report:?}");
    }

    #[test]
    fn the_manifest_keeps_what_was_adopted_and_check_sees_a_new_foreign_entry() {
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
        let read: Manifest = serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
        assert_eq!(
            read.harnesses[&Harness::Claude].adopted.len(),
            2,
            "the second install keeps the first one's adoptions"
        );
        let report = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            true,
            false,
        )
        .unwrap();
        assert!(
            !report.drift.iter().any(|d| d.contains("settings.json")),
            "{:?}",
            report.drift
        );
        let mut table: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        table["hooks"]["Stop"] =
            json!([{"hooks": [{"type": "command", "command": "/new/unknown-hook"}]}]);
        fs::write(&path, table.to_string()).unwrap();
        let report = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            true,
            false,
        )
        .unwrap();
        assert!(
            report
                .drift
                .iter()
                .any(|d| d.contains("/new/unknown-hook") && d.contains("Stop")),
            "{:?}",
            report.drift
        );
    }

    /// Trust the rune entry for `event` the way `/hooks` records it.
    fn trust_in_codex(home: &Path, event: &str) {
        trust_rune_entry(home, event, false);
    }

    /// Trust the first rune entry for `event`, or the last one.
    fn trust_rune_entry(home: &Path, event: &str, last: bool) {
        let table_path = home.join(".codex").join("hooks.json");
        let table: Value = serde_json::from_str(&fs::read_to_string(&table_path).unwrap()).unwrap();
        let groups = table["hooks"][event].as_array().unwrap();
        let slots: Vec<Slot> = groups
            .iter()
            .enumerate()
            .flat_map(|(g, group)| {
                group["hooks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .filter(|(_, hook)| is_owned(hook["command"].as_str().unwrap(), Harness::Codex))
                    .map(move |(h, _)| (g, h))
            })
            .collect();
        let slot = if last {
            slots[slots.len() - 1]
        } else {
            slots[0]
        };
        trust_codex_slot(home, event, slot);
    }

    /// Trust the hook that runs `command` on Codex's `event`.
    fn trust_codex_hook(home: &Path, event: &str, command: &str) {
        let table: Value = serde_json::from_str(
            &fs::read_to_string(home.join(".codex").join("hooks.json")).unwrap(),
        )
        .unwrap();
        let slot = table["hooks"][event]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .find_map(|(g, group)| {
                group["hooks"]
                    .as_array()?
                    .iter()
                    .position(|hook| hook["command"] == command)
                    .map(|h| (g, h))
            })
            .unwrap();
        trust_codex_slot(home, event, slot);
    }

    /// Trust the hook at `(g, h)` of Codex's `event` the way `/hooks`
    /// records it.
    fn trust_codex_slot(home: &Path, event: &str, (g, h): Slot) {
        let table_path = home.join(".codex").join("hooks.json");
        let table: Value = serde_json::from_str(&fs::read_to_string(&table_path).unwrap()).unwrap();
        let groups = table["hooks"][event].as_array().unwrap();
        let hash = codex_trust::hash(event, &groups[g], &groups[g]["hooks"][h]);
        let label = codex_trust::label(event);
        let config = home.join(".codex").join("config.toml");
        let mut text = fs::read_to_string(&config).unwrap_or_default();
        let _ = write!(
            text,
            "[hooks.state.\"{}:{label}:{g}:{h}\"]\ntrusted_hash = \"{hash}\"\n",
            table_path.display()
        );
        fs::write(&config, text).unwrap();
    }

    /// `plan()` with an observer on the guarded event, so the event's
    /// plan, generation, and command change.
    fn plan_with_observer() -> Plan {
        serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: dcg\n    exec: [rune-hook-dcg]\n    events: [tool.before]\n    on_failure: deny\n  - id: watch\n    exec: [watch]\n    events: [tool.before]\n  - id: git-ai\n    exec: [rune-hook-git-ai]\n    events: [tool.before, tool.after]\n  - id: capture\n    exec: [rune-hook-capture]\n    events: [session.end]\n",
        )
        .unwrap()
        .compile()
        .unwrap()
    }

    #[test]
    fn removing_rune_s_earlier_entry_waits_while_it_would_move_a_trusted_hook() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        let manifest = home.path().join("state").join("manifest.json");
        let table = home.path().join(".codex").join("hooks.json");
        let run = |plan: &Plan, check: bool| {
            apply(
                plan,
                home.path(),
                "/opt/rune",
                PLAN,
                &manifest,
                check,
                false,
            )
            .unwrap()
        };
        run(&plan(), false);
        // The owner adds a guard to rune's group, after rune's entry.
        let mut value: Value = serde_json::from_str(&fs::read_to_string(&table).unwrap()).unwrap();
        value["hooks"]["PreToolUse"][0]["hooks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "command", "command": "/owner/guard"}));
        fs::write(&table, value.to_string()).unwrap();
        run(&plan(), false);
        trust_in_codex(home.path(), "PreToolUse");
        // A hook in rune's group that Codex does not trust is drift.
        let check = run(&plan(), true);
        assert!(
            check
                .drift
                .iter()
                .any(|d| d.contains("`/owner/guard` on PreToolUse at group 1 handler 2")),
            "{:?}",
            check.drift
        );
        trust_codex_hook(home.path(), "PreToolUse", "/owner/guard");
        // The plan changes, and the owner trusts the new entry.
        run(&plan_with_observer(), false);
        trust_rune_entry(home.path(), "PreToolUse", true);
        // Removing the earlier entry would move the guard to handler 1.
        let report = run(&plan_with_observer(), false);
        let group: Value = serde_json::from_str::<Value>(&fs::read_to_string(&table).unwrap())
            .unwrap()["hooks"]["PreToolUse"][0]
            .clone();
        assert_eq!(group["hooks"].as_array().unwrap().len(), 2, "{group}");
        assert_eq!(group["hooks"][1]["command"], "/owner/guard");
        assert!(
            report.trust.iter().any(|t| t.contains("keeps rune's earlier entry, because removing it would move `/owner/guard` from group 1 handler 2 to group 1 handler 1")),
            "{:?}",
            report.trust
        );
        let check = run(&plan_with_observer(), true);
        assert!(
            check
                .drift
                .iter()
                .any(|d| d.contains("would move `/owner/guard`")),
            "{:?}",
            check.drift
        );
    }

    /// The rune entries of Codex's `event`, group by group.
    fn codex_rune_groups(home: &Path, event: &str) -> Vec<usize> {
        let table: Value = serde_json::from_str(
            &fs::read_to_string(home.join(".codex").join("hooks.json")).unwrap(),
        )
        .unwrap();
        table["hooks"][event]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| {
                group["hooks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|hook| is_owned(hook["command"].as_str().unwrap(), Harness::Codex))
                    .count()
            })
            .collect()
    }

    #[test]
    fn rune_s_trusted_codex_entry_stays_until_codex_trusts_its_replacement() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        let manifest = home.path().join("state").join("manifest.json");
        let run = |plan: &Plan, check: bool| {
            apply(
                plan,
                home.path(),
                "/opt/rune",
                PLAN,
                &manifest,
                check,
                false,
            )
            .unwrap()
        };
        run(&plan(), false);
        trust_in_codex(home.path(), "PreToolUse");
        // An observer joins the guarded event: its plan, generation, and
        // command change, and Codex does not trust the new command yet.
        let changed = plan_with_observer();
        let report = run(&changed, false);
        assert_eq!(
            codex_rune_groups(home.path(), "PreToolUse"),
            [1, 1],
            "the trusted entry stays, and the new one has a group of its own"
        );
        assert!(
            report.trust.iter().any(
                |t| t.contains("keeps rune's earlier entry") && t.contains("group 2 handler 1")
            ),
            "{:?}",
            report.trust
        );
        let check = run(&changed, true);
        assert!(
            check.drift.iter().any(|d| d.contains("PreToolUse")),
            "{:?}",
            check.drift
        );
        // A second install before the owner trusts it changes nothing.
        run(&changed, false);
        assert_eq!(codex_rune_groups(home.path(), "PreToolUse"), [1, 1]);
        // The owner trusts the new entry; the next install removes the
        // earlier one and leaves its group as a slot, so the new one keeps
        // its position and its trust.
        trust_rune_entry(home.path(), "PreToolUse", true);
        let report = run(&changed, false);
        assert!(
            report
                .written
                .iter()
                .any(|(h, e)| *h == Harness::Codex && e == "PreToolUse earlier entry removed"),
            "{report:?}"
        );
        assert_eq!(codex_rune_groups(home.path(), "PreToolUse"), [0, 1]);
        let check = run(&changed, true);
        assert!(
            !check.drift.iter().any(|d| d.contains("PreToolUse")),
            "{:?}",
            check.drift
        );
    }

    #[test]
    fn a_codex_predecessor_stays_until_codex_trusts_its_replacement() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex").join("hooks.json");
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        fs::write(
            &codex,
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/Users/x/.local/bin/dcg"}]}]}}"#,
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
        let commands = || -> Vec<String> {
            let table: Value = serde_json::from_str(&fs::read_to_string(&codex).unwrap()).unwrap();
            table["hooks"]["PreToolUse"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|g| g["hooks"].as_array().unwrap().clone())
                .map(|h| h["command"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(commands()[0], "/Users/x/.local/bin/dcg", "dcg still guards");
        assert!(
            report
                .trust
                .iter()
                .any(|t| t.contains("PreToolUse") && t.contains("not trusted yet")),
            "{:?}",
            report.trust
        );
        let check = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            true,
            false,
        )
        .unwrap();
        assert!(
            check
                .drift
                .iter()
                .any(|d| d.contains("PreToolUse") && d.contains("not trusted yet")),
            "{:?}",
            check.drift
        );
        // The owner trusts the rune entry in /hooks; the next install
        // adopts dcg and leaves a slot, so the rune entry keeps its place.
        trust_in_codex(home.path(), "PreToolUse");
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
            report
                .adopted
                .iter()
                .any(|(h, e, _)| *h == Harness::Codex && e == "PreToolUse"),
            "{report:?}"
        );
        let table: Value = serde_json::from_str(&fs::read_to_string(&codex).unwrap()).unwrap();
        assert_eq!(table["hooks"]["PreToolUse"][0], json!({"hooks": []}));
        assert!(is_owned(
            table["hooks"]["PreToolUse"][1]["hooks"][0]["command"]
                .as_str()
                .unwrap(),
            Harness::Codex
        ));
        let check = apply(
            &plan(),
            home.path(),
            "/opt/rune",
            PLAN,
            &manifest,
            true,
            false,
        )
        .unwrap();
        assert!(
            !check.drift.iter().any(|d| d.contains("PreToolUse")),
            "{:?}",
            check.drift
        );
    }

    #[test]
    fn a_codex_predecessor_before_rune_in_its_group_stays_although_rune_is_trusted() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex").join("hooks.json");
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        let plan = plan();
        let rune = rendered_for(&plan, Harness::Codex, "PreToolUse");
        let write = |hooks: Vec<Value>| {
            let table = json!({"hooks": {"PreToolUse": [{"hooks": hooks}]}});
            fs::write(&codex, table.to_string()).unwrap();
        };
        let dcg = json!({"type": "command", "command": "/Users/x/.local/bin/dcg"});
        write(vec![dcg, rune["hooks"][0].clone()]);
        // Codex trusts the rune entry at group 1 handler 2. Removing dcg
        // would move it to handler 1, where Codex trusts nothing.
        trust_in_codex(home.path(), "PreToolUse");
        let manifest = home.path().join("state").join("manifest.json");
        let run = |check: bool| {
            apply(
                &plan,
                home.path(),
                "/opt/rune",
                PLAN,
                &manifest,
                check,
                false,
            )
            .unwrap()
        };
        let report = run(false);
        let table: Value = serde_json::from_str(&fs::read_to_string(&codex).unwrap()).unwrap();
        assert_eq!(
            table["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "/Users/x/.local/bin/dcg",
            "dcg still guards"
        );
        assert!(report.adopted.is_empty(), "{report:?}");
        assert!(
            report
                .trust
                .iter()
                .any(|t| t.contains("would move it from group 1 handler 2 to group 1 handler 1")),
            "{:?}",
            report.trust
        );
        let check = run(true);
        assert!(
            check
                .drift
                .iter()
                .any(|d| d.contains("PreToolUse") && d.contains("would move")),
            "{:?}",
            check.drift
        );
        // The owner removes dcg by hand and trusts the rune entry where
        // it now sits: the check passes.
        write(vec![rune["hooks"][0].clone()]);
        trust_in_codex(home.path(), "PreToolUse");
        let check = run(true);
        assert!(
            !check.drift.iter().any(|d| d.contains("PreToolUse")),
            "{:?}",
            check.drift
        );
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
        // A comment, an inline table, a single-quoted string: TOML as Codex
        // reads it. The dispatcher line stays unnamed; dcg's is named by
        // its real line, not the comment above that quotes it.
        fs::write(
            codex.join("config.toml"),
            "# rendered by the dotfiles\n# command = '$HOME/.local/bin/dcg' moved below\n[[hooks.PreToolUse]]\nhooks = [{ type = \"command\", command = 'sh /Users/x/.sd/hook PreToolUse' }]\n\n[[hooks.PreToolUse]]\nmatcher = 'Bash'\n[[hooks.PreToolUse.hooks]]\ncommand = '$HOME/.local/bin/dcg'\ntype = \"command\"\n",
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
        assert_eq!(report.legacy_toml.len(), 1, "{report:?}");
        assert!(
            report.legacy_toml[0].contains("config.toml:9")
                && report.legacy_toml[0].contains("dcg")
                && report.legacy_toml[0].contains("split it in config.toml by hand"),
            "{report:?}"
        );
        assert!(
            report.trust.iter().any(|t| t.contains("/hooks")),
            "{report:?}"
        );
        let hooks: Value =
            serde_json::from_str(&fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
        assert_eq!(
            hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            rendered_for(&plan(), Harness::Codex, "PreToolUse")["hooks"][0]["command"]
        );
        assert!(
            hooks["hooks"].get("Notification").is_none(),
            "codex has no notification event"
        );
    }

    /// The owner's dispatcher at `.config/just/hook`, the parts it runs
    /// for each event under `.config/just/hooks/<Event>.d/`, the turn
    /// checkpoint script the `turn-checkpoint` adapter runs, and the tmux
    /// status binary the `tmux-status` adapter runs.
    fn owner_home(parts: &[(&str, &str, &str)]) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let just = home.path().join(".config").join("just");
        fs::create_dir_all(just.join("claude").join("turn")).unwrap();
        fs::write(just.join("hook"), super::super::legacy::tests::SCRIPT).unwrap();
        fs::write(
            just.join("claude").join("turn").join("checkpoint"),
            "#!/bin/sh\n",
        )
        .unwrap();
        let bin = home.path().join(".local").join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("claude-tmux-status"), "#!/bin/sh\n").unwrap();
        for (event, name, text) in parts {
            let dir = just.join("hooks").join(format!("{event}.d"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(name), text).unwrap();
        }
        home
    }

    /// The cover `prepared` stages for `event` on `harness`, parsed, or
    /// `None` when it stages none.
    fn cover_in(prepared: &Prepared, harness: Harness, event: &str) -> Option<cover::Cover> {
        let name = format!("{harness}-{event}.json");
        prepared
            .covers
            .iter()
            .find(|staged| staged.target.ends_with(&name))
            .map(|staged| serde_json::from_str(&staged.text).unwrap())
    }

    /// A runner that runs every handler as a pass and logs its id, with
    /// the hook record under `record` and `cover` checked under `home`.
    fn covered_runner(
        home: &Path,
        record: &Path,
        cover: cover::Cover,
    ) -> (
        super::super::run::Runner,
        std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    ) {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let log = std::rc::Rc::clone(&seen);
        let runner = super::super::run::Runner {
            record_dir: Some(record.to_path_buf()),
            record_error: None,
            log_dir: None,
            cover,
            env: cover::Environment::under(home),
            started: std::time::Instant::now(),
            spawn: Box::new(move |handler, _, _, _, _| {
                log.borrow_mut().push(handler.id.clone());
                Ok((Some(0), "{}".to_string(), String::new()))
            }),
        };
        (runner, seen)
    }

    /// The owner's live `Stop.d`: tmux attention, turn wrap, turn snapshot.
    const STOP_PARTS: &[(&str, &str, &str)] = &[
        (
            "Stop",
            "10-tmux-attention.sh",
            "#!/bin/sh\nexec \"$HOME/.local/bin/claude-tmux-status\" attention\n",
        ),
        (
            "Stop",
            "20-turn-wrap.sh",
            "#!/bin/sh\nexec sh \"$HOME/.config/just/claude/turn/wrap\"\n",
        ),
        (
            "Stop",
            "30-turn-snapshot.sh",
            "#!/bin/sh\nexec sh \"$HOME/.config/just/claude/turn/checkpoint\" snapshot\n",
        ),
    ];

    /// tmux-status, turn-checkpoint, and session-capture on turn.finish.
    fn stop_plan() -> Plan {
        serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: tmux\n    exec: [rune-hook-tmux-status]\n    events: [turn.finish]\n  - id: turn\n    exec: [rune-hook-turn-checkpoint]\n    events: [turn.finish]\n  - id: capture\n    exec: [rune-hook-session-capture]\n    events: [turn.finish]\n",
        )
        .unwrap()
        .compile()
        .unwrap()
    }

    /// The generation `prepared` wrote for `event` on `harness`, or `None`
    /// when it registers nothing there.
    fn generation_in<'a>(
        prepared: &'a Prepared,
        harness: Harness,
        event: &str,
    ) -> Option<&'a super::super::plan::Generation> {
        prepared.generations.iter().find(|generation| {
            let plan: Plan = serde_json::from_str(&generation.text).unwrap();
            plan.event(harness, event).is_some()
        })
    }

    /// The handler ids in the generation `prepared` wrote for `event` on
    /// `harness`, or `None` when it registers nothing there.
    fn handlers_in(prepared: &Prepared, harness: Harness, event: &str) -> Option<Vec<String>> {
        let generation = generation_in(prepared, harness, event)?;
        let plan: Plan = serde_json::from_str(&generation.text).unwrap();
        let planned = plan.event(harness, event)?;
        Some(planned.handlers.iter().map(|h| h.id.clone()).collect())
    }

    /// Every handler of [`stop_plan`], as a generation lists them.
    fn every_stop_handler() -> Vec<String> {
        vec![
            "capture".to_string(),
            "tmux".to_string(),
            "turn".to_string(),
        ]
    }

    fn commands_in(prepared: &Prepared, file: &str, event: &str) -> Vec<String> {
        let staged = prepared
            .files
            .iter()
            .find(|staged| staged.target.ends_with(file))
            .unwrap_or_else(|| panic!("{file} is staged: {:?}", prepared.files));
        let table: Value = serde_json::from_str(&staged.text).unwrap();
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
    }

    #[test]
    fn the_owner_s_stop_parts_cover_tmux_alone_on_claude() {
        let home = owner_home(STOP_PARTS);
        let dispatcher = "sh $HOME/.config/just/hook Stop";
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let manifest = home.path().join("state").join("manifest.json");
        let prepared = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            Path::new(PLAN),
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        let report = &prepared.report;
        let claude = commands_in(&prepared, "settings.json", "Stop");
        assert_eq!(claude[0], dispatcher, "the entry stays: {claude:?}");
        assert_eq!(claude.len(), 2, "{claude:?}");
        assert_eq!(
            handlers_in(&prepared, Harness::Claude, "Stop"),
            Some(every_stop_handler()),
            "every handler stays in the generation: {report:?}"
        );
        assert_eq!(
            handlers_in(&prepared, Harness::Codex, "Stop"),
            Some(every_stop_handler())
        );
        let stop_d = home.path().join(".config/just/hooks/Stop.d");
        // Only the first part covers: a failing earlier part ends the
        // dispatcher before a later one runs. The turn checkpoint part is
        // third, so turn runs twice.
        assert_eq!(report.covered.len(), 1, "{report:?}");
        assert!(
            report.covered[0].starts_with("claude: Stop covers tmux with the part ")
                && report.covered[0]
                    .contains(&stop_d.join("10-tmux-attention.sh").display().to_string())
                && report.covered[0].contains(dispatcher)
                && report.covered[0]
                    .ends_with("rune hook run skips tmux while that part runs its command"),
            "{:?}",
            report.covered
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.adopted.is_empty(), "{:?}", report.adopted);
        let summary = report.summary();
        assert_eq!(summary.covered, report.covered);
        assert!(
            summary
                .lines()
                .iter()
                .any(|l| l.starts_with("covered: claude: Stop covers tmux"))
        );
        // The cover file names the entry, the script, and the part per
        // handler, and goes beside the generations.
        assert_eq!(prepared.covers.len(), 1, "{:?}", prepared.covers);
        assert_eq!(
            prepared.covers[0].target,
            Path::new("/state/hooks/cover/claude-Stop.json")
        );
        let cover = cover_in(&prepared, Harness::Claude, "Stop").unwrap();
        assert_eq!(
            cover.handlers.keys().collect::<Vec<_>>(),
            ["tmux"],
            "{cover:?}"
        );
        let tmux = &cover.handlers["tmux"];
        assert_eq!(tmux.command, dispatcher);
        assert_eq!(tmux.file, home.path().join(".claude/settings.json"));
        assert_eq!(tmux.slot, (0, 0));
        assert_eq!(tmux.script, home.path().join(".config/just/hook"));
        assert_eq!(tmux.shell.as_deref(), Some("sh"));
        assert_eq!(tmux.folder, stop_d);
        assert_eq!(tmux.part, stop_d.join("10-tmux-attention.sh"));
        assert_eq!(tmux.exec, tmux.runs);
        assert_eq!(tmux.runs[1], "attention");
        assert_eq!(cover_in(&prepared, Harness::Codex, "Stop"), None);
    }

    #[test]
    fn a_covered_handler_is_skipped_at_each_firing_and_runs_once_its_part_goes() {
        let home = owner_home(STOP_PARTS);
        let dispatcher = "sh ~/.config/just/hook Stop";
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let state = home.path().join("state");
        let manifest = state.join("manifest.json");
        let plans = state.join("hooks").join("plans");
        let first = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            &plans,
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        write(&first).unwrap();
        let cover_path = state.join("hooks/cover/claude-Stop.json");
        let generation = generation_in(&first, Harness::Claude, "Stop")
            .unwrap()
            .clone();
        assert_eq!(
            super::super::cover::path_beside(&generation.path, Harness::Claude, "Stop").as_deref(),
            Some(cover_path.as_path())
        );
        let plan: Plan = serde_json::from_str(&generation.text).unwrap();
        let record = state.join("record");
        let fire = || {
            let cover = super::super::cover::read(&cover_path, Harness::Claude, "Stop");
            let (runner, seen) = covered_runner(home.path(), &record, cover);
            let raw = json!({"hook_event_name": "Stop", "session_id": "s", "cwd": "/"});
            let answer =
                super::super::run::dispatch(&plan, Harness::Claude, "Stop", Ok(raw), &runner);
            assert_eq!(answer.exit, 0, "{answer:?}");
            seen.borrow().clone()
        };
        assert_eq!(
            fire(),
            ["capture", "turn"],
            "tmux is skipped; turn is not first, so it runs"
        );
        let records = super::super::record::load(&record).unwrap();
        let skipped: BTreeMap<String, Option<String>> = records
            .iter()
            .map(|r| (r.hook.clone(), r.skipped.clone()))
            .collect();
        let stop_d = home.path().join(".config/just/hooks/Stop.d");
        assert_eq!(
            skipped,
            BTreeMap::from([
                ("capture".to_string(), None),
                (
                    "tmux".to_string(),
                    Some(stop_d.join("10-tmux-attention.sh").display().to_string())
                ),
                ("turn".to_string(), None),
            ]),
            "{records:?}"
        );
        let tmux = records.iter().find(|r| r.hook == "tmux").unwrap();
        assert_eq!(tmux.duration_ms, Some(0));
        assert_eq!(tmux.exit_status, None);
        assert_eq!(tmux.asked, "allow");
        // The tmux part goes: tmux runs at the next firing, with no
        // install in between. The wrap part is first now and covers no
        // handler, so the snapshot part behind it covers nothing either.
        fs::remove_file(stop_d.join("10-tmux-attention.sh")).unwrap();
        assert_eq!(fire(), ["capture", "tmux", "turn"]);
        // The next install changes no registration: only the cover, which
        // empties, because the wrap part runs no handler's command.
        let second = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            &plans,
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(
            !second
                .report
                .written
                .iter()
                .any(|(h, e)| *h == Harness::Claude && e == "Stop"),
            "the registration stays: {:?}",
            second.report.written
        );
        assert!(second.report.covered.is_empty(), "{:?}", second.report);
        let cover = cover_in(&second, Harness::Claude, "Stop").unwrap();
        assert!(cover.handlers.is_empty(), "{cover:?}");
        assert_eq!(
            generation_in(&second, Harness::Claude, "Stop"),
            Some(&generation)
        );
    }

    #[test]
    fn the_owner_s_guards_keep_running_beside_dcg_and_jj_guards() {
        let guard = "#!/bin/sh\ncase \"$tool\" in Bash) ;; *) exit 0 ;; esac\nprintf '%s' \"$cmd\" | grep -q 'jj abandon' && exit 2\nexit 0\n";
        let home = owner_home(&[
            ("PreToolUse", "10-jj-abandon.sh", guard),
            ("PreToolUse", "20-jj-squash.sh", guard),
            ("PreToolUse", "30-jj-rebase.sh", guard),
        ]);
        let dispatcher = "sh $HOME/.config/just/hook PreToolUse";
        let dcg = "~/.local/bin/dcg";
        settings(
            home.path(),
            &json!({"hooks": {"PreToolUse": [
                {"matcher": "Bash", "hooks": [{"type": "command", "command": dcg}]},
                {"hooks": [{"type": "command", "command": dispatcher}]},
            ]}})
            .to_string(),
        );
        let guards: Plan = serde_yaml::from_str::<Hooks>(
            "handlers:\n  - id: dcg\n    exec: [rune-hook-dcg]\n    events: [tool.before]\n    on_failure: deny\n  - id: jj\n    exec: [rune-hook-jj-guards]\n    events: [tool.before]\n    on_failure: deny\n",
        )
        .unwrap()
        .compile()
        .unwrap();
        let manifest = home.path().join("state").join("manifest.json");
        let prepared = prepare(
            &guards,
            home.path(),
            "/opt/rune",
            Path::new(PLAN),
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        let report = &prepared.report;
        let claude = commands_in(&prepared, "settings.json", "PreToolUse");
        assert_eq!(claude.len(), 2, "{claude:?}");
        assert_eq!(claude[0], dispatcher, "the entry stays: {claude:?}");
        assert!(claude[1].starts_with("/opt/rune hook run"), "{claude:?}");
        assert_eq!(
            handlers_in(&prepared, Harness::Claude, "PreToolUse"),
            Some(vec!["dcg".to_string(), "jj".to_string()]),
            "a part with no exec line matches nothing: both guards run"
        );
        assert!(report.covered.is_empty(), "{report:?}");
        assert!(prepared.covers.is_empty(), "{:?}", prepared.covers);
        assert!(
            report
                .adopted
                .iter()
                .any(|(_, e, c)| e == "PreToolUse" && c == dcg),
            "dcg follows the adoption path: {:?}",
            report.adopted
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn a_deny_handler_is_never_covered_even_by_a_part_that_runs_its_program() {
        let home = owner_home(&[("PreToolUse", "10-dcg.sh", "#!/bin/sh\nexec dcg\n")]);
        let dispatcher = "sh $HOME/.config/just/hook PreToolUse";
        settings(
            home.path(),
            &json!({"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let manifest = home.path().join("state").join("manifest.json");
        let prepared = prepare(
            &plan(),
            home.path(),
            "/opt/rune",
            Path::new(PLAN),
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        assert_eq!(
            handlers_in(&prepared, Harness::Claude, "PreToolUse"),
            Some(vec!["dcg".to_string(), "git-ai".to_string()])
        );
        assert!(prepared.report.covered.is_empty(), "{:?}", prepared.report);
        assert!(prepared.covers.is_empty(), "{:?}", prepared.covers);
        assert!(prepared.report.warnings.is_empty(), "{:?}", prepared.report);
    }

    #[test]
    fn a_rewriter_is_never_covered_even_by_a_part_that_runs_its_command() {
        // A rewriter's output is the call the handlers after it read. A
        // part's output is not: with rtk covered, dcg read `git status`
        // while Claude ran `rtk git status`.
        let home = owner_home(&[(
            "PreToolUse",
            "10-rtk.sh",
            "#!/bin/sh\nexec rtk hook claude\n",
        )]);
        let dispatcher = "sh $HOME/.config/just/hook PreToolUse";
        settings(
            home.path(),
            &json!({"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let manifest = home.path().join("state").join("manifest.json");
        let at = |rewrites: bool| {
            let plan = serde_yaml::from_str::<Hooks>(&format!(
                "handlers:\n  - id: rtk\n    exec: [rune-hook-rtk]\n    events: [tool.before]\n    rewrites: {rewrites}\n"
            ))
            .unwrap()
            .compile()
            .unwrap();
            prepare(
                &plan,
                home.path(),
                "/opt/rune",
                Path::new(PLAN),
                &manifest,
                false,
                chrono::Utc::now(),
            )
            .unwrap()
        };
        let plain = at(false);
        assert_eq!(plain.report.covered.len(), 1, "{:?}", plain.report);
        assert!(
            plain.report.covered[0].starts_with("claude: PreToolUse covers rtk"),
            "{:?}",
            plain.report
        );
        let rewriter = at(true);
        assert!(rewriter.report.covered.is_empty(), "{:?}", rewriter.report);
        assert!(rewriter.covers.is_empty(), "{:?}", rewriter.covers);
        assert!(rewriter.report.warnings.is_empty(), "{:?}", rewriter.report);
    }

    #[test]
    fn a_variable_that_changes_how_sh_reads_a_script_covers_nothing_at_install_and_at_the_firing() {
        // SHELLOPTS=noglob reaches the dispatcher from the harness's
        // environment, and its glob then lists no part. Install under such
        // a variable warns and covers nothing; a firing under one skips
        // nothing.
        let home = owner_home(STOP_PARTS);
        let dispatcher = "sh $HOME/.config/just/hook Stop";
        let table = settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let plan = stop_plan();
        let planned = plan.event(Harness::Claude, "Stop").unwrap();
        let groups = Table::read(&table).unwrap().groups("Stop");
        let system = std::env::join_paths([Path::new("/bin")]).unwrap();
        if legacy::resolve_shell("sh", Some(&system)).is_none() {
            return;
        }
        let with = |variable: Option<&str>| {
            let dispatchers = Dispatchers {
                home: home.path(),
                harness: Harness::Claude,
                table: &table,
                path_env: Some(system.clone()),
                shell_variable: variable.map(str::to_string),
            };
            let mut report = Report::default();
            let covered = dispatchers.covered("Stop", planned, &groups, &mut report);
            (covered.keys().cloned().collect::<Vec<_>>(), report.warnings)
        };
        let (covered, warnings) = with(None);
        assert_eq!(covered, ["tmux"]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let (covered, warnings) = with(Some("SHELLOPTS"));
        assert!(covered.is_empty(), "{covered:?}");
        assert_eq!(
            warnings,
            [format!(
                "claude: Stop reads no part of `{dispatcher}`, because SHELLOPTS is set, which changes how sh reads a script; every handler runs"
            )]
        );
        let installed = installed(&json!({"type": "command", "command": PLAIN_ENTRY}));
        let mut env = cover::Environment::under(installed.home.path());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(())
        );
        env.shell_variable = Some("BASH_FUNC_ls%%".to_string());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err(
                "BASH_FUNC_ls%% is set, which changes how sh reads the dispatcher and its parts"
                    .to_string()
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_part_that_expands_past_home_covers_nothing_at_install_and_at_the_firing() {
        // `$OTHER` in the part is a folder named `$OTHER` to rune and
        // whatever the environment says to sh. Even with such a folder in
        // place, the part has no shape.
        let home = owner_home(STOP_PARTS);
        let local = home.path().join(".local");
        std::os::unix::fs::symlink(local.join("bin"), local.join("$OTHER")).unwrap();
        let stop_d = home.path().join(".config/just/hooks/Stop.d");
        let part = stop_d.join("10-tmux-attention.sh");
        let other = "#!/bin/sh\nexec \"$HOME/.local/$OTHER/claude-tmux-status\" attention\n";
        let backquote =
            "#!/bin/sh\nexec `echo \"$HOME/.local/bin/claude-tmux-status\"` attention\n";
        let dispatcher = "sh $HOME/.config/just/hook Stop";
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let state = home.path().join("state");
        let at = || {
            prepare(
                &stop_plan(),
                home.path(),
                "/opt/rune",
                &state.join("hooks").join("plans"),
                &state.join("manifest.json"),
                false,
                chrono::Utc::now(),
            )
            .unwrap()
        };
        let unshaped = format!(
            "claude: Stop reads no part of `{dispatcher}`, because {} has a shape rune does not read; every handler runs",
            part.display()
        );
        for text in [other, backquote] {
            fs::write(&part, text).unwrap();
            let prepared = at();
            assert!(prepared.report.covered.is_empty(), "{:?}", prepared.report);
            assert_eq!(
                prepared.report.warnings,
                std::slice::from_ref(&unshaped),
                "{text}"
            );
        }
        // Installed with the plain part, then edited: the firing refuses.
        fs::write(&part, STOP_PARTS[0].2).unwrap();
        let prepared = at();
        assert_eq!(prepared.report.covered.len(), 1, "{:?}", prepared.report);
        write(&prepared).unwrap();
        let generation = generation_in(&prepared, Harness::Claude, "Stop").unwrap();
        let installed = Installed {
            cover: cover::read(
                &state.join("hooks/cover/claude-Stop.json"),
                Harness::Claude,
                "Stop",
            ),
            plan: serde_json::from_str(&generation.text).unwrap(),
            state: state.clone(),
            home,
        };
        let env = cover::Environment::under(installed.home.path());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(())
        );
        for text in [other, backquote] {
            fs::write(&part, text).unwrap();
            assert_eq!(
                holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
                Err(format!("{} has a shape rune does not read", part.display())),
                "{text}"
            );
        }
        // An entry that expands past `$HOME` is read by no install, and a
        // record that names one holds at no firing.
        let mut coverage = installed.cover.handlers["tmux"].clone();
        coverage.command = "sh $HOME/.config/$JUST/hook Stop".to_string();
        let planned = installed.plan.event(Harness::Claude, "Stop").unwrap();
        let tmux = planned.handlers.iter().find(|h| h.id == "tmux").unwrap();
        let table = installed.home.path().join(".claude/settings.json");
        let mut value: Value = serde_json::from_str(&fs::read_to_string(&table).unwrap()).unwrap();
        value["hooks"]["Stop"][0]["hooks"][0]["command"] = json!(coverage.command);
        fs::write(&table, value.to_string()).unwrap();
        assert_eq!(
            cover::holds(
                &coverage,
                tmux,
                Harness::Claude,
                "turn.finish",
                "Stop",
                &env,
                &mut cover::Cache::default(),
            ),
            Err("the entry's command expands past $HOME".to_string())
        );
    }

    #[test]
    fn a_first_part_without_the_shape_covers_nothing_and_is_named() {
        // The first part has no exec line, so the folder covers nothing,
        // even when a later part runs a handler's command: rune reads the
        // first part alone. Install names the part. A `-e` guard is not a
        // shape.
        let home = owner_home(&[
            (
                "Stop",
                "05-log.sh",
                "#!/bin/sh\nif [ -n \"$DEBUG\" ]; then echo stop; fi\nexit 0\n",
            ),
            STOP_PARTS[0],
            STOP_PARTS[2],
        ]);
        let dispatcher = "sh $HOME/.config/just/hook Stop";
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let manifest = home.path().join("state").join("manifest.json");
        let at = |home: &Path| {
            prepare(
                &stop_plan(),
                home,
                "/opt/rune",
                Path::new(PLAN),
                &manifest,
                false,
                chrono::Utc::now(),
            )
            .unwrap()
        };
        let prepared = at(home.path());
        assert!(prepared.report.covered.is_empty(), "{:?}", prepared.report);
        assert!(prepared.covers.is_empty(), "{:?}", prepared.covers);
        let stop_d = home.path().join(".config/just/hooks/Stop.d");
        let names = |prepared: &Prepared, part: &str| {
            prepared.report.warnings.iter().any(|w| {
                w.starts_with("claude: Stop reads no part of")
                    && w.ends_with(&format!(
                        "because {} has a shape rune does not read; every handler runs",
                        stop_d.join(part).display()
                    ))
            })
        };
        assert!(names(&prepared, "05-log.sh"), "{:?}", prepared.report);
        // The unshaped part goes: the tmux part is first and covers tmux.
        fs::remove_file(stop_d.join("05-log.sh")).unwrap();
        let prepared = at(home.path());
        assert_eq!(prepared.report.covered.len(), 1, "{:?}", prepared.report);
        assert!(prepared.report.covered[0].contains("covers tmux"));
        assert!(prepared.report.warnings.is_empty(), "{:?}", prepared.report);
        // A guard that is not `-x` or `-f` on the exec program is no
        // shape: the first part has none, so the folder covers nothing.
        fs::write(
            stop_d.join("10-tmux-attention.sh"),
            "#!/bin/sh\n[ -e \"$HOME/.local/bin/claude-tmux-status\" ] || exit 0\nexec \"$HOME/.local/bin/claude-tmux-status\" attention\n",
        )
        .unwrap();
        let prepared = at(home.path());
        assert!(prepared.report.covered.is_empty(), "{:?}", prepared.report);
        assert!(
            names(&prepared, "10-tmux-attention.sh"),
            "{:?}",
            prepared.report
        );
        // A `*.sh` entry that is no regular file first counts the folder as
        // nothing too.
        fs::remove_file(stop_d.join("10-tmux-attention.sh")).unwrap();
        fs::create_dir(stop_d.join("15-dir.sh")).unwrap();
        let prepared = at(home.path());
        assert!(prepared.report.covered.is_empty(), "{:?}", prepared.report);
        assert!(
            prepared.report.warnings.iter().any(|w| {
                w.starts_with("claude: Stop reads no part of")
                    && w.contains("15-dir.sh is no regular file; every handler runs")
            }),
            "{:?}",
            prepared.report
        );
        // The snapshot part is first once the folder is gone: it covers
        // turn, and a part behind it that the dispatcher runs with `nohup`
        // blocks nothing, because rune never reads it.
        fs::remove_dir(stop_d.join("15-dir.sh")).unwrap();
        fs::write(
            stop_d.join("40-nohup.sh"),
            "#!/bin/sh\nnohup \"$HOME/.local/bin/slow\" >/dev/null 2>&1 &\n",
        )
        .unwrap();
        let prepared = at(home.path());
        assert_eq!(prepared.report.covered.len(), 1, "{:?}", prepared.report);
        assert!(
            prepared.report.covered[0].contains("covers turn"),
            "{:?}",
            prepared.report
        );
        assert!(prepared.report.warnings.is_empty(), "{:?}", prepared.report);
    }

    /// The legacy dispatcher on Stop, as the owner's Claude table holds it
    /// twice: once behind a `Bash` matcher and once joined with a second
    /// command.
    const DISPATCHER: &str = "sh ~/.config/just/hook Stop";
    const COMPOUND: &str = "sh ~/.config/just/hook Stop && /owner/after-stop";

    /// An install over a home whose Claude table holds [`DISPATCHER`] and
    /// [`COMPOUND`] on Stop, whose Codex table holds the dispatcher on Stop
    /// and `PostToolUse`, and whose plan runs the Stop handlers and the
    /// git-ai checkpoint on both harnesses.
    struct Mixed {
        home: tempfile::TempDir,
        codex: PathBuf,
        both: Plan,
        prepared: Prepared,
    }

    fn mixed_install() -> Mixed {
        let home = owner_home(STOP_PARTS);
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [
                {"matcher": "Bash", "hooks": [{"type": "command", "command": DISPATCHER}]},
                {"hooks": [{"type": "command", "command": COMPOUND}]},
            ]}})
            .to_string(),
        );
        // Codex parts run the git-ai checkpoint for each harness; only the
        // codex one is what the Codex handler runs, and a shaped adapter
        // such as tmux is coverable on Claude alone.
        let posttool = home.path().join(".config/just/hooks/PostToolUse.d");
        fs::create_dir_all(&posttool).unwrap();
        fs::write(
            posttool.join("10-git-ai-claude.sh"),
            "#!/bin/sh\nexec git-ai checkpoint claude --hook-input stdin\n",
        )
        .unwrap();
        fs::write(
            posttool.join("20-git-ai-codex.sh"),
            "#!/bin/sh\nexec git-ai checkpoint codex --hook-input stdin\n",
        )
        .unwrap();
        let codex = home.path().join(".codex");
        fs::create_dir_all(&codex).unwrap();
        fs::write(
            codex.join("hooks.json"),
            json!({"hooks": {
                "Stop": [{"hooks": [{"type": "command", "command": DISPATCHER}]}],
                "PostToolUse": [{"hooks": [{"type": "command", "command": "sh ~/.config/just/hook PostToolUse"}]}],
            }})
            .to_string(),
        )
        .unwrap();
        let mut both = stop_plan();
        let git_ai = plan();
        for harness in [Harness::Claude, Harness::Codex] {
            let events = both.harnesses.entry(harness).or_default();
            let source = git_ai.harnesses.get(&harness).unwrap();
            events.insert(
                "PostToolUse".to_string(),
                source.get("PostToolUse").unwrap().clone(),
            );
        }
        let state = home.path().join("state");
        let manifest = state.join("manifest.json");
        let plans = state.join("hooks").join("plans");
        let prepared = prepare(
            &both,
            home.path(),
            "/opt/rune",
            &plans,
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        Mixed {
            home,
            codex,
            both,
            prepared,
        }
    }

    #[test]
    fn a_matcher_covers_nothing() {
        let mixed = mixed_install();
        let report = &mixed.prepared.report;
        assert_eq!(
            handlers_in(&mixed.prepared, Harness::Claude, "Stop"),
            Some(every_stop_handler()),
            "{report:?}"
        );
        assert_eq!(cover_in(&mixed.prepared, Harness::Claude, "Stop"), None);
        let claude = commands_in(&mixed.prepared, "settings.json", "Stop");
        assert_eq!(claude[0], DISPATCHER, "the entry stays: {claude:?}");
        assert!(
            report.warnings.iter().any(|w| {
                w.starts_with("claude: Stop reads no part of")
                    && w.contains(DISPATCHER)
                    && w.contains("because its matcher does not cover every call")
                    && w.contains("10-tmux-attention.sh")
            }),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_compound_line_covers_nothing() {
        let mixed = mixed_install();
        let report = &mixed.prepared.report;
        assert_eq!(
            handlers_in(&mixed.prepared, Harness::Claude, "Stop"),
            Some(every_stop_handler()),
            "{report:?}"
        );
        assert_eq!(cover_in(&mixed.prepared, Harness::Claude, "Stop"), None);
        let claude = commands_in(&mixed.prepared, "settings.json", "Stop");
        assert_eq!(&claude[..2], [DISPATCHER, COMPOUND], "both entries stay");
        assert!(
            report.warnings.iter().any(|w| {
                w.starts_with("claude: Stop reads no part of")
                    && w.contains(COMPOUND)
                    && w.contains("split it by hand")
            }),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_codex_entry_covers_nothing() {
        // Codex loads its hooks and their trust once per session, and rune
        // reads trust per firing: a handler skipped at a firing could be
        // one the session never runs. So no Codex record is written, with
        // no trust line to wait on, and the registration is the same.
        let mixed = mixed_install();
        let home = mixed.home.path();
        let report = &mixed.prepared.report;
        assert_eq!(
            handlers_in(&mixed.prepared, Harness::Codex, "Stop"),
            Some(every_stop_handler()),
            "{report:?}"
        );
        assert_eq!(
            commands_in(&mixed.prepared, "hooks.json", "Stop")[0],
            DISPATCHER
        );
        assert_eq!(cover_in(&mixed.prepared, Harness::Codex, "Stop"), None);
        assert_eq!(
            cover_in(&mixed.prepared, Harness::Codex, "PostToolUse"),
            None,
            "the codex part runs what git-ai runs, and still covers nothing"
        );
        assert!(
            !report.covered.iter().any(|c| c.starts_with("codex:")),
            "{:?}",
            report.covered
        );
        assert!(
            !report.trust.iter().any(|t| t.contains("skips no handler")),
            "{:?}",
            report.trust
        );
        write(&mixed.prepared).unwrap();
        assert!(
            !home
                .join("state/hooks/cover/codex-PostToolUse.json")
                .exists(),
            "no codex cover file"
        );
        // A Codex record written by hand, trusted at its slot, holds at no
        // firing: the check refuses the harness before it reads a file.
        trust_codex_slot(home, "PostToolUse", (0, 0));
        let folder = home.join(".config/just/hooks/PostToolUse.d");
        let runs = ["git-ai", "checkpoint", "codex", "--hook-input", "stdin"]
            .map(str::to_string)
            .to_vec();
        let coverage = cover::Coverage {
            home: home.to_path_buf(),
            handler_exec: vec!["rune-hook-git-ai".to_string()],
            file: mixed.codex.join("hooks.json"),
            slot: (0, 0),
            command: "sh ~/.config/just/hook PostToolUse".to_string(),
            script: home.join(".config/just/hook"),
            shell: Some("sh".to_string()),
            folder: folder.clone(),
            part: folder.join("20-git-ai-codex.sh"),
            exec: runs.clone(),
            runs,
        };
        let planned = mixed.both.event(Harness::Codex, "PostToolUse").unwrap();
        let handler = planned.handlers.iter().find(|h| h.id == "git-ai").unwrap();
        assert_eq!(
            cover::holds(
                &coverage,
                handler,
                Harness::Codex,
                "tool.after",
                "PostToolUse",
                &cover::Environment::under(home),
                &mut cover::Cache::default(),
            ),
            Err("codex loads its hooks once per session, so nothing is skipped there".to_string())
        );
    }

    #[test]
    fn a_gone_dispatcher_entry_an_earlier_install_adopted_is_named() {
        let home = owner_home(STOP_PARTS);
        let dispatcher = "sh ~/.config/just/hook Stop";
        settings(home.path(), &json!({"hooks": {}}).to_string());
        let state = home.path().join("state");
        let manifest = state.join("manifest.json");
        let plans = state.join("hooks").join("plans");
        let table = home.path().join(".claude/settings.json");
        let earlier = Manifest {
            version: 1,
            harnesses: BTreeMap::from([(
                Harness::Claude,
                HarnessManifest {
                    adopted: vec![Recorded {
                        event: "Stop".to_string(),
                        command: dispatcher.to_string(),
                        hash: None,
                        by: Some(LEGACY_SD_HOOK.to_string()),
                        source: Some(table.display().to_string()),
                    }],
                    ..HarnessManifest::default()
                },
            )]),
            ..Manifest::default()
        };
        fs::create_dir_all(&state).unwrap();
        fs::write(&manifest, serde_json::to_string_pretty(&earlier).unwrap()).unwrap();
        let prepared = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            &plans,
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        let warning = format!(
            "claude: Stop no longer has the legacy dispatcher entry `{dispatcher}`, which an earlier install removed; rune keeps that entry now, so put it back by hand if its parts should run"
        );
        assert!(
            prepared.report.warnings.contains(&warning),
            "{:?}",
            prepared.report.warnings
        );
        let next: Manifest = serde_json::from_str(
            &prepared
                .files
                .iter()
                .find(|staged| staged.target == manifest)
                .unwrap()
                .text,
        )
        .unwrap();
        assert_eq!(
            next.harnesses[&Harness::Claude].adopted.len(),
            1,
            "the record stays until the entry is back"
        );
        // The entry is back: the record is dropped, with no warning.
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                .to_string(),
        );
        let prepared = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            &plans,
            &manifest,
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(
            !prepared
                .report
                .warnings
                .iter()
                .any(|w| w.contains("no longer has")),
            "{:?}",
            prepared.report.warnings
        );
        let next: Manifest = serde_json::from_str(
            &prepared
                .files
                .iter()
                .find(|staged| staged.target == manifest)
                .unwrap()
                .text,
        )
        .unwrap();
        let adopted = &next.harnesses[&Harness::Claude].adopted;
        assert!(adopted.is_empty(), "the record is dropped: {adopted:?}");
    }

    /// An install over [`STOP_PARTS`] with `entry` alone on Claude's Stop,
    /// written, with the cover it wrote and the Stop generation's plan.
    struct Installed {
        home: tempfile::TempDir,
        state: PathBuf,
        cover: cover::Cover,
        plan: Plan,
    }

    fn installed(entry: &Value) -> Installed {
        let home = owner_home(STOP_PARTS);
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [entry]}]}}).to_string(),
        );
        let state = home.path().join("state");
        let prepared = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            &state.join("hooks").join("plans"),
            &state.join("manifest.json"),
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        write(&prepared).unwrap();
        let cover = cover::read(
            &state.join("hooks/cover/claude-Stop.json"),
            Harness::Claude,
            "Stop",
        );
        let generation = generation_in(&prepared, Harness::Claude, "Stop").unwrap();
        let plan: Plan = serde_json::from_str(&generation.text).unwrap();
        Installed {
            home,
            state,
            cover,
            plan,
        }
    }

    /// Whether the cover of `installed` holds for the Stop handler `id`
    /// at a Claude firing under `env`.
    fn holds_for(
        installed: &Installed,
        id: &str,
        env: &cover::Environment,
        cache: &mut cover::Cache,
    ) -> Result<(), String> {
        let planned = installed.plan.event(Harness::Claude, "Stop").unwrap();
        let handler = planned.handlers.iter().find(|h| h.id == id).unwrap();
        cover::holds(
            &installed.cover.handlers[id],
            handler,
            Harness::Claude,
            "turn.finish",
            "Stop",
            env,
            cache,
        )
    }

    const PLAIN_ENTRY: &str = "sh ~/.config/just/hook Stop";

    #[test]
    fn an_entry_with_a_key_rune_does_not_read_covers_nothing() {
        // Claude reads `if`, `args`, and `async` on a hook, and each
        // changes whether and how the command runs. Install names the key.
        let dispatcher = "sh $HOME/.config/just/hook Stop";
        for (key, value) in [
            ("if", json!("Bash(git *)")),
            ("args", json!([])),
            ("async", json!(true)),
        ] {
            let mut entry = json!({"type": "command", "command": dispatcher});
            entry[key] = value;
            let home = owner_home(STOP_PARTS);
            settings(
                home.path(),
                &json!({"hooks": {"Stop": [{"hooks": [entry]}]}}).to_string(),
            );
            let manifest = home.path().join("state").join("manifest.json");
            let prepared = prepare(
                &stop_plan(),
                home.path(),
                "/opt/rune",
                Path::new(PLAN),
                &manifest,
                false,
                chrono::Utc::now(),
            )
            .unwrap();
            assert!(
                prepared.report.covered.is_empty(),
                "{key}: {:?}",
                prepared.report
            );
            assert!(prepared.covers.is_empty(), "{key}: {:?}", prepared.covers);
            let why = format!("because the entry has the key `{key}`, which rune does not read");
            assert!(
                prepared.report.warnings.iter().any(|w| {
                    w.starts_with("claude: Stop reads no part of")
                        && w.contains(&why)
                        && w.contains("may run what tmux runs twice")
                }),
                "{key}: {:?}",
                prepared.report.warnings
            );
        }
        // The keys Claude documents on a command hook are read. A key added
        // by hand after the install stops the cover at the next firing, on
        // the hook and on its group alike.
        let installed = installed(&json!({
            "type": "command",
            "command": PLAIN_ENTRY,
            "timeout": 30,
            "statusMessage": "stop hooks",
        }));
        let env = cover::Environment::under(installed.home.path());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(())
        );
        let table = installed.home.path().join(".claude/settings.json");
        let mut value: Value = serde_json::from_str(&fs::read_to_string(&table).unwrap()).unwrap();
        value["hooks"]["Stop"][0]["hooks"][0]["if"] = json!("Bash(git *)");
        fs::write(&table, value.to_string()).unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err("the entry has the key `if`, which rune does not read".to_string())
        );
        value["hooks"]["Stop"][0]["hooks"][0]
            .as_object_mut()
            .unwrap()
            .remove("if");
        value["hooks"]["Stop"][0]["disabled"] = json!(true);
        fs::write(&table, value.to_string()).unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err("the entry's group has the key `disabled`, which rune does not read".to_string())
        );
    }

    #[test]
    fn a_firing_covers_nothing_unless_home_and_the_harness_folder_are_install_s() {
        let installed = installed(&json!({"type": "command", "command": PLAIN_ENTRY}));
        let home = installed.home.path();
        let check = |env: &cover::Environment| {
            holds_for(&installed, "tmux", env, &mut cover::Cache::default())
        };
        assert_eq!(check(&cover::Environment::under(home)), Ok(()));
        // HOME from the environment only: unset or empty, the dispatcher
        // stops at `set -u`, so there is no home to read under.
        let unset = "HOME is unset or empty, so the dispatcher runs no part".to_string();
        let mut env = cover::Environment::under(home);
        env.home = None;
        assert_eq!(check(&env), Err(unset.clone()));
        env.home = Some(std::ffi::OsString::new());
        assert_eq!(check(&env), Err(unset));
        let other = tempfile::tempdir().unwrap();
        env.home = Some(other.path().as_os_str().to_os_string());
        assert_eq!(
            check(&env),
            Err(format!(
                "the home is {} and install read {}",
                other.path().display(),
                home.display()
            ))
        );
        // Claude reads another folder under `CLAUDE_CONFIG_DIR`: the table
        // the record names is not the one read, so nothing holds.
        let mut env = cover::Environment::under(home);
        env.claude_config_dir = Some(other.path().as_os_str().to_os_string());
        assert_eq!(
            check(&env),
            Err(format!(
                "claude reads its hooks under {} now, not {}",
                other.path().display(),
                home.join(".claude/settings.json").display()
            ))
        );
        env.claude_config_dir = Some(std::ffi::OsString::from("relative"));
        assert_eq!(
            check(&env),
            Err("claude reads its hooks from a folder rune cannot name".to_string())
        );
        env.claude_config_dir = Some(home.join(".claude").into_os_string());
        assert_eq!(check(&env), Ok(()), "the override names the same folder");
        // A Claude record copied to Codex's name covers nothing on Codex:
        // the record names its harness.
        let claude_text =
            fs::read_to_string(installed.state.join("hooks/cover/claude-Stop.json")).unwrap();
        let codex_path = installed.state.join("hooks/cover/codex-Stop.json");
        fs::write(&codex_path, &claude_text).unwrap();
        let copied = cover::read(&codex_path, Harness::Codex, "Stop");
        assert!(copied.handlers.is_empty(), "{copied:?}");
    }

    #[test]
    fn a_handler_that_is_not_the_recorded_one_is_not_covered() {
        let installed = installed(&json!({"type": "command", "command": PLAIN_ENTRY}));
        let home = installed.home.path();
        let env = cover::Environment::under(home);
        let planned = installed.plan.event(Harness::Claude, "Stop").unwrap();
        let tmux = planned.handlers.iter().find(|h| h.id == "tmux").unwrap();
        let coverage = &installed.cover.handlers["tmux"];
        let check =
            |handler: &rune::hooks::PlannedHandler, harness: Harness, event: &str, native: &str| {
                cover::holds(
                    coverage,
                    handler,
                    harness,
                    event,
                    native,
                    &env,
                    &mut cover::Cache::default(),
                )
            };
        assert_eq!(check(tmux, Harness::Claude, "turn.finish", "Stop"), Ok(()));
        // The handler as this firing runs it: a plan that now denies on
        // failure, or runs the adapter with arguments, is another handler.
        let mut deny = tmux.clone();
        deny.on_failure = OnFailure::Deny;
        assert_eq!(
            check(&deny, Harness::Claude, "turn.finish", "Stop"),
            Err("the handler denies on failure".to_string())
        );
        // A rewriter's output feeds the handlers after it; a part's does
        // not, so a skipped rewriter would leave them the raw call.
        let mut rewriter = tmux.clone();
        rewriter.rewrites = true;
        assert_eq!(
            check(&rewriter, Harness::Claude, "turn.finish", "Stop"),
            Err(
                "the handler rewrites the tool call, which no part does for the handlers after it"
                    .to_string()
            )
        );
        let mut with_args = tmux.clone();
        with_args.exec.push("--quiet".to_string());
        assert_eq!(
            check(&with_args, Harness::Claude, "turn.finish", "Stop"),
            Err("the handler's command changed".to_string())
        );
        let mut other = tmux.clone();
        other.exec = vec!["rune-hook-dcg".to_string()];
        assert_eq!(
            check(&other, Harness::Claude, "turn.finish", "Stop"),
            Err("the handler's command changed".to_string())
        );
        // The record binds to its event and harness: on another event the
        // adapter runs another command, and on Codex nothing is skipped.
        assert_eq!(
            check(tmux, Harness::Claude, "session.end", "SessionEnd"),
            Err("tmux-status runs another command here".to_string())
        );
        assert_eq!(
            check(tmux, Harness::Codex, "turn.finish", "Stop"),
            Err("codex loads its hooks once per session, so nothing is skipped there".to_string())
        );
        assert_eq!(
            check(tmux, Harness::Gemini, "turn.finish", "Stop"),
            Err("gemini reads its hooks from a folder rune cannot name".to_string())
        );
    }

    #[test]
    fn one_firing_reads_the_table_the_script_and_the_parts_once() {
        let installed = installed(&json!({"type": "command", "command": PLAIN_ENTRY}));
        let home = installed.home.path();
        let env = cover::Environment::under(home);
        let mut cache = cover::Cache::default();
        assert_eq!(holds_for(&installed, "tmux", &env, &mut cache), Ok(()));
        // Everything the checks read goes: the firing's cache still answers
        // for the same handler, and a fresh firing reads nothing.
        fs::remove_file(home.join(".claude/settings.json")).unwrap();
        fs::remove_file(home.join(".config/just/hook")).unwrap();
        fs::remove_dir_all(home.join(".config/just/hooks/Stop.d")).unwrap();
        assert_eq!(holds_for(&installed, "tmux", &env, &mut cache), Ok(()));
        assert!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()).is_err(),
            "a fresh firing finds no table"
        );
    }

    #[test]
    fn a_cover_that_holds_past_the_budget_still_skips_its_handler() {
        let installed = installed(&json!({"type": "command", "command": PLAIN_ENTRY}));
        let home = installed.home.path().to_path_buf();
        // The clock started long before the cover was read: the cover
        // holds, and the deadline passed during the checks. The part runs
        // the command either way, so the covered handler is skipped with
        // its part, and only the uncovered ones record the budget.
        let record = installed.state.join("record");
        let (mut runner, seen) = covered_runner(&home, &record, installed.cover.clone());
        runner.started = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(600))
            .unwrap();
        let raw = json!({"hook_event_name": "Stop", "session_id": "s", "cwd": "/"});
        let _ =
            super::super::run::dispatch(&installed.plan, Harness::Claude, "Stop", Ok(raw), &runner);
        assert!(
            seen.borrow().is_empty(),
            "nothing is spawned past the budget: {:?}",
            seen.borrow()
        );
        let records = super::super::record::load(&record).unwrap();
        let stop_d = home.join(".config/just/hooks/Stop.d");
        let tmux = records.iter().find(|r| r.hook == "tmux").unwrap();
        assert_eq!(
            tmux.skipped.as_deref(),
            Some(stop_d.join("10-tmux-attention.sh").to_str().unwrap()),
            "tmux is skipped with its part: {records:?}"
        );
        assert_eq!(tmux.failure, None);
        for id in ["capture", "turn"] {
            let line = records.iter().find(|r| r.hook == id).unwrap();
            assert_eq!(line.skipped, None, "{records:?}");
            assert_eq!(line.failure.as_deref(), Some("budget"), "{id}");
        }
    }

    #[test]
    fn the_slot_is_the_entry_s_place_in_the_table_install_writes() {
        // An adopted entry before the dispatcher goes at this install, so
        // the dispatcher's slot is read from the table as written, and
        // the first firing after the install skips the covered handler.
        let home = owner_home(&[STOP_PARTS[2]]);
        let dispatcher = "sh $HOME/.config/just/hook Stop";
        let tmux = "~/.local/bin/claude-tmux-status attention";
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [
                {"hooks": [{"type": "command", "command": tmux}]},
                {"hooks": [{"type": "command", "command": dispatcher}]},
            ]}})
            .to_string(),
        );
        let state = home.path().join("state");
        let prepared = prepare(
            &stop_plan(),
            home.path(),
            "/opt/rune",
            &state.join("hooks").join("plans"),
            &state.join("manifest.json"),
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(
            prepared
                .report
                .adopted
                .iter()
                .any(|(_, e, c)| e == "Stop" && c == tmux),
            "{:?}",
            prepared.report.adopted
        );
        let claude = commands_in(&prepared, "settings.json", "Stop");
        assert_eq!(
            claude[0], dispatcher,
            "the dispatcher is first now: {claude:?}"
        );
        let cover = cover_in(&prepared, Harness::Claude, "Stop").unwrap();
        assert_eq!(cover.handlers["turn"].slot, (0, 0), "{cover:?}");
        write(&prepared).unwrap();
        let generation = generation_in(&prepared, Harness::Claude, "Stop").unwrap();
        let plan: Plan = serde_json::from_str(&generation.text).unwrap();
        let cover = cover::read(
            &state.join("hooks/cover/claude-Stop.json"),
            Harness::Claude,
            "Stop",
        );
        let (runner, seen) = covered_runner(home.path(), &state.join("record"), cover);
        let raw = json!({"hook_event_name": "Stop", "session_id": "s", "cwd": "/"});
        let answer = super::super::run::dispatch(&plan, Harness::Claude, "Stop", Ok(raw), &runner);
        assert_eq!(answer.exit, 0, "{answer:?}");
        assert_eq!(
            seen.borrow().clone(),
            ["capture", "tmux"],
            "turn is skipped at the first firing"
        );
    }

    #[test]
    fn a_codex_toml_hook_is_named_by_the_line_of_its_command_value() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        // A multi-line string above quotes the command: the line named is
        // the command value's own.
        fs::write(
            &config,
            "developer_instructions = \"\"\"\nNever call /Users/x/.local/bin/dcg by hand.\n\"\"\"\n\n[[hooks.PreToolUse]]\nmatcher = \"Bash\"\n[[hooks.PreToolUse.hooks]]\ntype = \"command\"\ncommand = \"/Users/x/.local/bin/dcg\"\n",
        )
        .unwrap();
        let hooks = codex_toml_hooks(&config);
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].command, "/Users/x/.local/bin/dcg");
        assert_eq!(hooks[0].line, Some(9));
        // Trust state beside the events, and a hook in an inline table.
        fs::write(
            &config,
            "[hooks.state.\"/x/hooks.json:stop:0:0\"]\ntrusted_hash = \"sha256:0\"\n[[hooks.Stop]]\nhooks = [{ type = \"command\", command = 'sh /x/.sd/hook Stop' }]\n",
        )
        .unwrap();
        let hooks = codex_toml_hooks(&config);
        assert_eq!(hooks.len(), 1, "hooks.state is no event");
        assert_eq!(hooks[0].event, "Stop");
        assert_eq!(hooks[0].line, Some(4));
    }

    #[test]
    fn a_shell_word_that_finds_a_fake_sh_first_covers_nothing_at_install() {
        let home = owner_home(STOP_PARTS);
        let fake_dir = home.path().join("fake");
        fs::create_dir_all(&fake_dir).unwrap();
        fs::write(fake_dir.join("sh"), "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(fake_dir.join("sh"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let plan = stop_plan();
        let planned = plan.event(Harness::Claude, "Stop").unwrap();
        let with = |dispatcher: &str, path_env: Option<std::ffi::OsString>| {
            let table = settings(
                home.path(),
                &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": dispatcher}]}]}})
                    .to_string(),
            );
            let groups = Table::read(&table).unwrap().groups("Stop");
            let dispatchers = Dispatchers {
                home: home.path(),
                harness: Harness::Claude,
                table: &table,
                path_env,
                shell_variable: None,
            };
            let mut report = Report::default();
            let covered = dispatchers.covered("Stop", planned, &groups, &mut report);
            covered.keys().cloned().collect::<Vec<_>>()
        };
        let bare = "sh $HOME/.config/just/hook Stop";
        let absolute = "/bin/sh $HOME/.config/just/hook Stop";
        let system = std::env::join_paths([Path::new("/bin")]).unwrap();
        if legacy::resolve_shell("sh", Some(&system)).is_some() {
            assert_eq!(with(bare, Some(system.clone())), ["tmux"]);
            assert_eq!(with(absolute, Some(system.clone())), ["tmux"]);
        }
        let fake_first = std::env::join_paths([fake_dir.as_path(), Path::new("/bin")]).unwrap();
        assert!(
            with(bare, Some(fake_first.clone())).is_empty(),
            "a file named sh first in PATH is no shell rune knows"
        );
        assert!(
            with(absolute, Some(fake_first)).is_empty(),
            "the dispatcher runs each part with the sh on PATH, whatever runs the dispatcher"
        );
        assert!(with(bare, None).is_empty(), "no PATH resolves no bare word");
        assert!(
            with(absolute, None).is_empty(),
            "no PATH resolves no inner sh either"
        );
        assert!(
            with(
                "/bin/zsh $HOME/.config/just/hook Stop",
                Some(system.clone())
            )
            .is_empty(),
            "zsh reads ~/.zshenv, which rune never reads"
        );
        assert!(
            with("bash $HOME/.config/just/hook Stop", Some(system)).is_empty(),
            "bash reads BASH_ENV, which rune never reads"
        );
    }

    #[test]
    fn a_firing_whose_sh_is_not_bin_sh_runs_every_handler() {
        let installed =
            installed(&json!({"type": "command", "command": "/bin/sh ~/.config/just/hook Stop"}));
        let home = installed.home.path();
        assert_eq!(
            installed.cover.handlers["tmux"].shell.as_deref(),
            Some("/bin/sh")
        );
        let fake_dir = home.join("fake");
        fs::create_dir_all(&fake_dir).unwrap();
        fs::write(fake_dir.join("sh"), "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(fake_dir.join("sh"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut env = cover::Environment::under(home);
        env.path = Some(std::env::join_paths([fake_dir.as_path(), Path::new("/bin")]).unwrap());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err(
                "`sh` on PATH is not /bin/sh, and the dispatcher runs each part with it"
                    .to_string()
            )
        );
        env.path = None;
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err(
                "`sh` on PATH is not /bin/sh, and the dispatcher runs each part with it"
                    .to_string()
            )
        );
        env.path = Some(std::env::join_paths([Path::new("/bin"), fake_dir.as_path()]).unwrap());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(()),
            "a fake sh after /bin is never run"
        );
    }

    #[test]
    fn a_folder_past_a_limit_covers_nothing_at_install_and_at_the_firing() {
        let installed = installed(&json!({"type": "command", "command": PLAIN_ENTRY}));
        let home = installed.home.path();
        let env = cover::Environment::under(home);
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(())
        );
        let stop_d = home.join(".config/just/hooks/Stop.d");
        let too_many = Err(format!(
            "{} holds more than {} entries",
            stop_d.display(),
            legacy::MAX_ENTRIES
        ));
        // One entry over the count, of any kind: the firing reads no part.
        let extra = legacy::MAX_ENTRIES + 1 - STOP_PARTS.len();
        for index in 0..extra {
            fs::write(stop_d.join(format!("note-{index:02}.txt")), "x\n").unwrap();
        }
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            too_many
        );
        fs::remove_file(stop_d.join(format!("note-{:02}.txt", extra - 1))).unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(()),
            "the count itself is read"
        );
        for index in 0..extra - 1 {
            fs::remove_file(stop_d.join(format!("note-{index:02}.txt"))).unwrap();
        }
        // A first part past the bytes one firing reads: the firing reads
        // no part. A later part of any size is never read.
        let big = format!("# {}\nexec true\n", "x".repeat(60 * 1024));
        fs::write(stop_d.join("40-big.sh"), &big).unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(()),
            "a later part is not read"
        );
        fs::write(stop_d.join("05-big.sh"), &big).unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err(format!(
                "{} is not the first part of {} now",
                stop_d.join("10-tmux-attention.sh").display(),
                stop_d.display()
            )),
            "a first part under the read limit is read, and is another part"
        );
        let huge = format!("# {}\nexec true\n", "x".repeat(70 * 1024));
        fs::write(stop_d.join("05-big.sh"), &huge).unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err(format!(
                "{} is no regular file under {} bytes",
                stop_d.join("05-big.sh").display(),
                legacy::READ_LIMIT
            ))
        );
        // The next install covers nothing over such a folder, and says so.
        let state = installed.state.clone();
        let again = prepare(
            &stop_plan(),
            home,
            "/opt/rune",
            &state.join("hooks").join("plans"),
            &state.join("manifest.json"),
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(again.report.covered.is_empty(), "{:?}", again.report);
        assert!(
            again.report.warnings.iter().any(|w| {
                w.starts_with("claude: Stop reads no part of")
                    && w.contains("05-big.sh is no regular file under")
            }),
            "{:?}",
            again.report.warnings
        );
        let cover = cover_in(&again, Harness::Claude, "Stop").unwrap();
        assert!(cover.handlers.is_empty(), "the cover is cleared: {cover:?}");
    }

    #[test]
    fn a_first_part_with_an_unquoted_glob_covers_nothing_and_a_later_one_blocks_nothing() {
        // `b[ab]` names one word to rune and the files it matches to the
        // shell, so a part that holds one has no shape rune reads. First
        // in its folder it covers nothing; after the first part it is
        // never read.
        let glob = "#!/bin/sh\nexec \"$HOME/.local/bin/claude-tmux-status\" b[ab]\n";
        let mut parts = STOP_PARTS.to_vec();
        parts.push(("Stop", "40-glob.sh", glob));
        let home = owner_home(&parts);
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": PLAIN_ENTRY}]}]}})
                .to_string(),
        );
        let state = home.path().join("state");
        let at = || {
            prepare(
                &stop_plan(),
                home.path(),
                "/opt/rune",
                &state.join("hooks").join("plans"),
                &state.join("manifest.json"),
                false,
                chrono::Utc::now(),
            )
            .unwrap()
        };
        let later = at();
        assert_eq!(
            later.report.covered.len(),
            1,
            "a later glob part blocks nothing: {:?}",
            later.report
        );
        assert!(later.report.warnings.is_empty(), "{:?}", later.report);
        let stop_d = home.path().join(".config/just/hooks/Stop.d");
        fs::rename(stop_d.join("40-glob.sh"), stop_d.join("05-glob.sh")).unwrap();
        let first = at();
        assert!(first.report.covered.is_empty(), "{:?}", first.report);
        assert!(
            first.report.warnings.iter().any(|w| {
                w.starts_with("claude: Stop reads no part of")
                    && w.contains("05-glob.sh has a shape rune does not read")
            }),
            "{:?}",
            first.report.warnings
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_direct_dispatcher_entry_needs_an_executable_script_with_a_plain_shebang() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = owner_home(STOP_PARTS);
        let script = home.path().join(".config/just/hook");
        let entry = "$HOME/.config/just/hook Stop";
        settings(
            home.path(),
            &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": entry}]}]}})
                .to_string(),
        );
        let state = home.path().join("state");
        let at = || {
            prepare(
                &stop_plan(),
                home.path(),
                "/opt/rune",
                &state.join("hooks").join("plans"),
                &state.join("manifest.json"),
                false,
                chrono::Utc::now(),
            )
            .unwrap()
        };
        // Written without a mode, the entry runs nothing.
        let prepared = at();
        assert!(prepared.report.covered.is_empty(), "{:?}", prepared.report);
        assert!(prepared.covers.is_empty(), "no cover is staged");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        if super::super::legacy::tests::system_sh().is_none() {
            return;
        }
        let prepared = at();
        assert_eq!(prepared.report.covered.len(), 1, "{:?}", prepared.report);
        let cover = cover_in(&prepared, Harness::Claude, "Stop").unwrap();
        assert_eq!(cover.handlers["tmux"].shell, None, "{cover:?}");
        write(&prepared).unwrap();
        let generation = generation_in(&prepared, Harness::Claude, "Stop").unwrap();
        let plan: Plan = serde_json::from_str(&generation.text).unwrap();
        let installed = Installed {
            cover: cover::read(
                &state.join("hooks/cover/claude-Stop.json"),
                Harness::Claude,
                "Stop",
            ),
            plan,
            state,
            home,
        };
        let env = cover::Environment::under(installed.home.path());
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Ok(())
        );
        // An option on the shebang after the install: `sh -n` runs no part,
        // so the next firing runs the handler, and the next install covers
        // nothing.
        fs::write(
            &script,
            super::super::legacy::tests::SCRIPT.replacen("#!/bin/sh", "#!/bin/sh -n", 1),
        )
        .unwrap();
        assert_eq!(
            holds_for(&installed, "tmux", &env, &mut cover::Cache::default()),
            Err(format!(
                "the shebang of {} is not `#!/bin/sh`",
                script.display()
            ))
        );
        let state = installed.state.clone();
        let again = prepare(
            &stop_plan(),
            installed.home.path(),
            "/opt/rune",
            &state.join("hooks").join("plans"),
            &state.join("manifest.json"),
            false,
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(again.report.covered.is_empty(), "{:?}", again.report);
        let cover = cover_in(&again, Harness::Claude, "Stop").unwrap();
        assert!(cover.handlers.is_empty(), "the cover is cleared: {cover:?}");
    }
}
