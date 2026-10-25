//! The hook record: one line per hook the dispatcher ran or failed to
//! start, in a monthly file under `<state>/hooks/record/`. The write
//! happens after the answer is encoded, in a writer the dispatcher waits
//! for no longer than the reserve it keeps for itself. A failed or late
//! write loses the records and costs one stderr line, never the answer.
//!
//! A record holds identifiers, counts, paths, and durations. The finding
//! text, the prompt, the file content, and the hook's output never enter
//! it: the metrics a hook returns are validated to identifiers and
//! integers before they are copied.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::mpsc;
use std::time::Duration;

use chrono::{DateTime, Datelike, SecondsFormat, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Serialized hook metrics may be this many UTF-8 bytes at most.
pub(crate) const METRICS_CAP: usize = 4096;

/// A metrics key, list item, or inner key.
static IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._:/-]{1,64}$").expect("identifier regex"));

/// One line of the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub v: u32,
    /// RFC 3339 in UTC with milliseconds, such as `2026-10-16T00:10:16.912Z`.
    pub ts: String,
    pub harness: String,
    pub event: String,
    pub native_event: String,
    pub hook: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// The first path the tool wrote, when the payload names one.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub exit_status: Option<i32>,
    /// What the hook's output asked for: allow, deny, or stop.
    pub asked: String,
    /// The dispatcher answer for the whole event.
    pub answer: String,
    /// null, timeout, exit, result, or budget.
    #[serde(default)]
    pub failure: Option<String>,
    #[serde(default)]
    pub metrics: Option<Value>,
    /// Absent for a live record; `import-claude` for a backfilled one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The legacy part that ran this hook's command instead, when the
    /// dispatcher skipped the hook; absent when the hook ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
}

impl Record {
    /// A timestamp in the record's spelling.
    pub(crate) fn stamp(ts: &DateTime<Utc>) -> String {
        ts.to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    /// The record's timestamp, or the epoch when the text does not parse.
    pub(crate) fn time(&self) -> DateTime<Utc> {
        self.ts
            .parse::<DateTime<Utc>>()
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
    }

    /// The identity the import's duplicate check uses, when the record has
    /// one: harness, session, tool-use id, and hook. A record without a
    /// session or a tool-use id has no identity and is never a duplicate.
    pub(crate) fn key(&self) -> Option<(String, String, String, String)> {
        Some((
            self.harness.clone(),
            self.session_id.clone()?,
            self.tool_use_id.clone()?,
            self.hook.clone(),
        ))
    }
}

/// The record directory under the state dir.
pub(crate) fn dir() -> Result<PathBuf, rune::error::Error> {
    Ok(crate::cli::state::dir()?.join("hooks").join("record"))
}

/// The monthly file a timestamp lands in.
pub(crate) fn path_for(dir: &Path, ts: &DateTime<Utc>) -> PathBuf {
    dir.join(format!("{:04}-{:02}.jsonl", ts.year(), ts.month()))
}

/// Check a hook's metrics: identifier keys, values that are integers,
/// lists of identifiers, or identifier-to-integer objects, at most
/// `METRICS_CAP` bytes serialized. The error names the first rule broken.
pub(crate) fn validate_metrics(metrics: &Value) -> Result<(), String> {
    let Some(map) = metrics.as_object() else {
        return Err("metrics is not an object".to_string());
    };
    let identifier = |s: &str, what: &str| -> Result<(), String> {
        if IDENTIFIER.is_match(s) {
            Ok(())
        } else {
            Err(format!(
                "{what} `{}` is not an identifier",
                s.chars().take(80).collect::<String>()
            ))
        }
    };
    for (key, value) in map {
        identifier(key, "key")?;
        match value {
            Value::Number(n) if n.is_i64() || n.is_u64() => {}
            Value::Array(items) => {
                for item in items {
                    match item.as_str() {
                        Some(s) => identifier(s, &format!("item of `{key}`"))?,
                        None => return Err(format!("item of `{key}` is not an identifier")),
                    }
                }
            }
            Value::Object(inner) => {
                for (k, v) in inner {
                    identifier(k, &format!("key of `{key}`"))?;
                    if v.as_i64().is_none() && v.as_u64().is_none() {
                        return Err(format!("value of `{key}.{k}` is not an integer"));
                    }
                }
            }
            _ => {
                return Err(format!(
                    "value of `{key}` is not an integer, a list, or an object"
                ));
            }
        }
    }
    let size = metrics.to_string().len();
    if size > METRICS_CAP {
        return Err(format!("metrics is {size} bytes; the cap is {METRICS_CAP}"));
    }
    Ok(())
}

/// The lock every writer and the import take, so two dispatchers or an
/// import and a dispatcher never interleave their lines.
pub(crate) fn lock(dir: &Path) -> std::io::Result<std::fs::File> {
    std::fs::create_dir_all(dir)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".lock"))?;
    file.lock()?;
    Ok(file)
}

