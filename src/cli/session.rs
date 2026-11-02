//! `rune session capture`, `rune session sweep`, and `rune session debt`:
//! the command layer over `rune::session`. The hook handler starts the
//! capture detached, the runner calls it in process after the harness
//! exits, and the launchd job runs the sweep. Nothing here writes to
//! stdout except the one result line or object.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rune::error::{Error, ErrorKind};
use rune::ontology;
use rune::session::{self, Config, Exclude, Harness, Request, Stores, Trigger};

use crate::cli::run::EXIT_RUNNER_ERROR;
use crate::cli::surface::Surface;

/// The `rune session` subcommands.
#[derive(clap::Subcommand)]
pub(crate) enum SessionAction {
    /// Copy one harness's session files into the data folder
    ///
    /// Scoped to one session when `--session` names it, else to the whole
    /// store. Exit 0 on success, a skip, or a deferral; 1 when a native
    /// copy failed and debt was recorded; 125 on a usage error.
    Capture {
        /// The harness: claude, codex, grok, or antigravity (agy).
        #[arg(long, value_name = "NAME")]
        harness: String,
        /// The session id, as the harness names it.
        #[arg(long, value_name = "ID")]
        session: Option<String>,
        /// The harness's exit status, recorded with any debt.
        #[arg(long, value_name = "N")]
        status: Option<i32>,
        /// Who asks: hook or runner. Recorded with any debt.
        #[arg(long, value_name = "SOURCE", default_value = "runner")]
        source: String,
    },
    /// Copy every supported source, the plans, and the daily database
    /// backup, and reconcile the capture debt. Always exits 0.
    Sweep,
    /// List the capture debt: open records first, then closed ones
    Debt,
}

/// Run one subcommand. The value is the process's exit status.
pub(crate) fn run(action: SessionAction, json: bool) -> i32 {
    match action {
        SessionAction::Capture {
            harness,
            session,
            status,
            source,
        } => capture_command(&harness, session, status, &source, json),
        SessionAction::Sweep => sweep_command(json),
        SessionAction::Debt => debt_command(json),
    }
}

fn capture_command(
    harness: &str,
    session: Option<String>,
    status: Option<i32>,
    source: &str,
    json: bool,
) -> i32 {
    let Some(harness) = Harness::parse(harness) else {
        return usage_error(&format!(
            "`{harness}` is not a captured harness (claude, codex, grok, antigravity)"
        ));
    };
    let Some(trigger) = Trigger::parse(source) else {
        return usage_error(&format!(
            "`{source}` is not a capture source (hook, runner)"
        ));
    };
    let config = match config() {
        Ok(config) => config,
        Err(error) => return fatal(&error, json),
    };
    let request = Request {
        harness,
        session,
        cwd: std::env::current_dir().ok(),
        status,
        trigger,
        since: None,
    };
    match session::capture(&config, &request) {
        Ok(outcome) => {
            if json {
                println!("{}", serde_json::to_string(&outcome).unwrap_or_default());
            } else {
                println!("{harness}: {}", describe(&outcome));
            }
            outcome.exit_code()
        }
        Err(message) => usage_error(&message),
    }
}

fn describe(outcome: &session::Outcome) -> String {
    match outcome {
        session::Outcome::Copied {
            files,
            versions,
            skipped,
            readable_error,
        } => format!(
            "{files} file(s) copied, {versions} version(s) kept, {skipped} skipped{}",
            readable_error
                .as_deref()
                .map(|error| format!("; readable copy failed: {error}"))
                .unwrap_or_default()
        ),
        session::Outcome::Skipped => "skipped, excluded by configuration".to_string(),
        session::Outcome::Deferred => "deferred, another capture holds the lock".to_string(),
        session::Outcome::NoSessionFile => "no session file".to_string(),
        session::Outcome::NoStore => "no store on this host".to_string(),
        session::Outcome::Failed { message, .. } => format!("failed: {message}"),
    }
}

fn sweep_command(json: bool) -> i32 {
    let config = match config() {
        Ok(config) => config,
        Err(error) => return fatal(&error, json),
    };
    let report = session::sweep(&config);
    if json {
        println!("{}", serde_json::to_string(&report).unwrap_or_default());
    } else {
        for (harness, outcome) in &report.harnesses {
            println!("{harness}: {}", describe_sweep(outcome));
        }
        println!(
            "plans {}, backups {}, transcripts {}, debt closed {}, debt open {}",
            report.plans, report.backups, report.transcripts, report.debt_closed, report.debt_open
        );
    }
    0
}

fn describe_sweep(outcome: &session::SweepOutcome) -> String {
    match outcome {
        session::SweepOutcome::Copied {
            files,
            versions,
            skipped,
        } => format!("{files} file(s) copied, {versions} version(s) kept, {skipped} skipped"),
        session::SweepOutcome::Deferred => "deferred, another capture holds the lock".to_string(),
        session::SweepOutcome::NoStore => "no store on this host".to_string(),
        session::SweepOutcome::Failed { message } => format!("failed: {message}"),
    }
}

