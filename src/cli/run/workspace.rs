//! The jj workspace a workspace-write run works in. The harness changes
//! only its own copy of the repository, and the owner reviews the result as
//! one jj change on top of the caller's working-copy change, named after
//! the route and the session. An empty run leaves no trace.
//!
//! The workspace starts from a copy of the caller's files: a new commit on
//! the caller's nearest immutable ancestor, given the caller's working-copy
//! tree. Nothing the caller does rewrites an immutable commit, so the copy
//! and the workspace never go stale, whatever the caller squashes or
//! rebases meanwhile. Inside the workspace, jj's own immutability guard is
//! one more layer: it protects the copy and every other workspace's
//! working copy with its stack, so the harness commits, describes, and
//! squashes only its own revisions. The store is still shared, so the
//! runner records the caller's commits and the git settings that run
//! commands at the start and reports a change at the end. The run's change
//! is built from the workspace's final tree on the recorded copy, moved
//! onto the caller's current change, and only revisions the run made are
//! abandoned. A run state file in the rune state directory names the run,
//! its operations, and its phase, so a kill leaves a trail to clean up by.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The prefix of every workspace name the runner creates, so a leftover
/// after a SIGKILL is findable in `jj workspace list`.
const NAME_PREFIX: &str = "run-";

/// The configuration key of jj's immutability guard, set per workspace.
const GUARD_KEY: &str = "revset-aliases.\"immutable_heads()\"";

/// Where the caller's repository stands before an isolated run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Vcs {
    /// A jj repository, with the root of the caller's workspace and whether
    /// git shares it (a colocated repository).
    Jj { root: PathBuf, colocated: bool },
    /// A git repository without jj: the runner colocates jj at its root
    /// before it creates the workspace.
    GitOnly { root: PathBuf },
}

impl Vcs {
    pub(crate) fn root(&self) -> &Path {
        match self {
            Self::Jj { root, .. } | Self::GitOnly { root } => root,
        }
    }

    /// Whether the run has to colocate jj first.
    pub(crate) fn colocate(&self) -> bool {
        matches!(self, Self::GitOnly { .. })
    }

    /// Whether git shares the root, now or after the colocation.
    fn has_git(&self) -> bool {
        match self {
            Self::Jj { colocated, .. } => *colocated,
            Self::GitOnly { .. } => true,
        }
    }
}

/// One isolated run as planned: the dry run prints it, `begin` and `add`
/// create it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) vcs: Vcs,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    /// Where `--repo` sits under the caller's workspace root: the harness
    /// starts at the same subfolder of the run's workspace.
    pub(crate) subfolder: PathBuf,
    pub(crate) description: String,
    /// The change id of the caller's working-copy change, whose files the
    /// run copies at the start and whose top the result lands on at the
    /// end. `None` until jj is colocated.
    pub(crate) parent_change_id: Option<String>,
    /// The lock every run of this repository takes while it colocates jj
    /// or registers or removes a workspace.
    lock_path: PathBuf,
    /// The run state file, beside the workspace folder.
    state_path: PathBuf,
}

impl Plan {
    /// The folder the harness starts in: the workspace, at the subfolder
    /// that matches `--repo`.
    pub(crate) fn harness_path(&self) -> PathBuf {
        self.path.join(&self.subfolder)
    }
}

/// The caller's working-copy change as recorded at the start: what a
/// snapshot leaves alone. The tree is left out on purpose. The caller
/// keeps editing while the run lasts, and every edit rewrites the commit,
/// so the commit id would flag almost every run. A new description or a
/// new parent is somebody's jj command, and that is reported.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedHead {
    change_id: String,
    description: String,
    parents: Vec<String>,
}

/// How many recorded commits one revset query names: a full commit id is
/// 40 characters, so a batch stays far below any argument length limit.
const COMMIT_QUERY_BATCH: usize = 200;

/// The record template: the change id, the parent commit ids on one line,
/// then the description, which may span lines, so it comes last.
const HEAD_TEMPLATE: &str =
    "change_id ++ \"\\n\" ++ parents.map(|p| p.commit_id()).join(\" \") ++ \"\\n\" ++ description";

/// The copy of the caller's files the workspace starts from, by both ids:
/// the change id while it is visible, and the commit id, which resolves
/// whatever the harness does to the change.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CopyIds {
    change_id: String,
    commit_id: String,
}

/// A run's copy and workspace, from the moment the copy exists. `finish`
/// takes it down, whether or not the workspace was added yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workspace {
    /// The root of the caller's jj workspace, where `jj workspace` commands run.
    pub(crate) root: PathBuf,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    /// The folder the harness starts in, under `path`.
    pub(crate) harness_dir: PathBuf,
    description: String,
    /// Whether this run colocated jj in a plain git repository.
    pub(crate) colocated: bool,
    /// Whether the repository has a git checkout beside jj, so the
    /// workspace is a git worktree too.
    has_git: bool,
    /// The copy's whole description, which finds it before its ids are
    /// known: an exact match, so a name that is a prefix of another run's
    /// name never finds the other run's copy.
    copy_description: String,
    /// The copy's ids once `prepare` gave it the caller's files.
    copy: Option<CopyIds>,
    /// The operation the run started from.
    start_operation: String,
    /// The caller's mutable commits below its working copy at the start,
    /// by commit id.
    caller_commits: Vec<String>,
    /// The caller's working-copy change at the start, without its tree.
    caller_head: Option<RecordedHead>,
    /// The git settings that run commands (hooks, drivers, helpers) at the
    /// start, as `key=value` lines.
    git_settings: Vec<String>,
    /// The folder that holds the workspace's own jj configuration (the
    /// guard), once the guard is set.
    config_dir: Option<PathBuf>,
    /// Why the guard could not be set, when it could not: the run goes on
    /// without that layer, and the result says so.
    pub(crate) guard_error: Option<String>,
    /// Whether `jj workspace add` succeeded.
    added: bool,
    lock_path: PathBuf,
    state_path: PathBuf,
}

/// The exclusive lock one run holds while it colocates jj or adds or
/// removes a workspace of a repository. jj merges concurrent operations,
/// but two seats that both find jj absent would both create it, and a
/// colocated workspace is also a git worktree, which git sets up under its
/// `.git/config` lock: two seats that start together would fail on that
/// lock. Dropping the guard releases it.
struct RepositoryLock(#[allow(dead_code)] std::fs::File);

fn lock_repository(path: &Path) -> Result<RepositoryLock, String> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|error| format!("cannot open workspace lock {}: {error}", path.display()))?;
    file.lock()
        .map_err(|error| format!("cannot take workspace lock {}: {error}", path.display()))?;
    Ok(RepositoryLock(file))
}