/// Append every record to its monthly file, under the directory lock,
/// each file written in one call. Returns the paths written.
fn append(dir: &Path, records: &[Record]) -> std::io::Result<BTreeSet<PathBuf>> {
    let _guard = lock(dir)?;
    append_under_lock(dir, records)
}

/// The append for a caller that already holds the directory lock.
pub(crate) fn append_under_lock(
    dir: &Path,
    records: &[Record],
) -> std::io::Result<BTreeSet<PathBuf>> {
    let mut by_path: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for record in records {
        let path = path_for(dir, &record.time());
        let buffer = by_path.entry(path).or_default();
        buffer.extend(serde_json::to_vec(record).map_err(std::io::Error::other)?);
        buffer.push(b'\n');
    }
    for (path, bytes) in &by_path {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        file.write_all(bytes)?;
    }
    Ok(by_path.into_keys().collect())
}

/// Write the records in a thread and wait for it no longer than
/// `deadline`. Returns the stderr lines a failure or a late writer costs:
/// at most one, naming the directory and the records lost.
pub(crate) fn write_all(dir: &Path, records: Vec<Record>, deadline: Duration) -> Vec<String> {
    write_all_with(dir, records, deadline, append)
}

/// `write_all` with the writer injected, so a test can block it.
pub(crate) fn write_all_with(
    dir: &Path,
    records: Vec<Record>,
    deadline: Duration,
    writer: fn(&Path, &[Record]) -> std::io::Result<BTreeSet<PathBuf>>,
) -> Vec<String> {
    if records.is_empty() {
        return Vec::new();
    }
    let count = records.len();
    let dir_owned = dir.to_path_buf();
    let (tx, rx) = mpsc::channel();
    // The thread is detached on purpose: a writer stuck on a device must
    // not hold the answer. The process exits and takes it along.
    let spawned = std::thread::Builder::new()
        .name("hook-record".into())
        .spawn(move || {
            let outcome = writer(&dir_owned, &records);
            let _ = tx.send(outcome);
        });
    if let Err(error) = spawned {
        return vec![format!(
            "rune hook: could not start the hook record writer: {error}; {count} record(s) lost under {}",
            dir.display()
        )];
    }
    match rx.recv_timeout(deadline) {
        Ok(Ok(_)) => Vec::new(),
        Ok(Err(error)) => vec![format!(
            "rune hook: could not write {count} hook record(s) under {}: {error}",
            dir.display()
        )],
        Err(_) => vec![format!(
            "rune hook: the hook record writer did not finish within {} ms; {count} record(s) lost under {}",
            deadline.as_millis(),
            dir.display()
        )],
    }
}

/// Every record under `dir`, oldest first, from every monthly file. A line
/// that is not a record, such as an unfinished tail after a crash, is
/// skipped and reported once on stderr; the rest of the file still reads.
pub(crate) fn load(dir: &Path) -> Result<Vec<Record>, String> {
    let mut records = Vec::new();
    let mut skipped = 0usize;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(records);
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        .collect();
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Record>(line) {
                Ok(record) => records.push(record),
                Err(error) => {
                    if skipped == 0 {
                        eprintln!(
                            "rune hook: {}:{}: not a hook record, skipped: {error}",
                            path.display(),
                            n + 1
                        );
                    }
                    skipped += 1;
                }
            }
        }
    }
    if skipped > 1 {
        eprintln!(
            "rune hook: {skipped} lines under {} are not hook records, skipped",
            dir.display()
        );
    }
    records.sort_by_key(Record::time);
    Ok(records)
}

#[cfg(test)]
mod tests;
