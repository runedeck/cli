//! `rune hook run --harness H --native-event N`: the dispatcher. Reads the
//! compiled plan and the harness payload, runs the subscribed handlers in
//! order under the event's mode and budget, and answers in the harness's
//! own shape. A failure of the dispatcher itself follows the strongest
//! policy among the handlers that wanted the event.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rune::hooks::{Canonical, Harness, Mode, OnFailure, Plan, PlannedEvent, PlannedHandler};
use serde_json::Value;

use super::cover::{self, Cover};
use super::encode::{Answer, Effect, HandlerResult, encode};
use super::payload::{HandlerPayload, decode, event_matches, payload_event};
use super::plan;
use super::record::{self, Record};
use crate::cli::process::{ProcessFailure, ProcessRequest, run_process_request};

/// The entry point of the CLI: read stdin, dispatch, print, exit. `plan`
/// is the path the registration names, else the state directory's.
/// `on_failure` is the strongest policy install wrote into the
/// registration: it decides the answer when the plan cannot load.
pub fn run(harness: &str, native_event: &str, plan: Option<&Path>, on_failure: &str) -> i32 {
    // The budget clock starts before anything is read: the payload, the
    // cover, the plan. The harness's own deadline runs from the start.
    let started = Instant::now();
    let Some(harness) = Harness::parse(harness) else {
        eprintln!("rune hook: `{harness}` is not a harness");
        return 2;
    };
    let mut text = String::new();
    let raw: Result<Value, String> = match std::io::stdin().read_to_string(&mut text) {
        Ok(_) if text.trim().is_empty() => Ok(Value::Object(serde_json::Map::new())),
        Ok(_) => {
            serde_json::from_str(&text).map_err(|error| format!("payload is not JSON: {error}"))
        }
        Err(error) => Err(format!("cannot read the payload: {error}")),
    };
    // Grok reads Claude's settings, so a Claude registration also fires
    // under Grok. Grok's own payload keys tell them apart; an inherited
    // environment variable does not.
    if harness == Harness::Claude && raw.as_ref().is_ok_and(fired_by_grok) {
        eprintln!("rune hook: claude registration fired by grok; nothing run");
        return 0;
    }
    let policy = if on_failure == "deny" {
        OnFailure::Deny
    } else {
        OnFailure::Warn
    };
    let path = plan.map_or_else(plan::path, |path| Ok(path.to_path_buf()));
    // The cover install wrote beside the plan: the handlers a part of a
    // kept legacy dispatcher entry runs for, checked again at this firing.
    let cover = path
        .as_ref()
        .ok()
        .and_then(|path| cover::path_beside(path, harness, native_event))
        .map(|path| cover::read(&path, harness, native_event))
        .unwrap_or_default();
    let env = cover::Environment::from_process();
    let loaded = path.and_then(|path| plan::read_at(&path));
    let answer = match loaded {
        // A plan that loads but names no handler for the registered event
        // is as good as none: a guarded registration still denies.
        Ok((plan, _))
            if plan.event(harness, native_event).is_none() && policy == OnFailure::Deny =>
        {
            no_plan(
                harness,
                native_event,
                policy,
                &format!("the plan names no handler for {native_event} on {harness}"),
                log_dir().as_deref(),
            )
        }
        Ok((plan, warning)) => {
            if let Some(warning) = warning {
                eprintln!("rune hook: {warning}");
            }
            dispatch(
                &plan,
                harness,
                native_event,
                raw,
                &Runner::real(cover, env, started),
            )
        }
        Err(error) => no_plan(
            harness,
            native_event,
            policy,
            &error.to_string(),
            log_dir().as_deref(),
        ),
    };
    if let Some(stdout) = &answer.stdout {
        println!("{stdout}");
    }
    for line in &answer.stderr {
        eprintln!("{line}");
    }
    answer.exit
}

/// Keys only Grok's payload carries: its camelCase spelling of the event,
/// the session, and the tool. A Claude payload is `snake_case` throughout.
pub(crate) fn fired_by_grok(raw: &Value) -> bool {
    ["hookEventName", "sessionId", "toolName", "toolInput"]
        .iter()
        .any(|key| raw.get(*key).is_some())
}