/// What the run left behind. Every field is filled as far as the finish
/// got: a step that fails after the change is known still names it, and
/// `error` says what stopped the rest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// The change that holds the run's work, once it exists.
    pub(crate) change_id: Option<String>,
    /// Whether the run changed files, once that is known.
    pub(crate) kept: Option<bool>,
    /// Whether the move onto the caller's current change left a conflict
    /// in the kept change, once the move ran.
    pub(crate) conflicted: Option<bool>,
    /// Whether any of the caller's commits or the git settings recorded at
    /// the start changed by the end of the run, once compared.
    pub(crate) outside_changes: Option<bool>,
    /// The change the harness moved its workspace to, off the copy or onto
    /// another workspace's working copy. Its files are not recorded, no
    /// result is built, and the folder stays.
    pub(crate) left_copy: Option<String>,
    /// The revisions the run left on the copy that are neither empty nor
    /// part of the result (a branch the harness or the caller made and the
    /// final tree does not hold): kept, never abandoned, by change id.
    pub(crate) kept_revisions: Vec<String>,
    /// The operation the run started from.
    pub(crate) start_operation: Option<String>,
    /// The operation the run ended with, once the workspace is down.
    pub(crate) end_operation: Option<String>,
    /// The workspace folder, when it stays on disk.
    pub(crate) path: Option<PathBuf>,
    /// The step that failed, or why the folder stays, when one did.
    pub(crate) error: Option<String>,
}

/// The run state file: what a kill at any phase leaves to clean up by.
#[derive(Debug, Serialize)]
struct RunState<'a> {
    name: &'a str,
    root: String,
    path: String,
    phase: &'a str,
    start_operation: &'a str,
    end_operation: Option<&'a str>,
    copy_change: Option<&'a str>,
    copy_commit: Option<&'a str>,
    config_dir: Option<String>,
}

/// The refusal for a folder that is in neither git nor jj.
fn no_vcs_message(repository: &Path) -> String {
    format!(
        "a workspace-write run needs a jj or git repository to isolate the harness, and {} is in neither; use --mode read-only, or run `jj git init --colocate {}` first",
        repository.display(),
        repository.display()
    )
}

/// The message for a jj binary that cannot start.
fn jj_spawn_error(error: &std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        "jj is not on the PATH, and a workspace-write run needs it to isolate the harness; install jj, or use --mode read-only".to_string()
    } else {
        format!("cannot run jj: {error}")
    }
}

/// The jj workspace root of `path`, as jj itself finds it from that folder
/// (`jj workspace root` searches upward, `jj -R` does not). `None` when jj
/// knows no repository there, and an error when jj is not installed, since
/// every workspace-write run needs it.
fn jj_root(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.is_dir() {
        return Ok(None);
    }
    let output = Command::new("jj")
        .args([
            "--color=never",
            "--ignore-working-copy",
            "workspace",
            "root",
        ])
        .current_dir(path)
        .output()
        .map_err(|error| jj_spawn_error(&error))?;
    if !output.status.success() {
        return Ok(None);
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!root.is_empty()).then(|| PathBuf::from(root)))
}

/// One git query at `root`, trimmed, when git answers.
fn git_query(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let answer = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!answer.is_empty()).then_some(answer)
}

/// The git top level of `path`, when git knows it as a repository.
fn git_root(path: &Path) -> Option<PathBuf> {
    git_query(path, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// Which version control the caller's repository is under. A folder under
/// neither is refused with the command that fixes it.
pub(crate) fn detect(repository: &Path) -> Result<Vcs, String> {
    if let Some(root) = jj_root(repository)? {
        let colocated = root.join(".git").exists();
        return Ok(Vcs::Jj { root, colocated });
    }
    git_root(repository)
        .map(|root| Vcs::GitOnly { root })
        .ok_or_else(|| no_vcs_message(repository))
}

/// The repository store behind a workspace root: the folder every workspace
/// of one repository shares. Where git is present, before or after the
/// colocation, it is the canonical git common directory, so the identity
/// does not change when jj gets colocated. A jj repository without git
/// names its `.jj/repo`, which a secondary workspace holds as a file
/// pointing at the main workspace's store.
fn repository_store(vcs: &Vcs) -> PathBuf {
    let root = vcs.root();
    let store = if vcs.has_git() {
        let common = git_query(root, &["rev-parse", "--git-common-dir"])
            .unwrap_or_else(|| ".git".to_string());
        root.join(common)
    } else {
        let marker = root.join(".jj/repo");
        match std::fs::read_to_string(&marker) {
            Ok(target) if !target.trim().is_empty() => root.join(".jj").join(target.trim()),
            _ => marker,
        }
    };
    std::fs::canonicalize(&store).unwrap_or(store)
}

/// The folder under the state directory that holds every workspace of one
/// repository: the repository's name and a short digest of its store path,
/// so every workspace of one repository shares it and two repositories with
/// one name stay apart.
fn repository_id(store: &Path) -> String {
    let digest = Sha256::digest(store.as_os_str().as_encoded_bytes());
    let short = digest.iter().take(6).fold(String::new(), |mut hex, byte| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    // `<name>/.jj/repo` or `<name>/.git`: the first ancestor that is not
    // part of the store names the repository.
    let name = store
        .ancestors()
        .filter_map(|ancestor| ancestor.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .find(|name| !matches!(name.as_str(), "repo" | ".jj" | ".git"))
        .unwrap_or_else(|| "root".to_string());
    format!("{name}-{short}")
}

/// The description of the run's change: the route and the session on the
/// first line, the workspace name below, so the owner can tell the runs
/// apart in `jj log`. The second line is also how the runner finds the
/// change again without parsing jj's human output.
fn description(route: &str, session: Option<&str>, name: &str) -> String {
    let subject = match session {
        Some(session) if !session.trim().is_empty() => format!("rune run {route}: {session}"),
        _ => format!("rune run {route}"),
    };
    format!("{subject}\n\n{}\n", result_marker(name))
}

/// The line that marks the run's result change.
fn result_marker(name: &str) -> String {
    format!("workspace: {name}")
}

/// The line that marks the copy the run started from.
fn copy_marker(name: &str) -> String {
    format!("base copy for {name}")
}

/// The description of a bookkeeping commit of the run: the subject line of
/// the result, then `marker`.
fn bookkeeping_description(description: &str, marker: &str) -> String {
    let subject = description.lines().next().unwrap_or_default();
    format!("{subject}\n\n{marker}\n")
}

/// A workspace name no other run of this process or this instant takes.
fn workspace_name() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    format!("{NAME_PREFIX}{millis}-{}", std::process::id())
}

/// The revset of one commit by its full id, which resolves hidden or
/// rewritten, so it names the same commit whatever the harness did to its
/// change.
fn commit_revset(commit_id: &str) -> String {
    format!("commit_id(\"{commit_id}\")")
}

/// `text` as a jj string literal: quotes, backslashes, and newlines
/// escaped, so a description with any of them is matched whole.
fn jj_string(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\t' => literal.push_str("\\t"),
            '\r' => literal.push_str("\\r"),
            other => literal.push(other),
        }
    }
    literal.push('"');
    literal
}

/// The revset of the visible revisions whose whole description is
/// `description`: the marker line is matched with its run name entire, so
/// `run-1-82` never finds `run-1-820`.
fn description_revset(description: &str) -> String {
    format!("description(exact:{})", jj_string(description))
}

/// jj's immutability guard for the run's workspace: its built-in immutable
/// heads, the copy, and every other workspace's working-copy commit (the
/// caller's included, with its whole stack below). The run's own working
/// copy is left out, so the harness can commit, describe, and squash its
/// own revisions. A side branch of the caller's stays open, and a harness
/// can pass `--ignore-immutable`: the guard is one layer, and the outside
/// change report stays.
fn guard_revset(name: &str, copy: &str) -> String {
    format!(
        "builtin_immutable_heads() | {} | (working_copies() ~ \"{name}\"@)",
        commit_revset(copy)
    )
}

/// Whether a git configuration key makes git run a command or load a
/// file: a hook path, a driver, a helper, an alias, an editor, a proxy, a
/// transport command, or an include. A harness in a colocated worktree
/// shares the repository's `.git/config`, and jj's operation log records
/// nothing of it, so these are recorded at the start and compared at the
/// end.
fn runs_a_command(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let (section, rest) = key.split_once('.').unwrap_or((key.as_str(), ""));
    let last = rest.rsplit('.').next().unwrap_or_default();
    match section {
        "core" => matches!(
            rest,
            "hookspath"
                | "fsmonitor"
                | "fsmonitorhookversion"
                | "sshcommand"
                | "gitproxy"
                | "pager"
                | "editor"
                | "askpass"
                | "alternaterefscommand"
        ),
        "alias" | "filter" | "uploadpack" | "receive" | "url" | "include" | "includeif" | "ssh" => {
            true
        }
        "credential" => last == "helper",
        "diff" => matches!(last, "command" | "textconv" | "external"),
        "merge" => last == "driver",
        "mergetool" | "difftool" => last == "cmd",
        "gpg" => last == "program",
        "sequence" => rest == "editor",
        "remote" => matches!(
            last,
            "uploadpack" | "receivepack" | "vcs" | "proxy" | "proxyauthmethod"
        ),
        "http" | "https" => {
            last.contains("proxy")
                || matches!(
                    last,
                    "sslcert" | "sslkey" | "sslcainfo" | "sslcapath" | "cookiefile"
                )
        }
        _ => false,
    }
}

/// The hook files of the repository's shared git directory, as sorted
/// `hook:<name>=<digest>` lines: a colocated worktree shares them, and a
/// hook a harness drops there runs on the caller's next git command.
fn git_hooks(root: &Path) -> Vec<String> {
    let Some(common) = git_query(root, &["rev-parse", "--git-common-dir"]) else {
        return Vec::new();
    };
    let hooks = root.join(common).join("hooks");
    let Ok(entries) = std::fs::read_dir(&hooks) else {
        return Vec::new();
    };
    let mut lines: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let content = std::fs::read(entry.path()).ok()?;
            let digest = Sha256::digest(&content);
            let hex = digest.iter().fold(String::new(), |mut hex, byte| {
                use std::fmt::Write as _;
                let _ = write!(hex, "{byte:02x}");
                hex
            });
            Some(format!(
                "hook:{}={hex}",
                entry.file_name().to_string_lossy()
            ))
        })
        .collect();
    lines.sort();
    lines
}

