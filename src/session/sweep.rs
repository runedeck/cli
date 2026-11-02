//! The scheduled sweep: every supported harness, the plans, the daily
//! database backup, the Grok transcripts, and the debt reconciliation.
//! A sweep never fails: what it could not do is in the log and the debt.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::capture::{copy_all, copy_exists, copy_plans};
use super::debt::{self, Closed, Record};
use super::source::{self, Harness, SessionFile, kind_of};
use super::{Config, Trigger, backup, copy, grok};

/// What the sweep did for one harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum HarnessOutcome {
    Copied {
        files: usize,
        versions: usize,
        skipped: usize,
    },
    Deferred,
    NoStore,
    Failed {
        message: String,
    },
}

/// What the whole sweep did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SweepReport {
    pub harnesses: BTreeMap<String, HarnessOutcome>,
    /// Plans copied or refreshed.
    pub plans: usize,
    /// Databases backed up today.
    pub backups: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_error: Option<String>,
    /// Grok transcripts rendered.
    pub transcripts: usize,
    /// Debt records closed by the reconciliation.
    pub debt_closed: usize,
    /// Debt records still open after it.
    pub debt_open: usize,
}

/// Sweep every source.
pub fn sweep(config: &Config) -> SweepReport {
    let log = config.log();
    log.note("sweep start");
    let store = debt::Store::new(&config.state_dir);
    let mut harnesses = BTreeMap::new();
    let mut swept = BTreeSet::new();
    for harness in Harness::ALL {
        let outcome = sweep_harness(config, harness, &store);
        if matches!(outcome, HarnessOutcome::Copied { .. }) {
            swept.insert(harness);
        }
        harnesses.insert(harness.as_str().to_string(), outcome);
    }
    let plans = match copy_plans(config) {
        Ok(count) => count,
        Err(message) => {
            log.note(&format!("plans: {message}"));
            0
        }
    };
    let backups = backup_databases(config, &store);
    let transcripts = match grok::render_all(
        &config.layout().sessions(Harness::Grok),
        &config.layout().transcripts(Harness::Grok),
        None,
    ) {
        Ok(count) => count,
        Err(message) => {
            log.note(&format!("grok transcripts: {message}"));
            0
        }
    };
    let (debt_closed, debt_open) = reconcile(config, &store, &swept);
    log.note(&format!(
        "sweep done: {debt_closed} debt closed, {debt_open} open"
    ));
    SweepReport {
        harnesses,
        plans,
        backups: backups.backed_up,
        backup_error: backups.failure,
        transcripts,
        debt_closed,
        debt_open,
    }
}

fn sweep_harness(config: &Config, harness: Harness, store: &debt::Store) -> HarnessOutcome {
    let log = config.log();
    let Some(lock) = super::lock::acquire(&config.state_dir, harness.as_str(), config.lock_wait)
    else {
        log.note(&format!(
            "{harness} locked by another run; left for the next sweep"
        ));
        return HarnessOutcome::Deferred;
    };
    let files = match source::list(harness, &config.stores) {
        Ok(Some(files)) => files,
        Ok(None) => {
            log.note(&format!(
                "{harness} store missing ({})",
                config.stores.root(harness).display()
            ));
            return HarnessOutcome::NoStore;
        }
        Err(message) => {
            log.note(&format!("{harness} store unreadable: {message}"));
            return HarnessOutcome::Failed { message };
        }
    };
    let (kept, skipped): (Vec<SessionFile>, Vec<SessionFile>) =
        files.into_iter().partition(|file| {
            !config
                .exclude
                .matches(file.session.as_deref(), file.cwd.as_deref())
        });
    let copied = copy_all(config, harness, &kept);
    drop(lock);
    log.note(&format!(
        "{harness}: {} copied, {} versioned, {} skipped, {} failed",
        copied.files,
        copied.versions,
        skipped.len(),
        copied.failures.len()
    ));
    if let Some(first) = copied.failures.first() {
        record(store, harness, "raw", Some(first), &log);
        return HarnessOutcome::Failed {
            message: first.clone(),
        };
    }
    HarnessOutcome::Copied {
        files: copied.files,
        versions: copied.versions,
        skipped: skipped.len(),
    }
}

