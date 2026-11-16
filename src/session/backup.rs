//! The daily backup of a harness's databases. A live `SQLite` file copied
//! byte for byte may be inconsistent, so once per UTC day each database
//! gets one online backup through `sqlite3 <db> ".backup <file>"`. A
//! database that did not change since its last backup is left alone.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What one daily backup pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The databases backed up this pass.
    pub backed_up: usize,
    /// The databases that already had today's backup or did not change.
    pub current: usize,
    /// The databases of excluded sessions, left without a backup.
    pub skipped: usize,
    /// `sqlite3` was not found; nothing was backed up.
    pub tool_missing: bool,
    /// The first failure, when one database could not be backed up.
    pub failure: Option<String>,
}

/// The folder of one day's backups under the harness's versions.
#[must_use]
pub fn day_dir(versions: &Path, day: &str) -> PathBuf {
    versions.join(format!("{day}-daily"))
}

/// Back up every `*.db` directly under `conversations` into the day's
/// folder, at most once per database per day. `excluded` gets the
/// conversation id of each database. A database it names gets no backup,
/// as it gets no native copy.
pub fn daily(
    conversations: &Path,
    versions: &Path,
    day: &str,
    sqlite3: &Path,
    excluded: &dyn Fn(&str) -> bool,
) -> Report {
    let mut report = Report::default();
    let Ok(entries) = std::fs::read_dir(conversations) else {
        return report;
    };
    let mut databases: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "db"))
        .collect();
    databases.sort();
    if databases.is_empty() {
        return report;
    }
    let today = day_dir(versions, day);
    for database in databases {
        let Some(name) = database.file_name() else {
            continue;
        };
        if super::source::antigravity_session(&name.to_string_lossy()).is_some_and(excluded) {
            report.skipped += 1;
            continue;
        }
        let target = today.join(name);
        if target.exists() || !changed_since_last_backup(&database, versions, name) {
            report.current += 1;
            continue;
        }
        match backup_one(sqlite3, &database, &target) {
            Ok(()) => report.backed_up += 1,
            Err(Failure::ToolMissing) => {
                report.tool_missing = true;
                return report;
            }
            Err(Failure::Other(message)) => {
                report.failure.get_or_insert(message);
            }
        }
    }
    report
}

enum Failure {
    ToolMissing,
    Other(String),
}

/// Whether the database changed after its newest backup in any day folder.
fn changed_since_last_backup(database: &Path, versions: &Path, name: &std::ffi::OsStr) -> bool {
    let Ok(modified) = std::fs::metadata(database).and_then(|m| m.modified()) else {
        return true;
    };
    let Ok(entries) = std::fs::read_dir(versions) else {
        return true;
    };
    let newest = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("-daily"))
        })
        .filter_map(|day| std::fs::metadata(day.join(name)).ok())
        .filter_map(|metadata| metadata.modified().ok())
        .max();
    newest.is_none_or(|backup| modified > backup)
}

fn backup_one(sqlite3: &Path, database: &Path, target: &Path) -> Result<(), Failure> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Failure::Other(format!("cannot create {}: {error}", parent.display()))
        })?;
    }
    let target_text = target.to_string_lossy();
    if target_text.contains('\'') {
        return Err(Failure::Other(format!(
            "cannot back up into a path with a quote: {target_text}"
        )));
    }
    let temporary = format!("{target_text}.tmp");
    let output = Command::new(sqlite3)
        .arg(database)
        .arg(format!(".backup '{temporary}'"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    let output = match output {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Failure::ToolMissing);
        }
        Err(error) => {
            return Err(Failure::Other(format!(
                "cannot start {}: {error}",
                sqlite3.display()
            )));
        }
    };
    if !output.status.success() {
        let _ = std::fs::remove_file(&temporary);
        return Err(Failure::Other(format!(
            "sqlite3 backup of {} failed: {}",
            database.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    std::fs::rename(&temporary, target).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        Failure::Other(format!(
            "cannot move {temporary} to {}: {error}",
            target.display()
        ))
    })
}
