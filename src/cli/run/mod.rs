use crate::cli::launch;
use crate::cli::process::{ProcessFailure, ProcessTermination, SignalWatch};
use crate::cli::surface::{
    PROMPT_PLACEHOLDER, Surface, SurfaceFailure, SurfaceInvocation, SurfaceReply,
    clean_config_keys, effective_env, filter_surface_args, invoke_surface, output_tail,
    parent_session_isolation, planned_request, prepare_clean_state, route_keys, session_name_flag,
};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub(crate) use crate::cli::surface::AccessMode;

mod artifacts;
mod output;
mod workspace;

/// The runner's exit statuses follow the POSIX `timeout` utility, which
/// supervises a child as `rune run` does: the harness's own status passes
/// through, and the codes from 124 up are the runner's. 124 is the time
/// limit. 125 is an error of the runner itself: a usage or configuration
/// error, a result file it could not write, an output limit it enforced.
/// 126 is a harness binary found but not runnable, 127 one not found, and
/// 128 plus the signal number a signal that ended the harness or the
/// runner.
const EXIT_TIMEOUT: i32 = 124;
pub(crate) const EXIT_RUNNER_ERROR: i32 = 125;
const EXIT_CANNOT_RUN: i32 = 126;
const EXIT_NOT_FOUND: i32 = 127;

const AGY_SUPERVISOR_MARGIN: Duration = Duration::from_secs(30);
/// The placeholder the dry run prints for the clean state directory a
/// real run creates.
const CLEAN_STATE_PLACEHOLDER: &str = "<clean-state>";

pub(crate) struct RunOptions {
    pub(crate) tool: String,
    pub(crate) prompt: Option<String>,
    pub(crate) prompt_file: Option<PathBuf>,
    pub(crate) system_prompt_file: Option<PathBuf>,
    pub(crate) binary: Option<PathBuf>,
    pub(crate) model: Option<String>,
    pub(crate) clean: bool,
    pub(crate) repository: PathBuf,
    pub(crate) mode: AccessMode,
    pub(crate) timeout: Option<String>,
    /// A name for the harness session; `session_name_flag` says which harness takes it.
    pub(crate) session_name: Option<String>,
    /// Where the files the run created are copied. Without it a clean run
    /// copies them under the user state directory, and any other run leaves
    /// them where the harness put them.
    pub(crate) artifacts_dir: Option<PathBuf>,
    /// The file the result goes to, for every outcome, instead of stdout.
    /// Checked before the harness starts (see [`output`]).
    pub(crate) output: Option<PathBuf>,
    pub(crate) plan: RunPlan,
    pub(crate) json: bool,
}

/// What `rune run` does with the resolved plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunPlan {
    /// Spawn the tool with the prompt.
    Execute,
    /// Print the resolved launch and supervision plan. No spawn.
    DryRun,
    /// Verify the plan's model ids against each base URL. No spawn, no prompt.
    Check,
}

impl RunPlan {
    pub(crate) fn from_flags(dry_run: bool, check: bool) -> Self {
        if check {
            Self::Check
        } else if dry_run {
            Self::DryRun
        } else {
            Self::Execute
        }
    }
}

/// One outcome of `rune run`: its exit code, the one JSON object `--json`
/// prints on stdout, and the text that goes out otherwise.
struct Outcome {
    exit_code: i32,
    json: Value,
    text: Text,
}

/// Where the text form of an outcome goes: a plan or an answer on stdout,
/// an error on stderr, and a configuration error through the CLI's own
/// `fatal:` printer.
enum Text {
    Stdout(String),
    Stderr(String),
    Fatal(String),
}

impl Text {
    fn as_str(&self) -> &str {
        match self {
            Self::Stdout(text) | Self::Stderr(text) | Self::Fatal(text) => text,
        }
    }
}

/// The one printer: every outcome, a configuration error included, is one
/// JSON object on stdout under `--json`, and text otherwise. With
/// `--output`, the same object or text goes to the file, and stdout
/// carries one confirmation object under `--json`. The value is the
/// process's exit status.
pub(crate) fn execute(options: &RunOptions) -> i32 {
    // The output path is accepted before anything else, so no later
    // outcome is lost to a bad path, and a bad path starts nothing.
    // The permitted repository is the one the caller works in, read from
    // the working directory. `--repo` names where the harness runs and
    // never widens where a file may be written.
    let destination = match options
        .output
        .as_deref()
        .map(|path| output::accept(path, output::caller_repository_root().as_deref()))
        .transpose()
    {
        Ok(destination) => destination,
        Err(message) => {
            return deliver(
                options,
                None,
                configuration_outcome(options, message.into()),
            );
        }
    };
    let outcome = match execute_inner(options) {
        Ok(outcome) => outcome,
        Err(error) => configuration_outcome(options, error),
    };
    deliver(options, destination.as_ref(), outcome)
}

/// A configuration error as an outcome: exit 125, the `configuration_error`
/// object, and the message for the CLI's fatal printer.
fn configuration_outcome(options: &RunOptions, error: launch::ResolveError) -> Outcome {
    let mut details = json!({ "message": error.message });
    if let (Some(profiles), Some(details)) = (error.profiles, details.as_object_mut()) {
        details.insert("profiles".to_string(), json!(profiles));
    }
    Outcome {
        exit_code: EXIT_RUNNER_ERROR,
        json: json!({
            "ok": false,
            "kind": "configuration_error",
            "tool": options.tool,
            "details": details,
        }),
        text: Text::Fatal(error.message),
    }
}

