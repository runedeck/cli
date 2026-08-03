//! Git history, Entire checkpoint enrichment, and deploy-time source recovery.

use super::sidecar::{Sidecar, parse_adoption};
use super::source::resolve_sidecar;
use crate::manifest;
use crate::view::{Adoption, GitCommit};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// A git invocation pinned to the given repository. Ambient `GIT_DIR`,
/// `GIT_WORK_TREE`, and `GIT_INDEX_FILE` (exported into hook environments)
/// would otherwise retarget the call at the enclosing repository.
fn git_in(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    command
}

/// Number of commits fetched by each background history request.
pub const DEFAULT_HISTORY_BATCH_SIZE: usize = 200;

/// Maximum number of commit records retained by the background walker.
pub const DEFAULT_HISTORY_METADATA_WINDOW: usize = DEFAULT_HISTORY_BATCH_SIZE * 3;

/// Selects whether history covers the whole deck repository or selected paths.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HistoryScope {
    #[default]
    Deck,
    Paths(Vec<PathBuf>),
}

/// Batch and memory bounds for a [`HistoryWalker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryOptions {
    pub batch_size: usize,
    pub metadata_window: usize,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_HISTORY_BATCH_SIZE,
            metadata_window: DEFAULT_HISTORY_METADATA_WINDOW,
        }
    }
}

/// A commit and its short ref decorations (for example `HEAD -> main`).
#[derive(Debug, Clone, Default)]
pub struct HistoryEntry {
    pub commit: GitCommit,
    pub refs: Vec<String>,
}

/// A bounded snapshot emitted after each history batch.
///
/// `window_start` is the absolute index of `entries[0]` in newest-first
/// history. Once `total_loaded` exceeds the configured metadata window, old
/// entries are evicted from the front and `window_start` advances.
#[derive(Debug, Clone, Default)]
pub struct HistoryUpdate {
    pub window_start: usize,
    pub total_loaded: usize,
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
struct HistoryWorkerState {
    has_more: AtomicBool,
    request_pending: AtomicBool,
}

/// Background, request-driven git history loader.
///
/// Construction starts the first batch. Call [`Self::request_more`] when the
/// viewport approaches the end of the current window, then poll the update
/// channel with [`Self::try_recv`] or [`Self::recv_timeout`].
pub struct HistoryWalker {
    requests: Sender<()>,
    updates: Receiver<HistoryUpdate>,
    state: Arc<HistoryWorkerState>,
    _worker: JoinHandle<()>,
}

impl HistoryWalker {
    /// Starts a walker with a batch size of 200 and a 600-entry metadata window.
    pub fn start(repo: impl Into<PathBuf>, scope: HistoryScope) -> std::io::Result<Self> {
        Self::with_options(repo, scope, HistoryOptions::default())
    }

    /// Starts a walker with explicit batching and metadata-window bounds.
    pub fn with_options(
        repo: impl Into<PathBuf>,
        scope: HistoryScope,
        options: HistoryOptions,
    ) -> std::io::Result<Self> {
        let options = HistoryOptions {
            batch_size: options.batch_size.max(1),
            metadata_window: options.metadata_window.max(options.batch_size.max(1)),
        };
        let repo = repo.into();
        let revision = history_revision(&repo)?;
        let (request_sender, request_receiver) = mpsc::channel();
        let (update_sender, update_receiver) = mpsc::channel();
        let state = Arc::new(HistoryWorkerState {
            has_more: AtomicBool::new(true),
            request_pending: AtomicBool::new(true),
        });
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("rune-history".to_string())
            .spawn(move || {
                walk_history(
                    &repo,
                    &revision,
                    &scope,
                    options,
                    &request_receiver,
                    &update_sender,
                    &worker_state,
                );
            })?;
        Ok(Self {
            requests: request_sender,
            updates: update_receiver,
            state,
            _worker: worker,
        })
    }

