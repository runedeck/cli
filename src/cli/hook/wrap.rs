//! Turn wrapping: at a turn's end, commit what the turn left in the
//! working copy as one jj change and tell the owner how to review or undo
//! it. Off until the owner runs `rune hook wrap on` in a workspace.
//!
//! One rule decides every wrap: a workspace that more than one session
//! has touched since the last wrap never wraps. One slot file per
//! workspace, `.jj/turn-wrap.open`, holds the set of session keys that
//! touched it since the last wrap. Every event that shows a session
//! working in the workspace adds its key: a firing whose directory is in
//! it, a tool event whose file is in it, a shell command that names a
//! path in it, and the stop itself. A payload without a session id adds
//! the key of an unknown session, which no stop can be alone with. A
//! wrap that commits empties the set, and
//! nothing else removes a key but the owner's `rune hook wrap reset`. The
//! slot is read and written only under an exclusive lock on
//! `.jj/turn-wrap.lock`, held from the check through the commit, so no
//! other session's touch can register between the two. A touch that
//! cannot get the lock in time writes a marker under
//! `.jj/turn-wrap.touched/` that the next check folds in before it
//! decides. A stop after a handler that failed or timed out wraps
//! nothing and says so, since the turn may be unfinished. The change's
//! design file maps each part to the review finding it closes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::encode::HandlerResult;
use super::payload::HandlerPayload;
use rune::error::{Error, ErrorKind};
use rune::hooks::Harness;

/// The switch: this file under the workspace's `.jj/` turns wrapping on.
const SWITCH: &str = "turn-wrap";
/// The slot, beside the switch: the sessions that touched the workspace
/// since the last wrap.
const SLOT: &str = "turn-wrap.open";
/// The lock every read and write of the slot holds.
const LOCK: &str = "turn-wrap.lock";
/// Touch markers, one file per session key, for a prompt that could not
/// take the lock in time. The next check under the lock folds them in.
const TOUCHED: &str = "turn-wrap.touched";
/// How long a prompt waits for the lock before it leaves a marker.
const PROMPT_WAIT: Duration = Duration::from_millis(250);
/// How long a turn's end waits for the lock before it wraps nothing.
const FINISH_WAIT: Duration = Duration::from_millis(400);
/// The time the stop needs where it wraps nothing and shows the card:
/// the lock wait at its bound, and 100 ms for the process start and the
/// slot's read and write. The 100 ms is an allowance, not a measurement.
pub(crate) const CARD_HOLD: Duration = FINISH_WAIT.saturating_add(Duration::from_millis(100));
/// How long the owner's reset waits for the lock.
const RESET_WAIT: Duration = Duration::from_millis(1_000);
/// The pause between two tries of the lock.
const RETRY: Duration = Duration::from_millis(10);
/// The slot format this version writes and reads.
const SLOT_VERSION: u32 = 2;
/// The key a payload without a session id registers. No stop is ever
/// alone with it, so the workspace wraps nothing until the owner resets.
const UNKNOWN: &str = "unknown";

/// The sessions that touched one workspace since its last wrap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Slot {
    v: u32,
    /// The session keys, sorted and unique: harness and a hash of the id.
    keys: BTreeSet<String>,
    /// The working-copy commit when the first key of this set arrived,
    /// after a snapshot: the base of the card's file stat on a described
    /// working copy. Absent when that touch was a marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    /// What the card says about each key: the harness and the first
    /// touch. A key without an entry is named by its key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    who: BTreeMap<String, Touch>,
}

/// One session's first touch of the workspace, as the card names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Touch {
    harness: Harness,
    /// UTC, to the second.
    since: String,
}

/// What the stop of a firing needs, by an unlocked read, so the
/// dispatcher holds that back from the handlers before the wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hold {
    /// Outside a workspace with the switch on: the wrapper does nothing.
    Nothing,
    /// In one that another session touched, or that an unknown session
    /// touched: the wrapper registers the stop and shows the card.
    Card,
    /// In one where the set shows this session alone or nobody: the
    /// wrapper may commit.
    Turn,
}

