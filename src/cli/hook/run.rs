//! `rune hook run --harness H --native-event N`: the dispatcher. Reads the
//! compiled plan and the harness payload, runs the subscribed handlers in
//! order under the event's mode and budget, and answers in the harness's
//! own shape. A failure of the dispatcher itself follows the strongest
//! policy among the handlers that wanted the event.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rune::hooks::{Harness, Mode, OnFailure, Plan, PlannedEvent, PlannedHandler};
use serde_json::Value;

use super::encode::{Answer, Effect, HandlerResult, encode};
use super::payload::{HandlerPayload, decode, event_matches, payload_event};
use super::plan;
use crate::cli::process::{ProcessFailure, ProcessRequest, run_process_request};

/// The entry point of the CLI: read stdin, dispatch, print, exit.
pub fn run(harness: &str, native_event: &str) -> i32 {
    let Some(harness) = Harness::parse(harness) else {
        eprintln!("rune hook: `{harness}` is not a harness");
        return 2;
    };
    // Grok reads Claude's settings, so a Claude registration also fires
    // under Grok; Grok's own environment tells them apart.
    if harness == Harness::Claude && std::env::var_os("GROK_SESSION_ID").is_some() {
        eprintln!("rune hook: claude registration fired by grok; nothing run");
        return 0;
    }
    let mut text = String::new();
    let raw: Result<Value, String> = match std::io::stdin().read_to_string(&mut text) {
        Ok(_) if text.trim().is_empty() => Ok(Value::Object(serde_json::Map::new())),
        Ok(_) => {
            serde_json::from_str(&text).map_err(|error| format!("payload is not JSON: {error}"))
        }
        Err(error) => Err(format!("cannot read the payload: {error}")),
    };
    let (plan, warning) = match plan::read() {
        Ok(read) => read,
        Err(error) => {
            // Without a plan no handler is known: the harness's default
            // stands, and the reason goes where a person reads it.
            eprintln!("rune hook: {error}");
            return 0;
        }
    };
    if let Some(warning) = warning {
        eprintln!("rune hook: {warning}");
    }
    let answer = dispatch(&plan, harness, native_event, raw, &Runner::real());
    if let Some(stdout) = &answer.stdout {
        println!("{stdout}");
    }
    for line in &answer.stderr {
        eprintln!("{line}");
    }
    answer.exit
}

/// What spawning a handler yields: its exit code, stdout, stderr.
pub(crate) type Spawned = Result<(Option<i32>, String, String), String>;

/// How a handler is spawned; a test replaces it with a closure.
pub(crate) type SpawnFn =
    dyn Fn(&PlannedHandler, &[u8], Duration, &[(OsString, OsString)], Option<&PathBuf>) -> Spawned;

pub(crate) struct Runner {
    pub(crate) spawn: Box<SpawnFn>,
}

