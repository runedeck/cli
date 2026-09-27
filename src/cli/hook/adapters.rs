//! Built-in handlers. Each one reads the normalized payload, calls the
//! executable it wraps with that tool's own input, and answers with one
//! `HandlerResult`. The dispatcher spawns them as
//! `rune hook adapter <name>` when a handler's `exec` names
//! `rune-hook-<name>`, so they run out of process like any handler.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::encode::{Effect, HandlerResult};
use rune::hooks::Harness;

use super::payload::HandlerPayload;

/// The adapters this release ships, with what each one adopts.
pub(crate) const NAMES: &[&str] = &[
    "author-identity",
    "dcg",
    "git-ai",
    "rtk",
    "lint-on-write",
    "session-capture",
    "turn-checkpoint",
    "tmux-status",
    "jj-guards",
];

/// Run one adapter over the payload on stdin; print its result.
pub fn run(name: &str, args: &[String]) -> i32 {
    use std::io::Read as _;
    let mut text = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut text) {
        eprintln!("rune hook adapter {name}: cannot read the payload: {error}");
        return 1;
    }
    let payload: HandlerPayload = match serde_json::from_str(&text) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("rune hook adapter {name}: payload is not a handler payload: {error}");
            return 1;
        }
    };
    let result = match dispatch(name, &payload, args) {
        Ok(result) => result,
        Err(message) => {
            eprintln!("rune hook adapter {name}: {message}");
            return 1;
        }
    };
    match serde_json::to_string(&result) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(error) => {
            eprintln!("rune hook adapter {name}: cannot serialize the result: {error}");
            1
        }
    }
}

pub(crate) fn dispatch(
    name: &str,
    payload: &HandlerPayload,
    args: &[String],
) -> Result<HandlerResult, String> {
    match name {
        "author-identity" => author_identity(payload, args.first().map(PathBuf::from)),
        "dcg" => dcg(payload),
        "git-ai" => git_ai(payload),
        "rtk" => rtk(payload),
        "lint-on-write" => lint_on_write(payload),
        "session-capture" => session_capture(payload),
        "turn-checkpoint" => turn_checkpoint(payload),
        "tmux-status" => tmux_status(payload),
        "jj-guards" => jj_guards(payload),
        other => Err(format!(
            "`{other}` is not a built-in adapter ({})",
            NAMES.join(", ")
        )),
    }
}

// ------------------------------------------------------------ identity ----

/// One roster line: `Display Name (model-id) <local@domain>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    pub name: String,
    pub email: String,
    pub model: String,
    pub domain: String,
}

/// The model as the roster spells it: the `[1m]` context marker stripped.
pub(crate) fn canonical_model(model: &str) -> String {
    let mut model = model.trim().to_string();
    if let Some(open) = model.rfind('[')
        && model.ends_with(']')
    {
        model.truncate(open);
    }
    model
}

/// Parse the `authors:` list of an `authors.yaml`.
pub(crate) fn roster(text: &str) -> Vec<Identity> {
    let Ok(value) = serde_yaml::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Some(authors) = value.get("authors").and_then(Value::as_array) else {
        return Vec::new();
    };
    authors
        .iter()
        .filter_map(Value::as_str)
        .filter_map(parse_identity)
        .collect()
}

fn parse_identity(line: &str) -> Option<Identity> {
    let (name, rest) = line.split_once(" <")?;
    let email = rest.strip_suffix('>')?;
    let (_, domain) = email.split_once('@')?;
    let open = name.rfind(" (")?;
    let model = name[open + 2..].strip_suffix(')')?;
    Some(Identity {
        name: name.to_string(),
        email: email.to_string(),
        model: model.to_string(),
        domain: domain.to_string(),
    })
}

/// The identity a harness and model resolve to in a roster: the listed
/// line whose model matches and whose domain names the harness, or the
/// first model match when no domain does.
pub(crate) fn resolve_identity(
    roster: &[Identity],
    harness: &str,
    model: &str,
) -> Option<Identity> {
    let model = canonical_model(model);
    let matches: Vec<&Identity> = roster.iter().filter(|i| i.model == model).collect();
    matches
        .iter()
        .find(|i| i.domain.starts_with(&format!("{harness}.")))
        .or_else(|| matches.first())
        .map(|i| (*i).clone())
}

