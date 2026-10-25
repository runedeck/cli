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
    "sandbox-guard",
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
        "sandbox-guard" => super::sandbox_guard::guard(payload, args),
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
    let notes = write_checkout_identity(&cwd, &name, &email, config_home)?;
    let mut result = HandlerResult::default();
    if let Some(warning) = warning {
        result = result.with_warning(&warning);
    }
    for note in &notes {
        result = result.with_warning(note);
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

/// The checkout the adapter writes. `config_home`, when a test gives it,
/// replaces jj's config directory and git's global config, so no test
/// reads or writes the owner's files.
struct Checkout<'a> {
    cwd: &'a Path,
    config_home: Option<&'a Path>,
}

impl Checkout<'_> {
    fn jj(&self, args: &[&str]) -> Command {
        let mut command = Command::new("jj");
        command.args(args).current_dir(self.cwd);
        if let Some(home) = self.config_home {
            command.env("XDG_CONFIG_HOME", home);
        }
        command
    }

    fn git(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command.args(args).current_dir(self.cwd);
        if let Some(home) = self.config_home {
            command.env("GIT_CONFIG_GLOBAL", home.join("git").join("config"));
        }
        command
    }

    /// `git config` with `args`: the value, `None` when the key is unset
    /// (exit 1), and the stderr as the error otherwise.
    fn git_get(&self, args: &[&str]) -> Result<Option<String>, String> {
        let output = self
            .git(&[&["config"], args].concat())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        match output.status.code() {
            Some(0) => Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            )),
            Some(1) => Ok(None),
            _ => Err(format!(
                "git config {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        }
    }
}

/// Run a jj or git command: its stdout on success, its stderr as the
/// error otherwise.
fn capture(mut command: Command) -> Result<String, String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// The jj config of the workspace the session runs in (jj 0.46 keeps it
/// under the user's config directory, one file per workspace, so a second
/// workspace of the repository never reads it), the author of that
/// workspace's own working-copy commit, and git's config for that
/// worktree when the checkout is colocated. The git write needs
/// `extensions.worktreeConfig`. When git cannot take it, no git identity
/// is written, because the only file left is the shared `.git/config`,
/// and the returned notes say so.
pub(crate) fn write_checkout_identity(
    cwd: &Path,
    name: &str,
    email: &str,
    config_home: Option<&Path>,
) -> Result<Vec<String>, String> {
    let checkout = Checkout { cwd, config_home };
    let has_jj = cwd.join(".jj").is_dir() || jj_root(cwd).is_some();
    let has_git = cwd.join(".git").exists();
    if !has_jj && !has_git {
        return Err(format!("{} is not a jj or git checkout", cwd.display()));
    }
    let pairs = [("user.name", name), ("user.email", email)];
    let mut notes = Vec::new();
    if has_jj {
        for (key, value) in pairs {
            capture(checkout.jj(&["config", "set", "--workspace", key, value]))
                .map_err(|e| format!("jj config set {key} failed: {e}"))?;
        }
        notes.extend(adopt_working_copy_author(&checkout)?);
    }
    if has_git {
        match git_identity_scope(&checkout)? {
            GitScope::Worktree => {
                for (key, value) in pairs {
                    capture(checkout.git(&["config", "--worktree", key, value]))
                        .map_err(|e| format!("git config --worktree {key} failed: {e}"))?;
                }
            }
            GitScope::Shared(why) => notes.push(format!(
                "{why}. No git identity was written: the one file left is the shared .git/config, which every worktree of the repository reads. A raw git commit here carries the global git identity; a jj commit carries the session's"
            )),
        }
    }
    Ok(notes)
}

/// jj sets a commit's author when it creates the commit, so a working-copy
/// commit made before the session picked its identity keeps the earlier
/// author through `jj commit`. The workspace's own working-copy commit
/// takes the identity now while it is undescribed. A described one keeps
/// its author and is named in the note; another workspace's commit is
/// never touched, because `@` is this workspace's alone.
fn adopt_working_copy_author(checkout: &Checkout<'_>) -> Result<Vec<String>, String> {
    let shown = capture(checkout.jj(&[
        "log",
        "-r",
        "@",
        "--no-graph",
        "-T",
        r#"change_id.short() ++ " " ++ if(description, "described", "undescribed")"#,
    ]))
    .map_err(|e| format!("jj log @ failed: {e}"))?;
    let Some((change, state)) = shown.trim().split_once(' ') else {
        return Err(format!("jj log @ printed `{}`", shown.trim()));
    };
    if state == "described" {
        return Ok(vec![format!(
            "the working-copy commit {change} is described and keeps its author"
        )]);
    }
    match capture(checkout.jj(&["metaedit", "--update-author", "-r", "@"])) {
        Ok(_) => Ok(Vec::new()),
        Err(error) => Ok(vec![format!(
            "the working-copy commit {change} keeps its author: jj metaedit failed: {error}"
        )]),
    }
}