/// One online backup per database per UTC day. A host without `sqlite3`
/// records one debt record, reason `backup-tool-missing`, and not a
/// second one while that record is open.
fn backup_databases(config: &Config, store: &debt::Store) -> backup::Report {
    let log = config.log();
    let conversations = &config.stores.antigravity_conversations;
    if !conversations.is_dir() {
        return backup::Report::default();
    }
    let report = backup::daily(
        conversations,
        &config.layout().versions(Harness::Antigravity),
        &super::today(),
        &config.sqlite3,
    );
    log.note(&format!(
        "antigravity backups: {} new, {} current",
        report.backed_up, report.current
    ));
    if report.tool_missing {
        let already = store.open().is_ok_and(|open| {
            open.iter()
                .any(|entry| entry.record.reason == "backup-tool-missing")
        });
        if !already {
            record(
                store,
                Harness::Antigravity,
                "backup-tool-missing",
                None,
                &log,
            );
        }
    } else if let Some(failure) = &report.failure {
        log.note(&format!("antigravity backup: {failure}"));
    }
    report
}

fn record(
    store: &debt::Store,
    harness: Harness,
    reason: &str,
    error: Option<&String>,
    log: &super::Log,
) {
    let record = Record {
        harness: harness.as_str().to_string(),
        session: None,
        cwd: None,
        source: Some(Trigger::Sweep.as_str().to_string()),
        harness_status: None,
        reason: reason.to_string(),
        error: error.cloned(),
        at: super::now(),
        closed_at: None,
        closed_reason: None,
    };
    match store.record(&record) {
        Ok(path) => log.note(&format!(
            "debt recorded: {harness} {reason} ({})",
            path.display()
        )),
        Err(message) => log.note(&format!("cannot record debt ({reason}): {message}")),
    }
}

/// Close the open records the sweep can settle, and count what stays.
fn reconcile(config: &Config, store: &debt::Store, swept: &BTreeSet<Harness>) -> (usize, usize) {
    let log = config.log();
    let open = match store.open() {
        Ok(open) => open,
        Err(message) => {
            log.note(&format!("debt unreadable: {message}"));
            return (0, 0);
        }
    };
    let mut listings: BTreeMap<Harness, Option<Vec<SessionFile>>> = BTreeMap::new();
    let mut closed = 0;
    let mut remaining = 0;
    for entry in open {
        let verdict = verdict(config, &entry.record, swept, &mut listings);
        match verdict {
            Some(reason) => match store.close(&entry, reason) {
                Ok(()) => {
                    closed += 1;
                    log.note(&format!(
                        "debt closed as {}: {}",
                        reason.as_str(),
                        entry.path.display()
                    ));
                }
                Err(message) => {
                    remaining += 1;
                    log.note(&format!("cannot close debt: {message}"));
                }
            },
            None => remaining += 1,
        }
    }
    (closed, remaining)
}

/// How one open record closes, or `None` to leave it open.
fn verdict(
    config: &Config,
    record: &Record,
    swept: &BTreeSet<Harness>,
    listings: &mut BTreeMap<Harness, Option<Vec<SessionFile>>>,
) -> Option<Closed> {
    let harness = Harness::parse(&record.harness);
    match (harness, record.session.as_deref()) {
        // A harness Rune no longer captures, or the script's own sweep
        // records: nothing will ever copy them.
        (None, None) => Some(Closed::NoSessionFile),
        (None, Some(_)) => Some(Closed::Lost),
        (Some(harness), None) => swept.contains(&harness).then_some(Closed::Complete),
        (Some(harness), Some(session)) => {
            // A store that is absent on this host settles nothing: the
            // record waits for the host that has it.
            let files = listings
                .entry(harness)
                .or_insert_with(|| source::list(harness, &config.stores).ok().flatten())
                .as_deref()?;
            let sessions = config.layout().sessions(harness);
            let sources: Vec<&SessionFile> = files
                .iter()
                .filter(|file| file.session.as_deref() == Some(session))
                .collect();
            if sources.is_empty() {
                return Some(if copy_exists(&sessions, session) {
                    Closed::Complete
                } else {
                    Closed::Lost
                });
            }
            let complete = sources.iter().all(|file| {
                copy::complete(
                    &file.path,
                    &sessions.join(&file.relative),
                    kind_of(&file.path),
                )
            });
            complete.then_some(Closed::Complete)
        }
    }
}