/// Print the outcome, or write it to the result file. Every JSON outcome
/// carries its exit code, so the file and the confirmation on stdout say
/// the same number. A file that cannot be written at the end never loses
/// the result: it goes to stdout whole, with the runner's own exit code
/// (125) in it, and the process exits 125.
fn deliver(
    options: &RunOptions,
    destination: Option<&output::Destination>,
    outcome: Outcome,
) -> i32 {
    let Outcome {
        exit_code,
        json,
        text,
    } = outcome;
    let json = with_exit_code(json, exit_code);
    let Some(destination) = destination else {
        if options.json {
            println!("{json}");
            return exit_code;
        }
        match text {
            Text::Stdout(text) => println!("{text}"),
            Text::Stderr(text) => eprintln!("{text}"),
            Text::Fatal(message) => crate::cli::print_fatal(&message),
        }
        return exit_code;
    };
    let body = if options.json {
        format!("{json}\n")
    } else {
        format!("{}\n", text.as_str())
    };
    match output::write(destination, &body) {
        Ok(()) => {
            if options.json {
                println!(
                    "{}",
                    json!({
                        "ok": json["ok"],
                        "kind": json["kind"],
                        "exit_code": exit_code,
                        "output": destination.path().display().to_string(),
                    })
                );
            } else {
                eprintln!("output: {}", destination.path().display());
            }
            exit_code
        }
        Err(error) => {
            eprintln!("error: {error}");
            if options.json {
                println!("{}", with_exit_code(json, EXIT_RUNNER_ERROR));
            } else {
                print!("{body}");
            }
            EXIT_RUNNER_ERROR
        }
    }
}

/// The outcome object with its exit code in it, under `exit_code`.
fn with_exit_code(mut json: Value, exit_code: i32) -> Value {
    if let Some(object) = json.as_object_mut() {
        object.insert("exit_code".to_string(), json!(exit_code));
    }
    json
}

