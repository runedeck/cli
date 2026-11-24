//! Capture debt: one JSON record per session file that existed and whose
//! native copy failed. Records live under `<state>/capture-debt/`, open
//! ones at the top and closed ones under `closed/` with the reason. The
//! reader also accepts the records the `session-sync` script wrote, which
//! say `provider` instead of `harness`. Nothing here is ever deleted.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One debt record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(alias = "provider")]
    pub harness: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub session: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub cwd: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub harness_status: Option<Value>,
    #[serde(default)]
    pub reason: String,
    /// The copy's error text, when rune recorded the miss.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_reason: Option<String>,
}

/// The script wrote `""` for an absent session or status; read it as none.
fn empty_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.filter(|text| !text.is_empty()))
}

/// Why a record closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// A later capture of the session succeeded.
    Copied,
    /// The sweep found a complete native copy.
    Complete,
    /// The source file is gone and no native copy exists.
    Lost,
    /// The session never wrote a file, or the record names no session.
    NoSessionFile,
}

impl Closed {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Copied => "copied",
            Self::Complete => "complete",
            Self::Lost => "lost",
            Self::NoSessionFile => "no-session-file",
        }
    }
}

/// An open record and the file that holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Open {
    pub path: PathBuf,
    pub record: Record,
}

/// The debt folder under the state directory.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
    counter: std::cell::Cell<u32>,
}

impl Store {
    #[must_use]
    pub fn new(state_dir: &Path) -> Self {
        Self {
            dir: state_dir.join("capture-debt"),
            counter: std::cell::Cell::new(0),
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every open record, oldest first. A file that is not a record is
    /// left where it is and skipped.
    pub fn open(&self) -> Result<Vec<Open>, String> {
        let mut records: Vec<Open> = read_records(&self.dir)?
            .into_iter()
            .map(|(path, record)| Open { path, record })
            .collect();
        records.sort_by(|a, b| a.record.at.cmp(&b.record.at).then(a.path.cmp(&b.path)));
        Ok(records)
    }

    /// Every closed record, newest closure first.
    pub fn closed(&self) -> Result<Vec<Record>, String> {
        let mut records: Vec<Record> = read_records(&self.dir.join("closed"))?
            .into_iter()
            .map(|(_, record)| record)
            .collect();
        records.sort_by(|a, b| b.closed_at.cmp(&a.closed_at).then(b.at.cmp(&a.at)));
        Ok(records)
    }

    /// Write one open record. The file name carries the harness, the
    /// stamp, the pid, and a counter, so two misses in one run never
    /// collide.
    pub fn record(&self, record: &Record) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|error| format!("cannot create {}: {error}", self.dir.display()))?;
        let index = self.counter.get();
        self.counter.set(index + 1);
        let path = self.dir.join(format!(
            "{}-{}-{}-{index}.json",
            record.harness,
            super::stamp(),
            std::process::id()
        ));
        let text = serde_json::to_string_pretty(record).map_err(|error| error.to_string())?;
        std::fs::write(&path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        restrict(&path);
        Ok(path)
    }

    /// Close one record: it moves to `closed/` with the time and the
    /// reason. The open file goes only after the closed one is written.
    pub fn close(&self, open: &Open, reason: Closed) -> Result<(), String> {
        let closed_dir = self.dir.join("closed");
        std::fs::create_dir_all(&closed_dir)
            .map_err(|error| format!("cannot create {}: {error}", closed_dir.display()))?;
        let mut record = open.record.clone();
        record.closed_at = Some(super::now());
        record.closed_reason = Some(reason.as_str().to_string());
        let name = open
            .path
            .file_name()
            .map(std::ffi::OsStr::to_os_string)
            .ok_or_else(|| format!("{} has no file name", open.path.display()))?;
        let target = free_name(&closed_dir.join(name));
        let text = serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?;
        std::fs::write(&target, text)
            .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
        restrict(&target);
        std::fs::remove_file(&open.path)
            .map_err(|error| format!("cannot remove {}: {error}", open.path.display()))?;
        Ok(())
    }
}

/// What the health check reports: the open count and the oldest date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub open: usize,
    /// The date part of the oldest open record's `at`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest: Option<String>,
}

/// The summary of the open records under `state_dir`. An absent folder is
/// an empty summary.
pub fn summary(state_dir: &Path) -> Result<Summary, String> {
    let records = Store::new(state_dir).open()?;
    let oldest = records
        .iter()
        .map(|open| open.record.at.as_str())
        .filter(|at| !at.is_empty())
        .min()
        .map(|at| at.chars().take(10).collect());
    Ok(Summary {
        open: records.len(),
        oldest,
    })
}

fn read_records(dir: &Path) -> Result<Vec<(PathBuf, Record)>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot read {}: {error}", dir.display())),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") || !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(record) = serde_json::from_str::<Record>(&text) {
            records.push((path, record));
        }
    }
    Ok(records)
}

/// A closed record with a name already taken gets a numeric suffix.
fn free_name(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map_or_default(|s| s.to_string_lossy().into_owned());
    let parent = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    (1..u32::MAX)
        .map(|index| parent.join(format!("{stem}-{index}.json")))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

/// A record names folders and sessions: owner-readable only, as the
/// script wrote them.
fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