/// What `@` is at the turn's end.
struct Probe {
    empty: bool,
    described: bool,
    change: String,
    title: String,
}

/// The lock on a workspace's slot, released on drop.
#[derive(Debug)]
struct Lock {
    _file: File,
}

// ------------------------------------------------------------- adapter ----

/// The `turn-wrap` adapter: registers the session's touch on every event
/// that shows it in a workspace with the switch on, wraps on
/// `turn.finish`, and does nothing while the switch is off.
pub(crate) fn adapter(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    adapter_in(
        payload,
        std::env::var(super::run::MERGED_EFFECT_VAR).ok().as_deref(),
        &std::env::var(super::run::FAILED_VAR).unwrap_or_default(),
        std::env::var_os("CLAUDE_PROJECT_DIR").as_deref(),
    )
}

/// `adapter` with the merged effect of the handlers before this one, the
/// ids of those that failed, and the harness's project directory as
/// parameters, so a test needs no environment.
pub(crate) fn adapter_in(
    payload: &HandlerPayload,
    merged: Option<&str>,
    failed: &str,
    project_dir: Option<&std::ffi::OsStr>,
) -> Result<HandlerResult, String> {
    let key = session_key(payload.harness, payload.session_id.as_deref());
    if payload.event == "turn.finish" {
        let Some(root) = armed_in(payload, project_dir) else {
            return Ok(HandlerResult::default());
        };
        return finish(payload.harness, &root, &key, merged, failed);
    }
    // Every other event is a touch of each workspace it shows the session
    // in. The base snapshot is right only before the turn's first write.
    let snapshot = matches!(payload.event.as_str(), "prompt.before" | "tool.before");
    for root in armed_roots(payload, project_dir) {
        open(&root, payload.harness, &key, snapshot)?;
    }
    Ok(HandlerResult::default())
}

/// What the stop of this firing needs, by an unlocked read: nothing
/// outside a workspace with the switch on, the card where the set there
/// shows another session, and the turn where it shows this session alone
/// or nobody. The locked check at the stop decides.
pub(crate) fn hold(payload: &HandlerPayload) -> Hold {
    let Some(root) = armed_in(payload, std::env::var_os("CLAUDE_PROJECT_DIR").as_deref()) else {
        return Hold::Nothing;
    };
    if alone(
        &root,
        &session_key(payload.harness, payload.session_id.as_deref()),
    ) {
        Hold::Turn
    } else {
        Hold::Card
    }
}

/// Whether the slot and the markers under `root`, read without the lock,
/// name `key` alone or nobody. A malformed slot never wraps, and neither
/// does the unknown session, so neither is alone.
fn alone(root: &Path, key: &str) -> bool {
    if key == UNKNOWN {
        return false;
    }
    let dir = root.join(".jj");
    let Ok(slot) = read_slot(&dir.join(SLOT)) else {
        return false;
    };
    let mut keys = slot.map(|slot| slot.keys).unwrap_or_default();
    if let Ok(markers) = fs::read_dir(dir.join(TOUCHED)) {
        keys.extend(
            markers
                .flatten()
                .map(|marker| marker.file_name().to_string_lossy().into_owned()),
        );
    }
    keys.iter().all(|k| k == key)
}

/// The workspace root the payload's directory sits in, when the owner
/// switched wrapping on there. The payload's directory wins; a payload
/// without one falls back to the project directory the harness exports.
fn armed_in(payload: &HandlerPayload, project_dir: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let dir = match payload.cwd.as_deref() {
        Some(cwd) => PathBuf::from(cwd),
        None => PathBuf::from(project_dir?),
    };
    armed_at(&dir)
}