#[allow(clippy::too_many_lines)]
fn execute_inner(options: &RunOptions) -> Result<Outcome, launch::ResolveError> {
    if options.plan == RunPlan::Check {
        return execute_check(options);
    }
    let dry_run = options.plan == RunPlan::DryRun;
    let prompt = read_prompt(options.prompt.as_deref(), options.prompt_file.as_deref())?;
    let system_prompt =
        read_optional_text_file(options.system_prompt_file.as_deref(), "system prompt")?;
    let repository = resolve_repository(&options.repository)?;
    let requested_timeout = options.timeout.as_deref().map(parse_duration).transpose()?;
    let resolve_args = if dry_run {
        vec![OsString::from("--dry-run")]
    } else {
        Vec::new()
    };
    let resolved = launch::resolve(&options.tool, &resolve_args)?;
    let surface = Surface::from_tool(&resolved.tool).ok_or_else(|| {
        format!(
            "automated execution is not supported for '{}'; use claude, codex, agy, grok, or opencode",
            resolved.tool
        )
    })?;
    // A mode the harness cannot take is refused here, before a profile's
    // pre-steps and before any state is prepared: nothing starts.
    if let Some(message) = crate::cli::surface::unsupported_mode(surface, options.mode) {
        return Err(message.into());
    }
    let unsupported = resolved.unsupported_wrappers();
    if !unsupported.is_empty() {
        return Err(format!(
            "rune run cannot supervise {} wrappers; remove them or use rune launch",
            unsupported.join(", ")
        )
        .into());
    }
    let (resolved_binary, extra_args) = resolved
        .argv
        .split_first()
        .ok_or_else(|| "resolved launch command is empty".to_string())?;
    let binary = options.binary.as_ref().map_or_else(
        || resolved_binary.clone(),
        |path| path.as_os_str().to_os_string(),
    );
    let native_timeout = (surface == Surface::Agy)
        .then_some(requested_timeout)
        .flatten();
    let outer_timeout = supervisor_timeout(surface, requested_timeout)?;
    let model = options
        .model
        .clone()
        .or_else(|| resolved.model.as_ref().map(|model| model.id.clone()));
    // A workspace-write run never works in the caller's checkout: the plan
    // for its own jj workspace is made before anything starts, so a folder
    // that cannot take one is refused here.
    let isolation = (options.mode == AccessMode::WorkspaceWrite)
        .then(|| workspace::plan(&repository, &options.tool, options.session_name.as_deref()))
        .transpose()?;
    let invocation = SurfaceInvocation {
        surface,
        binary,
        extra_args: extra_args.to_vec(),
        env: resolved.env.clone(),
        repository,
        mode: options.mode,
        system_prompt,
        prompt,
        model,
        native_timeout,
        timeout: outer_timeout,
        clean_state_root: None,
        session_name: options.session_name.clone(),
    };

    // Profile arguments the run owns are dropped here, once, and each drop
    // is a warning on stderr, in the plan, and in the JSON result.
    let filtered = filter_surface_args(&invocation);
    let invocation = SurfaceInvocation {
        extra_args: filtered.kept,
        ..invocation
    };

    if dry_run || resolved.dry_run {
        return Ok(dry_run_outcome(
            options,
            &resolved,
            &invocation,
            &filtered.warnings,
            isolation.as_ref(),
        )?);
    }

    for warning in resolved.warnings().iter().chain(&filtered.warnings) {
        eprintln!("warning: {warning}");
    }
    resolved.run_pre_steps();

    let clean_state = options
        .clean
        .then(|| tempfile::Builder::new().prefix("rune-clean-").tempdir())
        .transpose()
        .map_err(|error| format!("cannot create clean harness state: {error}"))?;
    if let Some(state) = &clean_state {
        prepare_clean_state(surface, state.path(), invocation.model.as_deref())
            .map_err(|error| error.to_string())?;
    }
    // The workspace is the harness's repository; the caller's checkout
    // stays out of the run. From here to the finish a signal only sets a
    // flag, and a panic or an early return drops the guard, which takes
    // the workspace down: nothing the harness wrote is left behind.
    let watch = isolation
        .as_ref()
        .map(|_| SignalWatch::install())
        .transpose()?;
    let mut isolated = Isolated { workspace: None };
    // The guard holds the copy from the moment it exists: a failure or a
    // signal while the copy is prepared or the workspace is added still
    // abandons it. A setup that fails is no early return: the finish and
    // the signal check below run for it as for a harness failure, so a
    // signal that arrives during that cleanup is the outcome.
    let setup = isolation
        .as_ref()
        .map_or(Ok(()), |plan| set_up_workspace(&mut isolated, plan));
    let repository = isolated.workspace.as_ref().map_or_else(
        || invocation.repository.clone(),
        |workspace| workspace.harness_dir.clone(),
    );
    let invocation = SurfaceInvocation {
        repository,
        clean_state_root: clean_state.as_ref().map(|state| state.path().to_path_buf()),
        ..invocation
    };
    let started = std::time::Instant::now();
    // The wall clock beside the monotonic one: a file the run created is
    // told from an older one by its modification time.
    let started_at = SystemTime::now();
    // A signal that arrived while the workspace was made stops the run
    // before the harness starts: the workspace is finished, the signal is
    // reported, and the exit code is the signal's. The outer error is the
    // setup's, the inner the harness's.
    let result: Result<Result<SurfaceReply, SurfaceFailure>, String> =
        match (setup, watch.as_ref().and_then(SignalWatch::received)) {
            (Err(message), _) => Err(message),
            (Ok(()), Some(signal)) => Ok(Err(stopped_before_start(signal))),
            (Ok(()), None) => Ok(invoke_surface(&invocation)),
        };
    // A signal the supervisor forwarded to the harness is in the result and
    // is acknowledged, so it is not reported twice. Any other signal, one
    // that arrived after the supervisor ended included, stays pending and
    // ends the run after the finish.
    if let (Some(watch), Ok(Err(failure))) = (&watch, &result)
        && let SurfaceFailure::Process {
            failure: ProcessFailure::ForwardedSignal { signal, .. },
            ..
        } = failure.unwrapped()
    {
        watch.acknowledge(*signal);
    }
    // On every outcome, a forced stop included: the files are recorded, the
    // workspace is forgotten, and an empty change leaves no trace.
    let finished = isolated.finish();
    let workspace_json = finished
        .as_ref()
        .map_or(Value::Null, |(workspace, outcome)| {
            workspace_json(workspace, outcome)
        });
    let workspace_lines = finished
        .as_ref()
        .map(|(workspace, outcome)| workspace_lines(workspace, outcome))
        .unwrap_or_default();
    let mut outcome = match result {
        Err(message) => {
            configuration_with_workspace(options, message, workspace_json, &workspace_lines)
        }
        Ok(Ok(reply)) => {
            if !reply.stderr.is_empty() {
                eprint!("{}", reply.stderr);
                if !reply.stderr.ends_with('\n') {
                    eprintln!();
                }
            }
            // Before `clean_state` drops: a clean run's files live inside it.
            // A copy that fails keeps the workspace object: the change the
            // run left is named whatever happens after it.
            match collect_artifacts(
                options,
                &invocation,
                reply.conversation_id.as_deref(),
                started_at,
            ) {
                Err(message) => {
                    configuration_with_workspace(options, message, workspace_json, &workspace_lines)
                }
                Ok(artifacts) => {
                    if !options.json {
                        for artifact in &artifacts {
                            eprintln!("artifact: {artifact}");
                        }
                        for line in &workspace_lines {
                            eprintln!("{line}");
                        }
                    }
                    Outcome {
                        exit_code: 0,
                        json: json!({
                            "ok": true,
                            "kind": "success",
                            "tool": resolved.tool,
                            "requested_route": options.tool,
                            "requested_provider": options.tool,
                            "resolved_route": resolved.tool,
                            "resolved_model": invocation.model,
                            "resolved_binary": invocation.binary.to_string_lossy(),
                            "model": invocation.model,
                            "text": reply.text,
                            "duration_ms": started.elapsed().as_millis(),
                            "clean": options.clean,
                            "session_name": options.session_name.as_deref(),
                            "session_name_flag": session_name_flag(invocation.surface),
                            "usage": {
                                "input_tokens": Value::Null,
                                "cache_creation_input_tokens": Value::Null,
                                "cache_read_input_tokens": Value::Null,
                                "output_tokens": reply.completion_tokens,
                                "total_tokens": Value::Null,
                            },
                            "completion_tokens": reply.completion_tokens,
                            "warnings": filtered.warnings,
                            "artifacts": artifacts,
                            "workspace": workspace_json,
                        }),
                        text: Text::Stdout(reply.text),
                    }
                }
            }
        }
        Ok(Err(failure)) => {
            let mut json = failure_json(&resolved.tool, &failure);
            if let Some(object) = json.as_object_mut() {
                object.insert("workspace".to_string(), workspace_json);
            }
            let mut text = format!("error: {failure}");
            for line in &workspace_lines {
                text.push('\n');
                text.push_str(line);
            }
            Outcome {
                exit_code: failure_exit_code(&failure),
                json,
                text: Text::Stderr(text),
            }
        }
    };
    // A signal that arrived during the finish was deferred until the
    // workspace was down: the result is whole, and the process ends with
    // the signal's code.
    // The signal is the outcome: the object says so beside the answer it
    // still carries, as a forwarded signal does.
    if let Some(signal) = watch.as_ref().and_then(SignalWatch::received) {
        outcome.exit_code = 128 + signal;
        if let Some(object) = outcome.json.as_object_mut() {
            object.insert("ok".to_string(), json!(false));
            object.insert("kind".to_string(), json!("signal"));
            object.insert("stopped_by_signal".to_string(), json!(signal));
        }
        eprintln!("stopped by signal {signal} after the run; the workspace was finished first");
    }
    Ok(outcome)
}

