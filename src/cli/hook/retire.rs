//! When a plan generation no table names may go. A session that loaded
//! its hooks before an install keeps calling the old `--plan` path, and
//! under `deny` each of its guarded calls fails once the file is gone. So
//! the manifest keeps a record for each generation no table names, and a
//! generation goes only when its record is `RETENTION_DAYS` old,
//! `RETENTION_INSTALLS` installs have run since, and installs on
//! `RETENTION_DAYS` different days have found it unnamed. A clock set
//! forward can age a record, and a burst of installs can raise the count,
//! but neither adds days on which an install ran.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// How long a generation stays after the tables stop naming it.
pub(crate) const RETENTION_DAYS: i64 = 14;

/// How many installs must follow the one that retired a generation.
pub(crate) const RETENTION_INSTALLS: u64 = 2;

/// A generation no table names, kept for a session that loaded its hooks
/// before the tables changed and still calls its path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Retired {
    pub generation: String,
    /// When an install found that no table names it, RFC 3339 in UTC.
    pub since: String,
    /// The manifest's install count at that time.
    #[serde(default)]
    pub install: Option<u64>,
    /// On how many different days an install has found it unnamed.
    #[serde(default)]
    pub days: u32,
    /// The last of those days, `YYYY-MM-DD` in UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_day: Option<String>,
}

fn day(now: DateTime<Utc>) -> String {
    now.format("%Y-%m-%d").to_string()
}

/// A record that a generation stopped being named now, at `install`.
pub(crate) fn fresh(path: &Path, now: DateTime<Utc>, install: u64) -> Retired {
    Retired {
        generation: path.display().to_string(),
        since: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        install: Some(install),
        days: 1,
        last_day: Some(day(now)),
    }
}

/// A path as the file system names it, or as given when it is gone.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn keys(paths: &BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
    paths.iter().map(|path| canonical(path)).collect()
}

/// The records without any for a generation the old or the new tables
/// name. Install writes these before it changes the tables, so an install
/// killed after the tables changed leaves no record older than the last
/// time a table named the generation. It can only delay a deletion.
pub(crate) fn unnamed_only(
    records: &[Retired],
    before: &BTreeSet<PathBuf>,
    after: &BTreeSet<PathBuf>,
) -> Vec<Retired> {
    let (before, after) = (keys(before), keys(after));
    records
        .iter()
        .filter(|record| {
            let key = canonical(Path::new(&record.generation));
            !after.contains(&key) && !before.contains(&key)
        })
        .cloned()
        .collect()
}

/// The records once install knows which generations the tables named
/// before it and name after it: none for a generation a table names, and
/// a fresh one for each generation the old tables named and the new ones
/// do not. Every other record stays.
pub(crate) fn reconcile(
    records: &[Retired],
    before: &BTreeSet<PathBuf>,
    after: &BTreeSet<PathBuf>,
    now: DateTime<Utc>,
    install: u64,
) -> Vec<Retired> {
    let after_keys = keys(after);
    let mut out = unnamed_only(records, before, after);
    for path in before {
        if !after_keys.contains(&canonical(path)) {
            out.push(fresh(path, now, install));
        }
    }
    out.sort_by(|a, b| a.generation.cmp(&b.generation));
    out.dedup_by(|a, b| a.generation == b.generation);
    out
}

/// What the cleanup makes of an unnamed generation's record: the record
/// it keeps, with today counted, or `None` when the generation may go. A
/// record that does not parse, or has no install count, starts afresh.
pub(crate) fn observe(
    record: Option<&Retired>,
    path: &Path,
    now: DateTime<Utc>,
    installs: u64,
) -> Option<Retired> {
    let Some(record) = record else {
        return Some(fresh(path, now, installs));
    };
    let (Ok(since), Some(install)) = (DateTime::parse_from_rfc3339(&record.since), record.install)
    else {
        return Some(fresh(path, now, installs));
    };
    let today = day(now);
    let mut kept = Retired {
        generation: path.display().to_string(),
        since: since
            .with_timezone(&Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        install: Some(install),
        days: record.days.max(1),
        last_day: record.last_day.clone(),
    };
    // A day counts once, and a clock set back adds none.
    if kept
        .last_day
        .as_deref()
        .is_none_or(|last| today.as_str() > last)
    {
        if kept.last_day.is_some() {
            kept.days += 1;
        }
        kept.last_day = Some(today);
    }
    let expired = now - since.with_timezone(&Utc) >= chrono::TimeDelta::days(RETENTION_DAYS)
        && installs >= install.saturating_add(RETENTION_INSTALLS)
        && i64::from(kept.days) >= RETENTION_DAYS;
    (!expired).then_some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn record(since: &str, install: u64, days: u32, last_day: &str) -> Retired {
        Retired {
            generation: "/plans/g.json".to_string(),
            since: since.to_string(),
            install: Some(install),
            days,
            last_day: Some(last_day.to_string()),
        }
    }

    #[test]
    fn a_clock_set_forward_and_two_quick_installs_delete_nothing() {
        // Recorded on day one, then the clock jumps 20 days ahead.
        let path = Path::new("/plans/g.json");
        let mut kept = record("2026-10-01T12:00:00Z", 1, 1, "2026-10-01");
        for (minute, installs) in [("12:00", 2), ("12:01", 3)] {
            let now = at(&format!("2026-10-21T{minute}:00Z"));
            kept = observe(Some(&kept), path, now, installs).expect("kept");
        }
        assert_eq!(kept.days, 2, "both installs ran on one day");
    }

    #[test]
    fn a_generation_goes_after_fourteen_days_two_installs_and_fourteen_install_days() {
        let path = Path::new("/plans/g.json");
        let now = at("2026-10-24T12:00:00Z");
        let old = "2026-10-10T12:00:00Z";
        // Thirteen days seen, today makes fourteen.
        assert_eq!(
            observe(Some(&record(old, 5, 13, "2026-10-23")), path, now, 7),
            None
        );
        // Thirteen days seen and today already counted.
        assert!(observe(Some(&record(old, 5, 13, "2026-10-24")), path, now, 7).is_some());
        // One install since.
        assert!(observe(Some(&record(old, 6, 13, "2026-10-23")), path, now, 7).is_some());
        // A second short of 14 days.
        let young = record("2026-10-10T12:00:01Z", 5, 13, "2026-10-23");
        assert!(observe(Some(&young), path, now, 7).is_some());
    }

    #[test]
    fn a_clock_set_back_adds_no_day() {
        let path = Path::new("/plans/g.json");
        let kept = observe(
            Some(&record("2026-10-01T12:00:00Z", 1, 4, "2026-10-20")),
            path,
            at("2026-10-15T12:00:00Z"),
            2,
        )
        .unwrap();
        assert_eq!(
            (kept.days, kept.last_day.as_deref()),
            (4, Some("2026-10-20"))
        );
    }

    #[test]
    fn a_record_without_a_time_or_a_count_starts_afresh() {
        let path = Path::new("/plans/g.json");
        let now = at("2026-10-24T12:00:00Z");
        let mut garbled = record("last week", 1, 9, "2026-10-01");
        assert_eq!(
            observe(Some(&garbled), path, now, 7),
            Some(fresh(path, now, 7))
        );
        garbled.since = "2026-01-01T00:00:00Z".to_string();
        garbled.install = None;
        assert_eq!(
            observe(Some(&garbled), path, now, 7),
            Some(fresh(path, now, 7))
        );
    }
}
