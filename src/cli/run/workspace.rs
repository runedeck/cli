//! The jj workspace a workspace-write run works in. The harness changes
//! only its own copy of the repository, and the owner reviews the result as
//! one jj change: a child of the caller's working-copy change, named after
//! the route and the session. An empty workspace leaves no trace.

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
    /// The change id of the caller's working-copy change, which the run's
    /// change is a child of. `None` until jj is colocated.
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
    /// Whether this run colocated jj in a plain git repository.
    pub(crate) colocated: bool,
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

/// What the run left behind: one change when it changed files, nothing
/// when it did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub(crate) change_id: String,
    pub(crate) kept: bool,
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

/// One jj command, with its stderr as the error when it fails. A missing
/// jj binary is named as such, since every workspace-write run needs it.
fn jj(args: &[&OsStr]) -> Result<String, String> {
    let output = Command::new("jj").args(args).output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "jj is not on the PATH, and a workspace-write run needs it to isolate the harness; install jj, or use --mode read-only".to_string()
        } else {
            format!("cannot run jj: {error}")
        }
    })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
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

fn jj_args<'a>(parts: impl IntoIterator<Item = &'a OsStr>) -> Vec<&'a OsStr> {
    parts.into_iter().collect()
}

/// The change id of the caller's working-copy change, read without a
/// snapshot: the id does not move when the files do.
fn parent_change_id(root: &Path) -> Result<String, String> {
    let output = jj(&jj_args([
        OsStr::new("-R"),
        root.as_os_str(),
        OsStr::new("--ignore-working-copy"),
        OsStr::new("log"),
        OsStr::new("-r"),
        OsStr::new("@"),
        OsStr::new("--no-graph"),
        OsStr::new("-T"),
        OsStr::new("change_id"),
    ]))?;
    let id = output.trim().to_string();
    if id.is_empty() {
        return Err("jj log printed no change id for the working copy".to_string());
    }
    Ok(id)
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

/// Create the planned workspace: colocate jj first when the repository has
/// only git, then add a workspace whose working-copy change is a child of
/// the caller's.
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
            jj(&jj_args([
                OsStr::new("git"),
                OsStr::new("init"),
                OsStr::new("--colocate"),
                root.as_os_str(),
            ]))?;
            true
        }
    };
    let mut args = vec![
        OsStr::new("-R"),
        root.as_os_str(),
        OsStr::new("workspace"),
        OsStr::new("add"),
        OsStr::new("--name"),
        OsStr::new(&plan.name),
        OsStr::new("--revision"),
        OsStr::new("@"),
        OsStr::new("--message"),
        OsStr::new(&plan.description),
    ];
    if colocated {
        args.push(OsStr::new("--colocate"));
    }
    args.push(plan.path.as_os_str());
    jj(&args)?;
    Ok(Workspace {
        root,
        name: plan.name.clone(),
        path: plan.path.clone(),
        colocated: plan.vcs.colocate(),
        lock_path: plan.lock_path.clone(),
    })
}

/// `<change id>\n<empty>` as `jj log` prints it for the run's change.
fn parse_change(output: &str) -> Result<(String, bool), String> {
    let mut lines = output.lines().map(str::trim);
    let change_id = lines
        .next()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "jj log printed no change id for the workspace".to_string())?;
    let kept = match lines.next() {
        Some("true") => false,
        Some("false") => true,
        other => {
            return Err(format!(
                "jj log printed {other:?} for the empty flag, expected true or false"
            ));
        }
    };
    Ok((change_id.to_string(), kept))
}

/// Take the workspace down after the run, whatever its outcome: record the
/// harness's files, stop tracking the workspace, drop an empty change, and
/// remove the folder. A step that fails stops the rest, so nothing the
/// harness wrote is lost: the error names the workspace that stays.
pub(crate) fn finish(workspace: &Workspace) -> Result<Outcome, String> {
    let inside = workspace.path.as_os_str();
    jj(&jj_args([
        OsStr::new("-R"),
        inside,
        OsStr::new("util"),
        OsStr::new("snapshot"),
    ]))?;
    let printed = jj(&jj_args([
        OsStr::new("-R"),
        inside,
        OsStr::new("--ignore-working-copy"),
        OsStr::new("log"),
        OsStr::new("-r"),
        OsStr::new("@"),
        OsStr::new("--no-graph"),
        OsStr::new("-T"),
        OsStr::new("change_id ++ \"\\n\" ++ empty"),
    ]))?;
    let (change_id, kept) = parse_change(&printed)?;
    // The caller's checkout is not snapshotted again: its files did not take
    // part in the run.
    let root = workspace.root.as_os_str();
    {
        let _lock = lock_repository(&workspace.lock_path)?;
        jj(&jj_args([
            OsStr::new("-R"),
            root,
            OsStr::new("--ignore-working-copy"),
            OsStr::new("workspace"),
            OsStr::new("forget"),
            OsStr::new(&workspace.name),
        ]))?;
    }
    if !kept {
        jj(&jj_args([
            OsStr::new("-R"),
            root,
            OsStr::new("--ignore-working-copy"),
            OsStr::new("abandon"),
            OsStr::new(&change_id),
        ]))?;
    }
    std::fs::remove_dir_all(&workspace.path).map_err(|error| {
        format!(
            "cannot remove workspace folder {}: {error}",
            workspace.path.display()
        )
    })?;
    Ok(Outcome { change_id, kept })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_workspace_is_kept_and_an_empty_one_dropped() {
        assert_eq!(
            parse_change("kpvmnyqtwzlrnwnryorsproqnolqktwx\nfalse\n"),
            Ok(("kpvmnyqtwzlrnwnryorsproqnolqktwx".to_string(), true))
        );
        assert_eq!(
            parse_change("kpvmnyqtwzlrnwnryorsproqnolqktwx\ntrue"),
            Ok(("kpvmnyqtwzlrnwnryorsproqnolqktwx".to_string(), false))
        );
        assert!(parse_change("").is_err());
        assert!(parse_change("abc\nmaybe").is_err());
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
    fn detect_refuses_a_folder_under_no_version_control() {
        let folder = tempfile::tempdir().expect("folder");
        let error = detect(folder.path()).expect_err("no repository");
        assert!(error.contains("--mode read-only"), "{error}");
        assert!(error.contains("jj git init --colocate"), "{error}");
    }
}