/// Make the run's workspace: the copy, the caller's files in it, and the
/// jj workspace on it, each step under the guard that takes it all down.
fn set_up_workspace(isolated: &mut Isolated, plan: &workspace::Plan) -> Result<(), String> {
    isolated.workspace = Some(workspace::begin(plan)?);
    if let Some(workspace) = isolated.workspace.as_mut() {
        workspace::prepare(workspace)?;
        workspace::add(workspace)?;
    }
    Ok(())
}

/// A configuration error that still names what the run's workspace left:
/// the `workspace` object in the JSON, and its lines after the message.
fn configuration_with_workspace(
    options: &RunOptions,
    message: String,
    workspace_json: Value,
    workspace_lines: &[String],
) -> Outcome {
    let mut outcome = configuration_outcome(options, message.into());
    if let Some(object) = outcome.json.as_object_mut() {
        object.insert("workspace".to_string(), workspace_json);
    }
    if let Text::Fatal(text) = &mut outcome.text {
        for line in workspace_lines {
            text.push('\n');
            text.push_str(line);
        }
    }
    outcome
}

/// The failure for a signal that arrived before the harness started: the
/// signal's kind and exit code, with a note that nothing ran.
fn stopped_before_start(signal: i32) -> SurfaceFailure {
    SurfaceFailure::Noted {
        inner: Box::new(SurfaceFailure::from(ProcessFailure::ForwardedSignal {
            signal,
            stdout: String::new(),
            stderr: String::new(),
        })),
        note: format!(
            "signal {signal} arrived while the workspace was made; the harness never started, and the workspace was finished"
        ),
    }
}

/// The run's workspace, taken down whatever happens after `prepare`. The
/// normal path calls `finish` and reports what it learned. A panic or an
/// early return drops the guard, which finishes the workspace and prints
/// the report on stderr, so no harness file is lost and no workspace
/// stays tracked.
struct Isolated {
    workspace: Option<workspace::Workspace>,
}

impl Isolated {
    fn finish(&mut self) -> Option<(workspace::Workspace, workspace::Outcome)> {
        let workspace = self.workspace.take()?;
        let outcome =
            workspace::finish(&workspace, crate::cli::process::last_run_left_a_survivor());
        Some((workspace, outcome))
    }
}

impl Drop for Isolated {
    fn drop(&mut self) {
        if let Some((workspace, outcome)) = self.finish() {
            for line in workspace_lines(&workspace, &outcome) {
                eprintln!("{line}");
            }
        }
    }
}

/// The `workspace` object of a result: the change the run left, whether it
/// was kept, whether anything outside the run changed, the change the
/// harness moved to when it left the copy, the operations the run spans,
/// and, when a step failed or the folder stays, the error and the folder.
/// A field the finish did not reach is null.
fn workspace_json(workspace: &workspace::Workspace, outcome: &workspace::Outcome) -> Value {
    let mut object = json!({
        "name": workspace.name,
        "change_id": outcome.change_id,
        "kept": outcome.kept,
        "conflicted": outcome.conflicted,
        "colocated": workspace.colocated,
        "guard": workspace.guard_error.is_none(),
        "guard_error": workspace.guard_error,
        "outside_changes": outcome.outside_changes,
        "left_copy": outcome.left_copy,
        "kept_revisions": outcome.kept_revisions,
        "start_operation": outcome.start_operation,
        "end_operation": outcome.end_operation,
    });
    if let Some(fields) = object.as_object_mut() {
        if let Some(error) = &outcome.error {
            fields.insert("error".to_string(), json!(error));
        }
        if let Some(path) = &outcome.path {
            fields.insert("path".to_string(), json!(path.display().to_string()));
        }
    }
    object
}

/// The text lines for the workspace: one names the change kept or says
/// nothing was, one more when this run colocated jj, one when the harness
/// left the copy, one that names the operations the run spans, and a
/// warning when the workspace could not be taken down.
fn workspace_lines(workspace: &workspace::Workspace, outcome: &workspace::Outcome) -> Vec<String> {
    let mut lines = Vec::new();
    if workspace.colocated {
        lines.push(format!(
            "workspace: colocated jj in {}",
            workspace.root.display()
        ));
    }
    if let Some(reason) = &workspace.guard_error {
        lines.push(format!(
            "warning: the guard could not be set in the run's workspace, so the harness could rewrite the copy there; the outside change report still ran ({reason})"
        ));
    }
    match (&outcome.change_id, outcome.kept) {
        (Some(change_id), Some(true)) => lines.push(format!(
            "workspace: kept change {} ({}){}; review it with jj, then squash or abandon it",
            change_id,
            workspace.name,
            if outcome.conflicted == Some(true) {
                ", with a conflict against your current change"
            } else {
                ""
            }
        )),
        (Some(_), Some(false)) => {
            lines.push("workspace: no file changed, nothing kept".to_string());
        }
        _ => {}
    }
    if let Some(change_id) = &outcome.left_copy {
        lines.push(format!(
            "workspace: the harness moved its workspace to change {change_id}, off the run's copy; its files were not recorded, and no result was built"
        ));
    }
    if !outcome.kept_revisions.is_empty() {
        lines.push(format!(
            "workspace: kept {} (neither empty nor part of the result); review them with jj, then squash or abandon them",
            outcome.kept_revisions.join(", ")
        ));
    }
    let operations = match (&outcome.start_operation, &outcome.end_operation) {
        (Some(start), Some(end)) => format!("jj op diff --from {start} --to {end}"),
        (Some(start), None) => format!("jj op log from operation {start}"),
        _ => "jj op log".to_string(),
    };
    if outcome.outside_changes == Some(true) {
        lines.push(format!(
            "warning: commits or git settings outside the run changed while it ran (a harness rewrote one, or your own squash, rebase, or describe did); review them with {operations}"
        ));
    } else if outcome.end_operation.is_some() {
        lines.push(format!(
            "workspace: review what the run did to the repository with {operations}"
        ));
    }
    let folder = outcome.path.as_deref().unwrap_or(&workspace.path);
    if let Some(error) = &outcome.error {
        lines.push(format!(
            "warning: workspace {} stays at {}: {error}",
            workspace.name,
            folder.display()
        ));
    }
    lines
}

