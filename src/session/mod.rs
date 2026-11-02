//! Session capture: the copy Rune makes of each harness's session files
//! into the owner's data folder. Three callers share it: the
//! `session-capture` hook handler, the runner after a harness exits, and
//! the hourly sweep. The module is read-only on the harness's files and
//! never deletes a copy it made.
//!
//! Layout, kind first and harness second: `Sessions/<harness>/` holds the
//! native copies at their relative path in the store,
//! `SessionVersions/<harness>/<stamp>-<pid>/` the replaced copies that held
//! lost bytes and the daily database backups, `Transcripts/<harness>/` the
//! readable copies, and `Plans/claude/` the plan-mode plans. The capture
//! folder is the file spool that a later importer reads.

pub mod backup;
pub mod copy;
pub mod debt;
pub mod exclude;
pub mod grok;
pub mod lock;
pub mod render;
pub mod source;

mod capture;
mod sweep;

#[cfg(test)]
mod tests;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};

pub use capture::{Outcome, Request, Trigger, capture};
pub use exclude::Exclude;
pub use source::{Harness, Stores};
pub use sweep::{SweepReport, sweep};

/// The lock wait the script used: a hook waits this long for a sweep of
/// the same harness before it defers.
pub const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(20);

/// Everything a capture needs to know about this host.
#[derive(Debug, Clone)]
pub struct Config {
    /// The owner's data folder, `~/Data` unless `DATA_DIR` or the user
    /// configuration says otherwise.
    pub data_dir: PathBuf,
    /// The rune state directory: the log, the locks, and the debt records.
    pub state_dir: PathBuf,
    /// Where each harness keeps its sessions on this host.
    pub stores: Stores,
    /// The sessions the owner classified as work.
    pub exclude: Exclude,
    /// How long a capture waits for the per-harness lock.
    pub lock_wait: Duration,
    /// The `specstory` binary when it is installed. `None` skips the
    /// readable copy of Claude Code sessions.
    pub specstory: Option<PathBuf>,
    /// The `sqlite3` binary for the daily database backup.
    pub sqlite3: PathBuf,
}

impl Config {
    /// The data folder layout.
    #[must_use]
    pub fn layout(&self) -> Layout {
        Layout {
            data_dir: self.data_dir.clone(),
        }
    }

    /// The capture log under the state directory.
    #[must_use]
    pub fn log(&self) -> Log {
        Log {
            path: self.state_dir.join("session-capture.log"),
        }
    }
}

/// The data folder: kind first, harness second, harness names lowercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub data_dir: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn sessions(&self, harness: Harness) -> PathBuf {
        self.data_dir.join("Sessions").join(harness.as_str())
    }

    #[must_use]
    pub fn versions(&self, harness: Harness) -> PathBuf {
        self.data_dir.join("SessionVersions").join(harness.as_str())
    }

    #[must_use]
    pub fn transcripts(&self, harness: Harness) -> PathBuf {
        self.data_dir.join("Transcripts").join(harness.as_str())
    }

    #[must_use]
    pub fn plans(&self) -> PathBuf {
        self.data_dir.join("Plans").join("claude")
    }

    /// The version folder of one run: `<stamp>-<pid>` under the harness's
    /// versions, created only when a replaced copy moves into it.
    #[must_use]
    pub fn version_run(&self, harness: Harness) -> PathBuf {
        self.versions(harness)
            .join(format!("{}-{}", stamp(), std::process::id()))
    }
}

/// The capture log: one line per event, appended. A log that cannot be
/// opened costs nothing; the capture goes on.
#[derive(Debug, Clone)]
pub struct Log {
    path: PathBuf,
}

impl Log {
    /// Append one line, stamped in UTC.
    pub fn note(&self, message: &str) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{} {message}", now());
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The current time as RFC 3339 in UTC, whole seconds.
#[must_use]
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// A compact UTC stamp for a folder or file name: `20261102T134200Z`.
#[must_use]
pub fn stamp() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

/// Today's UTC date as `20261102`, the key of a daily backup.
#[must_use]
pub fn today() -> String {
    Utc::now().format("%Y%m%d").to_string()
}

/// A session id is an identity: validated, never rewritten.
#[must_use]
pub fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