fn find_roster(cwd: &Path) -> Option<PathBuf> {
    let mut dir = Some(cwd);
    while let Some(d) = dir {
        let candidate = d.join("authors.yaml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Write the session's model identity into the checkout, so every tool
/// shell of the session commits as the model. An unlisted model gets an
/// identity the attribution check refuses, never a listed one.
fn author_identity(
    payload: &HandlerPayload,
    deck_roster: Option<PathBuf>,
) -> Result<HandlerResult, String> {
    author_identity_in(
        payload,
        deck_roster,
        None,
        std::env::var("ANTHROPIC_MODEL").ok(),
    )
}

/// `config_home` replaces jj's config directory, where jj 0.45 keeps the
/// repository-scoped config; a test points it at a scratch directory.
/// `env_model` is `ANTHROPIC_MODEL` as the hook inherited it: Claude's
/// `SessionStart` payload carries no model, and the launch profile that
/// started the session exports that variable, so it is the next witness.
fn author_identity_in(
    payload: &HandlerPayload,
    deck_roster: Option<PathBuf>,
    config_home: Option<&Path>,
    env_model: Option<String>,
) -> Result<HandlerResult, String> {
    let harness = payload.harness.as_str();
    let from_env = env_model.filter(|m| payload.harness == Harness::Claude && !m.trim().is_empty());
    let Some(model) = payload
        .model
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .map(str::to_string)
        .or(from_env)
    else {
        return Ok(HandlerResult {
            context: Vec::new(),
            ..HandlerResult::default()
        }
        .with_warning(
            "the payload names no model and ANTHROPIC_MODEL is unset; identity not written",
        ));
    };
    let model = model.as_str();
    let Some(cwd) = payload
        .cwd
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
    else {
        return Ok(HandlerResult::default()
            .with_warning("no checkout directory in the payload; identity not written"));
    };
    let roster_path = find_roster(&cwd).or(deck_roster);
    let listed = roster_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| roster(&text))
        .unwrap_or_default();
    let (name, email, warning) = if let Some(identity) = resolve_identity(&listed, harness, model) {
        (identity.name, identity.email, None)
    } else {
        {
            let id = canonical_model(model);
            (
                format!("Unlisted {harness} model ({id})"),
                format!("unlisted@{harness}.noreply.nexus.local"),
                Some(format!(
                    "model `{id}` is listed in no authors.yaml ({}); the checkout carries an unlisted identity the push gate refuses",
                    roster_path
                        .map_or_else(|| "none found".to_string(), |p| p.display().to_string())
                )),
            )
        }
    };
    write_checkout_identity(&cwd, &name, &email, config_home)?;
    let mut result = HandlerResult::default();
    if let Some(warning) = warning {
        result = result.with_warning(&warning);
    }
    Ok(result)
}

/// The files a write tool touched: `file_path` or `path` in the input,
/// or the `*** Add File:` and `*** Update File:` lines of a Codex patch.
pub(crate) fn written_paths(tool: &super::payload::ToolPayload) -> Vec<PathBuf> {
    if let Some(path) = tool.input["file_path"]
        .as_str()
        .or_else(|| tool.input["path"].as_str())
    {
        return vec![PathBuf::from(path)];
    }
    let patch = ["patch", "input", "command"]
        .iter()
        .find_map(|key| tool.input.get(*key).and_then(|v| v.as_str()))
        .unwrap_or_default();
    patch
        .lines()
        .filter_map(|line| {
            line.strip_prefix("*** Add File: ")
                .or_else(|| line.strip_prefix("*** Update File: "))
        })
        .map(|p| PathBuf::from(p.trim()))
        .collect()
}

/// One line that says what a tool call is about, for the dispatcher log:
/// the command, the file written, or the first file of a patch.
pub(crate) fn tool_subject(tool: &super::payload::ToolPayload) -> String {
    let subject = tool.input["command"]
        .as_str()
        .filter(|c| !c.starts_with("*** Begin Patch"))
        .map(str::to_string)
        .or_else(|| written_paths(tool).first().map(|p| p.display().to_string()))
        .unwrap_or_default();
    let line = subject.lines().next().unwrap_or_default();
    line.chars()
        .take(160)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// jj repository config, and git config when the checkout is colocated.
pub(crate) fn write_checkout_identity(
    cwd: &Path,
    name: &str,
    email: &str,
    config_home: Option<&Path>,
) -> Result<(), String> {
    let has_jj = cwd.join(".jj").is_dir() || jj_root(cwd).is_some();
    if !has_jj && !cwd.join(".git").exists() {
        return Err(format!("{} is not a jj or git checkout", cwd.display()));
    }
    if has_jj {
        for (key, value) in [("user.name", name), ("user.email", email)] {
            let mut command = Command::new("jj");
            command
                .args(["config", "set", "--repo", key, value])
                .current_dir(cwd);
            if let Some(home) = config_home {
                command.env("XDG_CONFIG_HOME", home);
            }
            let status = command
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| format!("cannot run jj: {e}"))?;
            if !status.status.success() {
                return Err(format!(
                    "jj config set {key} failed: {}",
                    String::from_utf8_lossy(&status.stderr).trim()
                ));
            }
        }
    }
    if cwd.join(".git").exists() {
        for (key, value) in [("user.name", name), ("user.email", email)] {
            let status = Command::new("git")
                .args(["config", key, value])
                .current_dir(cwd)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| format!("cannot run git: {e}"))?;
            if !status.status.success() {
                return Err(format!(
                    "git config {key} failed: {}",
                    String::from_utf8_lossy(&status.stderr).trim()
                ));
            }
        }
    }
    Ok(())
}

fn jj_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("jj")
        .args(["root"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

impl HandlerResult {
    /// A warning travels as context on events that carry it and as a
    /// stderr line otherwise; the dispatcher decides.
    fn with_warning(mut self, warning: &str) -> Self {
        self.context.push(format!("rune hook: {warning}"));
        self
    }
}

// ------------------------------------------------------------- guards ----

/// The Claude-shaped payload the guard tools read on stdin.
fn claude_shaped(payload: &HandlerPayload) -> Value {
    let mut raw = payload.raw.clone();
    if let Some(tool) = &payload.tool
        && let Some(map) = raw.as_object_mut()
    {
        map.insert("tool_name".into(), json!(tool.name));
        map.insert("tool_input".into(), tool.input.clone());
        if let Some(id) = &tool.id {
            map.insert("tool_use_id".into(), json!(id));
        }
    }
    if let Some(map) = raw.as_object_mut() {
        map.entry("hook_event_name")
            .or_insert_with(|| json!(payload.native_event));
        map.entry("session_id")
            .or_insert_with(|| json!(payload.session_id));
        map.entry("cwd").or_insert_with(|| json!(payload.cwd));
    }
    raw
}

fn run_tool(
    program: &str,
    args: &[&str],
    stdin: &[u8],
    cwd: Option<&Path>,
) -> Result<(Option<i32>, String, String), String> {
    use std::io::Write as _;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    if let Some(mut pipe) = child.stdin.take() {
        let _ = pipe.write_all(stdin);
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("{program} did not finish: {e}"))?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

/// dcg on a shell command: exit 2 or a deny decision is a denial.
fn dcg(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(tool) = &payload.tool else {
        return Ok(HandlerResult::default());
    };
    if !matches!(
        tool.name.as_str(),
        "Bash" | "shell" | "run_shell_command" | "bash" | "run_command"
    ) {
        return Ok(HandlerResult::default());
    }
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let (code, stdout, stderr) =
        run_tool("dcg", &[], &stdin, payload.cwd.as_deref().map(Path::new))?;
    if code == Some(2) {
        return Ok(HandlerResult::deny(stderr.trim().to_string()));
    }
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim())
        && value["hookSpecificOutput"]["permissionDecision"] == "deny"
    {
        let reason = value["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("denied by dcg")
            .to_string();
        return Ok(HandlerResult::deny(reason));
    }
    if code != Some(0) {
        return Err(format!("dcg exited {code:?}: {}", stderr.trim()));
    }
    Ok(HandlerResult::default())
}

/// git-ai checkpoints the working tree around edits; it never steers.
fn git_ai(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let preset = payload.harness.as_str();
    let stdin = serde_json::to_vec(&payload.raw).map_err(|e| e.to_string())?;
    let program = dirs::home_dir()
        .map(|h| h.join(".git-ai/bin/git-ai"))
        .filter(|p| p.is_file())
        .map_or_else(|| "git-ai".to_string(), |p| p.display().to_string());
    let (code, _, stderr) = run_tool(
        &program,
        &["checkpoint", preset, "--hook-input", "stdin"],
        &stdin,
        payload.cwd.as_deref().map(Path::new),
    )?;
    if code != Some(0) {
        return Err(format!("git-ai exited {code:?}: {}", stderr.trim()));
    }
    Ok(HandlerResult::default())
}

/// rtk rewrites a shell command for a smaller output; its `updatedInput`
/// becomes the replacement input.
fn rtk(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(tool) = &payload.tool else {
        return Ok(HandlerResult::default());
    };
    if !matches!(
        tool.name.as_str(),
        "Bash" | "shell" | "run_shell_command" | "bash"
    ) {
        return Ok(HandlerResult::default());
    }
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let (code, stdout, stderr) = run_tool(
        "rtk",
        &["hook", "claude"],
        &stdin,
        payload.cwd.as_deref().map(Path::new),
    )?;
    if code != Some(0) {
        return Err(format!("rtk exited {code:?}: {}", stderr.trim()));
    }
    let mut result = HandlerResult::default();
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim())
        && let Some(input) = value["hookSpecificOutput"].get("updatedInput")
    {
        result.input = Some(input.clone());
    }
    Ok(result)
}

/// The Markdown linters on a file an agent wrote: rumdl and typos
/// findings and Vale errors deny, Vale warnings and suggestions travel as
/// context.
#[allow(clippy::unnecessary_wraps)] // every adapter has the one signature the table calls
fn lint_on_write(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    Ok(lint_on_write_in(payload, true))
}

/// A markdown file under a scratch directory is skipped when `skip_scratch`
/// holds; a test that itself runs under one turns that off.
fn lint_on_write_in(payload: &HandlerPayload, skip_scratch: bool) -> HandlerResult {
    let Some(tool) = &payload.tool else {
        return HandlerResult::default();
    };
    if !matches!(
        tool.name.as_str(),
        "Edit" | "Write" | "MultiEdit" | "write_file" | "edit_file" | "apply_patch"
    ) {
        return HandlerResult::default();
    }
    let cwd = payload.cwd.as_deref().map(PathBuf::from);
    let paths: Vec<PathBuf> = written_paths(tool)
        .into_iter()
        .map(|p| match (&cwd, p.is_absolute()) {
            (Some(cwd), false) => cwd.join(p),
            _ => p,
        })
        .filter(|path| {
            let text = path.display().to_string();
            let scratch = text.contains("/.claude/projects/")
                || text.starts_with("/tmp/")
                || text.starts_with("/private/tmp/")
                || text.starts_with("/var/folders/");
            path.extension().and_then(|e| e.to_str()) == Some("md")
                && !(skip_scratch && scratch)
                && path.is_file()
        })
        .collect();
    let Some(path) = paths.into_iter().next() else {
        return HandlerResult::default();
    };
    let dir = path.parent().map(Path::to_path_buf);
    let file = path.display().to_string();
    let mut blocking = String::new();
    let mut advice = String::new();
    for (program, args) in [
        ("rumdl", vec!["check", file.as_str()]),
        ("typos", vec!["--force-exclude", file.as_str()]),
    ] {
        // A linter that is not installed here is skipped.
        if let Ok((code, out, err)) = run_tool(program, &args, &[], dir.as_deref())
            && code != Some(0)
        {
            blocking.push_str(out.trim());
            blocking.push_str(err.trim());
            blocking.push('\n');
        }
    }
    if let Ok((_, out, _)) = run_tool(
        "vale",
        &["--minAlertLevel=suggestion", "--output=JSON", file.as_str()],
        &[],
        dir.as_deref(),
    ) && let Ok(findings) = serde_json::from_str::<Value>(out.trim())
        && let Some(map) = findings.as_object()
        && !map.contains_key("Code")
    {
        let mut errors = 0;
        let mut lines = Vec::new();
        for list in map.values().filter_map(Value::as_array) {
            for f in list {
                let severity = f["Severity"].as_str().unwrap_or("");
                if severity == "error" {
                    errors += 1;
                }
                lines.push(format!(
                    "{file}:{}:{} {severity} {}: {}",
                    f["Line"],
                    f["Span"][0],
                    f["Check"].as_str().unwrap_or(""),
                    f["Message"].as_str().unwrap_or("")
                ));
            }
        }
        let joined = lines.join("\n");
        if errors > 0 {
            blocking.push_str(&joined);
            blocking.push('\n');
        } else if !joined.is_empty() {
            advice = joined;
        }
    }
    if !blocking.trim().is_empty() {
        let mut result = HandlerResult::deny(blocking.trim().to_string());
        if !advice.is_empty() {
            result.context.push(advice);
        }
        return result;
    }
    let mut result = HandlerResult::default();
    if !advice.is_empty() {
        result.context.push(format!(
            "Vale advice for the file you wrote, not a block. Rewrite a flagged sentence when the meaning survives it, keep the wording when it does not.\n{advice}"
        ));
    }
    result
}

/// The session's capture, detached: the launcher never waits on it.
fn session_capture(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let program = dirs::home_dir()
        .map(|h| h.join(".local/bin/session-sync"))
        .filter(|p| p.is_file())
        .map_or_else(|| "session-sync".to_string(), |p| p.display().to_string());
    let mut args: Vec<OsString> = vec![
        "--provider".into(),
        payload.harness.as_str().into(),
        "--source".into(),
        "hook".into(),
    ];
    if let Some(session) = &payload.session_id {
        args.push("--session".into());
        args.push(session.into());
    }
    let mut command = Command::new(&program);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = payload.cwd.as_deref().map(Path::new).filter(|p| p.is_dir()) {
        command.current_dir(cwd);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    command
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    Ok(HandlerResult::default())
}

fn sd_script(relative: &str) -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".sd").join(relative))
        .filter(|p| p.is_file())
}

/// The prompt checkpoint for turn-scoped review, by event.
fn turn_checkpoint(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let mode = match payload.event.as_str() {
        "prompt.before" => "begin",
        "turn.finish" => "snapshot",
        "session.end" => "clear",
        _ => return Ok(HandlerResult::default()),
    };
    let Some(script) = sd_script("claude/turn/checkpoint") else {
        return Ok(HandlerResult::default());
    };
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let _ = run_tool(
        "sh",
        &[&script.display().to_string(), mode],
        &stdin,
        payload.cwd.as_deref().map(Path::new),
    )?;
    Ok(HandlerResult::default())
}

/// The tmux window flag, by event.
fn tmux_status(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let state = match payload.event.as_str() {
        "prompt.before" => "working",
        "turn.finish" | "notification" => "attention",
        "session.end" => "clear",
        _ => return Ok(HandlerResult::default()),
    };
    let Some(program) = dirs::home_dir()
        .map(|h| h.join(".local/bin/claude-tmux-status"))
        .filter(|p| p.is_file())
    else {
        return Ok(HandlerResult::default());
    };
    let stdin = serde_json::to_vec(&claude_shaped(payload)).map_err(|e| e.to_string())?;
    let _ = run_tool(&program.display().to_string(), &[state], &stdin, None)?;
    Ok(HandlerResult::default())
}

/// The three jj guards: no git worktree, no worktree-isolated agent, no
/// file tool inside `.git` or `.jj`.
#[allow(clippy::unnecessary_wraps)] // every adapter has the one signature the table calls
fn jj_guards(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(tool) = &payload.tool else {
        return Ok(HandlerResult::default());
    };
    let cwd = payload.cwd.as_deref().map(Path::new);
    let colocated =
        || cwd.is_some_and(|c| git_root(c).is_some_and(|root| root.join(".jj").is_dir()));
    match tool.name.as_str() {
        "EnterWorktree" if colocated() => Ok(HandlerResult::deny(
            "jj-colocated repo: git worktrees conflict with jj. Use jj workspace add/forget instead (VersionControl/Jujutsu.md, JujutsuToolkit).",
        )),
        "Agent" if tool.input["isolation"] == "worktree" && colocated() => Ok(HandlerResult::deny(
            "jj-colocated repo: worktree-isolated agents conflict with jj. Spawn without isolation and give each agent its own jj workspace (jj workspace add; see JujutsuToolkit).",
        )),
        "Edit" | "Write" | "NotebookEdit" | "MultiEdit" => {
            let path = tool.input["file_path"]
                .as_str()
                .or_else(|| tool.input["notebook_path"].as_str())
                .unwrap_or("");
            let wrapped = format!("/{path}/");
            if wrapped.contains("/.git/") || wrapped.contains("/.jj/") {
                return Ok(HandlerResult::deny(
                    "Path is inside .git/.jj: file tools must not touch VCS internals. Drive changes through git or jj commands instead.",
                ));
            }
            Ok(HandlerResult::default())
        }
        _ => Ok(HandlerResult::default()),
    }
}

fn git_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

#[allow(dead_code)]
fn effect_name(effect: Effect) -> &'static str {
    match effect {
        Effect::Pass => "pass",
        Effect::Deny => "deny",
        Effect::ContinueTurn => "continue_turn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::hooks::Harness;

    fn payload(
        harness: Harness,
        event: &str,
        tool: Option<(&str, Value)>,
        cwd: &str,
        model: Option<&str>,
    ) -> HandlerPayload {
        HandlerPayload {
            v: 1,
            harness,
            event: event.to_string(),
            native_event: event.to_string(),
            session_id: Some("s".into()),
            turn_id: None,
            cwd: Some(cwd.to_string()),
            transcript_path: None,
            model: model.map(str::to_string),
            tool: tool.map(|(name, input)| super::super::payload::ToolPayload {
                name: name.to_string(),
                id: None,
                input,
                response: None,
            }),
            prompt: None,
            stop_active: None,
            raw: json!({}),
        }
    }

    const ROSTER: &str = "authors:\n    - Martin Zeman <N4M3Z@users.noreply.github.com>\n    - Claude Fable 5.1 (claude-fable-5-1) <claude-fable-5-1@claude.noreply.nexus.local>\n    - Codex Gpt 6 Astra (gpt-6-astra) <gpt-6-astra@codex.noreply.nexus.local>\n";

    #[test]
    fn the_roster_resolves_a_model_by_harness_domain() {
        let list = roster(ROSTER);
        assert_eq!(list.len(), 2, "a human line has no model");
        let fable = resolve_identity(&list, "claude", "claude-fable-5-1[1m]").unwrap();
        assert_eq!(fable.name, "Claude Fable 5.1 (claude-fable-5-1)");
        assert_eq!(fable.email, "claude-fable-5-1@claude.noreply.nexus.local");
        assert!(resolve_identity(&list, "codex", "gpt-6-luna").is_none());
    }

    fn config_home() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn jj_get(dir: &Path, home: &Path, key: &str) -> String {
        let out = Command::new("jj")
            .args(["config", "get", key])
            .current_dir(dir)
            .env("XDG_CONFIG_HOME", home)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn jj_checkout() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let ok = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(dir.path())
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "git init");
        let ok = Command::new("jj")
            .args(["git", "init", "--colocate"])
            .current_dir(dir.path())
            .env("JJ_CONFIG", "")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "jj init");
        dir
    }

    #[test]
    fn a_listed_model_is_written_into_the_checkout_and_an_unlisted_one_is_loud() {
        let home = config_home();
        let dir = jj_checkout();
        std::fs::write(dir.path().join("authors.yaml"), ROSTER).unwrap();
        let cwd = dir.path().display().to_string();
        let result = author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &cwd,
                Some("claude-fable-5-1[1m]"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert!(result.context.is_empty(), "{result:?}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Claude Fable 5.1 (claude-fable-5-1)"
        );
        let git = Command::new("git")
            .args(["config", "user.email"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&git.stdout).trim(),
            "claude-fable-5-1@claude.noreply.nexus.local"
        );

        let result = author_identity_in(
            &payload(
                Harness::Codex,
                "session.start",
                None,
                &cwd,
                Some("gpt-6-luna"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert!(result.context[0].contains("gpt-6-luna"), "{result:?}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Unlisted codex model (gpt-6-luna)"
        );
    }

    #[test]
    fn two_checkouts_keep_their_own_identity() {
        let home = config_home();
        let a = jj_checkout();
        let b = jj_checkout();
        std::fs::write(a.path().join("authors.yaml"), ROSTER).unwrap();
        std::fs::write(b.path().join("authors.yaml"), ROSTER).unwrap();
        author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &a.path().display().to_string(),
                Some("claude-fable-5-1"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        author_identity_in(
            &payload(
                Harness::Codex,
                "session.start",
                None,
                &b.path().display().to_string(),
                Some("gpt-6-astra"),
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert_eq!(
            jj_get(a.path(), home.path(), "user.email"),
            "claude-fable-5-1@claude.noreply.nexus.local"
        );
        assert_eq!(
            jj_get(b.path(), home.path(), "user.email"),
            "gpt-6-astra@codex.noreply.nexus.local"
        );
    }

    #[test]
    fn a_payload_without_a_model_writes_nothing() {
        let home = config_home();
        let dir = jj_checkout();
        let result = author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &dir.path().display().to_string(),
                None,
            ),
            None,
            Some(home.path()),
            None,
        )
        .unwrap();
        assert!(result.context[0].contains("names no model"));
        assert!(!jj_get(dir.path(), home.path(), "user.name").contains("Unlisted"));
        // Claude's SessionStart carries no model; the launch profile's
        // variable stands in for it.
        std::fs::write(dir.path().join("authors.yaml"), ROSTER).unwrap();
        let result = author_identity_in(
            &payload(
                Harness::Claude,
                "session.start",
                None,
                &dir.path().display().to_string(),
                None,
            ),
            None,
            Some(home.path()),
            Some("claude-fable-5-1".to_string()),
        )
        .unwrap();
        assert!(result.context.is_empty(), "{result:?}");
        assert_eq!(
            jj_get(dir.path(), home.path(), "user.name"),
            "Claude Fable 5.1 (claude-fable-5-1)"
        );
    }

    #[test]
    fn a_codex_patch_names_the_files_it_writes() {
        let patch = "*** Begin Patch\n*** Add File: NOTE.md\n+#Bad\n*** Update File: docs/a.md\n+x\n*** End Patch";
        let tool = super::super::payload::ToolPayload {
            name: "apply_patch".to_string(),
            id: None,
            input: json!({"command": patch}),
            response: None,
        };
        assert_eq!(
            written_paths(&tool),
            vec![PathBuf::from("NOTE.md"), PathBuf::from("docs/a.md")]
        );
        assert_eq!(tool_subject(&tool), "NOTE.md");
        let bash = super::super::payload::ToolPayload {
            name: "Bash".to_string(),
            id: None,
            input: json!({"command": "git status\n--short"}),
            response: None,
        };
        assert_eq!(tool_subject(&bash), "git status");
    }

    #[test]
    fn the_jj_guards_refuse_the_three_cases() {
        let dir = jj_checkout();
        let cwd = dir.path().display().to_string();
        let deny = |name: &str, input: Value| {
            jj_guards(&payload(
                Harness::Claude,
                "tool.before",
                Some((name, input)),
                &cwd,
                None,
            ))
            .unwrap()
            .effect
        };
        assert_eq!(deny("EnterWorktree", json!({})), Effect::Deny);
        assert_eq!(
            deny("Agent", json!({"isolation": "worktree"})),
            Effect::Deny
        );
        assert_eq!(deny("Agent", json!({})), Effect::Pass);
        assert_eq!(
            deny("Edit", json!({"file_path": format!("{cwd}/.jj/repo/x")})),
            Effect::Deny
        );
        assert_eq!(
            deny("Edit", json!({"file_path": format!("{cwd}/src/x.rs")})),
            Effect::Pass
        );
        assert_eq!(deny("Bash", json!({"command": "ls"})), Effect::Pass);
    }

    #[test]
    fn lint_on_write_ignores_other_files_and_flags_a_bad_markdown() {
        // The scratch skip is off: a validation clone runs under /tmp.
        let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let cwd = dir.path().display().to_string();
        let rust = dir.path().join("x.rs");
        std::fs::write(&rust, "fn main() {}\n").unwrap();
        let result = lint_on_write(&payload(
            Harness::Claude,
            "tool.after",
            Some(("Write", json!({"file_path": rust.display().to_string()}))),
            &cwd,
            None,
        ))
        .unwrap();
        assert_eq!(result.effect, Effect::Pass);
        if run_tool("rumdl", &["--version"], &[], None).is_err() {
            return; // no linter on this machine
        }
        let md = dir.path().join("note.md");
        std::fs::write(&md, "#Bad heading\n\nline with trailing space   \n").unwrap();
        let result = lint_on_write_in(
            &payload(
                Harness::Claude,
                "tool.after",
                Some(("Write", json!({"file_path": md.display().to_string()}))),
                &cwd,
                None,
            ),
            false,
        );
        assert_eq!(result.effect, Effect::Deny, "{result:?}");
    }
}