/// The workspace root `path` sits in, when the switch is on there. A path
/// that does not exist yet, such as a file a tool is about to write, is
/// read from its nearest existing ancestor.
fn armed_at(path: &Path) -> Option<PathBuf> {
    let existing = path.ancestors().find(|p| p.exists())?;
    let root = workspace_root(existing)?;
    root.join(".jj").join(SWITCH).is_file().then_some(root)
}

/// Every armed workspace the payload shows the session in: the one its
/// directory sits in, and on a tool event the one each written file sits
/// in and the one each path a shell command names sits in, which can
/// differ when the session works from another directory.
fn armed_roots(
    payload: &HandlerPayload,
    project_dir: Option<&std::ffi::OsStr>,
) -> BTreeSet<PathBuf> {
    let mut roots = BTreeSet::new();
    roots.extend(armed_in(payload, project_dir));
    if let Some(tool) = &payload.tool {
        let base = payload
            .cwd
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| project_dir.map(PathBuf::from));
        for path in super::adapters::written_paths(tool) {
            let path = match &base {
                Some(base) if path.is_relative() => base.join(path),
                _ => path,
            };
            roots.extend(armed_at(&path));
        }
        if let Some(command) = super::adapters::shell_command(tool) {
            let home = std::env::var_os("HOME").map(PathBuf::from);
            for path in named_paths(&command, base.as_deref(), home.as_deref()) {
                roots.extend(armed_at(&path));
            }
        }
    }
    roots
}

/// Every path a shell command may name, read from its words without a
/// shell: each word, or the value of a `--flag=value` word, with the
/// redirect and operator marks stripped from its ends, a leading `~`
/// replaced by `home`, and a relative word joined to `base`. A `cd`,
/// `-R`, `-C`, or `--repo` target is a word like any other, and so is a
/// plain argument. A word that names no path lands under `base`, which
/// the event's directory registers already. Without `base`, a relative
/// word names nothing. A path the shell builds, by a variable, a glob,
/// or a script, is not here.
fn named_paths(command: &str, base: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    super::install::words(command)
        .iter()
        .filter_map(|word| {
            let word = match word.split_once('=') {
                Some((flag, value)) if flag.starts_with('-') => value,
                _ => word.as_str(),
            };
            // `2>err`, then `>>out`, `<in`, `&>err`, `(cd`, `;`, `&&`, `||`.
            let word = match word.strip_prefix(|c: char| c.is_ascii_digit()) {
                Some(rest) if rest.starts_with(['<', '>']) => rest,
                _ => word,
            };
            let word = word
                .trim_start_matches(['<', '>', '|', '&', ';', '('])
                .trim_end_matches([';', '&', '|', ')']);
            if word.is_empty() || word.starts_with('-') {
                return None;
            }
            let path = match (word.strip_prefix('~'), home) {
                (Some(""), Some(home)) => home.to_path_buf(),
                (Some(rest), Some(home)) if rest.starts_with('/') => {
                    home.join(rest.trim_start_matches('/'))
                }
                _ => PathBuf::from(word),
            };
            if path.is_absolute() {
                Some(path)
            } else {
                base.map(|base| base.join(path))
            }
        })
        .collect()
}

/// Register this session's touch: add its key to the set under the lock,
/// or leave a marker when the lock does not come in time.
fn open(root: &Path, harness: Harness, key: &str, snapshot: bool) -> Result<HandlerResult, String> {
    let registered = match Lock::take(root, PROMPT_WAIT) {
        Ok(_lock) => register(root, harness, key, snapshot).map(drop),
        Err(why) => Err(why),
    };
    if let Err(why) = registered {
        // The safe direction: a touch the next check must see, whatever
        // kept the slot from taking it.
        touch(root, harness, key).map_err(|e| format!("{why}; and {e}"))?;
    }
    Ok(HandlerResult::default())
}