    /// Queues one more batch. Returns `false` when a request is already pending
    /// or the worker has reached the end of history.
    pub fn request_more(&self) -> bool {
        if !self.state.has_more.load(Ordering::Acquire)
            || self
                .state
                .request_pending
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        if self.requests.send(()).is_err() {
            self.state.request_pending.store(false, Ordering::Release);
            self.state.has_more.store(false, Ordering::Release);
            return false;
        }
        true
    }

    pub fn try_recv(&self) -> Result<HistoryUpdate, TryRecvError> {
        self.updates.try_recv()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<HistoryUpdate, RecvTimeoutError> {
        self.updates.recv_timeout(timeout)
    }
}

fn walk_history(
    repo: &Path,
    revision: &str,
    scope: &HistoryScope,
    options: HistoryOptions,
    requests: &Receiver<()>,
    updates: &Sender<HistoryUpdate>,
    state: &HistoryWorkerState,
) {
    let mut window = VecDeque::with_capacity(options.metadata_window);
    let mut total_loaded = 0;
    loop {
        let batch =
            match load_history_batch(repo, revision, scope, total_loaded, options.batch_size) {
                Ok(batch) => batch,
                Err(error) => {
                    state.has_more.store(false, Ordering::Release);
                    state.request_pending.store(false, Ordering::Release);
                    let _ = updates.send(HistoryUpdate {
                        window_start: total_loaded.saturating_sub(window.len()),
                        total_loaded,
                        entries: window.into_iter().collect(),
                        has_more: false,
                        error: Some(error),
                    });
                    return;
                }
            };
        let has_more = batch.len() > options.batch_size;
        let loaded = batch.len().min(options.batch_size);
        window.extend(batch.into_iter().take(loaded));
        total_loaded += loaded;
        if window.len() > options.metadata_window {
            window.drain(..window.len() - options.metadata_window);
        }

        state.has_more.store(has_more, Ordering::Release);
        state.request_pending.store(false, Ordering::Release);
        if updates
            .send(HistoryUpdate {
                window_start: total_loaded.saturating_sub(window.len()),
                total_loaded,
                entries: window.iter().cloned().collect(),
                has_more,
                error: None,
            })
            .is_err()
            || !has_more
        {
            return;
        }
        if requests.recv().is_err() {
            return;
        }
    }
}

fn load_history_batch(
    repo: &Path,
    revision: &str,
    scope: &HistoryScope,
    skip: usize,
    batch_size: usize,
) -> Result<Vec<HistoryEntry>, String> {
    let mut command = git_in(repo);
    command
        .arg("log")
        .arg(format!("--skip={skip}"))
        .arg("-n")
        .arg((batch_size + 1).to_string())
        .arg("--decorate=short")
        .arg(HISTORY_GIT_LOG_FORMAT)
        .arg(revision);
    if let HistoryScope::Paths(paths) = scope
        && !paths.is_empty()
    {
        command.arg("--").args(paths);
    }
    let output = command
        .output()
        .map_err(|error| format!("could not start git log: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log failed: {}", detail.trim()));
    }
    let mut entries = parse_history_log(&String::from_utf8_lossy(&output.stdout));
    for entry in &mut entries {
        enrich_commits_with_entire(repo, std::slice::from_mut(&mut entry.commit));
    }
    Ok(entries)
}

fn history_revision(repo: &Path) -> std::io::Result<String> {
    let output = git_in(repo)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "cannot resolve history revision: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

const HISTORY_GIT_LOG_FORMAT: &str = "--format=%H%x00%s%x00%ai%x00%an%x00%(trailers:key=Entire-Checkpoint,valueonly,separator=%x20)%x00%D";

fn parse_history_log(raw: &str) -> Vec<HistoryEntry> {
    raw.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut fields = line.split('\u{0}');
            let sha = fields.next()?.to_string();
            if sha.is_empty() {
                return None;
            }
            let commit = GitCommit {
                sha,
                message: fields.next().unwrap_or_default().to_string(),
                date: fields.next().unwrap_or_default().to_string(),
                author: fields.next().unwrap_or_default().to_string(),
                checkpoint: fields.next().unwrap_or_default().trim().to_string(),
                ..GitCommit::default()
            };
            let refs = fields
                .next()
                .unwrap_or_default()
                .split(", ")
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();
            Some(HistoryEntry { commit, refs })
        })
        .collect()
}