/// The git settings that run commands, as sorted `key=value` lines from
/// every scope git reads at `root`, and the shared hook files with their
/// digests. Empty without git.
fn git_settings(root: &Path, has_git: bool) -> Vec<String> {
    if !has_git {
        return Vec::new();
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--list"])
        .output()
        .ok()
        .filter(|output| output.status.success());
    let Some(output) = output else {
        return Vec::new();
    };
    let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            line.split_once('=')
                .is_some_and(|(key, _)| runs_a_command(key))
        })
        .map(str::to_string)
        .collect();
    lines.extend(git_hooks(root));
    lines.sort();
    lines
}

/// What one jj command printed on stdout.
struct JjOutput {
    stdout: String,
}

/// One jj command, run from `cwd` so the paths it prints are relative to
/// that folder, in its own process group so a Ctrl-C at the terminal does
/// not cut it off halfway, without colors so its output stays readable,
/// and with its stderr as the error when it fails. A missing jj binary is
/// named as such, since every workspace-write run needs it.
fn jj_in(cwd: &Path, args: &[&OsStr]) -> Result<JjOutput, String> {
    let mut command = Command::new("jj");
    command.arg("--color=never").args(args).current_dir(cwd);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let output = command.output().map_err(|error| jj_spawn_error(&error))?;
    if output.status.success() {
        return Ok(JjOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        });
    }
    let shown: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    Err(format!(
        "jj {} failed ({}): {}",
        shown.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// A jj command in the caller's workspace that leaves the caller's working
/// copy alone: no snapshot, no update. The caller's files took no part in
/// the run, and the owner may be editing them.
fn jj_root_command(root: &Path, args: &[&OsStr]) -> Result<JjOutput, String> {
    let mut full = vec![
        OsStr::new("-R"),
        root.as_os_str(),
        OsStr::new("--ignore-working-copy"),
    ];
    full.extend_from_slice(args);
    jj_in(root, &full)
}

/// `jj log` of `revset` in the caller's repository with `template`, one
/// line per revision, read with a template and never from jj's human
/// output.
fn log_lines(root: &Path, revset: &str, template: &str) -> Result<Vec<String>, String> {
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(revset),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new(&format!("{template} ++ \"\\n\"")),
        ],
    )?
    .stdout;
    Ok(printed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// The first line `log_lines` gives, or `None` when the revset is empty.
fn log_first(root: &Path, revset: &str, template: &str) -> Result<Option<String>, String> {
    Ok(log_lines(root, revset, template)?.into_iter().next())
}

/// The change id and commit id pairs `revset` names.
fn id_pairs(root: &Path, revset: &str) -> Result<Vec<(String, String)>, String> {
    Ok(log_lines(root, revset, "change_id ++ \" \" ++ commit_id")?
        .iter()
        .filter_map(|line| line.split_once(' '))
        .map(|(change, commit)| (change.to_string(), commit.to_string()))
        .collect())
}

/// One boolean template keyword (`empty`, `conflict`) of a revision.
fn flag(root: &Path, revset: &str, keyword: &str) -> Result<bool, String> {
    match log_first(root, revset, keyword)?.as_deref() {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        other => Err(format!(
            "jj log printed {other:?} for {keyword}, expected true or false"
        )),
    }
}

/// The change id of the caller's working-copy change, read without a
/// snapshot: the id does not move when the files do.
fn parent_change_id(root: &Path) -> Result<String, String> {
    log_first(root, "@", "change_id")?
        .ok_or_else(|| "jj log printed no change id for the working copy".to_string())
}

/// The id of the latest operation of the repository.
fn latest_operation(root: &Path) -> Result<String, String> {
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("op"),
            OsStr::new("log"),
            OsStr::new("-n"),
            OsStr::new("1"),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new("id ++ \"\\n\""),
        ],
    )?
    .stdout;
    printed
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "jj op log printed no operation id".to_string())
}

/// The change ids among `changes` that existed at `operation`: one query
/// at that operation answers for all of them.
fn changes_at(root: &Path, operation: &str, changes: &[String]) -> Result<HashSet<String>, String> {
    if changes.is_empty() {
        return Ok(HashSet::new());
    }
    let revset = changes
        .iter()
        .map(|change| format!("present({change})"))
        .collect::<Vec<_>>()
        .join(" | ");
    Ok(jj_root_command(
        root,
        &[
            OsStr::new("--at-op"),
            OsStr::new(operation),
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(&revset),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new("change_id ++ \"\\n\""),
        ],
    )?
    .stdout
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_string)
    .collect())
}