/// Add `key` to the slot, under the lock the caller holds, and give back
/// the set with it in. With `snapshot`, the first key since the last wrap
/// also records the working copy's commit as the base of the card's stat.
fn register(root: &Path, harness: Harness, key: &str, snapshot: bool) -> Result<Slot, String> {
    let mut slot = read_state(root)?;
    let first = slot.keys.is_empty();
    let path = root.join(".jj").join(SLOT);
    if slot.keys.insert(key.to_string()) {
        slot.who
            .entry(key.to_string())
            .or_insert_with(|| touch_of(harness));
        // The key goes down before any jj call: a snapshot the dispatcher
        // kills at its timeout loses the base, never the touch.
        write_slot(&path, &slot)?;
    }
    if first && snapshot {
        slot.commit = Some(jj(
            root,
            &["log", "-r", "@", "--no-graph", "-T", "commit_id"],
        )?);
        write_slot(&path, &slot)?;
    }
    Ok(slot)
}

/// Close this session's turn: register its touch, then wrap `@` when
/// this session alone touched the workspace since the last wrap and
/// every handler before this one ran, otherwise say why not.
fn finish(
    harness: Harness,
    root: &Path,
    key: &str,
    merged: Option<&str>,
    failed: &str,
) -> Result<HandlerResult, String> {
    // An earlier handler kept the turn going, or a gate said no: the turn
    // is not over. The stop is still a touch, and the set waits for the
    // real end.
    if matches!(merged, Some("stop" | "deny")) {
        return open(root, harness, key, false);
    }
    let _lock = match Lock::take(root, FINISH_WAIT) {
        Ok(lock) => lock,
        Err(why) => {
            touch(root, harness, key)?;
            return Ok(note(&format!("nothing wrapped: {why}")));
        }
    };
    let slot = match register(root, harness, key, false) {
        Ok(slot) => slot,
        Err(why) => {
            touch(root, harness, key)?;
            return Ok(note(&format!(
                "nothing wrapped: {why}; sort out the working copy, then run rune hook wrap reset"
            )));
        }
    };
    if key == UNKNOWN || slot.keys.len() != 1 {
        return Ok(note(&shared(&slot, key)));
    }
    if !failed.is_empty() {
        return Ok(note(&unfinished(failed)));
    }
    let probe = probe(root)?;
    let slot_path = root.join(".jj").join(SLOT);
    if probe.empty {
        // Nothing to attribute: this session alone touched the workspace
        // and left nothing in it.
        remove(&slot_path)?;
        return Ok(HandlerResult::default());
    }
    let stat = match (probe.described, &slot.commit) {
        (true, Some(base)) => jj(
            root,
            &[
                "--ignore-working-copy",
                "diff",
                "--from",
                base,
                "--to",
                "@",
                "--stat",
            ],
        )?,
        _ => jj(
            root,
            &["--ignore-working-copy", "diff", "-r", "@", "--stat"],
        )?,
    };
    let stat = stat.lines().last().unwrap_or("no files").trim().to_string();
    if probe.described {
        jj(root, &["new"])?;
    } else {
        let stamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        jj(root, &["commit", "-m", &format!("{harness}: turn {stamp}")])?;
    }
    // The wrap committed: the set empties. The lock is still held, so no
    // prompt registered between the check and here.
    remove(&slot_path)?;
    Ok(HandlerResult {
        message: Some(card(harness, &probe, &stat)),
        ..HandlerResult::default()
    })
}

/// The message for a workspace that more than one session touched, or
/// whose touch names no session: who touched it, and the reset.
fn shared(slot: &Slot, key: &str) -> String {
    let keys = &slot.keys;
    let others: Vec<String> = keys
        .iter()
        .map(String::as_str)
        .filter(|k| *k != key)
        .map(|k| label(k, slot.who.get(k)))
        .collect();
    let who = if key == UNKNOWN {
        let also = if others.is_empty() {
            String::new()
        } else {
            format!(
                ", and {} touched the workspace since the last wrap",
                others.join(", ")
            )
        };
        format!("the payload names no session, so its touch is nobody's{also}")
    } else if others.is_empty() {
        "no session registered a touch here".to_string()
    } else if keys.contains(key) {
        format!(
            "this session and {} touched the workspace since the last wrap",
            others.join(", ")
        )
    } else {
        format!(
            "{} touched the workspace since the last wrap and this session did not register",
            others.join(", ")
        )
    };
    format!(
        "nothing wrapped: {who}; the edits stay in the working copy. Sort it out, then run rune hook wrap reset in it"
    )
}

