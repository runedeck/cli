//! The jj workspace a workspace-write run works in. The harness changes
//! only its own copy of the repository, and the owner reviews the result as
//! one jj change on top of the caller's working-copy change, named after
//! the route and the session. An empty run leaves no trace.
//!
//! The workspace starts from a copy of the caller's change (`jj
//! duplicate`), a sibling that the caller's own snapshots never rewrite,
//! so the workspace cannot go stale while the harness works. At the end
//! every revision the run made, the harness's own commits included, is
//! folded into one change, rebased onto the caller's current change, and
//! the copy is abandoned.

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
}

/// One isolated run as planned: the dry run prints it, `prepare` creates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) vcs: Vcs,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) description: String,
    /// The change id of the caller's working-copy change, which the run
    /// copies at the start and lands its result on at the end. `None`
    /// until jj is colocated.
    pub(crate) parent_change_id: Option<String>,
    /// The lock every run of this repository takes while it registers or
    /// forgets a workspace.
    lock_path: PathBuf,
}

/// A workspace that exists. `finish` takes it down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workspace {
    /// The root of the caller's jj workspace, where `jj workspace` commands run.
    pub(crate) root: PathBuf,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    description: String,
    /// Whether this run colocated jj in a plain git repository.
    pub(crate) colocated: bool,
    /// The change id of the copy of the caller's change that the workspace
    /// started from.
    copy: String,
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

/// The git top level of `path`, when git knows it as a repository.
fn git_root(path: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!root.is_empty()).then(|| PathBuf::from(root))
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

/// The folder under the state directory that holds every workspace of one
/// repository: its name and a short digest of its path, so two checkouts
/// with one name stay apart.
fn repository_id(root: &Path) -> String {
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    let short = digest.iter().take(6).fold(String::new(), |mut hex, byte| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    let name = root.file_name().map_or_else(
        || "root".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!("{name}-{short}")
}

/// The description of the run's change: the route and the session on the
/// first line, the workspace name below, so the owner can tell the runs
/// apart in `jj log`.
fn description(route: &str, session: Option<&str>, name: &str) -> String {
    let subject = match session {
        Some(session) if !session.trim().is_empty() => format!("rune run {route}: {session}"),
        _ => format!("rune run {route}"),
    };
    format!("{subject}\n\nworkspace: {name}\n")
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
/// that folder, with its stderr as the error when it fails. A missing jj
/// binary is named as such, since every workspace-write run needs it.
fn jj_in(cwd: &Path, args: &[&OsStr]) -> Result<JjOutput, String> {
    let output = Command::new("jj")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| {
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

/// The full change id of `revset` in the caller's repository, or `None`
/// when no visible revision matches.
fn change_id(root: &Path, revset: &str) -> Result<Option<String>, String> {
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(revset),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new("change_id ++ \"\\n\""),
        ],
    )?
    .stdout;
    Ok(printed
        .lines()
        .next()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string))
}

/// The change ids `revset` names, one per line.
fn change_ids(root: &Path, revset: &str) -> Result<Vec<String>, String> {
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(revset),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new("change_id ++ \"\\n\""),
        ],
    )?
    .stdout;
    Ok(printed
        .lines()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect())
}

/// One boolean template keyword (`empty`, `conflict`) of a revision.
fn flag(root: &Path, revset: &str, keyword: &str) -> Result<bool, String> {
    let printed = jj_root_command(
        root,
        &[
            OsStr::new("log"),
            OsStr::new("-r"),
            OsStr::new(revset),
            OsStr::new("--no-graph"),
            OsStr::new("-T"),
            OsStr::new(keyword),
        ],
    )?
    .stdout;
    match printed.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "jj log printed {other:?} for {keyword}, expected true or false"
        )),
    }
}

/// The short change id in a jj line such as `Duplicated 8cffebf67191 as
/// smlzuoxt 38ef79a6 base` or `Created new commit yzsqusmy d82dfbfa ...`:
/// the word after `marker`.
fn id_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.split_once(marker)
        .and_then(|(_, rest)| rest.split_whitespace().next())
}

/// The change id of the caller's working-copy change, read without a
/// snapshot: the id does not move when the files do.
fn parent_change_id(root: &Path) -> Result<String, String> {
    change_id(root, "@")?
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
    let repository_id = repository_id(vcs.root());
    let workspaces = state.join("workspaces");
    let path = workspaces.join(&repository_id).join(&name);
    refuse_inside_checkout(&path, vcs.root())?;
    let lock_path = workspaces.join(format!("{repository_id}.lock"));
    let parent_change_id = match &vcs {
        Vcs::Jj { root, .. } => Some(parent_change_id(root)?),
        Vcs::GitOnly { .. } => None,
    };
    Ok(Plan {
        description: description(route, session, &name),
        vcs,
        name,
        path,
        parent_change_id,
        lock_path,
    })
}