/// The answer when the plan does not load: the policy the registration
/// carries, which install took from the plan's handlers for this event.
/// A `deny` registration denies where the harness can block the event.
pub(crate) fn no_plan(
    harness: Harness,
    native_event: &str,
    policy: OnFailure,
    why: &str,
    log_dir: Option<&Path>,
) -> Answer {
    let canonical = Canonical::from_native(harness, native_event);
    let event = PlannedEvent {
        canonical,
        mode: canonical.map_or(Mode::Passive, Canonical::mode),
        budget_ms: 0,
        response_reserve_ms: 0,
        handlers: vec![PlannedHandler {
            id: "registration".to_string(),
            exec: Vec::new(),
            order: 0,
            timeout_ms: 0,
            on_failure: policy,
            rewrites: false,
        }],
    };
    dispatcher_failure(harness, native_event, &event, why, log_dir)
}

/// Where the dispatcher log lives: `hooks/` under the state directory.
fn log_dir() -> Option<PathBuf> {
    crate::cli::state::dir().ok().map(|dir| dir.join("hooks"))
}

/// The time a handler that outlived its timeout gets between SIGTERM and
/// SIGKILL. A guard that ignores SIGTERM must not hold the answer past the
/// harness deadline, so the grace is near zero.
pub(crate) const HANDLER_GRACE: Duration = Duration::from_millis(10);

/// What stopping a timed-out handler costs on top of its timeout: the poll
/// that notices the deadline, the grace, the kill, and the drain of its
/// pipes, each rounded up to the supervisor's 25 ms poll, with room to
/// spare. The event budget pays it, so the answer leaves before the
/// harness deadline even when the last handler runs to its timeout.
pub(crate) const STOP_ALLOWANCE: Duration = Duration::from_millis(150);

/// What spawning a handler yields: its exit code, stdout, stderr.
pub(crate) type Spawned = Result<(Option<i32>, String, String), String>;

/// How a handler is spawned; a test replaces it with a closure.
pub(crate) type SpawnFn =
    dyn Fn(&PlannedHandler, &[u8], Duration, &[(OsString, OsString)], Option<&PathBuf>) -> Spawned;

pub(crate) struct Runner {
    pub(crate) spawn: Box<SpawnFn>,
    /// Where the hook record is written; `None` writes no record.
    pub(crate) record_dir: Option<PathBuf>,
    /// Why no record directory could be resolved, reported once per event
    /// that had records to write.
    pub(crate) record_error: Option<String>,
    /// Where the dispatcher log is written; `None` writes none.
    pub(crate) log_dir: Option<PathBuf>,
    /// The handlers a part of a kept legacy dispatcher entry covers on
    /// this event, each skipped while its coverage still holds.
    pub(crate) cover: Cover,
    /// The environment the coverage is checked under: the home and the
    /// harness folders as the dispatcher and the harness read them.
    pub(crate) env: cover::Environment,
    /// When the budget clock started: before the first read.
    pub(crate) started: Instant,
}

impl Runner {
    pub(crate) fn real(cover: Cover, env: cover::Environment, started: Instant) -> Self {
        Self {
            cover,
            env,
            started,
            log_dir: log_dir(),
            record_dir: record::dir().ok(),
            record_error: record::dir().err().map(|e| e.to_string()),
            spawn: Box::new(|handler, stdin, timeout, env, cwd| {
                // `rune-hook-<name>` is a built-in adapter: this binary,
                // `hook adapter <name>`, with the handler's own arguments.
                let (binary, args): (OsString, Vec<OsString>) =
                    match handler.exec[0].strip_prefix("rune-hook-") {
                        Some(name) => {
                            let exe = std::env::current_exe()
                                .map_or_else(|_| OsString::from("rune"), OsString::from);
                            let mut args: Vec<OsString> =
                                vec!["hook".into(), "adapter".into(), name.into()];
                            args.extend(handler.exec[1..].iter().map(OsString::from));
                            (exe, args)
                        }
                        None => (
                            OsString::from(&handler.exec[0]),
                            handler.exec[1..].iter().map(OsString::from).collect(),
                        ),
                    };
                let mut request = ProcessRequest::new(binary);
                request.args = args;
                request.stdin = Some(stdin.to_vec());
                request.timeout = Some(timeout);
                request.termination_grace = HANDLER_GRACE;
                request.env = env.to_vec();
                request.current_dir = cwd.cloned();
                match run_process_request(&request) {
                    Ok(output) => Ok((output.code(), output.stdout, output.stderr)),
                    Err(ProcessFailure::Timeout { timeout, .. }) => {
                        Err(format!("timed out after {} ms", timeout.as_millis()))
                    }
                    Err(ProcessFailure::Spawn(why)) => Err(format!("cannot start: {why}")),
                    Err(other) => Err(format!("{other:?}")),
                }
            }),
        }
    }
}