/// A key as the owner reads it: the unknown session by what it is, a
/// recorded session by its harness and its first touch, and a key with
/// no record by itself. No harness puts a session name in its hook
/// payload, so none is here.
fn label(key: &str, touch: Option<&Touch>) -> String {
    if key == UNKNOWN {
        return "a session without an id".to_string();
    }
    match touch {
        Some(touch) => format!("a {} session (first touch {})", touch.harness, touch.since),
        None => key.to_string(),
    }
}

/// The record of a touch made now.
fn touch_of(harness: Harness) -> Touch {
    Touch {
        harness,
        since: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    }
}

/// The message for a turn a handler before the wrapper failed or timed
/// out in: the turn may be unfinished, so it waits for the next stop.
fn unfinished(failed: &str) -> String {
    format!(
        "nothing wrapped: {} failed or timed out before the wrapper, so the turn may be unfinished; the edits stay in the working copy for this session's next turn",
        failed.split(',').collect::<Vec<_>>().join(", ")
    )
}

/// The owner's card: what was wrapped, how to review it, how to undo it.
/// The undo works by content, never by the operation log.
fn card(harness: Harness, probe: &Probe, stat: &str) -> String {
    let change = &probe.change;
    if probe.described {
        format!(
            "Turn added to {change} ({}): {stat}. Review: jj diff -r {change}. Continue in it: jj squash --from @ --into {change} -u. Undo: jj edit {change}, which drops the empty working copy.",
            probe.title
        )
    } else {
        let rewind = if harness == Harness::Claude {
            ", or /rewind"
        } else {
            ""
        };
        format!(
            "Turn wrapped: {stat} as {change}. Review: jj diff -r {change}. Undo: jj squash --from {change} --into @ -u{rewind}; the undo conflicts when a later turn changed the same file."
        )
    }
}

fn note(text: &str) -> HandlerResult {
    HandlerResult {
        message: Some(text.to_string()),
        ..HandlerResult::default()
    }
}

/// `<harness>-<16 hex>`: the harness and a digest of the session id, so
/// the key says nothing about the id and two harnesses never collide. A
/// payload without an id gets the unknown session's key, which touches
/// the workspace like any other and is never alone.
pub(crate) fn session_key(harness: Harness, session_id: Option<&str>) -> String {
    let Some(id) = session_id.filter(|id| !id.is_empty()) else {
        return UNKNOWN.to_string();
    };
    let mut hasher = Sha256::new();
    hasher.update(harness.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(id.as_bytes());
    let digest = hasher.finalize();
    format!("{harness}-{}", crate::cli::sign::seal::hex(&digest[..8]))
}

/// The nearest ancestor of `cwd` that holds a `.jj/` directory: the jj
/// workspace root, found without a process.
fn workspace_root(cwd: &Path) -> Option<PathBuf> {
    let start = cwd.canonicalize().ok()?;
    start
        .ancestors()
        .find(|dir| dir.join(".jj").is_dir())
        .map(Path::to_path_buf)
}

/// The owner's legacy wrap script, which the `20-turn-wrap.sh` part of
/// the legacy Stop folder runs, when it is installed.
pub(crate) fn legacy_script(home: &Path) -> Option<PathBuf> {
    let script = home.join(".config/just/claude/turn/wrap");
    script.is_file().then_some(script)
}

// ---------------------------------------------------------------- slot ----

impl Lock {
    /// The lock on `root`'s slot, tried every `RETRY` until `wait` is up.
    fn take(root: &Path, wait: Duration) -> Result<Self, String> {
        let path = root.join(".jj").join(LOCK);
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        let deadline = Instant::now() + wait;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(RETRY);
                }
                Err(fs::TryLockError::WouldBlock) => {
                    return Err(format!(
                        "another wrapper held {} for {} ms",
                        path.display(),
                        wait.as_millis()
                    ));
                }
                Err(fs::TryLockError::Error(e)) => {
                    return Err(format!("cannot lock {}: {e}", path.display()));
                }
            }
        }
    }
}