/// Copy the caller's working-copy change, latest edits included: `jj
/// duplicate` snapshots the checkout first and makes a sibling that the
/// caller's later snapshots do not rewrite.
fn duplicate_caller_change(root: &Path, description: &str) -> Result<String, String> {
    let printed = jj_in(
        root,
        &[
            OsStr::new("-R"),
            root.as_os_str(),
            OsStr::new("duplicate"),
            OsStr::new("@"),
        ],
    )?;
    let short = printed
        .stderr
        .lines()
        .chain(printed.stdout.lines())
        .find_map(|line| id_after(line, "Duplicated ").and_then(|_| id_after(line, " as ")))
        .ok_or_else(|| format!("jj duplicate named no copy: {}", printed.stderr.trim()))?;
    let copy = change_id(root, short)?
        .ok_or_else(|| format!("jj duplicate named a copy jj does not list: {short}"))?;
    // A leftover copy after a kill names the run, not the caller's change.
    jj_root_command(
        root,
        &[
            OsStr::new("describe"),
            OsStr::new(&copy),
            OsStr::new("-m"),
            OsStr::new(&format!(
                "{description}\nbase: copy of the caller's change\n"
            )),
        ],
    )?;
    Ok(copy)
}

/// Create the planned workspace: colocate jj first when the repository has
/// only git, copy the caller's change, and add a workspace on the copy.
pub(crate) fn prepare(plan: &Plan) -> Result<Workspace, String> {
    let root = plan.vcs.root().to_path_buf();
    if let Some(parent) = plan.path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create workspace folder {}: {error}",
                parent.display()
            )
        })?;
    }
    let _lock = lock_repository(&plan.lock_path)?;
    let colocated = match &plan.vcs {
        Vcs::Jj { colocated, .. } => *colocated,
        // Another run may have colocated the repository while this one
        // waited for the lock.
        Vcs::GitOnly { .. } if root.join(".jj").is_dir() => true,
        Vcs::GitOnly { .. } => {
            jj_in(
                &root,
                &[
                    OsStr::new("git"),
                    OsStr::new("init"),
                    OsStr::new("--colocate"),
                    root.as_os_str(),
                ],
            )?;
            true
        }
    };
    let copy = duplicate_caller_change(&root, &plan.description)?;
    // `workspace add` refuses `--ignore-working-copy`: it snapshots the
    // caller's checkout once more, which changes nothing the copy holds.
    let mut args = vec![
        OsStr::new("-R"),
        root.as_os_str(),
        OsStr::new("workspace"),
        OsStr::new("add"),
        OsStr::new("--name"),
        OsStr::new(&plan.name),
        OsStr::new("--revision"),
        OsStr::new(&copy),
        OsStr::new("--message"),
        OsStr::new(&plan.description),
    ];
    if colocated {
        args.push(OsStr::new("--colocate"));
    }
    args.push(plan.path.as_os_str());
    if let Err(error) = jj_in(&root, &args) {
        // The copy is the run's own: nothing of the caller's is lost with
        // it, and a half-made folder holds nothing yet.
        let _ = abandon_existing(&root, &[copy]);
        let _ = std::fs::remove_dir_all(&plan.path);
        return Err(error);
    }
    Ok(Workspace {
        root,
        name: plan.name.clone(),
        path: plan.path.clone(),
        description: plan.description.clone(),
        colocated: plan.vcs.colocate(),
        copy,
        lock_path: plan.lock_path.clone(),
    })
}

