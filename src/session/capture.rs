//! One capture: the native copy of a harness's session files, scoped to a
//! session id when the caller knows it, under the per-harness lock, then
//! the plans, the readable copy, and the debt bookkeeping.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use super::debt::{self, Closed, Record};
use super::source::{self, Harness, SessionFile, kind_of};
use super::{Config, copy, grok, lock, render, valid_session_id};

/// Who asked for the capture. Recorded in a debt record as `source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Trigger {
    /// The `session-capture` hook handler, detached, at session end.
    Hook,
    /// `rune run` or `rune launch` after the harness exited.
    Runner,
    /// The scheduled sweep.
    Sweep,
}

impl Trigger {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hook => "hook",
            Self::Runner => "runner",
            Self::Sweep => "sweep",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "hook" => Some(Self::Hook),
            "runner" | "launcher" => Some(Self::Runner),
            "sweep" | "timer" => Some(Self::Sweep),
            _ => None,
        }
    }
}

/// What to capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub harness: Harness,
    /// The session, when the caller knows it. Without one, the whole store
    /// is the scope, narrowed by `since` when set.
    pub session: Option<String>,
    /// The session's working folder, when the caller knows it.
    pub cwd: Option<PathBuf>,
    /// The harness's exit status, when the runner knows it.
    pub status: Option<i32>,
    pub trigger: Trigger,
    /// Only the files modified at or after this time: the runner's scope
    /// when the harness reported no session id.
    pub since: Option<SystemTime>,
}

/// What one capture did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// The native copy succeeded.
    Copied {
        /// Files copied or refreshed.
        files: usize,
        /// Replaced copies kept as versions.
        versions: usize,
        /// Files of excluded sessions left alone.
        skipped: usize,
        /// Why the readable copy failed, when it did. Never debt.
        #[serde(skip_serializing_if = "Option::is_none")]
        readable_error: Option<String>,
    },
    /// The session is classified as work. Nothing copied, no debt.
    Skipped,
    /// Another capture holds the harness's lock. The next sweep copies.
    Deferred,
    /// The store has no file for the session. No debt.
    NoSessionFile,
    /// The store folder is absent on this host. No debt.
    NoStore,
    /// A native copy failed.
    Failed {
        message: String,
        /// The debt record, when a session file existed.
        #[serde(skip_serializing_if = "Option::is_none")]
        debt: Option<PathBuf>,
    },
}

impl Outcome {
    /// The exit status `rune session capture` answers with: 1 for a
    /// failed native copy, 0 for everything else.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        i32::from(matches!(self, Self::Failed { .. }))
    }
}