/// The slot with every touch marker folded in, under the lock the caller
/// holds. A marker is removed once the slot holds its key. An unreadable
/// or malformed slot or marker folder is an error, and the caller wraps
/// nothing on it. A marker's content is its touch record; one that holds
/// none, or that nothing reads, folds its key alone.
fn read_state(root: &Path) -> Result<Slot, String> {
    let dir = root.join(".jj");
    let path = dir.join(SLOT);
    let mut slot = read_slot(&path)?.unwrap_or_else(|| Slot {
        v: SLOT_VERSION,
        keys: BTreeSet::new(),
        commit: None,
        who: BTreeMap::new(),
    });
    let touched = dir.join(TOUCHED);
    let markers = match fs::read_dir(&touched) {
        Ok(markers) => markers,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(slot),
        Err(e) => return Err(format!("cannot read {}: {e}", touched.display())),
    };
    let mut folded = Vec::new();
    for marker in markers {
        let marker = marker.map_err(|e| format!("cannot read {}: {e}", touched.display()))?;
        let key = marker.file_name().to_string_lossy().into_owned();
        if let Some(touch) = fs::read_to_string(marker.path())
            .ok()
            .and_then(|text| serde_json::from_str::<Touch>(&text).ok())
        {
            slot.who.entry(key.clone()).or_insert(touch);
        }
        slot.keys.insert(key);
        folded.push(marker.path());
    }
    if !folded.is_empty() {
        write_slot(&path, &slot)?;
        for marker in folded {
            remove(&marker)?;
        }
    }
    Ok(slot)
}

/// A touch marker for `key`, made without the lock: one file whose name
/// is the key, so two markers for one key are one marker, and whose
/// content is the touch record the card reads.
fn touch(root: &Path, harness: Harness, key: &str) -> Result<(), String> {
    let dir = root.join(".jj").join(TOUCHED);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let path = dir.join(key);
    let record = serde_json::to_string(&touch_of(harness)).map_err(|e| e.to_string())?;
    fs::write(&path, record).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn read_slot(path: &Path) -> Result<Option<Slot>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a turn-wrap slot: {e}", path.display()))?;
    match value.get("v").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(SLOT_VERSION) => {}
        Some(v) => {
            return Err(format!(
                "{} is a version {v} slot, and this rune reads version {SLOT_VERSION}",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{} is not a turn-wrap slot: it names no version",
                path.display()
            ));
        }
    }
    let slot: Slot = serde_json::from_value(value)
        .map_err(|e| format!("{} is not a turn-wrap slot: {e}", path.display()))?;
    Ok(Some(slot))
}

/// Write the slot whole: a temporary file beside it, then a rename, so a
/// reader never sees a partial slot.
fn write_slot(path: &Path, slot: &Slot) -> Result<(), String> {
    let text = serde_json::to_string(slot).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("open.{}", std::process::id()));
    fs::write(&temp, text).map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        format!("cannot replace {}: {e}", path.display())
    })
}

/// Remove a file that may be gone already.
fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
    }
}

// ------------------------------------------------------------------ jj ----