/// Reads the source-repo adoption sidecar for a deployed artifact.
/// `skills/X/SKILL.md` -> `skills/X/.provenance/SKILL.yaml`,
/// `agents/X.md` -> `agents/.provenance/X.yaml`.
pub fn read_source_adoption(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> Option<Adoption> {
    parse_adoption(&read_source_sidecar(source_uri, source_path, local_repos)?)
}

/// Returns the raw provenance sidecar YAML for a source artifact, or `None`.
pub fn read_source_sidecar(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> Option<String> {
    let normalized = source_uri.trim_end_matches(".git");
    let repo_path = local_repos.get(normalized)?;
    let file_rel = Path::new(source_path?);
    let parent_dir = repo_path.join(file_rel.parent()?);
    let sidecar = resolve_sidecar(&parent_dir, file_rel)?;
    fs::read_to_string(&sidecar).ok()
}

pub fn extract_frontmatter_field(content: &str, field: &str) -> String {
    let Some(rest) = content.strip_prefix("---") else {
        return String::new();
    };
    let Some(end) = rest.find("\n---") else {
        return String::new();
    };
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix(&format!("{field}:")) {
            return value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
    }
    String::new()
}

/// Reads the first `resolvedDependencies` digest from an `assemble/v1`
/// sidecar (the source input SHA captured at deploy time).
pub(super) fn recorded_input_sha(sidecar_content: &str) -> Option<String> {
    let sidecar: Sidecar = serde_yaml::from_str(sidecar_content).ok()?;
    sidecar
        .provenance
        .predicate
        .build_definition
        .resolved_dependencies
        .first()
        .map(|dependency| dependency.digest.sha256.clone())
        .filter(|sha| !sha.is_empty())
}

/// Source file content at the commit that was deployed, found by matching the
/// deployed sidecar's recorded input hash (`recorded_sha`) against the source
/// file's recent git history. Returns `None` when the current source already
/// matches (no drift) or the deploy commit is not in recent history.
pub fn source_at_deploy(
    recorded_sha: &str,
    source_uri: &str,
    source_path: &str,
    local_repos: &HashMap<String, PathBuf>,
) -> Option<String> {
    let repo = local_repos.get(source_uri.trim_end_matches(".git"))?;
    let current = fs::read_to_string(repo.join(source_path)).ok()?;
    if manifest::content_sha256(&current) == recorded_sha {
        return None;
    }
    // Bounded history scan: if the deploy commit is older than this window the
    // drift diff is silently unavailable (the artifact still shows as stale via
    // its provenance). A richer "deploy predates history" signal is a follow-up.
    for sha in recent_commit_shas(repo, source_path, 200) {
        if let Some(content) = git_show_file(repo, &sha, source_path)
            && manifest::content_sha256(&content) == recorded_sha
        {
            return Some(content);
        }
    }
    None
}

/// Recent commit SHAs touching a file, newest first.
fn recent_commit_shas(repo: &Path, file_rel: &str, limit: usize) -> Vec<String> {
    let output = git_in(repo)
        .args([
            "log",
            "--follow",
            "-n",
            &limit.to_string(),
            "--format=%H",
            "--",
            file_rel,
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// File content at a specific commit (`git show {sha}:{path}`).
fn git_show_file(repo: &Path, sha: &str, path: &str) -> Option<String> {
    let output = git_in(repo)
        .args(["show", &format!("{sha}:{path}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn git_log_for_artifact(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> Vec<GitCommit> {
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo_path) = local_repos.get(normalized) else {
        return Vec::new();
    };
    let Some(file_path) = source_path else {
        return Vec::new();
    };
    let output = git_in(repo_path)
        .args([
            "log",
            "--follow",
            "-n",
            "5",
            GIT_LOG_FORMAT,
            "--",
            file_path,
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut commits = parse_git_log(&String::from_utf8_lossy(&output.stdout));
    enrich_commits_with_entire(repo_path, &mut commits);
    commits
}

/// One NUL-delimited record per commit: sha, subject, author-date, author,
/// `Entire-Checkpoint` trailer. NUL fields survive subjects containing any
/// printable character; records are newline-separated (every field is
/// single-line).
pub(super) const GIT_LOG_FORMAT: &str =
    "--format=%H%x00%s%x00%ai%x00%an%x00%(trailers:key=Entire-Checkpoint,valueonly,separator=%x20)";

pub(super) fn parse_git_log(raw: &str) -> Vec<GitCommit> {
    raw.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut fields = line.split('\u{0}');
            let sha = fields.next()?.to_string();
            if sha.is_empty() {
                return None;
            }
            Some(GitCommit {
                message: fields.next().unwrap_or_default().to_string(),
                date: fields.next().unwrap_or_default().to_string(),
                author: fields.next().unwrap_or_default().to_string(),
                checkpoint: fields.next().unwrap_or_default().trim().to_string(),
                sha,
                ..GitCommit::default()
            })
        })
        .collect()
}

/// Fills the agent-intent facets (`prompt`, `session_count`) for every commit
/// that carries an `Entire-Checkpoint` trailer, reading the checkpoint's
/// sessions from the committed `entire/checkpoints/v1` branch. Commits without
/// a checkpoint, or repos without the branch, are left untouched.
pub(super) fn enrich_commits_with_entire(repo: &Path, commits: &mut [GitCommit]) {
    for commit in commits.iter_mut() {
        if commit.checkpoint.len() < 3 {
            continue;
        }
        let (shard, rest) = commit.checkpoint.split_at(2);
        let base = format!("entire/checkpoints/v1:{shard}/{rest}");
        let mut sessions: Vec<usize> = git_show_lines(repo, &format!("{base}/"))
            .into_iter()
            .filter_map(|name| name.trim_end_matches('/').parse::<usize>().ok())
            .collect();
        sessions.sort_unstable();
        commit.session_count = sessions.len();
        commit.prompt = checkpoint_prompt(repo, &base, &sessions);
    }
}

/// Picks a one-line intent teaser from a checkpoint's sessions: the first
/// session prompt that is not a compaction-continuation summary, falling back
/// to the first session's opening line.
fn checkpoint_prompt(repo: &Path, base: &str, sessions: &[usize]) -> String {
    let mut fallback = String::new();
    for index in sessions {
        let prompt = git_show(repo, &format!("{base}/{index}/prompt.txt")).unwrap_or_default();
        let first_line = prompt
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        if first_line.is_empty() {
            continue;
        }
        if fallback.is_empty() {
            fallback = first_line.to_string();
        }
        if !first_line.starts_with("This session is being continued") {
            return truncate_prompt(first_line);
        }
    }
    truncate_prompt(&fallback)
}

fn truncate_prompt(line: &str) -> String {
    const LIMIT: usize = 110;
    if line.chars().count() <= LIMIT {
        return line.to_string();
    }
    let cut: String = line.chars().take(LIMIT).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// Runs `git show <object>` in a repo, returning its stdout or `None` on any
/// failure (missing branch, missing path, non-utf8).
fn git_show(repo: &Path, object: &str) -> Option<String> {
    let output = git_in(repo).args(["show", object]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Lists the entry names directly under a tree object (`git show <tree>/`).
fn git_show_lines(repo: &Path, tree: &str) -> Vec<String> {
    git_show(repo, tree)
        .map(|text| {
            text.lines()
                .skip_while(|line| !line.trim().is_empty())
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::process::Stdio;
    use std::time::Duration;

    const FULL_SLSA_SIDECAR: &str = "provenance:\n  _type: https://in-toto.io/Statement/v1\n  predicateType: https://slsa.dev/provenance/v1\n  subject:\n    - name: rules/Demo.md\n      digest:\n        sha256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n  predicate:\n    buildDefinition:\n      buildType: https://github.com/runedeck/rune/assemble/v1\n      externalParameters:\n        invocation:\n          configSource: deck.yaml\n      resolvedDependencies:\n        - name: source\n          uri: git+https://example.test/repo@main\n          digest:\n            sha256: abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\n    runDetails:\n      builder:\n        id: https://github.com/runedeck/rune\n      metadata:\n        startedOn: 2026-07-14T10:00:00Z\n        finishedOn: 2026-07-14T10:00:01Z\n";

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            // A pre-push hook exports GIT_DIR and GIT_WORK_TREE; inherited,
            // they retarget the fixture's git calls at the enclosing repo.
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn history_fixture(commit_count: usize) -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("fixture repo");
        git(repo.path(), &["init", "-q"]);
        git(repo.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);

        let mut input = String::new();
        for index in 0..commit_count {
            let blob = format!("fixture {index}\n");
            let blob_mark = index * 2 + 1;
            let commit_mark = index * 2 + 2;
            let message = format!("commit {index}\n");
            let path = if index % 2 == 0 {
                "tracked.txt"
            } else {
                "other.txt"
            };
            let timestamp = 1_700_000_000 + index;
            write!(
                input,
                "blob\nmark :{blob_mark}\ndata {}\n{blob}\n\
                 commit refs/heads/main\nmark :{commit_mark}\n\
                 author Fixture <fixture@example.com> {timestamp} +0000\n\
                 committer Fixture <fixture@example.com> {timestamp} +0000\n\
                 data {}\n{message}\nM 100644 :{blob_mark} {path}\n\n",
                blob.len(),
                message.len()
            )
            .expect("write fast-import fixture");
        }

        let mut child = Command::new("git")
            .args(["fast-import", "--quiet"])
            .current_dir(repo.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("git fast-import should start");
        child
            .stdin
            .take()
            .expect("fast-import stdin")
            .write_all(input.as_bytes())
            .expect("write fast-import stream");
        let output = child.wait_with_output().expect("wait for fast-import");
        assert!(
            output.status.success(),
            "git fast-import failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        git(repo.path(), &["tag", "--no-sign", "fixture-tip", "HEAD"]);
        repo
    }

    #[test]
    fn parse_git_log_captures_checkpoint_trailer() {
        let raw = "abc123\u{0}feat: add thing\u{0}2026-06-12 10:21:46 +0200\u{0}Alice Example\u{0}933ba0519d0a\n\
                   def456\u{0}fix: tidy up\u{0}2026-06-03 01:47:07 +0200\u{0}Alice Example\u{0}\n";
        let commits = parse_git_log(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "abc123");
        assert_eq!(commits[0].message, "feat: add thing");
        assert_eq!(commits[0].author, "Alice Example");
        assert_eq!(commits[0].checkpoint, "933ba0519d0a");
        assert!(commits[1].checkpoint.is_empty());
    }

    #[test]
    fn parse_git_log_skips_blank_and_empty_sha_lines() {
        let raw = "\nabc123\u{0}subject\u{0}date\u{0}author\u{0}\n\u{0}orphan\u{0}\u{0}\u{0}\n";
        let commits = parse_git_log(raw);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].sha, "abc123");
    }

    #[test]
    fn read_source_sidecar_preserves_complete_slsa_payload() {
        let repo = tempfile::tempdir().expect("source repo");
        let provenance = repo.path().join("rules/.provenance");
        fs::create_dir_all(&provenance).expect("provenance directory");
        fs::write(provenance.join("Demo.yaml"), FULL_SLSA_SIDECAR)
            .expect("write provenance fixture");
        let source_uri = "https://example.test/repo";
        let repos = HashMap::from([(source_uri.to_string(), repo.path().to_path_buf())]);

        let raw = read_source_sidecar(source_uri, Some("rules/Demo.md"), &repos)
            .expect("canonical sidecar");

        assert_eq!(raw, FULL_SLSA_SIDECAR);
        assert!(raw.contains("predicateType: https://slsa.dev/provenance/v1"));
        assert!(raw.contains("id: https://github.com/runedeck/rune"));
        assert!(raw.contains("sha256: 0123456789abcdef"));
        assert!(raw.contains("invocation:"));
        assert!(raw.contains("metadata:"));
    }

    #[test]
    fn truncate_prompt_caps_long_lines_with_ellipsis() {
        let short = "tighten the sign gutter";
        assert_eq!(truncate_prompt(short), short);
        let long = "x".repeat(200);
        let capped = truncate_prompt(&long);
        assert!(capped.ends_with('\u{2026}'));
        assert!(capped.chars().count() <= 111);
    }

    #[test]
    fn history_walker_loads_500_commits_in_bounded_batches() {
        let repo = history_fixture(505);
        let walker = HistoryWalker::with_options(
            repo.path(),
            HistoryScope::Deck,
            HistoryOptions {
                batch_size: 200,
                metadata_window: 400,
            },
        )
        .expect("start history walker");

        let first = walker
            .recv_timeout(Duration::from_secs(5))
            .expect("first history batch");
        assert_eq!(first.window_start, 0);
        assert_eq!(first.total_loaded, 200);
        assert_eq!(first.entries.len(), 200);
        assert!(first.has_more);
        assert!(first.error.is_none());
        assert_eq!(first.entries[0].commit.message, "commit 504");
        assert!(
            first.entries[0]
                .refs
                .iter()
                .any(|name| name.contains("main"))
        );
        assert!(walker.request_more());

        let second = walker
            .recv_timeout(Duration::from_secs(5))
            .expect("second history batch");
        assert_eq!(second.window_start, 0);
        assert_eq!(second.total_loaded, 400);
        assert_eq!(second.entries.len(), 400);
        assert!(second.has_more);
        assert!(walker.request_more());

        let third = walker
            .recv_timeout(Duration::from_secs(5))
            .expect("final history batch");
        assert_eq!(third.window_start, 105);
        assert_eq!(third.total_loaded, 505);
        assert_eq!(third.entries.len(), 400);
        assert!(!third.has_more);
        assert_eq!(
            third.entries.last().expect("oldest commit").commit.message,
            "commit 0"
        );
        assert!(!walker.request_more());
    }

    #[test]
    fn history_walker_scopes_history_to_selected_paths() {
        let repo = history_fixture(505);
        let walker = HistoryWalker::with_options(
            repo.path(),
            HistoryScope::Paths(vec![PathBuf::from("tracked.txt")]),
            HistoryOptions {
                batch_size: 200,
                metadata_window: 400,
            },
        )
        .expect("start history walker");

        let first = walker
            .recv_timeout(Duration::from_secs(5))
            .expect("first path history batch");
        assert_eq!(first.total_loaded, 200);
        assert!(first.has_more);
        assert!(first.entries.iter().all(|entry| {
            entry.commit.message["commit ".len()..]
                .parse::<usize>()
                .is_ok_and(|index| index % 2 == 0)
        }));
        assert!(walker.request_more());

        let second = walker
            .recv_timeout(Duration::from_secs(5))
            .expect("final path history batch");
        assert_eq!(second.total_loaded, 253);
        assert_eq!(second.entries.len(), 253);
        assert!(!second.has_more);
        assert_eq!(
            second.entries.last().expect("oldest commit").commit.message,
            "commit 0"
        );
    }
}