/// The one commit this run made whose whole description is `description`:
/// a description is a label and no identity, so the candidates that
/// existed at the start operation are left out, and anything other than
/// exactly one new candidate is refused instead of picked from.
fn find_marked(root: &Path, description: &str, start_operation: &str) -> Result<CopyIds, String> {
    let marker = description.lines().nth(2).unwrap_or(description);
    let candidates = id_pairs(root, &description_revset(description))?;
    let changes: Vec<String> = candidates
        .iter()
        .map(|(change, _)| change.clone())
        .collect();
    let existed = changes_at(root, start_operation, &changes)?;
    let created: Vec<&(String, String)> = candidates
        .iter()
        .filter(|(change, _)| !existed.contains(change))
        .collect();
    match created.as_slice() {
        [(change_id, commit_id)] => Ok(CopyIds {
            change_id: change_id.clone(),
            commit_id: commit_id.clone(),
        }),
        [] => Err(format!(
            "no commit made since the run started carries the marker {marker:?}"
        )),
        several => Err(format!(
            "{} commits made since the run started carry the marker {marker:?}, so the runner cannot tell its own apart and stops",
            several.len()
        )),
    }
}

/// Where `path` resolves on disk: its deepest existing ancestor made
/// canonical, with the rest of the path appended. A folder that does not
/// exist yet is judged by where it would be created.
fn resolved_location(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(canonical) = std::fs::canonicalize(ancestor) {
            let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return canonical.join(rest);
        }
    }
    path.to_path_buf()
}

/// Refuse a workspace folder that resolves inside the caller's checkout,
/// through a state directory set inside it or a symlink that leads there:
/// the caller's own snapshot would pick the harness's files up, and the
/// harness would work inside the checkout it must not touch.
fn refuse_inside_checkout(path: &Path, root: &Path) -> Result<(), String> {
    let location = resolved_location(path);
    let checkout = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if location.starts_with(&checkout) {
        return Err(format!(
            "the workspace folder {} would lie inside the checkout {}; set RUNE_STATE_DIR or XDG_STATE_HOME to a folder outside the repository, or use --mode read-only",
            path.display(),
            root.display()
        ));
    }
    Ok(())
}

/// Where `repository` sits under `root`: as given, else canonical, since
/// jj prints a canonical root and `--repo` may name the same folder
/// through a symlink.
fn subfolder_of(repository: &Path, root: &Path) -> PathBuf {
    if let Ok(rest) = repository.strip_prefix(root) {
        return rest.to_path_buf();
    }
    let canonical = std::fs::canonicalize(repository).unwrap_or_else(|_| repository.to_path_buf());
    canonical
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// Plan the workspace for one run: where it goes, what it is called, and
/// what its change says. Nothing is created.
pub(crate) fn plan(repository: &Path, route: &str, session: Option<&str>) -> Result<Plan, String> {
    let vcs = detect(repository)?;
    let state = crate::cli::sign::state_dir().map_err(|error| error.to_string())?;
    let name = workspace_name();
    let repository_id = repository_id(&repository_store(&vcs));
    let workspaces = state.join("workspaces");
    let folder = workspaces.join(&repository_id);
    let path = folder.join(&name);
    refuse_inside_checkout(&path, vcs.root())?;
    let lock_path = workspaces.join(format!("{repository_id}.lock"));
    let state_path = folder.join(format!("{name}.json"));
    let parent_change_id = match &vcs {
        Vcs::Jj { root, .. } => Some(parent_change_id(root)?),
        Vcs::GitOnly { .. } => None,
    };
    let subfolder = subfolder_of(repository, vcs.root());
    Ok(Plan {
        description: description(route, session, &name),
        vcs,
        name,
        path,
        subfolder,
        parent_change_id,
        lock_path,
        state_path,
    })
}

/// The caller's mutable commits below its working copy, by commit id: a
/// snapshot never rewrites them, so a new commit id is a rewrite.
fn caller_commits(root: &Path) -> Result<Vec<String>, String> {
    log_lines(root, "::@- & mutable()", "commit_id")
}

/// The record of the one visible revision `revset` names, or `None` when
/// it names none or several (an abandoned or a divergent change).
fn head_record(root: &Path, revset: &str) -> Result<Option<RecordedHead>, String> {
    let commits = log_lines(root, revset, "commit_id")?;
    let [commit] = commits.as_slice() else {
        return Ok(None);
    };
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(&commit_revset(commit)),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new(HEAD_TEMPLATE),
        ],
    )?
    .stdout;
    let mut parts = printed.splitn(3, '\n');
    let change_id = parts.next().unwrap_or_default().trim().to_string();
    if change_id.is_empty() {
        return Err(format!("jj printed no change id for {revset}"));
    }
    let parents = parts
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let description = parts.next().unwrap_or_default().to_string();
    Ok(Some(RecordedHead {
        change_id,
        description,
        parents,
    }))
}

impl Workspace {
    /// The revset of the copy and everything on top of it: by both ids
    /// once they are known (the commit id resolves hidden or rewritten),
    /// else by the marker line.
    fn lineage(&self) -> String {
        match &self.copy {
            Some(copy) => format!(
                "(change_id(\"{}\") | {})::",
                copy.change_id,
                commit_revset(&copy.commit_id)
            ),
            None => format!("({})::", description_revset(&self.copy_description)),
        }
    }

    /// Write the run state file for `phase`: to a temporary file beside it,
    /// then renamed over the old one, so a kill in the middle of the write
    /// leaves the previous record whole. A failure to write it loses a
    /// recovery aid and nothing of the run, so it is not an error.
    fn write_state(&self, phase: &str, end_operation: Option<&str>) {
        let state = RunState {
            name: &self.name,
            root: self.root.display().to_string(),
            path: self.path.display().to_string(),
            phase,
            start_operation: &self.start_operation,
            end_operation,
            copy_change: self.copy.as_ref().map(|copy| copy.change_id.as_str()),
            copy_commit: self.copy.as_ref().map(|copy| copy.commit_id.as_str()),
            config_dir: self
                .config_dir
                .as_ref()
                .map(|dir| dir.display().to_string()),
        };
        let Ok(text) = serde_json::to_string_pretty(&state) else {
            return;
        };
        let temporary = self.state_path.with_extension("json.tmp");
        if std::fs::write(&temporary, text).is_ok() {
            let _ = std::fs::rename(&temporary, &self.state_path);
        }
    }
}