/// Where git keeps a session identity for this worktree.
enum GitScope {
    /// `git config --worktree` reaches this worktree's own file.
    Worktree,
    /// Only the shared `.git/config` is left, for the reason given.
    Shared(String),
}

/// `--worktree` once `extensions.worktreeConfig` is on in the repository
/// config, turned on here when git allows it. A global
/// `extensions.worktreeConfig` is ignored by git, so it is read with
/// `--local`. git-config(1) says a `core.worktree`, or a `core.bare` that
/// is true, has to move into the main worktree's `config.worktree` by
/// hand first. And git ignores every `config.worktree` while the extension
/// is off, so one that already sets either key would apply the moment it
/// turns on: a dormant `core.bare = true` broke `git status` in review.
/// With either, the answer names the cause and the caller writes no git
/// identity.
fn git_identity_scope(checkout: &Checkout<'_>) -> Result<GitScope, String> {
    if checkout
        .git_get(&[
            "--local",
            "--type=bool",
            "--get",
            "extensions.worktreeConfig",
        ])?
        .as_deref()
        == Some("true")
    {
        return Ok(GitScope::Worktree);
    }
    if checkout
        .git_get(&["--type=bool", "--get", "core.bare"])?
        .as_deref()
        == Some("true")
        || checkout.git_get(&["--get", "core.worktree"])?.is_some()
    {
        return Ok(GitScope::Shared(
            "git keeps one config for every worktree of this repository: core.bare or core.worktree is set, and extensions.worktreeConfig needs it moved into config.worktree by hand first".to_string(),
        ));
    }
    for file in worktree_config_files(checkout)? {
        if let Some(fault) = work_tree_fault(checkout, &file) {
            return Ok(GitScope::Shared(format!(
                "{} {fault}, and git would read it the moment extensions.worktreeConfig turns on, so the extension stays off",
                file.display()
            )));
        }
    }
    capture(checkout.git(&["config", "--local", "extensions.worktreeConfig", "true"]))
        .map_err(|e| format!("git config --local extensions.worktreeConfig failed: {e}"))?;
    Ok(GitScope::Worktree)
}

/// Every `config.worktree` git reads once the extension is on: the main
/// worktree's under the common dir, then one per linked worktree.
fn worktree_config_files(checkout: &Checkout<'_>) -> Result<Vec<PathBuf>, String> {
    let common =
        capture(checkout.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]))
            .map_err(|e| format!("git rev-parse --git-common-dir failed: {e}"))?;
    let common = PathBuf::from(common.trim());
    let mut files = vec![common.join("config.worktree")];
    if let Ok(entries) = std::fs::read_dir(common.join("worktrees")) {
        let mut linked: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path().join("config.worktree"))
            .collect();
        linked.sort();
        files.extend(linked);
    }
    Ok(files.into_iter().filter(|file| file.exists()).collect())
}