/// Run jj in the workspace with signing off, so a hook never waits on the
/// owner's key. The trimmed stdout on success.
fn jj(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("jj")
        .args(["--color", "never", "--no-pager", "--quiet"])
        .args(["--config", "signing.behavior=\"drop\""])
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run jj: {e}"))?;
    if !output.status.success() {
        let tail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "jj {} failed: {}",
            args.iter()
                .find(|a| !a.starts_with("--"))
                .copied()
                .unwrap_or_default(),
            tail.lines().last().unwrap_or("no output").trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Snapshot the working copy and read what `@` is.
fn probe(root: &Path) -> Result<Probe, String> {
    let line = jj(
        root,
        &[
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            r#"if(empty, "empty", "full") ++ "\t" ++ if(description, "described", "bare") ++ "\t" ++ change_id.short(8) ++ "\t" ++ description.first_line()"#,
        ],
    )?;
    let mut fields = line.splitn(4, '\t');
    let (Some(empty), Some(described), Some(change)) =
        (fields.next(), fields.next(), fields.next())
    else {
        return Err(format!("jj log answered `{line}`, not a probe line"));
    };
    Ok(Probe {
        empty: empty == "empty",
        described: described == "described",
        change: change.to_string(),
        title: fields.next().unwrap_or_default().to_string(),
    })
}

// -------------------------------------------------------------- switch ----

/// `rune hook wrap on|off|status|reset` in the current jj workspace.
pub(crate) fn switch(action: &crate::cli::WrapAction, json: bool) -> Result<i32, Error> {
    use crate::cli::WrapAction;
    if !matches!(action, WrapAction::Status)
        && let Some(name) = crate::cli::surface::model_session()
    {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "rune hook wrap is the owner's switch, and this process runs inside a model session ({name} is set); the owner runs it from a shell of their own"
            ),
        )
        .with_code("hook.wrap_in_session"));
    }
    let cwd = std::env::current_dir()
        .map_err(|e| Error::io(format!("cannot read the working directory: {e}")))?;
    let Some(root) = workspace_root(&cwd) else {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{} is not inside a jj workspace; turn wrapping needs one",
                cwd.display()
            ),
        )
        .with_code("hook.wrap_no_jj")
        .with_fix_command("jj git init --colocate"));
    };
    let path = root.join(".jj").join(SWITCH);
    let mut reset: Option<BTreeSet<String>> = None;
    match action {
        WrapAction::On => {
            fs::write(&path, "on\n")
                .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
        }
        WrapAction::Off => {
            if let Err(error) = fs::remove_file(&path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                return Err(Error::io(format!(
                    "cannot remove {}: {error}",
                    path.display()
                )));
            }
        }
        WrapAction::Reset => {
            reset = Some(clear(&root).map_err(|why| {
                Error::new(ErrorKind::Io, why).with_code("hook.wrap_reset_failed")
            })?);
        }
        WrapAction::Status => {}
    }
    let state = if path.is_file() { "on" } else { "off" };
    if json {
        let mut out = serde_json::json!({"turn_wrap": state, "root": root});
        if let Some(keys) = &reset {
            out["reset"] = serde_json::json!(keys);
        }
        println!("{out}");
    } else if let Some(keys) = &reset {
        if keys.is_empty() {
            println!(
                "turn wrap reset in {}: no session had touched it",
                root.display()
            );
        } else {
            println!(
                "turn wrap reset in {}: forgot {}",
                root.display(),
                keys.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
    } else if matches!(action, WrapAction::Status) {
        println!("{state}");
    } else {
        println!("turn wrap {state} in {}", root.display());
    }
    Ok(0)
}

/// Forget every touch of the workspace, under the lock: the slot and the
/// markers go. The keys that were forgotten, for the owner's eyes.
fn clear(root: &Path) -> Result<BTreeSet<String>, String> {
    let _lock = Lock::take(root, RESET_WAIT)?;
    let dir = root.join(".jj");
    let keys = match read_state(root) {
        Ok(slot) => slot.keys,
        // A malformed slot is what the reset is for.
        Err(_) => BTreeSet::new(),
    };
    remove(&dir.join(SLOT))?;
    let touched = dir.join(TOUCHED);
    match fs::remove_dir_all(&touched) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("cannot remove {}: {e}", touched.display())),
    }
    Ok(keys)
}

#[cfg(test)]
mod tests;