/// Start the run's isolation: under the repository lock, colocate jj when
/// the repository has only git (another run may have done it while this
/// one waited, so the state is checked again under the lock); then record
/// the operation, the caller's commits, and the git settings, and make the
/// copy on the caller's nearest immutable ancestor as the last step.
/// Nothing before the copy needs cleanup, and nothing after it can fail:
/// the caller hands the result to its cleanup guard the moment this
/// returns, then calls `prepare` and `add` under the guard, so a failure
/// or a signal anywhere after the copy exists abandons it.
pub(crate) fn begin(plan: &Plan) -> Result<Workspace, String> {
    let root = plan.vcs.root().to_path_buf();
    if let Some(parent) = plan.path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create workspace folder {}: {error}",
                parent.display()
            )
        })?;
    }
    {
        let _lock = lock_repository(&plan.lock_path)?;
        if plan.vcs.colocate() && jj_root(&root)?.is_none() {
            jj_in(
                &root,
                &[
                    OsStr::new("git"),
                    OsStr::new("init"),
                    OsStr::new("--colocate"),
                    root.as_os_str(),
                ],
            )?;
        }
    }
    // Recorded before the copy: the operation the run started from (so the
    // copy's change id is new to it), the caller's commits, which no
    // snapshot rewrites in what is recorded, and the git settings.
    let start_operation = latest_operation(&root)?;
    let caller_commits = caller_commits(&root)?;
    let caller_head = head_record(&root, "@")?;
    let git_settings = git_settings(&root, plan.vcs.has_git());
    let copy_description = bookkeeping_description(&plan.description, &copy_marker(&plan.name));
    // The copy is made on the nearest immutable ancestor of the caller's
    // working copy, the root commit in a repository with no bookmark yet.
    // This snapshots the caller's checkout, as any jj command does.
    jj_in(
        &root,
        &[
            OsStr::new("-R"),
            root.as_os_str(),
            OsStr::new("new"),
            OsStr::new("heads(::@ & immutable())"),
            OsStr::new("--no-edit"),
            OsStr::new("-m"),
            OsStr::new(&copy_description),
        ],
    )?;
    let workspace = Workspace {
        root,
        name: plan.name.clone(),
        path: plan.path.clone(),
        harness_dir: plan.harness_path(),
        description: plan.description.clone(),
        colocated: plan.vcs.colocate(),
        has_git: plan.vcs.has_git(),
        copy_description,
        copy: None,
        start_operation,
        caller_commits,
        caller_head,
        git_settings,
        config_dir: None,
        guard_error: None,
        added: false,
        lock_path: plan.lock_path.clone(),
        state_path: plan.state_path.clone(),
    };
    workspace.write_state("copied", None);
    Ok(workspace)
}

/// Find the copy `begin` made and give it the caller's working-copy tree.
/// `jj restore` snapshots the caller's checkout, so the copy holds the
/// caller's latest edits, and it rewrites the copy, so its ids are read
/// again afterwards. Runs under the guard: a failure here leaves the copy
/// to `finish`.
pub(crate) fn prepare(workspace: &mut Workspace) -> Result<(), String> {
    let root = &workspace.root;
    let before = find_marked(
        root,
        &workspace.copy_description,
        &workspace.start_operation,
    )?;
    jj_in(
        root,
        &[
            OsStr::new("-R"),
            root.as_os_str(),
            OsStr::new("restore"),
            OsStr::new("--from"),
            OsStr::new("@"),
            OsStr::new("--into"),
            OsStr::new(&commit_revset(&before.commit_id)),
        ],
    )?;
    workspace.copy = Some(find_marked(
        root,
        &workspace.copy_description,
        &workspace.start_operation,
    )?);
    workspace.write_state("prepared", None);
    Ok(())
}

/// Add the workspace on the copy and set its guard, under the repository
/// lock. `workspace add` refuses `--ignore-working-copy`, so it snapshots
/// the caller's checkout once more, which changes nothing the copy holds.
/// A failure leaves the copy, and after the add the workspace, to the
/// cleanup guard.
pub(crate) fn add(workspace: &mut Workspace) -> Result<(), String> {
    let root = &workspace.root;
    let Some(copy) = &workspace.copy else {
        return Err("the copy was not prepared before the workspace add".to_string());
    };
    let _lock = lock_repository(&workspace.lock_path)?;
    let message = bookkeeping_description(
        &workspace.description,
        &format!("working copy of {}", workspace.name),
    );
    let revision = commit_revset(&copy.commit_id);
    let mut args = vec![
        OsStr::new("-R"),
        root.as_os_str(),
        OsStr::new("workspace"),
        OsStr::new("add"),
        OsStr::new("--name"),
        OsStr::new(&workspace.name),
        OsStr::new("-r"),
        OsStr::new(&revision),
        OsStr::new("-m"),
        OsStr::new(&message),
    ];
    if workspace.has_git {
        args.push(OsStr::new("--colocate"));
    }
    args.push(workspace.path.as_os_str());
    if let Err(error) = jj_in(root, &args) {
        // A half-made folder holds nothing of the harness's yet.
        let _ = std::fs::remove_dir_all(&workspace.path);
        return Err(error);
    }
    workspace.added = true;
    // The guard goes into the workspace's own configuration, so it binds
    // every jj command the harness runs there and none the caller runs. It
    // is one layer: where that configuration cannot be written (a sandbox
    // that denies the user's jj configuration folder), the run goes on
    // without it, says so, and the outside change report still runs.
    let guard = guard_revset(&workspace.name, &copy.commit_id);
    match jj_in(
        &workspace.path,
        &[
            OsStr::new("-R"),
            workspace.path.as_os_str(),
            OsStr::new("--ignore-working-copy"),
            OsStr::new("config"),
            OsStr::new("set"),
            OsStr::new("--workspace"),
            OsStr::new(GUARD_KEY),
            OsStr::new(&guard),
        ],
    ) {
        Ok(_) => workspace.config_dir = workspace_config_dir(workspace),
        Err(error) => workspace.guard_error = Some(error),
    }
    workspace.write_state("added", None);
    Ok(())
}

/// Take the workspace down after the run, whatever its outcome: record the
/// harness's files, build one change from the workspace's final tree on
/// the recorded copy and move it onto the caller's current change, compare
/// what was recorded at the start, remove the workspace, abandon the run's
/// own leftovers, and name the operations. A step that fails stops the
/// rest, so nothing the harness wrote is lost: the outcome keeps what the
/// finish learned before the failure, names the error, and names the
/// folder that stays. `survivor` says the supervisor saw a process of the
/// harness outlive the run: the folder then stays, since it may still be
/// written to.
pub(crate) fn finish(workspace: &Workspace, survivor: bool) -> Outcome {
    let mut outcome = Outcome {
        start_operation: Some(workspace.start_operation.clone()),
        ..Outcome::default()
    };
    workspace.write_state("finishing", None);
    if let Err(error) = finish_steps(workspace, &mut outcome, survivor) {
        outcome.error = Some(error);
    }
    if workspace.path.exists() {
        outcome.path = Some(workspace.path.clone());
        workspace.write_state("stayed", outcome.end_operation.as_deref());
    } else {
        let _ = std::fs::remove_file(&workspace.state_path);
    }
    outcome
}

/// jj's own limits for new files: a file above `snapshot.max-new-file-size`
/// (1 MiB by default, 0 for no limit) or outside `snapshot.auto-track` is
/// skipped while the snapshot exits 0, so a harness's file would go with
/// the folder. The finish lifts both for its snapshot.
const TRACK_ALL: &str = "snapshot.auto-track=\"all()\"";
const NO_SIZE_LIMIT: &str = "snapshot.max-new-file-size=0";