/// Why a handler produced no result.
struct Failure {
    why: String,
    policy: OnFailure,
    /// The record's failure word: timeout, exit, result, or budget.
    kind: &'static str,
    exit_status: Option<i32>,
}

/// The record of a handler the cover skips at this firing, because the
/// legacy dispatcher part install recorded still runs its whole command.
/// `None` runs the handler. Either way the log says why. `cache` holds
/// what the checks read, once per firing. A cover that holds skips the
/// handler whether or not the budget is spent by then: the part runs its
/// command either way, and the handler would only run it again.
#[allow(clippy::too_many_arguments)]
fn skipped(
    event: &PlannedEvent,
    handler: &PlannedHandler,
    harness: Harness,
    native_event: &str,
    payload: &HandlerPayload,
    runner: &Runner,
    cache: &mut cover::Cache,
    log: &mut String,
) -> Option<Record> {
    let coverage = runner.cover.handlers.get(&handler.id)?;
    let canonical = event
        .canonical
        .map_or_else(|| native_event.to_string(), |c| c.as_str().to_string());
    let held = cover::holds(
        coverage,
        handler,
        harness,
        &canonical,
        native_event,
        &runner.env,
        cache,
    );
    match held {
        Ok(()) => {
            let _ = writeln!(
                log,
                "{} skipped: the part {} of `{}` runs its command",
                handler.id,
                coverage.part.display(),
                coverage.command
            );
            let mut line = record_line(harness, native_event, payload, &handler.id, Instant::now());
            line.duration_ms = Some(0);
            line.skipped = Some(coverage.part.display().to_string());
            Some(line)
        }
        Err(why) => {
            let _ = writeln!(log, "{} not skipped: {why}", handler.id);
            None
        }
    }
}

/// Dispatch one event. Pure over the plan, the payload, and the runner.
pub(crate) fn dispatch(
    plan: &Plan,
    harness: Harness,
    native_event: &str,
    raw: Result<Value, String>,
    runner: &Runner,
) -> Answer {
    let Some(event) = plan.event(harness, native_event) else {
        return unregistered(harness, native_event);
    };
    let raw = match admit(harness, native_event, event, raw, runner.log_dir.as_deref()) {
        Ok(raw) => raw,
        Err(answer) => return answer,
    };
    let can_block = event.canonical.is_some_and(|c| c.can_block(harness));
    let mut payload = decode(harness, native_event, raw);
    let reserve = Duration::from_millis(event.response_reserve_ms);
    let mut merged = HandlerResult::default();
    let mut stderr = Vec::new();
    let mut log = log_preamble(&payload);
    let mut records: Vec<Record> = Vec::new();
    // One firing reads each table, script, and folder once.
    let mut cache = cover::Cache::default();
    for handler in &event.handlers {
        if let Some(line) = skipped(
            event,
            handler,
            harness,
            native_event,
            &payload,
            runner,
            &mut cache,
            &mut log,
        ) {
            records.push(line);
            continue;
        }
        let remaining = remaining(event, runner.started);
        let handler_started = Instant::now();
        let outcome = if remaining.is_zero() {
            Err(budget_spent(
                native_event,
                event.budget_ms,
                handler.on_failure,
            ))
        } else {
            let timeout = Duration::from_millis(handler.timeout_ms).min(remaining);
            run_handler(handler, &payload, timeout, runner)
                .and_then(|result| admit_result(result, handler, event))
        };
        let mut line = record_line(
            harness,
            native_event,
            &payload,
            &handler.id,
            handler_started,
        );
        match outcome {
            Ok(mut result) => {
                line.exit_status = Some(0);
                line.asked = result.effect.word().to_string();
                line.metrics = take_metrics(&mut result, &handler.id, &mut stderr);
                log_result(&mut log, &handler.id, &result);
                if event.mode == Mode::Passive {
                    passive_notes(native_event, harness, &handler.id, &result, &mut stderr);
                }
                merge(&mut payload, &mut merged, result, event.mode);
            }
            Err(failure) => {
                let _ = writeln!(log, "{} failed {}", handler.id, failure.kind);
                line.failure = Some(failure.kind.to_string());
                line.exit_status = failure.exit_status;
                if failure.kind == "budget" {
                    line.duration_ms = Some(0);
                }
                apply_failure(
                    &failure,
                    &handler.id,
                    can_block && event.mode != Mode::Passive,
                    harness,
                    native_event,
                    &mut merged,
                    &mut stderr,
                );
            }
        }
        records.push(line);
        if event.mode == Mode::Gate && merged.effect == Effect::Deny {
            break;
        }
    }
    // The reserve is the dispatcher's own allowance for the answer and the
    // record: the hooks never get it, and a hook that overran its timeout
    // does not take it away. The writer gets what the answer left of it.
    let own_phase = Instant::now();
    let mut answer = encode(harness, native_event, &merged);
    stderr.append(&mut answer.stderr);
    stderr.extend(write_records(
        runner,
        records,
        merged.effect,
        reserve.saturating_sub(own_phase.elapsed()),
    ));
    answer.stderr = stderr;
    append_log(runner.log_dir.as_deref(), harness, native_event, &log);
    answer
}