/// Capture by the request. The error is a usage error: a session id that
/// is not an identity.
pub fn capture(config: &Config, request: &Request) -> Result<Outcome, String> {
    let log = config.log();
    let started_at = super::now();
    let harness = request.harness;
    if let Some(id) = &request.session
        && !valid_session_id(id)
    {
        return Err(format!("`{id}` is not a session id"));
    }
    log.note(&format!(
        "{} capture: {harness}{} in {}",
        request.trigger.as_str(),
        request
            .session
            .as_deref()
            .map_or_default(|id| format!(" session {id}")),
        request
            .cwd
            .as_deref()
            .map_or_else(|| "?".to_string(), |cwd| cwd.display().to_string())
    ));
    if config
        .exclude
        .matches(request.session.as_deref(), request.cwd.as_deref())
    {
        log.note(&format!("{harness} skipped: excluded by configuration"));
        return Ok(Outcome::Skipped);
    }
    let Some(lock) = lock::acquire(&config.state_dir, harness.as_str(), config.lock_wait) else {
        log.note(&format!("{harness} locked by another capture; deferred"));
        return Ok(Outcome::Deferred);
    };
    let files = match source::list(harness, &config.stores) {
        Ok(Some(files)) => files,
        Ok(None) => {
            log.note(&format!(
                "{harness} store missing ({})",
                config.stores.root(harness).display()
            ));
            return Ok(Outcome::NoStore);
        }
        Err(message) => {
            log.note(&format!("{harness} store unreadable: {message}"));
            return Ok(Outcome::Failed {
                message,
                debt: None,
            });
        }
    };
    let selected = source::select(files, request.session.as_deref(), request.since);
    let store = debt::Store::new(&config.state_dir);
    if request.session.is_some() && selected.is_empty() {
        log.note(&format!(
            "{harness} session {} has no session file",
            request.session.as_deref().unwrap_or_default()
        ));
        close_matching(&store, request, &started_at, Closed::NoSessionFile, &log);
        return Ok(Outcome::NoSessionFile);
    }
    let (kept, skipped): (Vec<SessionFile>, Vec<SessionFile>) =
        selected.into_iter().partition(|file| {
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
    if harness == Harness::Claude
        && let Err(message) = copy_plans(config)
    {
        log.note(&format!("plans: {message}"));
    }
    let readable_error = readable_copy(config, request, &kept).err();
    if let Some(error) = &readable_error {
        log.note(&format!("{harness} readable copy: {error}"));
    }
    if let Some(first) = copied.failures.first() {
        let debt = record_failure(&store, request, first, &log);
        return Ok(Outcome::Failed {
            message: first.clone(),
            debt,
        });
    }
    close_matching(&store, request, &started_at, Closed::Copied, &log);
    Ok(Outcome::Copied {
        files: copied.files,
        versions: copied.versions,
        skipped: skipped.len(),
        readable_error,
    })
}

/// What copying a set of files did.
#[derive(Debug, Default)]
pub(super) struct Copied {
    pub files: usize,
    pub versions: usize,
    pub failures: Vec<String>,
}

/// Copy each file to its place under the harness's native copies. A
/// replaced copy with lost bytes goes to this run's version folder.
pub(super) fn copy_all(config: &Config, harness: Harness, files: &[SessionFile]) -> Copied {
    let layout = config.layout();
    let sessions = layout.sessions(harness);
    let version_run = layout.version_run(harness);
    let mut copied = Copied::default();
    for file in files {
        let destination = sessions.join(&file.relative);
        let version = version_run.join(&file.relative);
        match copy::copy_file(&file.path, &destination, &version, kind_of(&file.path)) {
            Ok(copy::Action::Unchanged) => {}
            Ok(copy::Action::Versioned) => {
                copied.files += 1;
                copied.versions += 1;
            }
            Ok(copy::Action::Created | copy::Action::Overwritten) => copied.files += 1,
            Err(message) => copied.failures.push(message),
        }
    }
    copied
}

/// Copy the Claude Code plan-mode plans into the plans folder. A plan is
/// edited in place, so the copy is refreshed without a version.
pub(super) fn copy_plans(config: &Config) -> Result<usize, String> {
    let source = &config.stores.claude_plans;
    if !source.is_dir() {
        return Ok(0);
    }
    let destination = config.layout().plans();
    let entries = std::fs::read_dir(source)
        .map_err(|error| format!("cannot read {}: {error}", source.display()))?;
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = destination.join(name);
        if copy::relation(&path, &target).is_ok_and(|relation| relation == copy::Relation::Equal) {
            continue;
        }
        copy::copy_atomic(&path, &target)?;
        count += 1;
    }
    Ok(count)
}

/// The readable copy, a projection of the native copy: Grok through
/// rune's own renderer, Claude Code through `SpecStory` when installed,
/// nothing for the others.
fn readable_copy(config: &Config, request: &Request, files: &[SessionFile]) -> Result<(), String> {
    let layout = config.layout();
    match request.harness {
        Harness::Grok => grok::render_all(
            &layout.sessions(Harness::Grok),
            &layout.transcripts(Harness::Grok),
            request.session.as_deref(),
        )
        .map(|_| ()),
        Harness::Claude => {
            let Some(binary) = &config.specstory else {
                return Ok(());
            };
            let cwd = request
                .cwd
                .clone()
                .or_else(|| files.iter().find_map(|file| file.cwd.clone()));
            let Some(cwd) = cwd.filter(|cwd| cwd.is_dir()) else {
                return Ok(());
            };
            render::specstory(
                binary,
                request.session.as_deref(),
                &cwd,
                &layout.transcripts(Harness::Claude),
            )
        }
        Harness::Codex | Harness::Antigravity => Ok(()),
    }
}

/// Record the debt of a failed native copy.
fn record_failure(
    store: &debt::Store,
    request: &Request,
    message: &str,
    log: &super::Log,
) -> Option<PathBuf> {
    let record = Record {
        harness: request.harness.as_str().to_string(),
        session: request.session.clone(),
        cwd: request.cwd.as_ref().map(|cwd| cwd.display().to_string()),
        source: Some(request.trigger.as_str().to_string()),
        harness_status: request.status.map(serde_json::Value::from),
        reason: "raw".to_string(),
        error: Some(message.to_string()),
        at: super::now(),
        closed_at: None,
        closed_reason: None,
    };
    match store.record(&record) {
        Ok(path) => {
            log.note(&format!(
                "debt recorded: {} ({})",
                request.harness,
                path.display()
            ));
            Some(path)
        }
        Err(error) => {
            log.note(&format!("cannot record debt: {error}"));
            None
        }
    }
}

/// Close the open records this capture covers: the same harness, the same
/// session, or the same working folder when no session is known, and
/// recorded before this capture started.
fn close_matching(
    store: &debt::Store,
    request: &Request,
    started_at: &str,
    reason: Closed,
    log: &super::Log,
) {
    let Ok(open) = store.open() else {
        return;
    };
    for entry in open {
        let record = &entry.record;
        if record.harness != request.harness.as_str() || record.at.as_str() > started_at {
            continue;
        }
        let covered = match &request.session {
            Some(id) => record.session.as_deref() == Some(id.as_str()),
            None => {
                record.session.is_none()
                    && request.cwd.as_ref().is_none_or(|cwd| {
                        record.cwd.as_deref() == Some(cwd.display().to_string().as_str())
                    })
            }
        };
        if !covered {
            continue;
        }
        match store.close(&entry, reason) {
            Ok(()) => log.note(&format!(
                "debt closed as {}: {}",
                reason.as_str(),
                entry.path.display()
            )),
            Err(error) => log.note(&format!("cannot close debt: {error}")),
        }
    }
}

/// Whether any native copy of the harness names the session.
pub(super) fn copy_exists(sessions: &Path, session: &str) -> bool {
    let mut paths = Vec::new();
    if source::walk(sessions, &mut paths).is_err() {
        return false;
    }
    paths.iter().any(|path| {
        path.strip_prefix(sessions).is_ok_and(|relative| {
            relative.components().any(|component| {
                let text = component.as_os_str().to_string_lossy();
                text == session
                    || text.split('.').next().is_some_and(|stem| {
                        stem == session || stem.ends_with(&format!("-{session}"))
                    })
            })
        })
    })
}