/// Record every file the repository does not ignore, whatever its size.
fn record_files(workspace: &Workspace) -> Result<(), String> {
    jj_in(
        &workspace.path,
        &[
            OsStr::new("-R"),
            workspace.path.as_os_str(),
            OsStr::new("--config"),
            OsStr::new(TRACK_ALL),
            OsStr::new("--config"),
            OsStr::new(NO_SIZE_LIMIT),
            OsStr::new("util"),
            OsStr::new("snapshot"),
        ],
    )
    .map(|_| ())
}

/// The commit id of the workspace's working-copy commit, read without a
/// snapshot: after `record_files` its tree is the run's final state.
fn final_commit(workspace: &Workspace) -> Result<String, String> {
    let printed = jj_in(
        &workspace.path,
        &[
            OsStr::new("-R"),
            workspace.path.as_os_str(),
            OsStr::new("--ignore-working-copy"),
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new("@"),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new("commit_id ++ \"\\n\""),
        ],
    )?
    .stdout;
    printed
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "jj log printed no commit id for the workspace".to_string())
}

/// The change the workspace stands on when it is not the run's own: off
/// the copy's lineage (the harness ran `jj new main`), or another
/// workspace's working copy (`jj edit default@` past the guard). A
/// snapshot there would record the harness's files into somebody else's
/// change, and a result built from its files would revert the caller's
/// unpushed work, so the runner asks before it snapshots anything. `None`
/// while the workspace is on the copy's lineage and on its own change.
fn foreign_head(workspace: &Workspace, lineage: &str) -> Result<Option<String>, String> {
    let own = format!("\"{}\"@", workspace.name);
    log_first(
        &workspace.root,
        &format!("{own} ~ ({lineage} ~ (working_copies() ~ {own}))"),
        "change_id",
    )
}

/// Abandon `commits`, by commit id, in one command. Nothing to abandon is
/// fine.
fn abandon_commits(root: &Path, commits: &[String]) -> Result<(), String> {
    if commits.is_empty() {
        return Ok(());
    }
    let revsets: Vec<String> = commits.iter().map(|commit| commit_revset(commit)).collect();
    let mut args = vec![OsStr::new("abandon")];
    args.extend(revsets.iter().map(|revset| OsStr::new(revset.as_str())));
    jj_root_command(root, &args).map(|_| ())
}

/// Take the run's leftovers on the copy down without losing content. The
/// candidates are the copy's descendants minus the caller's line and the
/// kept result. A candidate goes when it is empty, or when its changes are
/// in the result already (it is an ancestor of the workspace's final
/// commit, whose tree the result holds). Every other candidate stays,
/// whoever made it: a branch the harness or the caller made during the run
/// and the final tree does not hold. The copy itself goes last, and only
/// when nothing stands on it. What stays is listed in `kept_revisions`,
/// and `outside_changes` says so.
fn clean_up(
    workspace: &Workspace,
    outcome: &mut Outcome,
    lineage: &str,
    final_commit: Option<&str>,
) -> Result<(), String> {
    let root = &workspace.root;
    let kept_result = match (&outcome.change_id, outcome.kept) {
        (Some(change_id), Some(true)) => format!(" ~ change_id(\"{change_id}\")"),
        _ => String::new(),
    };
    let candidates = format!("({lineage} ~ ::@{kept_result}) & ::visible_heads()");
    let contained = match final_commit {
        Some(commit) => format!("({candidates}) & (::{} | empty())", commit_revset(commit)),
        None => format!("({candidates}) & empty()"),
    };
    // What stays is read before the abandon: an abandoned parent moves
    // the survivors off the copy's lineage, and their change ids hold.
    let all = id_pairs(root, &candidates)?;
    let gone: HashSet<String> = id_pairs(root, &contained)?
        .into_iter()
        .map(|(_, commit)| commit)
        .collect();
    let copy_commit = workspace.copy.as_ref().map(|copy| copy.commit_id.as_str());
    let mut remaining: Vec<String> = all
        .iter()
        .filter(|(_, commit)| !gone.contains(commit) && Some(commit.as_str()) != copy_commit)
        .map(|(change, _)| change.clone())
        .collect();
    abandon_commits(root, &gone.iter().cloned().collect::<Vec<_>>())?;
    if let Some(copy) = &workspace.copy
        && !gone.contains(&copy.commit_id)
    {
        let copy_revset = commit_revset(&copy.commit_id);
        let visible = log_first(
            root,
            &format!("{copy_revset} & ::visible_heads()"),
            "commit_id",
        )?;
        let children = log_lines(
            root,
            &format!("({copy_revset}+) & ::visible_heads()"),
            "commit_id",
        )?;
        match (visible, children.is_empty()) {
            (Some(commit), true) => abandon_commits(root, &[commit])?,
            (Some(_), false) => remaining.push(copy.change_id.clone()),
            (None, _) => {}
        }
    }
    if !remaining.is_empty() {
        outcome.outside_changes = Some(true);
    }
    outcome.kept_revisions = remaining;
    Ok(())
}

/// The folder that holds the workspace's own configuration (the guard),
/// which jj keeps under the user's configuration directory and leaves
/// behind when the workspace goes. Only a folder under `workspaces` is
/// named, so nothing else is ever removed.
fn workspace_config_dir(workspace: &Workspace) -> Option<PathBuf> {
    let printed = jj_in(
        &workspace.path,
        &[
            OsStr::new("-R"),
            workspace.path.as_os_str(),
            OsStr::new("--ignore-working-copy"),
            OsStr::new("config"),
            OsStr::new("path"),
            OsStr::new("--workspace"),
        ],
    )
    .ok()?
    .stdout;
    let file = PathBuf::from(printed.trim());
    let dir = file.parent()?.to_path_buf();
    let under_workspaces = dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "workspaces");
    under_workspaces.then_some(dir)
}

/// Stop tracking the workspace, without touching its files: for a folder
/// that stays, and as the fallback of a removal that failed, so no stale
/// workspace stays tracked.
fn forget_workspace(workspace: &Workspace) -> Result<(), String> {
    jj_root_command(
        &workspace.root,
        &[
            OsStr::new("workspace"),
            OsStr::new("forget"),
            OsStr::new(&workspace.name),
        ],
    )
    .map(|_| ())
}

/// Stop tracking the workspace and delete its files, under the repository
/// lock that guards git's worktree registration. `workspace remove`
/// refuses `--ignore-working-copy`, so it snapshots the caller's checkout
/// once more. When it fails, the workspace is at least forgotten, and the
/// error goes to the outcome.
fn remove_workspace(workspace: &Workspace) -> Result<(), String> {
    let _lock = lock_repository(&workspace.lock_path)?;
    let root = &workspace.root;
    if let Err(error) = jj_in(
        root,
        &[
            OsStr::new("-R"),
            root.as_os_str(),
            OsStr::new("workspace"),
            OsStr::new("remove"),
            OsStr::new(&workspace.name),
        ],
    ) {
        let _ = forget_workspace(workspace);
        return Err(format!(
            "cannot remove workspace folder {}: {error}",
            workspace.path.display()
        ));
    }
    Ok(())
}