/// What the budget leaves a handler started now: the budget less the time
/// since the clock started, the reserve, and the stop allowance.
fn remaining(event: &PlannedEvent, started: Instant) -> Duration {
    Duration::from_millis(event.budget_ms)
        .saturating_sub(started.elapsed())
        .saturating_sub(Duration::from_millis(event.response_reserve_ms))
        .saturating_sub(STOP_ALLOWANCE)
}

/// Fold one handler's result into the effective input and the answer.
fn merge(
    payload: &mut HandlerPayload,
    merged: &mut HandlerResult,
    result: HandlerResult,
    mode: Mode,
) {
    // A passive event steers nothing, a replacement input included.
    if mode == Mode::Passive {
        return;
    }
    if let Some(input) = result.input {
        if let Some(tool) = payload.tool.as_mut() {
            tool.input = input.clone();
        }
        merged.input = Some(input);
    }
    // A denial outranks every other effect, whichever handler ran first.
    let replaces = match result.effect {
        Effect::Deny => merged.effect != Effect::Deny,
        Effect::ContinueTurn => merged.effect == Effect::Pass,
        Effect::Pass => false,
    };
    if replaces {
        merged.effect = result.effect;
        merged.reason = result.reason;
    }
    merged.context.extend(result.context);
}

/// A result whose effect the event does not take fails like any other
/// bad result: `continue_turn` means something on a turn end only.
fn admit_result(
    result: HandlerResult,
    handler: &PlannedHandler,
    event: &PlannedEvent,
) -> Result<HandlerResult, Failure> {
    let refuse = |why: String| Failure {
        why,
        policy: handler.on_failure,
        kind: "result",
        exit_status: Some(0),
    };
    if result.effect == Effect::ContinueTurn && event.canonical != Some(Canonical::TurnFinish) {
        return Err(refuse(format!(
            "returned continue_turn, which only turn.finish takes, on {}",
            event
                .canonical
                .map_or("a native extension", Canonical::as_str)
        )));
    }
    // The plan orders only declared rewriters before the guards, so a
    // replacement from any other handler could reach the harness unjudged.
    if result.input.is_some() && !handler.rewrites {
        return Err(refuse(
            "returned a replacement input but does not declare `rewrites: true`".to_string(),
        ));
    }
    Ok(result)
}

/// Spawn one handler with the payload on stdin and parse its one result.
fn run_handler(
    handler: &PlannedHandler,
    payload: &HandlerPayload,
    timeout: Duration,
    runner: &Runner,
) -> Result<HandlerResult, Failure> {
    let fail = |why: String, kind: &'static str, exit_status: Option<i32>| Failure {
        why,
        policy: handler.on_failure,
        kind,
        exit_status,
    };
    if handler.exec.is_empty() {
        return Err(fail("names no executable".to_string(), "exit", None));
    }
    let stdin = serde_json::to_vec(payload)
        .map_err(|e| fail(format!("cannot serialize the payload: {e}"), "result", None))?;
    let env = vec![
        (
            OsString::from("RUNE_HARNESS"),
            OsString::from(payload.harness.as_str()),
        ),
        (OsString::from("RUNE_EVENT"), OsString::from(&payload.event)),
        (
            OsString::from("RUNE_NATIVE_EVENT"),
            OsString::from(&payload.native_event),
        ),
        (
            OsString::from("RUNE_HANDLER_ID"),
            OsString::from(&handler.id),
        ),
        (
            OsString::from("RUNE_REMAINING_MS"),
            OsString::from(timeout.as_millis().to_string()),
        ),
    ];
    let cwd = payload
        .cwd
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| p.is_dir());
    let (code, stdout, stderr) = (runner.spawn)(handler, &stdin, timeout, &env, cwd.as_ref())
        .map_err(|why| {
            let kind = if why.starts_with("timed out") {
                "timeout"
            } else {
                "exit"
            };
            fail(why, kind, None)
        })?;
    if code != Some(0) {
        let tail = stderr.lines().last().unwrap_or_default();
        return Err(fail(
            format!(
                "exited {}{}",
                code.map_or_else(|| "by signal".to_string(), |c| c.to_string()),
                if tail.is_empty() {
                    String::new()
                } else {
                    format!(": {tail}")
                }
            ),
            "exit",
            code,
        ));
    }
    let text = stdout.trim();
    if text.is_empty() {
        return Err(fail(
            "printed nothing; a handler prints one result".to_string(),
            "result",
            Some(0),
        ));
    }
    let result = serde_json::from_str::<HandlerResult>(text).map_err(|e| {
        fail(
            format!("printed something that is not one result: {e}"),
            "result",
            Some(0),
        )
    })?;
    if result.v != 1 {
        return Err(fail(
            format!(
                "printed a result of version {}; this rune reads 1",
                result.v
            ),
            "result",
            Some(0),
        ));
    }
    Ok(result)
}