/// Why a dormant `config.worktree` keeps the extension off: it sets
/// `core.bare` or `core.worktree`, the two keys git-config(1) names as
/// moving where git finds the work tree, or git cannot read it. `None`
/// when it is safe to wake.
fn work_tree_fault(checkout: &Checkout<'_>, file: &Path) -> Option<String> {
    let output = checkout
        .git(&[
            "config",
            "--file",
            &file.display().to_string(),
            "--list",
            "--name-only",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .find(|name| *name == "core.bare" || *name == "core.worktree")
            .map(|name| format!("sets {name}")),
        Ok(output) => Some(format!(
            "cannot be read ({})",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) => Some(format!("cannot be read ({error})")),
    }
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

/// The command line of a shell tool call, under every name the harnesses
/// give their shell tool: `Bash` (Claude, Grok, and Codex hooks),
/// `run_shell_command` (Gemini), `run_command` (Antigravity), and Codex's
/// own `shell` (an argument list), `shell_command`, and `exec_command`
/// (whose command is `cmd`). Any other tool has none.
pub(crate) fn shell_command(tool: &super::payload::ToolPayload) -> Option<String> {
    if !matches!(
        tool.name.as_str(),
        "Bash"
            | "bash"
            | "shell"
            | "shell_command"
            | "exec_command"
            | "run_shell_command"
            | "run_command"
    ) {
        return None;
    }
    ["command", "cmd"]
        .iter()
        .find_map(|key| match tool.input.get(*key)? {
            Value::String(line) => Some(line.clone()),
            Value::Array(words) => {
                let words: Option<Vec<String>> = words
                    .iter()
                    .map(|w| w.as_str().map(crate::cli::shell_quote))
                    .collect();
                words.map(|w| w.join(" "))
            }
            _ => None,
        })
}

/// The payload dcg reads: Claude's shape, with every shell tool named
/// `Bash` and its command line under `tool_input.command`.
fn dcg_payload(payload: &HandlerPayload) -> Option<Value> {
    let tool = payload.tool.as_ref()?;
    let command = shell_command(tool)?;
    let mut shaped = claude_shaped(payload);
    if let Some(map) = shaped.as_object_mut() {
        map.insert("tool_name".into(), json!("Bash"));
        map.insert("tool_input".into(), json!({ "command": command }));
    }
    Some(shaped)
}

/// dcg on a shell command: exit 2 or a deny decision is a denial.
fn dcg(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let Some(shaped) = dcg_payload(payload) else {
        return Ok(HandlerResult::default());
    };
    let stdin = serde_json::to_vec(&shaped).map_err(|e| e.to_string())?;
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
    let metrics = Some(lint_metrics(&blocking, &advice));
    if !blocking.trim().is_empty() {
        let mut result = HandlerResult::deny(blocking.trim().to_string());
        if !advice.is_empty() {
            result.context.push(advice);
        }
        result.metrics = metrics;
        return result;
    }
    let mut result = HandlerResult {
        metrics,
        ..HandlerResult::default()
    };
    if !advice.is_empty() {
        result.context.push(format!(
            "Vale advice for the file you wrote, not a block. Rewrite a flagged sentence when the meaning survives it, keep the wording when it does not.\n{advice}"
        ));
    }
    result
}

/// The hook metrics of a lint run: rule counts from everything reported,
/// and the causes from the lines that deny. Counts only, no text.
fn lint_metrics(blocking: &str, advice: &str) -> Value {
    let mut found = super::findings::Findings::parse(blocking);
    let mut advised = super::findings::Findings::parse(advice);
    advised.cause.clear();
    found.extend(advised);
    found.metrics()
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

/// The turn checkpoint script: the just tree first, where the dotfiles moved
/// it from `~/.sd`, then the old place for a home not yet applied. Install
/// names a legacy part that runs this script as the turn-checkpoint
/// handler's program.
pub(crate) fn checkpoint_script(home: &Path) -> Option<PathBuf> {
    [
        ".config/just/claude/turn/checkpoint",
        ".sd/claude/turn/checkpoint",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .find(|p| p.is_file())
}

/// The prompt checkpoint for turn-scoped review, by event.
fn turn_checkpoint(payload: &HandlerPayload) -> Result<HandlerResult, String> {
    let mode = match payload.event.as_str() {
        "prompt.before" => "begin",
        "turn.finish" => "snapshot",
        "session.end" => "clear",
        _ => return Ok(HandlerResult::default()),
    };
    let Some(script) = dirs::home_dir().and_then(|home| checkpoint_script(&home)) else {
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

    #[test]
    fn the_checkpoint_script_is_found_in_the_just_tree_first() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(checkpoint_script(home.path()), None);
        let place = |relative: &str| {
            let path = home.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "#!/bin/sh\n").unwrap();
            path
        };
        let legacy = place(".sd/claude/turn/checkpoint");
        assert_eq!(checkpoint_script(home.path()), Some(legacy));
        let just = place(".config/just/claude/turn/checkpoint");
        assert_eq!(checkpoint_script(home.path()), Some(just));
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
            .args(["init", "-q", "-b", "main", "--template="])
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
        assert_eq!(git_get(dir.path(), home.path(), "user.email"), FABLE);
        assert_eq!(
            jj_author(dir.path(), home.path()),
            FABLE,
            "the working-copy commit takes the identity"
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

    /// A git value as the adapter's git sees it: the scratch home stands
    /// in for the global config, so the owner's never leaks in.
    fn git_get(dir: &Path, home: &Path, key: &str) -> String {
        let out = Command::new("git")
            .args(["config", "--get", key])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", home.join("git").join("config"))
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The author email of the workspace's working-copy commit.
    fn jj_author(dir: &Path, home: &Path) -> String {
        let out = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "author.email()"])
            .current_dir(dir)
            .env("XDG_CONFIG_HOME", home)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The git worktree jj made for a workspace: the one entry under the
    /// main checkout's `.git/worktrees/`.
    fn linked_worktree_dir(main: &Path) -> PathBuf {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(main.join(".git/worktrees"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(entries.len(), 1, "{entries:?}");
        entries.remove(0)
    }

    fn write_identity(harness: Harness, dir: &Path, home: &Path, model: &str) -> HandlerResult {
        author_identity_in(
            &payload(
                harness,
                "session.start",
                None,
                &dir.display().to_string(),
                Some(model),
            ),
            None,
            Some(home),
            None,
        )
        .unwrap()
    }

    const FABLE: &str = "claude-fable-5-1@claude.noreply.nexus.local";
    const ASTRA: &str = "gpt-6-astra@codex.noreply.nexus.local";

    #[test]
    fn two_workspaces_of_one_repository_keep_their_own_identity() {
        let home = config_home();
        let a = jj_checkout();
        let parent = tempfile::tempdir().unwrap();
        let b = parent.path().join("b");
        let added = Command::new("jj")
            .args(["workspace", "add", "--colocate", &b.display().to_string()])
            .current_dir(a.path())
            .env("XDG_CONFIG_HOME", home.path())
            .env("JJ_CONFIG", "")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        assert!(added, "jj workspace add");
        assert!(b.join(".git").is_file(), "the workspace is a git worktree");
        std::fs::write(a.path().join("authors.yaml"), ROSTER).unwrap();
        std::fs::write(b.join("authors.yaml"), ROSTER).unwrap();

        let result = write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
        assert!(result.context.is_empty(), "{result:?}");
        assert_ne!(jj_get(&b, home.path(), "user.email"), FABLE, "b untouched");
        assert_ne!(
            git_get(&b, home.path(), "user.email"),
            FABLE,
            "b's git untouched"
        );
        assert_ne!(jj_author(&b, home.path()), FABLE, "b's commit untouched");

        // b's working-copy commit was made before b's session picked
        // Astra, so it carried the earlier author into `jj commit`.
        let result = write_identity(Harness::Codex, &b, home.path(), "gpt-6-astra");
        assert!(result.context.is_empty(), "{result:?}");
        assert_eq!(jj_get(a.path(), home.path(), "user.email"), FABLE);
        assert_eq!(jj_get(&b, home.path(), "user.email"), ASTRA);
        assert_eq!(jj_author(a.path(), home.path()), FABLE);
        assert_eq!(jj_author(&b, home.path()), ASTRA);
        assert_eq!(git_get(a.path(), home.path(), "user.email"), FABLE);
        assert_eq!(git_get(&b, home.path(), "user.email"), ASTRA);
        assert_eq!(
            git_get(a.path(), home.path(), "extensions.worktreeConfig"),
            "true",
            "git keeps the identity per worktree"
        );
        assert!(a.path().join(".git/config.worktree").is_file());
    }

    #[test]
    fn a_described_working_copy_commit_keeps_its_author_and_is_named() {
        let home = config_home();
        let dir = jj_checkout();
        std::fs::write(dir.path().join("authors.yaml"), ROSTER).unwrap();
        write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        assert_eq!(jj_author(dir.path(), home.path()), FABLE);
        let described = Command::new("jj")
            .args(["describe", "-m", "half done"])
            .current_dir(dir.path())
            .env("XDG_CONFIG_HOME", home.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        assert!(described, "jj describe");
        let result = write_identity(Harness::Codex, dir.path(), home.path(), "gpt-6-astra");
        assert_eq!(jj_get(dir.path(), home.path(), "user.email"), ASTRA);
        assert_eq!(
            jj_author(dir.path(), home.path()),
            FABLE,
            "described, left alone"
        );
        assert_eq!(result.context.len(), 1, "{result:?}");
        assert!(result.context[0].contains("is described"), "{result:?}");
    }

    #[test]
    fn a_core_worktree_keeps_the_git_identity_out_of_the_shared_config() {
        let home = config_home();
        let dir = jj_checkout();
        std::fs::write(dir.path().join("authors.yaml"), ROSTER).unwrap();
        let set = Command::new("git")
            .args(["config", "--local", "core.worktree", "."])
            .current_dir(dir.path())
            .status()
            .is_ok_and(|s| s.success());
        assert!(set, "git config core.worktree");
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        assert_eq!(result.context.len(), 1, "{result:?}");
        assert!(result.context[0].contains("core.worktree"), "{result:?}");
        assert!(
            result.context[0].contains("global git identity"),
            "{result:?}"
        );
        assert_eq!(git_get(dir.path(), home.path(), "user.email"), "");
        assert_eq!(git_get(dir.path(), home.path(), "user.name"), "");
        assert_eq!(
            git_get(dir.path(), home.path(), "extensions.worktreeConfig"),
            ""
        );
        assert_eq!(jj_get(dir.path(), home.path(), "user.email"), FABLE);
    }

    #[test]
    fn a_dormant_worktree_config_keeps_the_extension_off() {
        let home = config_home();
        let a = jj_checkout();
        let parent = tempfile::tempdir().unwrap();
        let b = parent.path().join("b");
        let added = Command::new("jj")
            .args(["workspace", "add", "--colocate", &b.display().to_string()])
            .current_dir(a.path())
            .env("XDG_CONFIG_HOME", home.path())
            .env("JJ_CONFIG", "")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        assert!(added, "jj workspace add");
        std::fs::write(a.path().join("authors.yaml"), ROSTER).unwrap();
        let git_status = |dir: &Path| {
            Command::new("git")
                .args(["status", "--short"])
                .current_dir(dir)
                .env("GIT_CONFIG_GLOBAL", home.path().join("git").join("config"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .code()
        };
        // git ignores both files while the extension is off, and would
        // read them the moment it turns on.
        let main = a.path().join(".git/config.worktree");
        let linked = linked_worktree_dir(a.path()).join("config.worktree");
        for (file, body, key) in [
            (&main, "[core]\n\tworktree = /elsewhere\n", "core.worktree"),
            (&linked, "[core]\n\tbare = true\n", "core.bare"),
        ] {
            std::fs::write(file, body).unwrap();
            let result = write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
            assert_eq!(result.context.len(), 1, "{result:?}");
            assert!(
                result.context[0].contains(&file.display().to_string()),
                "{result:?}"
            );
            assert!(result.context[0].contains(key), "{result:?}");
            assert_eq!(
                git_get(a.path(), home.path(), "extensions.worktreeConfig"),
                "",
                "the extension stays off"
            );
            assert_eq!(git_get(a.path(), home.path(), "user.email"), "");
            assert_eq!(jj_get(a.path(), home.path(), "user.email"), FABLE);
            assert_eq!(git_status(&b), Some(0), "b still works");
            std::fs::remove_file(file).unwrap();
        }
        let result = write_identity(Harness::Claude, a.path(), home.path(), "claude-fable-5-1");
        assert!(result.context.is_empty(), "{result:?}");
        assert_eq!(
            git_get(a.path(), home.path(), "extensions.worktreeConfig"),
            "true"
        );
        assert_eq!(git_get(a.path(), home.path(), "user.email"), FABLE);
        assert_eq!(git_status(&b), Some(0));
    }

    #[test]
    fn a_global_worktree_config_extension_is_not_taken_for_the_repository() {
        let home = config_home();
        let global = home.path().join("git");
        std::fs::create_dir_all(&global).unwrap();
        std::fs::write(
            global.join("config"),
            "[extensions]\n\tworktreeConfig = true\n",
        )
        .unwrap();
        let dir = jj_checkout();
        std::fs::write(dir.path().join("authors.yaml"), ROSTER).unwrap();
        let result = write_identity(Harness::Claude, dir.path(), home.path(), "claude-fable-5-1");
        assert!(result.context.is_empty(), "{result:?}");
        let local = Command::new("git")
            .args(["config", "--local", "--get", "extensions.worktreeConfig"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&local.stdout).trim(),
            "true",
            "the repository config carries the extension, not the global"
        );
        let worktree = std::fs::read_to_string(dir.path().join(".git/config.worktree"))
            .expect("the identity went to config.worktree");
        assert!(worktree.contains(FABLE), "{worktree}");
    }

    #[test]
    fn the_unlisted_identity_is_refused_by_the_push_gate() {
        if run_tool("python3", &["--version"], &[], None).is_err() {
            eprintln!("skipped: python3 is not installed");
            return;
        }
        let home = config_home();
        let dir = jj_checkout();
        let roster = dir.path().join("authors.yaml");
        std::fs::write(&roster, ROSTER).unwrap();
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/author-identity.py");
        let gate = |model: &str| {
            let name = jj_get(dir.path(), home.path(), "user.name");
            let email = jj_get(dir.path(), home.path(), "user.email");
            assert!(name.contains(model), "{name}");
            run_tool(
                "python3",
                &[
                    "-I",
                    script,
                    "validate",
                    "--policy",
                    &roster.display().to_string(),
                    "--identity",
                    &format!("{name} <{email}>"),
                ],
                &[],
                None,
            )
            .unwrap()
        };
        write_identity(Harness::Codex, dir.path(), home.path(), "gpt-6-luna");
        let (code, _, stderr) = gate("gpt-6-luna");
        assert_eq!(
            code,
            Some(1),
            "the gate refuses the unlisted model: {stderr}"
        );
        assert!(stderr.contains("must match"), "{stderr}");
        write_identity(Harness::Codex, dir.path(), home.path(), "gpt-6-astra");
        let (code, _, stderr) = gate("gpt-6-astra");
        assert_eq!(code, Some(0), "the gate accepts the listed model: {stderr}");
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
    }

    #[test]
    fn dcg_reads_every_shell_tool_as_one_bash_command() {
        let shaped = |name: &str, input: Value| {
            dcg_payload(&payload(
                Harness::Codex,
                "tool.before",
                Some((name, input)),
                "/w",
                None,
            ))
            .map(|v| (v["tool_name"].clone(), v["tool_input"]["command"].clone()))
        };
        let bash = (json!("Bash"), json!("git reset --hard"));
        assert_eq!(
            shaped("exec_command", json!({"cmd": "git reset --hard"})),
            Some(bash.clone())
        );
        assert_eq!(
            shaped("shell_command", json!({"command": "git reset --hard"})),
            Some(bash.clone())
        );
        assert_eq!(
            shaped("Bash", json!({"command": "git reset --hard"})),
            Some(bash)
        );
        assert_eq!(
            shaped(
                "shell",
                json!({"command": ["bash", "-lc", "git push --force origin main"]})
            ),
            Some((
                json!("Bash"),
                json!("bash -lc 'git push --force origin main'")
            ))
        );
        assert_eq!(shaped("apply_patch", json!({"command": "*** Begin"})), None);
        assert_eq!(shaped("exec_command", json!({})), None);
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
        // The metrics count the rumdl rules that denied and carry no text.
        let metrics = result.metrics.clone().expect("metrics");
        let rules = metrics["rules"].as_object().unwrap();
        assert!(rules.keys().any(|k| k.starts_with("MD")), "{metrics}");
        let cause: Vec<&str> = metrics["cause"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(cause.iter().all(|c| rules.contains_key(*c)), "{metrics}");
        assert!(!metrics.to_string().contains("Bad heading"));
        assert_eq!(super::super::record::validate_metrics(&metrics), Ok(()));
    }

    #[test]
    fn lint_on_write_reports_empty_metrics_for_a_clean_file() {
        // With or without the linters installed, a clean file yields empty metrics.
        let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let cwd = dir.path().display().to_string();
        let md = dir.path().join("clean.md");
        std::fs::write(&md, "# Clean\n\nOne plain sentence.\n").unwrap();
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
        assert_eq!(result.effect, Effect::Pass, "{result:?}");
        let metrics = result.metrics.clone().expect("metrics");
        assert_eq!(metrics["cause"], json!([]));
        assert_eq!(metrics["rules"], json!({}), "{result:?}");
    }
}