/// Whether any of the caller's commits recorded at the start is gone or
/// rewritten: a harness that describes or abandons one by id, or the
/// caller's own squash, rebase, or describe meanwhile. A commit below the
/// working copy counts by commit id, which resolves hidden or not, so the
/// question is whether it is still visible. The working-copy change counts
/// by its record without the tree, so the caller's own edits and snapshots
/// change nothing recorded.
fn caller_commits_changed(workspace: &Workspace) -> Result<bool, String> {
    // One query per batch of recorded commits, never one per commit: a
    // long local-only history (200 mutable commits, reviewer repro
    // `perf.sh`) cost a jj process each. Every `commit_id()` yields that
    // commit, hidden or not, so the visible ones are counted.
    for batch in workspace.caller_commits.chunks(COMMIT_QUERY_BATCH) {
        let revset = batch
            .iter()
            .map(|commit| commit_revset(commit))
            .collect::<Vec<_>>()
            .join(" | ");
        let still_there = log_lines(
            &workspace.root,
            &format!("({revset}) & ::visible_heads()"),
            "commit_id",
        )?;
        if still_there.len() != batch.len() {
            return Ok(true);
        }
    }
    if let Some(recorded) = &workspace.caller_head {
        let now = head_record(
            &workspace.root,
            &format!("change_id(\"{}\")", recorded.change_id),
        )?;
        if now.as_ref() != Some(recorded) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Build the run's change from the workspace's final tree: a new commit on
/// the recorded copy takes that tree, so its diff is what the harness
/// changed, whatever the harness did to the revisions in between. A kept
/// result is moved onto the caller's current change. Every id is read
/// again after each rewrite, and the result is found as the one commit of
/// this run with its description. The conflict flag is read later, after
/// the workspace removal, which can snapshot the caller and move the
/// result once more.
fn build_result(
    workspace: &Workspace,
    outcome: &mut Outcome,
    final_commit: &str,
) -> Result<(), String> {
    let root = &workspace.root;
    let Some(copy) = &workspace.copy else {
        return Err("the workspace was added without a prepared copy".to_string());
    };
    let marker = workspace.description.as_str();
    jj_root_command(
        root,
        &[
            OsStr::new("new"),
            OsStr::new(&commit_revset(&copy.commit_id)),
            OsStr::new("--no-edit"),
            OsStr::new("-m"),
            OsStr::new(&workspace.description),
        ],
    )?;
    let result = find_marked(root, marker, &workspace.start_operation)?;
    outcome.change_id = Some(result.change_id.clone());
    jj_root_command(
        root,
        &[
            OsStr::new("restore"),
            OsStr::new("--from"),
            OsStr::new(&commit_revset(final_commit)),
            OsStr::new("--into"),
            OsStr::new(&commit_revset(&result.commit_id)),
        ],
    )?;
    let result = find_marked(root, marker, &workspace.start_operation)?;
    let kept = !flag(root, &commit_revset(&result.commit_id), "empty")?;
    outcome.kept = Some(kept);
    outcome.conflicted = Some(false);
    if kept {
        jj_root_command(
            root,
            &[
                OsStr::new("rebase"),
                OsStr::new("-r"),
                OsStr::new(&commit_revset(&result.commit_id)),
                OsStr::new("-o"),
                OsStr::new("@"),
            ],
        )?;
    }
    Ok(())
}

/// The conflict flag of the kept result, read once the workspace is gone
/// and the leftovers are cleaned: `jj workspace remove` snapshots the
/// caller's checkout, which can move the result onto a rewritten working
/// copy and leave a conflict the earlier rebase did not have.
fn read_conflict(workspace: &Workspace, outcome: &mut Outcome) -> Result<(), String> {
    if outcome.kept != Some(true) {
        return Ok(());
    }
    let result = find_marked(
        &workspace.root,
        &workspace.description,
        &workspace.start_operation,
    )?;
    outcome.conflicted = Some(flag(
        &workspace.root,
        &commit_revset(&result.commit_id),
        "conflict",
    )?);
    Ok(())
}

/// The finish, step by step, writing into `outcome` as each fact is known.
fn finish_steps(
    workspace: &Workspace,
    outcome: &mut Outcome,
    survivor: bool,
) -> Result<(), String> {
    let root = &workspace.root;
    let lineage = workspace.lineage();
    if !workspace.added {
        // The copy exists and nothing else: it goes, and a half-made folder
        // with it.
        clean_up(workspace, outcome, &lineage, None)?;
        if workspace.path.exists() {
            std::fs::remove_dir_all(&workspace.path).map_err(|error| {
                format!(
                    "cannot remove workspace folder {}: {error}",
                    workspace.path.display()
                )
            })?;
        }
        outcome.end_operation = Some(latest_operation(root)?);
        return Ok(());
    }
    // Before any snapshot: a workspace that stands off the copy's lineage
    // or on another workspace's working copy is not the run's to record.
    // Its files stay in the folder, the workspace is forgotten, the copy's
    // leftovers are cleaned, and the result names the change.
    if let Some(change) = foreign_head(workspace, &lineage)? {
        outcome.left_copy = Some(change.clone());
        outcome.kept = Some(false);
        outcome.conflicted = Some(false);
        outcome.outside_changes = Some(true);
        forget_workspace(workspace)?;
        if let Some(dir) = &workspace.config_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
        clean_up(workspace, outcome, &lineage, None)?;
        outcome.end_operation = Some(latest_operation(root)?);
        return Err(format!(
            "the harness moved its workspace to change {change}, off the run's copy: its files were not recorded, no result was built, the workspace is forgotten, and the folder stays with the files"
        ));
    }
    record_files(workspace)?;
    let final_commit = final_commit(workspace)?;
    build_result(workspace, outcome, &final_commit)?;
    outcome.outside_changes = Some(
        caller_commits_changed(workspace)?
            || git_settings(root, workspace.has_git) != workspace.git_settings,
    );
    // The workspace goes, with its files, unless a process of the harness
    // may still write there: then it is only forgotten, and the folder
    // stays for the owner.
    let removal = if survivor {
        forget_workspace(workspace)?;
        Err(format!(
            "a process of the harness outlived the run and may still write; the workspace folder {} stays, and its files are recorded as of the run's end",
            workspace.path.display()
        ))
    } else {
        remove_workspace(workspace)
    };
    if let Some(dir) = &workspace.config_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
    clean_up(workspace, outcome, &lineage, Some(&final_commit))?;
    read_conflict(workspace, outcome)?;
    outcome.end_operation = Some(latest_operation(root)?);
    removal?;
    if workspace.path.exists() {
        std::fs::remove_dir_all(&workspace.path).map_err(|error| {
            format!(
                "cannot remove workspace folder {}: {error}",
                workspace.path.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_descriptions_name_the_route_the_session_and_the_workspace() {
        let text = description("sol@claude", Some("council/sol"), "run-1-2");
        assert_eq!(
            text,
            "rune run sol@claude: council/sol\n\nworkspace: run-1-2\n"
        );
        let bare = description("codex", None, "run-1-2");
        assert!(bare.starts_with("rune run codex\n"), "{bare}");
        let blank = description("codex", Some("  "), "run-1-2");
        assert_eq!(blank, bare);
        // The bookkeeping commits keep the subject and carry their own
        // marker, which never contains the result's marker.
        let copy = bookkeeping_description(&text, &copy_marker("run-1-2"));
        assert_eq!(
            copy,
            "rune run sol@claude: council/sol\n\nbase copy for run-1-2\n"
        );
        assert!(!copy.contains(&result_marker("run-1-2")));
        // The lookup matches the whole description, so a run name that is
        // a prefix of another's never finds the other's commit.
        assert_eq!(
            description_revset(&copy),
            "description(exact:\"rune run sol@claude: council/sol\\n\\nbase copy for run-1-2\\n\")"
        );
        assert_eq!(
            jj_string("say \"hi\" \\ twice\n"),
            "\"say \\\"hi\\\" \\\\ twice\\n\""
        );
    }

    #[test]
    fn the_guard_names_the_copy_and_the_other_working_copies() {
        assert_eq!(
            guard_revset("run-1-2", "aaaa"),
            "builtin_immutable_heads() | commit_id(\"aaaa\") | (working_copies() ~ \"run-1-2\"@)"
        );
    }

    #[test]
    fn the_git_keys_that_run_commands_are_told_from_the_rest() {
        for key in [
            "core.hooksPath",
            "core.fsmonitor",
            "core.sshCommand",
            "alias.co",
            "credential.helper",
            "credential.https://example.test.helper",
            "diff.lfs.textconv",
            "diff.external",
            "merge.ours.driver",
            "filter.lfs.clean",
            "gpg.program",
            "gpg.x509.program",
            "sequence.editor",
            "remote.origin.uploadpack",
            "url.ssh://git@example.test/.insteadOf",
            "include.path",
            "http.proxy",
            "uploadpack.packObjectsHook",
        ] {
            assert!(runs_a_command(key), "{key}");
        }
        for key in [
            "user.name",
            "core.bare",
            "core.repositoryformatversion",
            "remote.origin.url",
            "branch.main.remote",
            "diff.renames",
            "merge.conflictstyle",
            "http.sslverify",
        ] {
            assert!(!runs_a_command(key), "{key}");
        }
    }

    #[test]
    fn the_workspace_name_starts_with_the_prefix() {
        let name = workspace_name();
        assert!(name.starts_with(NAME_PREFIX), "{name}");
        assert!(name.len() > NAME_PREFIX.len());
    }

    #[test]
    fn repository_id_names_the_repository_and_tells_two_stores_apart() {
        let one = repository_id(Path::new("/home/me/cli/.jj/repo"));
        let two = repository_id(Path::new("/home/you/cli/.jj/repo"));
        assert!(one.starts_with("cli-"), "{one}");
        assert_ne!(one, two);
        assert_eq!(one, repository_id(Path::new("/home/me/cli/.jj/repo")));
        assert!(repository_id(Path::new("/home/me/cli/.git")).starts_with("cli-"));
    }

    #[test]
    fn every_workspace_of_one_repository_shares_the_store() {
        let root = tempfile::tempdir().expect("root");
        let main = root.path().join("main");
        std::fs::create_dir_all(main.join(".jj/repo")).expect("main store");
        let secondary = root.path().join("second");
        std::fs::create_dir_all(secondary.join(".jj")).expect("secondary .jj");
        std::fs::write(secondary.join(".jj/repo"), "../../main/.jj/repo\n").expect("marker");
        let main_store = repository_store(&Vcs::Jj {
            root: main.clone(),
            colocated: false,
        });
        let secondary_store = repository_store(&Vcs::Jj {
            root: secondary,
            colocated: false,
        });
        assert_eq!(main_store, secondary_store);
        assert_eq!(
            main_store,
            std::fs::canonicalize(main.join(".jj/repo")).expect("canonical")
        );
        assert_eq!(repository_id(&main_store), repository_id(&secondary_store));
        assert!(repository_id(&main_store).starts_with("main-"));
    }

    #[test]
    fn a_repository_with_git_keeps_its_store_identity_through_the_colocation() {
        let root = tempfile::tempdir().expect("root");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let status = Command::new("git")
            .args(["init", "-q"])
            .arg(&repo)
            .status()
            .expect("git runs");
        assert!(status.success());
        let before = repository_store(&Vcs::GitOnly { root: repo.clone() });
        std::fs::create_dir_all(repo.join(".jj/repo")).expect("jj store");
        let after = repository_store(&Vcs::Jj {
            root: repo.clone(),
            colocated: true,
        });
        assert_eq!(before, after);
        assert_eq!(
            before,
            std::fs::canonicalize(repo.join(".git")).expect("canonical")
        );
    }

    #[test]
    fn detect_asks_jj_for_the_root_above_a_subfolder_and_sees_colocation() {
        let root = tempfile::tempdir().expect("root");
        let repo = std::fs::canonicalize(root.path()).expect("canonical root");
        let status = Command::new("jj")
            .args(["--config", "git.colocate=false", "git", "init"])
            .arg(&repo)
            .env("JJ_USER", "Test")
            .env("JJ_EMAIL", "test@example.test")
            .status()
            .expect("jj runs");
        assert!(status.success());
        let nested = repo.join("src/deep");
        std::fs::create_dir_all(&nested).expect("nested");
        assert_eq!(
            detect(&nested),
            Ok(Vcs::Jj {
                root: repo.clone(),
                colocated: false
            })
        );
        std::fs::write(repo.join(".git"), "gitdir: elsewhere\n").expect(".git file");
        assert_eq!(
            detect(&nested),
            Ok(Vcs::Jj {
                root: repo.clone(),
                colocated: true
            })
        );
        assert_eq!(subfolder_of(&nested, &repo), PathBuf::from("src/deep"));
        assert_eq!(subfolder_of(&repo, &repo), PathBuf::new());
    }

    #[test]
    fn a_workspace_folder_inside_the_checkout_is_refused_through_a_symlink_too() {
        let root = tempfile::tempdir().expect("root");
        let checkout = root.path().join("repo");
        std::fs::create_dir_all(checkout.join(".jj")).expect("checkout");
        let inside = checkout.join(".state/workspaces/repo-1/run-1");
        let error = refuse_inside_checkout(&inside, &checkout).expect_err("inside");
        assert!(error.contains("would lie inside the checkout"), "{error}");
        assert!(error.contains("RUNE_STATE_DIR"), "{error}");

        let outside = root.path().join("state/workspaces/repo-1/run-1");
        assert_eq!(refuse_inside_checkout(&outside, &checkout), Ok(()));

        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&checkout, &alias).expect("symlink");
            let through_alias = alias.join("state/workspaces/repo-1/run-1");
            assert!(refuse_inside_checkout(&through_alias, &checkout).is_err());
        }
    }

    #[test]
    fn detect_refuses_a_folder_under_no_version_control() {
        let folder = tempfile::tempdir().expect("folder");
        let error = detect(folder.path()).expect_err("no repository");
        assert!(error.contains("--mode read-only"), "{error}");
        assert!(error.contains("jj git init --colocate"), "{error}");
    }
}