/// The dry run's `workspace` object: the one a workspace-write run would
/// create, with the caller's change it would branch from, else null.
fn planned_workspace_json(plan: Option<&workspace::Plan>) -> Value {
    plan.map_or(Value::Null, |plan| {
        json!({
            "name": plan.name,
            "path": plan.path.display().to_string(),
            "parent_change_id": plan.parent_change_id,
            "colocate": plan.vcs.colocate(),
        })
    })
}

/// The dry run's text lines for the workspace, `workspace: none` when the
/// run stays in the caller's checkout.
fn planned_workspace_text(plan: Option<&workspace::Plan>) -> String {
    plan.map_or_else(
        || "workspace: none".to_string(),
        |plan| {
            format!(
                "workspace: {}\nworkspace_parent: {}\nworkspace_colocate: {}",
                plan.path.display(),
                plan.parent_change_id
                    .as_deref()
                    .unwrap_or("<the change jj git init --colocate creates>"),
                plan.vcs.colocate()
            )
        },
    )
}

/// The dry run: the process the surface would start, rendered from the
/// same argument builders the spawn uses, with the prompt as a placeholder
/// and every credential value hidden. The text keeps the launch plan's
/// lines (`model_id:` included) and adds the run's own; the JSON object
/// carries the same fields.
fn dry_run_outcome(
    options: &RunOptions,
    resolved: &launch::ResolvedLaunch,
    invocation: &SurfaceInvocation,
    filter_warnings: &[String],
    workspace_plan: Option<&workspace::Plan>,
) -> Result<Outcome, String> {
    // The harness would start in the run's workspace, so the plan's argv
    // names that path, never the caller's checkout.
    let planned_invocation = SurfaceInvocation {
        clean_state_root: options
            .clean
            .then(|| PathBuf::from(CLEAN_STATE_PLACEHOLDER)),
        repository: workspace_plan.map_or_else(
            || invocation.repository.clone(),
            workspace::Plan::harness_path,
        ),
        ..invocation.clone()
    };
    let planned = planned_request(&planned_invocation).map_err(|failure| failure.to_string())?;
    let argv: Vec<OsString> = std::iter::once(planned.binary.clone())
        .chain(planned.args.iter().cloned())
        .collect();
    let isolation = parent_session_isolation(&route_keys(invocation));
    let env_removed = isolation.lines();
    let warnings: Vec<String> = resolved
        .warnings()
        .iter()
        .chain(filter_warnings)
        .cloned()
        .collect();
    // The endpoint from the child's view: the planned process env, then the
    // settings env, which claude applies over it.
    let mut effective = planned.env.clone();
    effective.extend(
        effective_env(invocation)
            .into_iter()
            .skip(invocation.env.len()),
    );
    let base_url = resolved.endpoint(&effective, &launch::inherited_env(&isolation.removals()));
    let stdin = planned.stdin.as_ref().map(|_| PROMPT_PLACEHOLDER);
    let artifacts_dir = planned_artifacts_dir(options, invocation.surface)?
        .map(|dir| dir.to_string_lossy().into_owned());
    let clean_config_keys = planned_clean_config_keys(options, invocation.surface)?;
    let parts = launch::DryRunParts {
        tool: &resolved.tool,
        argv: &argv,
        env: &planned.env,
        env_removed: &env_removed,
        base_url: base_url.as_deref(),
        model: resolved.model.as_ref(),
        model_override: options.model.as_deref(),
        warnings: &warnings,
    };
    let text = format!(
        "{}\nstdin: {}\nrepository: {}\nmode: {}\ntimeout: {}\nnative_timeout: {}\nsession_name: {}\nsession_name_flag: {}\nartifacts_dir: {}\nclean_config_keys: {}\n{}",
        resolved.format_dry_run_with(&parts),
        stdin.unwrap_or("none"),
        planned_invocation.repository.display(),
        options.mode.label(),
        duration_label(invocation.timeout),
        duration_label(invocation.native_timeout),
        options.session_name.as_deref().unwrap_or("none"),
        session_name_flag(invocation.surface).unwrap_or("none"),
        artifacts_dir.as_deref().unwrap_or("none"),
        if clean_config_keys.is_empty() {
            "none".to_string()
        } else {
            clean_config_keys.join(", ")
        },
        planned_workspace_text(workspace_plan)
    );
    // The launcher's plan object, plus the runner's own fields.
    let mut json = resolved.plan_json(&parts, &isolation);
    if let Some(object) = json.as_object_mut() {
        for (key, value) in [
            ("requested_route", json!(options.tool)),
            ("resolved_route", json!(resolved.tool)),
            ("resolved_binary", json!(planned.binary.to_string_lossy())),
            ("stdin", json!(stdin)),
            (
                "repository",
                json!(planned_invocation.repository.display().to_string()),
            ),
            ("mode", json!(options.mode.label())),
            (
                "timeout",
                json!(invocation.timeout.map(|timeout| timeout.as_millis())),
            ),
            (
                "native_timeout",
                json!(invocation.native_timeout.map(|timeout| timeout.as_millis())),
            ),
            ("session_name", json!(options.session_name.as_deref())),
            (
                "session_name_flag",
                json!(session_name_flag(invocation.surface)),
            ),
            ("clean", json!(options.clean)),
            ("artifacts_dir", json!(artifacts_dir)),
            ("clean_config_keys", json!(clean_config_keys)),
            ("workspace", planned_workspace_json(workspace_plan)),
        ] {
            object.insert(key.to_string(), value);
        }
    }
    Ok(Outcome {
        exit_code: 0,
        json,
        text: Text::Stdout(text),
    })
}