/// The first log line of a tool event: the tool's name. The command or
/// file it acts on stays out, because a command line can carry a token.
fn log_preamble(payload: &HandlerPayload) -> String {
    let mut log = String::new();
    if let Some(tool) = &payload.tool {
        let _ = writeln!(log, "tool {}", tool.name);
    }
    log
}

fn dispatcher_failure(
    harness: Harness,
    native_event: &str,
    event: &PlannedEvent,
    why: &str,
    log_dir: Option<&Path>,
) -> Answer {
    let can_block = event.canonical.is_some_and(|c| c.can_block(harness));
    let guarded = event
        .handlers
        .iter()
        .any(|h| h.on_failure == OnFailure::Deny);
    let denies = guarded && can_block && event.mode == Mode::Gate;
    let mut answer = if denies {
        encode(
            harness,
            native_event,
            &HandlerResult::deny(format!("rune hook could not run its handlers: {why}")),
        )
    } else {
        Answer::default()
    };
    answer.stderr.insert(
        0,
        format!(
            "rune hook: {why}; {}",
            if denies { "denied" } else { "proceeding" }
        ),
    );
    append_log(
        log_dir,
        harness,
        native_event,
        if denies {
            "dispatcher failure denied\n"
        } else {
            "dispatcher failure proceeding\n"
        },
    );
    answer
}

/// The size at which `run.log` moves to `run.log.1`, so the log holds two
/// generations at most.
pub(crate) const LOG_LIMIT: u64 = 256 * 1024;