/// Abandon the revisions among `ids` that still exist. A harness's own
/// `jj abandon`, or a forget that dropped an empty undescribed change,
/// may have removed one already.
fn abandon_existing(root: &Path, ids: &[String]) -> Result<(), String> {
    let mut existing = Vec::new();
    for id in ids {
        if change_id(root, id).is_ok_and(|found| found.is_some()) {
            existing.push(id.clone());
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    let mut args = vec![OsStr::new("abandon")];
    args.extend(existing.iter().map(OsStr::new));
    jj_root_command(root, &args).map(|_| ())
}

/// Take the workspace down after the run, whatever its outcome: record the
/// harness's files, stop tracking the workspace, fold every revision the
/// run made into one change on top of the caller's current change, drop
/// the copy and an empty result, and remove the folder. A step that fails
/// stops the rest, so nothing the harness wrote is lost: the outcome keeps
/// what the finish learned before the failure, names the error, and names
/// the folder that stays.
pub(crate) fn finish(workspace: &Workspace) -> Outcome {
    let mut outcome = Outcome::default();
    if let Err(error) = finish_steps(workspace, &mut outcome) {
        outcome.error = Some(error);
        outcome.path = Some(workspace.path.clone());
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

/// The finish, step by step, writing into `outcome` as each fact is known.
fn finish_steps(workspace: &Workspace, outcome: &mut Outcome) -> Result<(), String> {
    outcome.unrecorded = record_files(workspace)?;
    let root = &workspace.root;
    {
        let _lock = lock_repository(&workspace.lock_path)?;
        jj_root_command(
            root,
            &[
                OsStr::new("workspace"),
                OsStr::new("forget"),
                OsStr::new(&workspace.name),
            ],
        )?;
    }
    // One change takes everything the run made: the workspace's own
    // change, and any commit the harness made with jj or git.
    let created = jj_root_command(
        root,
        &[
            OsStr::new("new"),
            OsStr::new(&workspace.copy),
            OsStr::new("--no-edit"),
            OsStr::new("-m"),
            OsStr::new(&workspace.description),
        ],
    )?;
    let short = created
        .stderr
        .lines()
        .chain(created.stdout.lines())
        .find_map(|line| id_after(line, "Created new commit "))
        .ok_or_else(|| format!("jj new named no change: {}", created.stderr.trim()))?;
    let folded = change_id(root, short)?
        .ok_or_else(|| format!("jj new named a change jj does not list: {short}"))?;
    outcome.change_id = Some(folded.clone());
    let run_revisions = format!("({copy}:: ~ {copy}) ~ {folded}", copy = workspace.copy);
    if !change_ids(root, &run_revisions)?.is_empty() {
        jj_root_command(
            root,
            &[
                OsStr::new("squash"),
                OsStr::new("--from"),
                OsStr::new(&run_revisions),
                OsStr::new("--into"),
                OsStr::new(&folded),
                OsStr::new("--use-destination-message"),
            ],
        )?;
    }
    let kept = !flag(root, &folded, "empty")?;
    outcome.kept = Some(kept);
    if kept {
        jj_root_command(
            root,
            &[
                OsStr::new("rebase"),
                OsStr::new("-r"),
                OsStr::new(&folded),
                OsStr::new("-o"),
                OsStr::new("@"),
            ],
        )?;
        outcome.conflicted = Some(flag(root, &folded, "conflict")?);
        abandon_existing(root, std::slice::from_ref(&workspace.copy))?;
    } else {
        outcome.conflicted = Some(false);
        abandon_existing(root, &[folded.clone(), workspace.copy.clone()])?;
    }
    // A file jj did not record is never deleted: the folder stays, and the
    // result names it with the files.
    if !outcome.unrecorded.is_empty() {
        outcome.path = Some(workspace.path.clone());
        return Ok(());
    }
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
    fn the_change_id_is_read_after_the_marker() {
        assert_eq!(
            id_after("Duplicated 8cffebf67191 as smlzuoxt 38ef79a6 base", " as "),
            Some("smlzuoxt")
        );
        assert_eq!(
            id_after(
                "Created new commit yzsqusmy d82dfbfa (empty) rune run x",
                "Created new commit "
            ),
            Some("yzsqusmy")
        );
        assert_eq!(id_after("Nothing changed.", " as "), None);
    }

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
    fn the_description_names_the_route_the_session_and_the_workspace() {
        let text = description("sol@claude", Some("council/sol"), "run-1-2");
        assert_eq!(
            text,
            "rune run sol@claude: council/sol\n\nworkspace: run-1-2\n"
        );
        let bare = description("codex", None, "run-1-2");
        assert!(bare.starts_with("rune run codex\n"), "{bare}");
        let blank = description("codex", Some("  "), "run-1-2");
        assert_eq!(blank, bare);
    }

    #[test]
    fn the_workspace_name_starts_with_the_prefix() {
        let name = workspace_name();
        assert!(name.starts_with(NAME_PREFIX), "{name}");
        assert!(name.len() > NAME_PREFIX.len());
    }

    #[test]
    fn repository_id_keeps_the_name_and_tells_two_paths_apart() {
        let one = repository_id(Path::new("/home/me/cli"));
        let two = repository_id(Path::new("/home/you/cli"));
        assert!(one.starts_with("cli-"), "{one}");
        assert_ne!(one, two);
        assert_eq!(one, repository_id(Path::new("/home/me/cli")));
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