/// The configuration keys a clean run keeps from the owner's files, by
/// name: a grok key map names environment variables, never their values.
/// Empty for a run that is not clean.
fn planned_clean_config_keys(
    options: &RunOptions,
    surface: Surface,
) -> Result<Vec<String>, String> {
    if !options.clean {
        return Ok(Vec::new());
    }
    clean_config_keys(surface).map_err(|failure| failure.to_string())
}

/// The files the run created, as absolute path strings for the result:
/// the copies when they were copied (into `--artifacts-dir`, or under the
/// user state directory on a clean run), else the originals. Empty for
/// every surface but agy, for an agy reply without a conversation id, and
/// for a conversation folder that holds nothing new.
fn collect_artifacts(
    options: &RunOptions,
    invocation: &SurfaceInvocation,
    conversation_id: Option<&str>,
    since: SystemTime,
) -> Result<Vec<String>, String> {
    let (Some(data_dir), Some(conversation_id)) =
        (artifacts::agy_data_dir(invocation), conversation_id)
    else {
        return Ok(Vec::new());
    };
    let folder = artifacts::conversation_dir(&data_dir, conversation_id);
    let found = artifacts::find_artifacts(&folder, since);
    let destination = match (&options.artifacts_dir, options.clean) {
        (Some(dir), _) => Some(dir.clone()),
        (None, true) => Some(artifacts::state_artifacts_dir(conversation_id)?),
        (None, false) => None,
    };
    let files = match destination {
        Some(dir) => artifacts::copy_artifacts(&found, &dir)?,
        None => found,
    };
    Ok(files
        .iter()
        .map(|file| file.to_string_lossy().into_owned())
        .collect())
}

/// Where the dry run says the files would go: the flag's directory, the
/// state folder with a placeholder id for a clean agy run, else nowhere.
fn planned_artifacts_dir(
    options: &RunOptions,
    surface: Surface,
) -> Result<Option<PathBuf>, String> {
    if let Some(dir) = &options.artifacts_dir {
        return Ok(Some(dir.clone()));
    }
    if options.clean && surface == Surface::Agy {
        return artifacts::state_artifacts_dir(artifacts::CONVERSATION_PLACEHOLDER).map(Some);
    }
    Ok(None)
}

/// `--check`: verify the model ids the run would send against each base
/// URL in the plan. No prompt is read and nothing is spawned. The exit
/// code is the check's: 0 served, 1 missing, 2 endpoint failure.
fn execute_check(options: &RunOptions) -> Result<Outcome, launch::ResolveError> {
    let resolved = launch::resolve(&options.tool, &[])?;
    let report = launch::check::run_checks(&resolved, options.model.as_deref());
    Ok(Outcome {
        exit_code: report.exit_code,
        json: launch::check::report_json(&report),
        text: Text::Stdout(launch::check::format_report(&report)),
    })
}

fn read_prompt(argument: Option<&str>, file: Option<&Path>) -> Result<String, String> {
    let prompt = match (argument, file) {
        (Some(_), Some(_)) => {
            return Err("pass a prompt argument or --prompt-file, not both".to_string());
        }
        (Some(prompt), None) => prompt.to_string(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read prompt file {}: {error}", path.display()))?,
        (None, None) => {
            let mut stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Err(
                    "provide a prompt argument, --prompt-file, or pipe a prompt on stdin"
                        .to_string(),
                );
            }
            let mut prompt = String::new();
            stdin
                .read_to_string(&mut prompt)
                .map_err(|error| format!("cannot read prompt from stdin: {error}"))?;
            prompt
        }
    };
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".to_string());
    }
    Ok(prompt)
}

fn read_optional_text_file(path: Option<&Path>, label: &str) -> Result<String, String> {
    let Some(path) = path else {
        return Ok(String::new());
    };
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {label} file {}: {error}", path.display()))?;
    if text.trim().is_empty() {
        return Err(format!(
            "{label} file must not be empty: {}",
            path.display()
        ));
    }
    Ok(text)
}

fn resolve_repository(path: &Path) -> Result<PathBuf, String> {
    let repository = std::fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve repository {}: {error}", path.display()))?;
    if !repository.is_dir() {
        return Err(format!(
            "repository is not a directory: {}",
            repository.display()
        ));
    }
    Ok(repository)
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1_u64)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600_000)
    } else {
        (value, 1_000)
    };
    let amount = number
        .parse::<u64>()
        .map_err(|_| format!("invalid timeout '{value}'; use values such as 30s, 5m, or 1h"))?;
    let milliseconds = amount
        .checked_mul(multiplier)
        .ok_or_else(|| format!("timeout '{value}' is too large"))?;
    if milliseconds == 0 {
        return Err("timeout must be greater than zero".to_string());
    }
    Ok(Duration::from_millis(milliseconds))
}

fn supervisor_timeout(
    surface: Surface,
    requested: Option<Duration>,
) -> Result<Option<Duration>, String> {
    if surface != Surface::Agy {
        return Ok(requested);
    }
    requested
        .map(|timeout| {
            timeout
                .checked_add(AGY_SUPERVISOR_MARGIN)
                .ok_or_else(|| "agy supervisor timeout is too large".to_string())
        })
        .transpose()
}

fn duration_label(timeout: Option<Duration>) -> String {
    timeout.map_or_else(
        || "none".to_string(),
        |duration| format!("{}ms", duration.as_millis()),
    )
}