fn debt_command(json: bool) -> i32 {
    let state_dir = match crate::cli::state::dir() {
        Ok(dir) => dir,
        Err(error) => return fatal(&error, json),
    };
    let store = session::debt::Store::new(&state_dir);
    let (open, closed) = match (store.open(), store.closed()) {
        (Ok(open), Ok(closed)) => (open, closed),
        (Err(message), _) | (_, Err(message)) => {
            return fatal(&Error::new(ErrorKind::Io, message), json);
        }
    };
    if json {
        let open: Vec<_> = open.iter().map(|entry| &entry.record).collect();
        println!(
            "{}",
            serde_json::json!({ "open": open, "closed": closed, "dir": store.dir() })
        );
        return 0;
    }
    if open.is_empty() {
        println!("no open capture debt");
    }
    for entry in &open {
        let record = &entry.record;
        println!(
            "open    {} {} {} {}{}",
            record.at,
            record.harness,
            record.session.as_deref().unwrap_or("-"),
            record.reason,
            record
                .cwd
                .as_deref()
                .map(|cwd| format!(" in {cwd}"))
                .unwrap_or_default()
        );
    }
    for record in &closed {
        println!(
            "closed  {} {} {} {} -> {} at {}",
            record.at,
            record.harness,
            record.session.as_deref().unwrap_or("-"),
            record.reason,
            record.closed_reason.as_deref().unwrap_or("?"),
            record.closed_at.as_deref().unwrap_or("?")
        );
    }
    0
}

fn usage_error(message: &str) -> i32 {
    eprintln!("rune session: {message}");
    EXIT_RUNNER_ERROR
}

fn fatal(error: &Error, json: bool) -> i32 {
    if json {
        println!(
            "{}",
            serde_json::json!({ "code": error.code(), "message": error.to_string() })
        );
    } else {
        crate::cli::print_fatal(error);
    }
    EXIT_RUNNER_ERROR
}

/// The capture configuration of this host: the data folder from
/// `DATA_DIR`, else the user configuration, else `~/Data`; the state
/// directory; the stores by the harness variables; the excluded sessions;
/// and the tools the readable copy and the backup need.
pub(crate) fn config() -> Result<Config, Error> {
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))?;
    let user = ontology::load()?;
    let data_dir = std::env::var_os("DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| user.capture.data_dir.as_deref().map(ontology::expand_tilde))
        .unwrap_or_else(|| home.join("Data"));
    let specstory = std::env::var_os("SPECSTORY_BIN")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            session::render::find_in_path("specstory", std::env::var_os("PATH").as_deref())
        });
    Ok(Config {
        data_dir,
        state_dir: crate::cli::state::dir()?,
        stores: Stores::resolve(&home, &|name| std::env::var_os(name)),
        exclude: Exclude::resolve(
            &user.capture.exclude.paths,
            &user.capture.exclude.sessions,
            &home,
        ),
        lock_wait: lock_wait(),
        specstory,
        sqlite3: std::env::var_os("SQLITE3_BIN")
            .filter(|value| !value.is_empty())
            .map_or_else(|| PathBuf::from("sqlite3"), PathBuf::from),
    })
}

/// `SESSION_SYNC_LOCK_WAIT` in seconds, the script's knob, else 20 s.
fn lock_wait() -> Duration {
    std::env::var("SESSION_SYNC_LOCK_WAIT")
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map_or(session::DEFAULT_LOCK_WAIT, Duration::from_secs)
}

/// The capture harness of a run surface. `None` for a surface Rune does
/// not capture.
pub(crate) fn harness_of(surface: Surface) -> Option<Harness> {
    match surface {
        Surface::Claude => Some(Harness::Claude),
        Surface::Codex => Some(Harness::Codex),
        Surface::Agy => Some(Harness::Antigravity),
        Surface::Grok => Some(Harness::Grok),
        Surface::Opencode | Surface::Cursor => None,
    }
}

/// The runner's capture after a harness exited: the files of its store
/// that changed since `started_at`, in process. A failure is one warning
/// on standard error and a debt record. The caller's exit status never
/// changes.
pub(crate) fn capture_after_run(
    harness: Option<Harness>,
    cwd: &Path,
    started_at: SystemTime,
    status: i32,
) {
    let Some(harness) = harness else {
        return;
    };
    let config = match config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("warning: session capture skipped: {error}");
            return;
        }
    };
    let request = Request {
        harness,
        session: None,
        cwd: Some(cwd.to_path_buf()),
        status: Some(status),
        trigger: Trigger::Runner,
        since: Some(started_at),
    };
    match session::capture(&config, &request) {
        Ok(session::Outcome::Failed { message, .. }) => {
            eprintln!("warning: session capture of {harness} failed: {message}");
        }
        Ok(_) => {}
        Err(message) => eprintln!("warning: session capture of {harness} skipped: {message}"),
    }
}

/// The arguments the hook handler starts this binary with: a detached
/// `rune session capture` for the payload's harness and session.
pub(crate) fn hook_arguments(harness: &str, session: Option<&str>) -> Option<Vec<OsString>> {
    Harness::parse(harness)?;
    let mut args: Vec<OsString> = vec![
        "session".into(),
        "capture".into(),
        "--harness".into(),
        harness.into(),
        "--source".into(),
        "hook".into(),
    ];
    if let Some(session) = session.filter(|id| !id.is_empty()) {
        args.push("--session".into());
        args.push(session.into());
    }
    Some(args)
}
