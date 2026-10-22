//! The jj workspace a workspace-write run works in. The harness changes
//! only its own copy of the repository, and the owner reviews the result as
//! one jj change on top of the caller's working-copy change, named after
//! the route and the session. An empty run leaves no trace.
//!
//! The workspace starts from a copy of the caller's files: a new commit on
//! the caller's nearest immutable ancestor, given the caller's working-copy
//! tree. Nothing the caller does rewrites an immutable commit, so the copy
//! and the workspace never go stale, whatever the caller squashes or
//! rebases meanwhile. At the end the run's change is built from trees, not
//! from revisions: a new commit on the recorded copy commit takes the
//! workspace's final tree, so a harness that squashes, absorbs or abandons
//! revisions cannot lose its work. That change is rebased onto the caller's
//! current change, and whatever of the copy and the run's revisions still
//! exists is abandoned. The caller's mutable commits are recorded at the
//! start and compared at the end: a difference is reported with the
//! operation the run started from.

use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The prefix of every workspace name the runner creates, so a leftover
/// after a SIGKILL is findable in `jj workspace list`.
const NAME_PREFIX: &str = "run-";

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
    /// The lock every run of this repository takes while it registers or
    /// forgets a workspace.
    lock_path: PathBuf,
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

/// The copy of the caller's files the workspace starts from: its change
/// id while it is visible, and its commit id, which resolves whatever the
/// harness does to the change.
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
    /// The commit the copy was made on, and the marker line that finds the
    /// copy again before its ids are known.
    copy_base: String,
    copy_marker: String,
    /// The copy's ids once `prepare` found it: `None` between the `jj new`
    /// that made it and that lookup, when the guard already owns it.
    copy: Option<CopyIds>,
    /// The operation the run started from, for `jj op log`.
    operation_id: String,
    /// The caller's mutable commits below its working copy at the start,
    /// by commit id.
    caller_commits: Vec<String>,
    /// The caller's working-copy change at the start, without its tree.
    caller_head: Option<RecordedHead>,
    /// Whether `jj workspace add` succeeded.
    added: bool,
    lock_path: PathBuf,
}

/// The exclusive lock one run holds while it adds or forgets a workspace
/// of a repository. jj merges concurrent operations, but a colocated
/// workspace is also a git worktree, and git locks `.git/config` while it
/// sets one up: two seats that start together would fail on that lock.
/// Dropping the guard releases it.
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
    /// Whether the rebase onto the caller's current change left a conflict
    /// in the kept change, once the rebase ran.
    pub(crate) conflicted: Option<bool>,
    /// The workspace folder, when it stays on disk.
    pub(crate) path: Option<PathBuf>,
    /// The files, relative to the folder, that jj did not record. The
    /// folder stays while this is not empty.
    pub(crate) unrecorded: Vec<String>,
    /// Whether any of the caller's commits recorded at the start changed
    /// by the end of the run, once compared.
    pub(crate) outside_changes: Option<bool>,
    /// The operation the run started from, for `jj op log`.
    pub(crate) operation_id: Option<String>,
    /// The step that failed, when one did.
    pub(crate) error: Option<String>,
}

/// The refusal for a folder that is in neither git nor jj.
fn no_vcs_message(repository: &Path) -> String {
    format!(
        "a workspace-write run needs a jj or git repository to isolate the harness, and {} is in neither; use --mode read-only, or run `jj git init --colocate {}` first",
        repository.display(),
        repository.display()
    )
}

/// The jj workspace root at or above `path`: the closest ancestor with a
/// `.jj` directory.
fn jj_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| ancestor.join(".jj").is_dir())
        .map(Path::to_path_buf)
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
    if let Some(root) = jj_root(repository) {
        let colocated = root.join(".git").exists();
        return Ok(Vcs::Jj { root, colocated });
    }
    git_root(repository)
        .map(|root| Vcs::GitOnly { root })
        .ok_or_else(|| no_vcs_message(repository))
}

/// The repository store behind a workspace root: the folder every workspace
/// of one repository shares. Where git is present, before or after the
/// colocation, it is the git common directory, so the identity does not
/// change when jj gets colocated. A jj repository without git names its
/// `.jj/repo`, which a secondary workspace holds as a file pointing at the
/// main workspace's store.
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