/// The exit status of a failed run, by the POSIX `timeout` table: the
/// harness's own status passes through, and the runner's own codes start
/// at 124. A harness that exited 0 without an answer keeps 1, because no
/// row of the table names it.
fn failure_exit_code(failure: &SurfaceFailure) -> i32 {
    match failure {
        SurfaceFailure::Process {
            failure: ProcessFailure::Timeout { .. },
            ..
        } => EXIT_TIMEOUT,
        SurfaceFailure::Process {
            failure: ProcessFailure::ForwardedSignal { signal, .. },
            ..
        }
        | SurfaceFailure::Exit {
            termination: ProcessTermination::Signaled(signal),
            ..
        } => 128 + signal,
        SurfaceFailure::Process {
            failure: ProcessFailure::NotFound(_),
            ..
        } => EXIT_NOT_FOUND,
        SurfaceFailure::Process {
            failure: ProcessFailure::Spawn(_),
            ..
        } => EXIT_CANNOT_RUN,
        SurfaceFailure::Process {
            failure: ProcessFailure::OutputLimit { .. },
            ..
        } => EXIT_RUNNER_ERROR,
        SurfaceFailure::Exit {
            termination: ProcessTermination::Exited(code),
            ..
        } => *code,
        SurfaceFailure::Noted { inner, .. } => failure_exit_code(inner),
        // The runner's own failures: a scratch directory or prompt file it
        // cannot create, an argument it cannot build, a supervisor that
        // cannot wait for, read, or signal the harness.
        SurfaceFailure::Process { .. } | SurfaceFailure::Io(_) | SurfaceFailure::Arguments(_) => {
            EXIT_RUNNER_ERROR
        }
        // A harness that exited 0 without an answer, or reported its own
        // failure, keeps 1: its status would read as success.
        SurfaceFailure::NoAnswer { .. } | SurfaceFailure::Reported(_) => 1,
    }
}