impl Runner {
    pub(crate) fn real() -> Self {
        Self {
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
                request.env = env.to_vec();
                request.current_dir = cwd.cloned();
                match run_process_request(&request) {
                    Ok(output) => Ok((output.code(), output.stdout, output.stderr)),
                    Err(ProcessFailure::Timeout(after)) => {
                        Err(format!("timed out after {} ms", after.as_millis()))
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
        // A registration without handlers: the config changed since the
        // table was written. Nothing to run, nothing to say on stdout.
        return Answer {
            stderr: vec![format!(
                "rune hook: no handler subscribes to {native_event} on {harness}; run `rune install`"
            )],
            ..Answer::default()
        };
    };
    let raw = match raw {
        Ok(raw) => raw,
        Err(why) => return dispatcher_failure(harness, native_event, event, &why),
    };
    if let Some(carried) = payload_event(harness, &raw)
        && !event_matches(harness, &carried, native_event)
    {
        return dispatcher_failure(
            harness,
            native_event,
            event,
            &format!("the payload names `{carried}` but the registration is for `{native_event}`"),
        );
    }
    let can_block = event.canonical.is_some_and(|c| c.can_block(harness));
    let mut payload = decode(harness, native_event, raw);
    let started = Instant::now();
    let budget = Duration::from_millis(event.budget_ms);
    let reserve = Duration::from_millis(event.response_reserve_ms);
    let mut merged = HandlerResult::default();
    let mut stderr = Vec::new();
    let mut log = log_preamble(&payload);
    for handler in &event.handlers {
        let remaining = budget
            .saturating_sub(started.elapsed())
            .saturating_sub(reserve);
        let outcome = if remaining.is_zero() {
            Err(Failure {
                why: format!(
                    "the {} ms budget of {native_event} was spent before it started",
                    event.budget_ms
                ),
                policy: handler.on_failure,
            })
        } else {
            let timeout = Duration::from_millis(handler.timeout_ms).min(remaining);
            run_handler(handler, &payload, timeout, runner)
        };
        match outcome {
            Ok(result) => {
                let _ = writeln!(log, "{} {:?}", handler.id, result.effect);
                if let Some(reason) = result.reason.as_deref().filter(|r| !r.is_empty()) {
                    let first = reason.lines().next().unwrap_or_default();
                    let _ = writeln!(
                        log,
                        "reason: {}",
                        first.chars().take(160).collect::<String>()
                    );
                }
                if event.mode == Mode::Passive && result.effect != Effect::Pass {
                    stderr.push(format!(
                        "rune hook: {native_event} is passive on {harness}; {}'s {:?} is logged, not applied",
                        handler.id, result.effect
                    ));
                }
                merge(&mut payload, &mut merged, result, event.mode);
            }
            Err(failure) => {
                let _ = writeln!(log, "{} failed: {}", handler.id, failure.why);
                match failure.policy {
                    OnFailure::Warn => stderr.push(format!(
                        "rune hook: {} failed: {} (warn, proceeding)",
                        handler.id, failure.why
                    )),
                    OnFailure::Deny if can_block && event.mode != Mode::Passive => {
                        if merged.effect == Effect::Pass {
                            merged.effect = Effect::Deny;
                            merged.reason = Some(format!("{} failed: {}", handler.id, failure.why));
                        }
                        stderr.push(format!("rune hook: {} failed: {} (deny)", handler.id, failure.why));
                    }
                    OnFailure::Deny => stderr.push(format!(
                        "rune hook: {} failed: {} (deny, but {harness} cannot block {native_event})",
                        handler.id, failure.why
                    )),
                }
            }
        }
        if event.mode == Mode::Gate && merged.effect == Effect::Deny {
            break;
        }
    }
    let mut answer = encode(harness, native_event, &merged);
    stderr.append(&mut answer.stderr);
    answer.stderr = stderr;
    append_log(harness, native_event, &log);
    answer
}

/// Fold one handler's result into the effective input and the answer.
fn merge(
    payload: &mut HandlerPayload,
    merged: &mut HandlerResult,
    result: HandlerResult,
    mode: Mode,
) {
    if let Some(input) = result.input {
        if let Some(tool) = payload.tool.as_mut() {
            tool.input = input.clone();
        }
        merged.input = Some(input);
    }
    if mode == Mode::Passive {
        return;
    }
    if result.effect != Effect::Pass && merged.effect == Effect::Pass {
        merged.effect = result.effect;
        merged.reason = result.reason;
    }
    merged.context.extend(result.context);
}

/// Spawn one handler with the payload on stdin and parse its one result.
fn run_handler(
    handler: &PlannedHandler,
    payload: &HandlerPayload,
    timeout: Duration,
    runner: &Runner,
) -> Result<HandlerResult, Failure> {
    let fail = |why: String| Failure {
        why,
        policy: handler.on_failure,
    };
    if handler.exec.is_empty() {
        return Err(fail("names no executable".to_string()));
    }
    let stdin = serde_json::to_vec(payload)
        .map_err(|e| fail(format!("cannot serialize the payload: {e}")))?;
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
    let (code, stdout, stderr) =
        (runner.spawn)(handler, &stdin, timeout, &env, cwd.as_ref()).map_err(fail)?;
    if code != Some(0) {
        let tail = stderr.lines().last().unwrap_or_default();
        return Err(fail(format!(
            "exited {}{}",
            code.map_or_else(|| "by signal".to_string(), |c| c.to_string()),
            if tail.is_empty() {
                String::new()
            } else {
                format!(": {tail}")
            }
        )));
    }
    let text = stdout.trim();
    if text.is_empty() {
        return Ok(HandlerResult::default());
    }
    serde_json::from_str::<HandlerResult>(text)
        .map_err(|e| fail(format!("printed something that is not one result: {e}")))
}

/// The first log line of a tool event: the tool and its subject, so the
/// log alone tells which call a verdict answered, including one the
/// harness leaves out of its own transcript.
fn log_preamble(payload: &HandlerPayload) -> String {
    let mut log = String::new();
    if let Some(tool) = &payload.tool {
        let _ = writeln!(
            log,
            "tool: {} {}",
            tool.name,
            super::adapters::tool_subject(tool)
        );
    }
    log
}

fn dispatcher_failure(
    harness: Harness,
    native_event: &str,
    event: &PlannedEvent,
    why: &str,
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
        harness,
        native_event,
        &format!("dispatcher failure: {why}\n"),
    );
    answer
}

fn append_log(harness: Harness, native_event: &str, text: &str) {
    use std::io::Write as _;
    let Ok(dir) = crate::cli::state::dir() else {
        return;
    };
    let hooks = dir.join("hooks");
    let _ = std::fs::create_dir_all(&hooks);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(hooks.join("run.log"))
    {
        let _ = write!(
            file,
            "{} {harness} {native_event}\n{text}",
            chrono::Utc::now().to_rfc3339()
        );
    }
}