/// What one jj command printed on both streams.
struct JjOutput {
    stdout: String,
    stderr: String,
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
    let output = command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "jj is not on the PATH, and a workspace-write run needs it to isolate the harness; install jj, or use --mode read-only".to_string()
        } else {
            format!("cannot run jj: {error}")
        }
    })?;
    if output.status.success() {
        return Ok(JjOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
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

/// `revset` narrowed to the visible revisions, so a hidden id (which jj
/// still resolves) counts as gone.
fn visible(revset: &str) -> String {
    format!("({revset}) & ::visible_heads()")
}

/// The change id and commit id of the one revision `revset` names.
fn ids_of(root: &Path, revset: &str) -> Result<(String, String), String> {
    let line = log_first(root, revset, "change_id ++ \" \" ++ commit_id")?
        .ok_or_else(|| format!("jj lists no revision for {revset}"))?;
    let (change_id, commit_id) = line
        .split_once(' ')
        .ok_or_else(|| format!("jj printed no ids for {revset}: {line}"))?;
    Ok((change_id.to_string(), commit_id.to_string()))
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

/// Plan the workspace for one run: where it goes, what it is called, and
/// what its change says. Nothing is created.
pub(crate) fn plan(repository: &Path, route: &str, session: Option<&str>) -> Result<Plan, String> {
    let vcs = detect(repository)?;
    let state = crate::cli::sign::state_dir().map_err(|error| error.to_string())?;
    let name = workspace_name();
    let repository_id = repository_id(&repository_store(&vcs));
    let workspaces = state.join("workspaces");
    let path = workspaces.join(&repository_id).join(&name);
    refuse_inside_checkout(&path, vcs.root())?;
    let lock_path = workspaces.join(format!("{repository_id}.lock"));
    let parent_change_id = match &vcs {
        Vcs::Jj { root, .. } => Some(parent_change_id(root)?),
        Vcs::GitOnly { .. } => None,
    };
    // `--repo` below the root: the same subfolder of the workspace. The
    // root came from the repository's own ancestors, so the prefix holds.
    let subfolder = repository
        .strip_prefix(vcs.root())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    Ok(Plan {
        description: description(route, session, &name),
        vcs,
        name,
        path,
        subfolder,
        parent_change_id,
        lock_path,
    })
}

/// The commit the copy is made on: the caller's nearest immutable
/// ancestor, which nothing the caller does rewrites, else the root.
fn immutable_base(root: &Path) -> Result<String, String> {
    let base = log_first(root, "heads(::@ & immutable())", "commit_id")?;
    match base {
        Some(commit) => Ok(commit),
        None => log_first(root, "root()", "commit_id")?
            .ok_or_else(|| "jj lists no root commit".to_string()),
    }
}

impl Workspace {
    /// The revset of the copy and everything on top of it: by its ids once
    /// they are known (the commit id resolves hidden or rewritten), else by
    /// the marker line under the base.
    fn lineage(&self) -> String {
        match &self.copy {
            Some(copy) => format!(
                "(change_id({}) | present({}))::",
                copy.change_id, copy.commit_id
            ),
            None => format!(
                "(children({}) & description(substring:\"{}\"))::",
                self.copy_base, self.copy_marker
            ),
        }
    }
}

/// Find the copy `begin` made and give it the caller's working-copy tree.
/// `jj restore` snapshots the caller's checkout, so the copy holds the
/// caller's latest edits. Runs under the guard: a failure here leaves the
/// copy to `finish`.
pub(crate) fn prepare(workspace: &mut Workspace) -> Result<(), String> {
    let root = &workspace.root;
    let (change_id, _) = ids_of(
        root,
        &format!(
            "children({}) & description(substring:\"{}\")",
            workspace.copy_base, workspace.copy_marker
        ),
    )?;
    jj_in(
        root,
        &[
            OsStr::new("-R"),
            root.as_os_str(),
            OsStr::new("restore"),
            OsStr::new("--from"),
            OsStr::new("@"),
            OsStr::new("--to"),
            OsStr::new(&change_id),
        ],
    )?;
    let (_, commit_id) = ids_of(root, &change_id)?;
    workspace.copy = Some(CopyIds {
        change_id,
        commit_id,
    });
    Ok(())
}

/// The caller's mutable commits below its working copy, by commit id: a
/// snapshot never rewrites them, so a new commit id is a rewrite.
fn caller_commits(root: &Path) -> Result<Vec<String>, String> {
    log_lines(root, "::@- & mutable()", "commit_id")
}

/// The record of the one visible revision `revset` names, or `None` when
/// it names none or several (an abandoned or a divergent change).
fn head_record(root: &Path, revset: &str) -> Result<Option<RecordedHead>, String> {
    let commits = log_lines(root, &visible(revset), "commit_id")?;
    let [commit] = commits.as_slice() else {
        return Ok(None);
    };
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(commit),
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

/// The id of the latest operation of the repository.
fn operation_id(root: &Path) -> Result<String, String> {
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

/// Start the run's isolation: colocate jj first when the repository has
/// only git, record the operation and the caller's mutable commits, and
/// make the copy as the last step. Nothing before the copy needs cleanup,
/// and nothing after it can fail: the caller hands the result to its
/// cleanup guard the moment this returns, then calls `prepare` and `add`
/// under the guard, so a failure or a signal anywhere after the copy
/// exists abandons it.
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
        // Another run may have colocated the repository while this one
        // waited for the lock.
        if plan.vcs.colocate() && !root.join(".jj").is_dir() {
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
    // copy's change id is new to it), and the caller's commits, which no
    // snapshot rewrites in what is recorded.
    let operation_id = operation_id(&root)?;
    let caller_commits = caller_commits(&root)?;
    let caller_head = head_record(&root, "@")?;
    let copy_base = immutable_base(&root)?;
    let copy_marker = copy_marker(&plan.name);
    jj_in(
        &root,
        &[
            OsStr::new("-R"),
            root.as_os_str(),
            OsStr::new("new"),
            OsStr::new(&copy_base),
            OsStr::new("--no-edit"),
            OsStr::new("-m"),
            OsStr::new(&bookkeeping_description(&plan.description, &copy_marker)),
        ],
    )?;
    Ok(Workspace {
        root,
        name: plan.name.clone(),
        path: plan.path.clone(),
        harness_dir: plan.harness_path(),
        description: plan.description.clone(),
        colocated: plan.vcs.colocate(),
        has_git: plan.vcs.has_git(),
        copy_base,
        copy_marker,
        copy: None,
        operation_id,
        caller_commits,
        caller_head,
        added: false,
        lock_path: plan.lock_path.clone(),
    })
}

/// Add the workspace on the copy. `workspace add` refuses
/// `--ignore-working-copy`, so it snapshots the caller's checkout once
/// more, which changes nothing the copy holds. A failure leaves the copy
/// to the cleanup guard.
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
    let mut args = vec![
        OsStr::new("-R"),
        root.as_os_str(),
        OsStr::new("workspace"),
        OsStr::new("add"),
        OsStr::new("--name"),
        OsStr::new(&workspace.name),
        OsStr::new("--revision"),
        OsStr::new(&copy.commit_id),
        OsStr::new("--message"),
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
    Ok(())
}

/// Abandon the run's own revisions among the visible ones `revset` names.
/// A revision is the run's own when its change id did not exist at the
/// operation the run started from: one query at that operation answers
/// for every candidate. A candidate that existed then belongs to someone
/// else (a harness rebased a caller's branch onto its work): it stays, and
/// `outside_changes` says so. The abandon goes by commit id, so a change
/// that jj lists twice (a copy the harness squashed into, next to the
/// recorded copy commit) goes without a divergence error.
fn abandon_run_owned(
    workspace: &Workspace,
    outcome: &mut Outcome,
    revset: &str,
) -> Result<(), String> {
    let root = &workspace.root;
    let candidates: Vec<(String, String)> =
        log_lines(root, &visible(revset), "change_id ++ \" \" ++ commit_id")?
            .iter()
            .filter_map(|line| line.split_once(' '))
            .map(|(change, commit)| (change.to_string(), commit.to_string()))
            .collect();
    if candidates.is_empty() {
        return Ok(());
    }
    let at_start: Vec<String> = candidates
        .iter()
        .map(|(change, _)| format!("present({change})"))
        .collect();
    let existed: std::collections::HashSet<String> = jj_root_command(
        root,
        &[
            OsStr::new("--at-op"),
            OsStr::new(&workspace.operation_id),
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(&at_start.join(" | ")),
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
    .collect();
    let (foreign, owned): (Vec<_>, Vec<_>) = candidates
        .iter()
        .partition(|(change, _)| existed.contains(change));
    if !foreign.is_empty() {
        outcome.outside_changes = Some(true);
    }
    if owned.is_empty() {
        return Ok(());
    }
    let mut args = vec![OsStr::new("abandon")];
    args.extend(owned.iter().map(|(_, commit)| OsStr::new(commit)));
    jj_root_command(root, &args).map(|_| ())
}

/// Take the workspace down after the run, whatever its outcome: record the
/// harness's files, stop tracking the workspace, build one change from the
/// workspace's final tree on top of the caller's current change, drop the
/// copy and an empty result, and remove the folder. A step that fails
/// stops the rest, so nothing the harness wrote is lost: the outcome keeps
/// what the finish learned before the failure, names the error, and names
/// the folder that stays.
pub(crate) fn finish(workspace: &Workspace) -> Outcome {
    let mut outcome = Outcome {
        operation_id: Some(workspace.operation_id.clone()),
        ..Outcome::default()
    };
    if let Err(error) = finish_steps(workspace, &mut outcome) {
        outcome.error = Some(error);
        if workspace.path.exists() {
            outcome.path = Some(workspace.path.clone());
        }
    }
    outcome
}

/// jj's own limits for new files: a file above `snapshot.max-new-file-size`
/// (1 MiB by default) or outside `snapshot.auto-track` is skipped with a
/// warning while the snapshot exits 0, so a harness's file would go with
/// the folder. The finish lifts both for its snapshot.
const TRACK_ALL: &str = "snapshot.auto-track=\"all()\"";
const TRACK_NONE: &str = "snapshot.auto-track=\"none()\"";
const NO_SIZE_LIMIT: &str = "snapshot.max-new-file-size=\"1TiB\"";

/// The files jj refused in a snapshot's stderr, under `Refused to snapshot
/// some files:`, one indented `<path>: <size>; ...` line each.
fn refused_paths(stderr: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut listing = false;
    for line in stderr.lines() {
        if line.contains("Refused to snapshot some files:") {
            listing = true;
            continue;
        }
        if !listing {
            continue;
        }
        let Some(entry) = line.strip_prefix("  ") else {
            listing = false;
            continue;
        };
        let path = entry
            .split_once(": ")
            .map_or(entry, |(path, _)| path)
            .trim();
        if !path.is_empty() {
            paths.push(path.to_string());
        }
    }
    paths
}

/// The files `jj status` lists under `Untracked paths:` as `? <path>`.
fn untracked_paths(stdout: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut listing = false;
    for line in stdout.lines() {
        if line.starts_with("Untracked paths:") {
            listing = true;
            continue;
        }
        if !listing {
            continue;
        }
        match line.strip_prefix("? ") {
            Some(path) if !path.trim().is_empty() => paths.push(path.trim().to_string()),
            _ => listing = false,
        }
    }
    paths
}

/// Record every file the repository does not ignore, whatever its size,
/// and return the paths jj still left unrecorded: the ones a warning
/// refused, and the ones a status with tracking off still lists as
/// untracked. Both run from the workspace folder, so the paths are
/// relative to it.
fn record_files(workspace: &Workspace) -> Result<Vec<String>, String> {
    let path = workspace.path.as_os_str();
    let snapshot = jj_in(
        &workspace.path,
        &[
            OsStr::new("-R"),
            path,
            OsStr::new("--config"),
            OsStr::new(TRACK_ALL),
            OsStr::new("--config"),
            OsStr::new(NO_SIZE_LIMIT),
            OsStr::new("util"),
            OsStr::new("snapshot"),
        ],
    )?;
    let mut unrecorded = refused_paths(&snapshot.stderr);
    let status = jj_in(
        &workspace.path,
        &[
            OsStr::new("-R"),
            path,
            OsStr::new("--config"),
            OsStr::new(TRACK_NONE),
            OsStr::new("--config"),
            OsStr::new(NO_SIZE_LIMIT),
            OsStr::new("status"),
        ],
    )?;
    unrecorded.extend(untracked_paths(&status.stdout));
    unrecorded.sort();
    unrecorded.dedup();
    Ok(unrecorded)
}

/// Stop tracking the workspace, under the repository lock that guards
/// git's worktree registration.
fn forget_workspace(workspace: &Workspace) -> Result<(), String> {
    let _lock = lock_repository(&workspace.lock_path)?;
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

/// The commit id of the workspace's working-copy commit, read without a
/// snapshot: before `record_files` it is the revision the harness left
/// the workspace on, after it its tree is the run's final state.
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
    // `perf.sh`) cost a jj process each. Every `present(<commit>)` yields
    // that commit or nothing, so fewer lines than commits means one is gone.
    for batch in workspace.caller_commits.chunks(COMMIT_QUERY_BATCH) {
        let revset = batch
            .iter()
            .map(|commit| format!("present({commit})"))
            .collect::<Vec<_>>()
            .join(" | ");
        let still_there = log_lines(&workspace.root, &visible(&revset), "commit_id")?;
        if still_there.len() != batch.len() {
            return Ok(true);
        }
    }
    if let Some(recorded) = &workspace.caller_head {
        let now = head_record(
            &workspace.root,
            &format!("change_id({})", recorded.change_id),
        )?;
        if now.as_ref() != Some(recorded) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The workspace's own change must still stand on the copy. A harness
/// that moved to another revision (`jj new main`, `jj edit default@`, or
/// `jj abandon @-` on the copy itself) left the run's lineage: a change
/// built from its files would revert the caller's work, and a snapshot
/// would rewrite somebody else's commit, so neither happens. The
/// workspace is forgotten, the folder stays, the flag is set, and the
/// error says so.
fn require_on_copy(
    workspace: &Workspace,
    outcome: &mut Outcome,
    lineage: &str,
) -> Result<(), String> {
    let root = &workspace.root;
    let before_snapshot = final_commit(workspace)?;
    let stands_on_copy = log_first(
        root,
        &visible(&format!("present({before_snapshot}) & ({lineage})")),
        "commit_id",
    )?
    .is_some();
    if stands_on_copy {
        return Ok(());
    }
    forget_workspace(workspace)?;
    outcome.outside_changes = Some(true);
    outcome.path = Some(workspace.path.clone());
    let change = log_first(root, &before_snapshot, "change_id")?.unwrap_or_default();
    Err(format!(
        "the workspace's change {change} does not stand on the run's copy: the harness moved to another revision (jj new or jj edit), so no change was built from its files, nothing was abandoned, and the folder stays"
    ))
}

/// The finish, step by step, writing into `outcome` as each fact is known.
fn finish_steps(workspace: &Workspace, outcome: &mut Outcome) -> Result<(), String> {
    let root = &workspace.root;
    if !workspace.added {
        // The copy exists and nothing else: it goes, and a half-made folder
        // with it.
        abandon_run_owned(
            workspace,
            outcome,
            &format!("{} ~ ::@", workspace.lineage()),
        )?;
        if workspace.path.exists() {
            std::fs::remove_dir_all(&workspace.path).map_err(|error| {
                format!(
                    "cannot remove workspace folder {}: {error}",
                    workspace.path.display()
                )
            })?;
        }
        return Ok(());
    }
    let Some(copy) = &workspace.copy else {
        return Err("the workspace was added without a prepared copy".to_string());
    };
    let lineage = workspace.lineage();
    require_on_copy(workspace, outcome, &lineage)?;
    outcome.unrecorded = record_files(workspace)?;
    let final_commit = final_commit(workspace)?;
    forget_workspace(workspace)?;
    // The run's change is built from trees: a new commit on the recorded
    // copy commit (which resolves hidden or rewritten) takes the
    // workspace's final tree, so its diff is what the harness changed,
    // whatever the harness did to the revisions in between.
    let marker = result_marker(&workspace.name);
    jj_root_command(
        root,
        &[
            OsStr::new("new"),
            OsStr::new(&copy.commit_id),
            OsStr::new("--no-edit"),
            OsStr::new("-m"),
            OsStr::new(&workspace.description),
        ],
    )?;
    let (result, _) = ids_of(
        root,
        &format!(
            "children({}) & description(substring:\"{marker}\")",
            copy.commit_id
        ),
    )?;
    outcome.change_id = Some(result.clone());
    jj_root_command(
        root,
        &[
            OsStr::new("restore"),
            OsStr::new("--from"),
            OsStr::new(&final_commit),
            OsStr::new("--to"),
            OsStr::new(&result),
        ],
    )?;
    let kept = !flag(root, &result, "empty")?;
    outcome.kept = Some(kept);
    outcome.conflicted = Some(false);
    if kept {
        jj_root_command(
            root,
            &[
                OsStr::new("rebase"),
                OsStr::new("-r"),
                OsStr::new(&result),
                OsStr::new("-o"),
                OsStr::new("@"),
            ],
        )?;
        outcome.conflicted = Some(flag(root, &result, "conflict")?);
    }
    outcome.outside_changes = Some(caller_commits_changed(workspace)?);
    // A file jj did not record is never deleted, and no revision is
    // abandoned then: the folder stays, and the result names a change that
    // still exists.
    if !outcome.unrecorded.is_empty() {
        outcome.path = Some(workspace.path.clone());
        return Ok(());
    }
    // The copy under every commit it had (a harness's squash rewrites it
    // beside the recorded commit), the workspace's own change, and whatever
    // the run made on top of either. The caller's line stays, and so does
    // a kept result; a revision that existed before the run stays too.
    let kept_result = if kept {
        format!(" ~ change_id({result})")
    } else {
        String::new()
    };
    abandon_run_owned(workspace, outcome, &format!("{lineage} ~ ::@{kept_result}"))?;
    std::fs::remove_dir_all(&workspace.path).map_err(|error| {
        format!(
            "cannot remove workspace folder {}: {error}",
            workspace.path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refused_and_untracked_files_are_read_from_jj_output() {
        let stderr = concat!(
            "Warning: Refused to snapshot some files:\n",
            "  big.bin: 1.9MiB (2000000 bytes); the maximum size allowed is 1.0MiB (1048576 bytes)\n",
            "  out/huge.iso: 3.0GiB (3221225472 bytes); the maximum size allowed is 1.0MiB (1048576 bytes)\n",
            "Hint: This is to prevent large files from being added by accident. To fix this:\n",
            "  * Add the file(s) to `.gitignore`\n",
            "Snapshot complete.\n",
        );
        assert_eq!(refused_paths(stderr), ["big.bin", "out/huge.iso"]);
        assert_eq!(refused_paths("Snapshot complete.\n"), Vec::<String>::new());

        let status = concat!(
            "Working copy changes:\n",
            "A small.txt\n",
            "Untracked paths:\n",
            "? big.bin\n",
            "? notes/draft.md\n",
            "Working copy  (@) : zqysnyzq ee341d4e rune run x\n",
            "Parent commit (@-): smlzuoxt 38ef79a6 base\n",
        );
        assert_eq!(untracked_paths(status), ["big.bin", "notes/draft.md"]);
        assert_eq!(
            untracked_paths("Working copy changes:\nA small.txt\n"),
            Vec::<String>::new()
        );
    }

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
        assert_eq!(visible("abc"), "(abc) & ::visible_heads()");
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
    fn detect_finds_the_jj_root_above_a_subfolder_and_sees_colocation() {
        let root = tempfile::tempdir().expect("root");
        std::fs::create_dir_all(root.path().join(".jj")).expect(".jj");
        let nested = root.path().join("src/deep");
        std::fs::create_dir_all(&nested).expect("nested");
        assert_eq!(
            detect(&nested),
            Ok(Vcs::Jj {
                root: root.path().to_path_buf(),
                colocated: false
            })
        );
        std::fs::write(root.path().join(".git"), "gitdir: elsewhere\n").expect(".git file");
        assert_eq!(
            detect(&nested),
            Ok(Vcs::Jj {
                root: root.path().to_path_buf(),
                colocated: true
            })
        );
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