/// The failure as one JSON object. A failure that carries the harness's
/// output puts its last lines in `details.stdout_tail` and
/// `details.stderr_tail`; `hint` names an outer sandbox when the output
/// shows one, else null.
fn failure_json(tool: &str, failure: &SurfaceFailure) -> Value {
    if let SurfaceFailure::Noted { inner, note } = failure {
        let mut json = failure_json(tool, inner);
        if let Some(details) = json["details"].as_object_mut() {
            details.insert("note".to_string(), Value::String(note.clone()));
        }
        return json;
    }
    let (kind, mut details) = match failure {
        SurfaceFailure::Process {
            failure: ProcessFailure::Timeout { timeout, .. },
            ..
        } => ("timeout", json!({ "timeout_ms": timeout.as_millis() })),
        SurfaceFailure::Process {
            failure: ProcessFailure::ForwardedSignal { signal, .. },
            ..
        }
        | SurfaceFailure::Exit {
            termination: ProcessTermination::Signaled(signal),
            ..
        } => ("signal", json!({ "signal": signal })),
        SurfaceFailure::Process {
            failure:
                ProcessFailure::OutputLimit {
                    stream,
                    limit,
                    stdout,
                    stderr,
                },
            ..
        } => (
            "output_limit",
            json!({
                "stream": stream,
                "limit": limit,
                "tail": output_tail(if *stream == "stdout" { stdout } else { stderr }),
            }),
        ),
        SurfaceFailure::Process { failure, .. } => {
            ("process_failure", json!({ "message": failure.to_string() }))
        }
        SurfaceFailure::Exit {
            termination: ProcessTermination::Exited(code),
            ..
        } => ("exit", json!({ "exit_code": code })),
        SurfaceFailure::NoAnswer { message, .. } | SurfaceFailure::Reported(message) => {
            ("surface_error", json!({ "message": message }))
        }
        SurfaceFailure::Io(message) => ("io_error", json!({ "message": message })),
        SurfaceFailure::Arguments(message) => ("argument_error", json!({ "message": message })),
        SurfaceFailure::Noted { .. } => unreachable!("a note is unwrapped above"),
    };
    if let (Some((stdout_tail, stderr_tail)), Some(details)) =
        (failure.tails(), details.as_object_mut())
    {
        details.insert(
            "stdout_tail".to_string(),
            Value::String(output_tail(stdout_tail)),
        );
        details.insert(
            "stderr_tail".to_string(),
            Value::String(output_tail(stderr_tail)),
        );
    }
    // Every failure that carries the harness's output carries its reported
    // errors too, a forced stop included.
    if let (Some(_), Some(details)) = (failure.tails(), details.as_object_mut()) {
        details.insert(
            "harness_errors".to_string(),
            json!(failure.harness_errors()),
        );
    }
    json!({
        "ok": false,
        "kind": kind,
        "tool": tool,
        "details": details,
        "hint": failure.hint(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_absent_by_default() {
        assert_eq!(supervisor_timeout(Surface::Codex, None), Ok(None));
    }

    #[test]
    fn system_prompt_file_requires_content() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("system.md");
        std::fs::write(&path, "Use the rule.\n").expect("write system prompt");
        assert_eq!(
            read_optional_text_file(Some(&path), "system prompt"),
            Ok("Use the rule.\n".to_string())
        );
        std::fs::write(&path, " \n").expect("write empty system prompt");
        assert!(read_optional_text_file(Some(&path), "system prompt").is_err());
    }

    #[test]
    fn duration_parser_accepts_documented_units() {
        assert_eq!(parse_duration("250ms"), Ok(Duration::from_millis(250)));
        assert_eq!(parse_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_duration("5m"), Ok(Duration::from_mins(5)));
        assert_eq!(parse_duration("1h"), Ok(Duration::from_hours(1)));
    }

    #[test]
    fn agy_supervisor_deadline_follows_native_timeout() {
        let native = Duration::from_mins(5);
        assert_eq!(
            supervisor_timeout(Surface::Agy, Some(native)),
            Ok(Some(native + AGY_SUPERVISOR_MARGIN))
        );
    }

    #[test]
    fn typed_json_carries_both_tails_and_the_sandbox_hint() {
        let exit = SurfaceFailure::Exit {
            termination: ProcessTermination::Exited(1),
            stdout_tail: "API Error: 400".to_string(),
            stderr_tail: "OSStatus -26276".to_string(),
            harness_errors: vec!["quota exhausted".to_string()],
        };
        let json = failure_json("claude", &exit);
        assert_eq!(json["kind"], "exit");
        assert_eq!(json["details"]["exit_code"], 1);
        assert_eq!(json["details"]["stdout_tail"], "API Error: 400");
        assert_eq!(json["details"]["stderr_tail"], "OSStatus -26276");
        assert_eq!(
            json["details"]["harness_errors"],
            json!(["quota exhausted"])
        );
        assert!(
            json["hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("certificate")),
            "{json}"
        );

        let timeout = SurfaceFailure::from(ProcessFailure::Timeout {
            timeout: Duration::from_secs(5),
            stdout: "partial".to_string(),
            stderr: String::new(),
        });
        let json = failure_json("codex", &timeout);
        assert_eq!(json["kind"], "timeout");
        assert_eq!(json["details"]["timeout_ms"], 5000);
        assert_eq!(json["details"]["stdout_tail"], "partial");
        assert_eq!(json["details"]["stderr_tail"], "");
        assert_eq!(json["details"]["harness_errors"], json!([]));
        assert!(json["hint"].is_null(), "{json}");
        assert_eq!(failure_exit_code(&timeout), 124);

        let stopped = SurfaceFailure::Process {
            failure: ProcessFailure::ForwardedSignal {
                signal: 15,
                stdout: "API Error: 403 OSStatus -26276\n".to_string(),
                stderr: String::new(),
            },
            harness_errors: vec!["API Error: 403 OSStatus -26276".to_string()],
        };
        let json = failure_json("claude", &stopped);
        assert_eq!(json["kind"], "signal");
        assert_eq!(
            json["details"]["harness_errors"],
            json!(["API Error: 403 OSStatus -26276"])
        );
        assert!(
            json["hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("certificate")),
            "{json}"
        );

        let silent = SurfaceFailure::NoAnswer {
            message: "claude print produced no final response".to_string(),
            stdout_tail: String::new(),
            stderr_tail: "model refused".to_string(),
            harness_errors: Vec::new(),
        };
        let json = failure_json("claude", &silent);
        assert_eq!(json["kind"], "surface_error");
        assert_eq!(json["details"]["stderr_tail"], "model refused");
        assert_eq!(failure_exit_code(&silent), 1);
    }

    #[test]
    fn a_cleanup_note_keeps_the_failures_kind_tails_and_exit_code() {
        let noted = SurfaceFailure::Noted {
            inner: Box::new(SurfaceFailure::Exit {
                termination: ProcessTermination::Exited(3),
                stdout_tail: "half".to_string(),
                stderr_tail: "OSStatus -26276".to_string(),
                harness_errors: Vec::new(),
            }),
            note: "cannot remove scratch directory /x: gone".to_string(),
        };
        let json = failure_json("codex", &noted);
        assert_eq!(json["kind"], "exit");
        assert_eq!(json["details"]["exit_code"], 3);
        assert_eq!(json["details"]["stdout_tail"], "half");
        assert_eq!(
            json["details"]["note"],
            "cannot remove scratch directory /x: gone"
        );
        assert!(
            json["hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("certificate"))
        );
        assert_eq!(failure_exit_code(&noted), 3);

        let wait = SurfaceFailure::from(ProcessFailure::Wait {
            message: "cannot signal process group 7: gone".to_string(),
            stdout: "read so far".to_string(),
            stderr: String::new(),
        });
        let json = failure_json("codex", &wait);
        assert_eq!(json["kind"], "process_failure");
        assert_eq!(json["details"]["stdout_tail"], "read so far");
    }

    #[test]
    fn typed_json_distinguishes_output_limit() {
        let failure = SurfaceFailure::from(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: 64,
            stdout: "last bytes".to_string(),
            stderr: "a warning".to_string(),
        });
        assert_eq!(
            failure_json("codex", &failure),
            json!({
                "ok": false,
                "kind": "output_limit",
                "tool": "codex",
                "details": {
                    "stream": "stdout",
                    "limit": 64,
                    "tail": "last bytes",
                    "stdout_tail": "last bytes",
                    "stderr_tail": "a warning",
                    "harness_errors": [],
                },
                "hint": null,
            })
        );
        let signal = SurfaceFailure::from(ProcessFailure::ForwardedSignal {
            signal: 15,
            stdout: "half".to_string(),
            stderr: String::new(),
        });
        let json = failure_json("codex", &signal);
        assert_eq!(json["kind"], "signal");
        assert_eq!(json["details"]["stdout_tail"], "half");
        assert_eq!(failure_exit_code(&signal), 143);
    }

    /// The POSIX timeout table: 125 for the runner's own limit, 126 for a
    /// binary that is there but cannot start, 127 for one that is not.
    #[test]
    fn the_runners_own_codes_follow_the_posix_timeout_table() {
        let limit = SurfaceFailure::from(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: 64,
            stdout: String::new(),
            stderr: String::new(),
        });
        assert_eq!(failure_exit_code(&limit), 125);
        let cannot_run = SurfaceFailure::from(ProcessFailure::Spawn(
            "claude not runnable: Permission denied".to_string(),
        ));
        assert_eq!(failure_exit_code(&cannot_run), 126);
        let not_found = SurfaceFailure::from(ProcessFailure::NotFound(
            "claude not found: No such file or directory".to_string(),
        ));
        assert_eq!(failure_exit_code(&not_found), 127);
        assert_eq!(
            with_exit_code(json!({"ok": true, "kind": "success"}), 0),
            json!({"ok": true, "kind": "success", "exit_code": 0})
        );
    }
}