/// The dispatcher log, `hooks/run.log` under the state directory: per
/// event its time, harness, native event, tool name, and each hook's id
/// with its effect or failure kind. It holds no command, reason, or other
/// text a hook or the harness wrote, it is readable by the owner alone,
/// and it rotates at `LOG_LIMIT`.
fn append_log(dir: Option<&Path>, harness: Harness, native_event: &str, text: &str) {
    use std::io::Write as _;
    let Some(dir) = dir else {
        return;
    };
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("run.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= LOG_LIMIT) {
        let _ = std::fs::rename(&path, dir.join("run.log.1"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(&path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        let _ = write!(
            file,
            "{} {harness} {native_event}\n{text}",
            chrono::Utc::now().to_rfc3339()
        );
    }
}

/// The record line for one hook, before its outcome is known.
fn record_line(
    harness: Harness,
    native_event: &str,
    payload: &HandlerPayload,
    hook: &str,
    started: Instant,
) -> Record {
    Record {
        v: 1,
        ts: Record::stamp(&chrono::Utc::now()),
        harness: harness.as_str().to_string(),
        event: payload.event.clone(),
        native_event: native_event.to_string(),
        hook: hook.to_string(),
        session_id: payload.session_id.clone(),
        tool_use_id: payload.tool.as_ref().and_then(|t| t.id.clone()),
        cwd: payload.cwd.clone(),
        file: payload
            .tool
            .as_ref()
            .and_then(|t| super::adapters::written_paths(t).into_iter().next())
            .map(|p| p.display().to_string()),
        duration_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        exit_status: None,
        asked: Effect::Pass.word().to_string(),
        answer: String::new(),
        failure: None,
        metrics: None,
        source: None,
        skipped: None,
    }
}

/// Write the event's records after the answer is encoded, bounded by what
/// the answer left of the reserve the dispatcher keeps for itself. Never
/// changes the answer.
fn write_records(
    runner: &Runner,
    mut records: Vec<Record>,
    answer: Effect,
    reserve: Duration,
) -> Vec<String> {
    let Some(dir) = &runner.record_dir else {
        return match (&runner.record_error, records.len()) {
            (Some(why), count) if count > 0 => vec![format!(
                "rune hook: no record directory: {why}; {count} hook record(s) lost"
            )],
            _ => Vec::new(),
        };
    };
    let word = answer.word();
    for line in &mut records {
        line.answer = word.to_string();
    }
    record::write_all(dir, records, reserve)
}

/// A hook's metrics, kept when they pass validation and dropped with one
/// stderr line when they do not.
fn take_metrics(result: &mut HandlerResult, hook: &str, stderr: &mut Vec<String>) -> Option<Value> {
    let metrics = result.metrics.take()?;
    match record::validate_metrics(&metrics) {
        Ok(()) => Some(metrics),
        Err(why) => {
            stderr.push(format!(
                "rune hook: {hook} returned metrics the record refuses: {why}; dropped"
            ));
            None
        }
    }
}

/// A failed hook under its policy: warn and proceed, or deny when the
/// harness can block the event.
fn apply_failure(
    failure: &Failure,
    hook: &str,
    can_deny: bool,
    harness: Harness,
    native_event: &str,
    merged: &mut HandlerResult,
    stderr: &mut Vec<String>,
) {
    match failure.policy {
        OnFailure::Warn => stderr.push(format!(
            "rune hook: {hook} failed: {} (warn, proceeding)",
            failure.why
        )),
        OnFailure::Deny if can_deny => {
            if merged.effect != Effect::Deny {
                merged.effect = Effect::Deny;
                merged.reason = Some(format!("{hook} failed: {}", failure.why));
            }
            stderr.push(format!("rune hook: {hook} failed: {} (deny)", failure.why));
        }
        OnFailure::Deny => stderr.push(format!(
            "rune hook: {hook} failed: {} (deny, but {harness} cannot block {native_event})",
            failure.why
        )),
    }
}

/// The log line of one result: the hook and the effect it asked for. The
/// reason stays out, because a guard often quotes the command it refused.
fn log_result(log: &mut String, hook: &str, result: &HandlerResult) {
    let _ = writeln!(log, "{hook} {}", result.effect.word());
}

/// The failure of a hook the budget left no time for.
fn budget_spent(native_event: &str, budget_ms: u64, policy: OnFailure) -> Failure {
    Failure {
        why: format!("the {budget_ms} ms budget of {native_event} was spent before it started"),
        policy,
        kind: "budget",
        exit_status: None,
    }
}

/// A steering result on a passive event is logged and never applied.
/// The notes a passive event's result earns: one for an effect other than
/// pass, one for a replacement input. Neither is applied.
fn passive_notes(
    native_event: &str,
    harness: Harness,
    hook: &str,
    result: &HandlerResult,
    stderr: &mut Vec<String>,
) {
    if result.effect != Effect::Pass {
        stderr.push(passive_note(
            native_event,
            harness,
            hook,
            &format!("{:?}", result.effect),
        ));
    }
    if result.input.is_some() {
        stderr.push(passive_note(
            native_event,
            harness,
            hook,
            "replacement input",
        ));
    }
}

fn passive_note(native_event: &str, harness: Harness, hook: &str, what: &str) -> String {
    format!(
        "rune hook: {native_event} is passive on {harness}; {hook}'s {what} is logged, not applied"
    )
}

/// A registration without handlers: the config changed since the table
/// was written. Nothing to run, nothing to say on stdout.
fn unregistered(harness: Harness, native_event: &str) -> Answer {
    Answer {
        stderr: vec![format!(
            "rune hook: no handler subscribes to {native_event} on {harness}; run `rune install`"
        )],
        ..Answer::default()
    }
}

/// The payload, once it decoded and names the event it was registered for.
fn admit(
    harness: Harness,
    native_event: &str,
    event: &PlannedEvent,
    raw: Result<Value, String>,
    log_dir: Option<&Path>,
) -> Result<Value, Answer> {
    let raw = raw.map_err(|why| dispatcher_failure(harness, native_event, event, &why, log_dir))?;
    if let Some(carried) = payload_event(harness, &raw)
        && !event_matches(harness, &carried, native_event)
    {
        return Err(dispatcher_failure(
            harness,
            native_event,
            event,
            &format!("the payload names `{carried}` but the registration is for `{native_event}`"),
            log_dir,
        ));
    }
    Ok(raw)
}
