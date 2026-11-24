//! The harnesses Rune captures, where each keeps its sessions, and the
//! files of a store with the session id and the working folder each file
//! belongs to. The layouts are the ones each harness had on 2026-11-02.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::exclude::{self, CompressedCwd};

/// A harness whose sessions Rune copies. `OpenCode` and Gemini are not
/// here: the owner dropped them on 2026-11-02.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Harness {
    Claude,
    Codex,
    Grok,
    Antigravity,
}

impl Harness {
    pub const ALL: [Self; 4] = [Self::Claude, Self::Codex, Self::Grok, Self::Antigravity];

    /// The harness by its name: the hook dispatcher's spelling, the
    /// runner's `agy`, or the folder name in the data folder.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "grok" => Some(Self::Grok),
            "antigravity" | "agy" => Some(Self::Antigravity),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::Antigravity => "antigravity",
        }
    }

    /// Whether the harness keeps its sessions in databases, which take one
    /// consistent backup per day and no version per sweep.
    #[must_use]
    pub fn database(self) -> bool {
        matches!(self, Self::Antigravity)
    }
}

impl std::fmt::Display for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where each harness keeps its sessions on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stores {
    pub claude_projects: PathBuf,
    pub claude_plans: PathBuf,
    pub codex_sessions: PathBuf,
    pub grok_sessions: PathBuf,
    pub antigravity_conversations: PathBuf,
}

impl Stores {
    /// The stores under `home`, moved by the variables each harness reads:
    /// `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and `ANTIGRAVITY_EXECUTABLE_DATA_DIR`.
    pub fn resolve(home: &Path, env: &dyn Fn(&str) -> Option<OsString>) -> Self {
        let claude = env("CLAUDE_CONFIG_DIR").map_or_else(|| home.join(".claude"), PathBuf::from);
        let codex = env("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from);
        let antigravity = env("ANTIGRAVITY_EXECUTABLE_DATA_DIR")
            .map_or_else(|| home.join(".gemini/antigravity-cli"), PathBuf::from);
        Self {
            claude_projects: claude.join("projects"),
            claude_plans: claude.join("plans"),
            codex_sessions: codex.join("sessions"),
            grok_sessions: home.join(".grok/sessions"),
            antigravity_conversations: antigravity.join("conversations"),
        }
    }

    /// The store root of one harness.
    #[must_use]
    pub fn root(&self, harness: Harness) -> &Path {
        match harness {
            Harness::Claude => &self.claude_projects,
            Harness::Codex => &self.codex_sessions,
            Harness::Grok => &self.grok_sessions,
            Harness::Antigravity => &self.antigravity_conversations,
        }
    }
}

/// How a replaced native copy is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An append-only session file: a version is kept only for lost bytes.
    Append,
    /// A compressed session file, complete when its bytes are equal.
    Compressed,
    /// A database file, overwritten without a version.
    Database,
}

/// The kind of a file by its name.
#[must_use]
pub fn kind_of(path: &Path) -> Kind {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if has_suffix(name, ".zst") {
        Kind::Compressed
    } else if DATABASE_SUFFIXES
        .iter()
        .any(|suffix| has_suffix(name, suffix))
    {
        Kind::Database
    } else {
        Kind::Append
    }
}

const DATABASE_SUFFIXES: [&str; 3] = [".db", ".db-wal", ".db-shm"];

/// A file name suffix test that ignores case, as the harnesses name their
/// files in lower case and a file system may not.
fn has_suffix(name: &str, suffix: &str) -> bool {
    name.len() >= suffix.len() && name[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

fn has_any_suffix(name: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|suffix| has_suffix(name, suffix))
}

/// One file of a harness's store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFile {
    pub path: PathBuf,
    /// The path under the store root, kept in the native copy.
    pub relative: PathBuf,
    /// The session the file belongs to, when the layout names one.
    pub session: Option<String>,
    /// The working folder of the session, when the file or the layout
    /// records one. A compressed file has none here: [`Self::excluded`]
    /// reads it when a path glob needs it.
    pub cwd: Option<PathBuf>,
}

impl SessionFile {
    /// Whether the exclude configuration keeps the file out of the data
    /// folder: its session id is listed, or its working folder matches a
    /// glob.
    ///
    /// The working folder of a compressed file is read here and not in
    /// the listing, because the read decompresses the start of the file.
    /// A listing covers the whole store, and most captures copy one
    /// session of it. So the read happens only for a file that a capture
    /// selected, and only while a path glob is set. A compressed file
    /// that cannot be read then stays out: its working folder is unknown,
    /// and it can be a session of an excluded folder.
    #[must_use]
    pub fn excluded(&self, exclude: &exclude::Exclude) -> bool {
        if exclude.matches(self.session.as_deref(), self.cwd.as_deref()) {
            return true;
        }
        if exclude.paths.is_empty() || kind_of(&self.path) != Kind::Compressed {
            return false;
        }
        match exclude::cwd_in_compressed_file(&self.path) {
            CompressedCwd::Found(cwd) => exclude.folder(&cwd),
            CompressedCwd::Absent => false,
            CompressedCwd::Unreadable => true,
        }
    }
}

/// The files of a store. `None` when the store folder is absent, which a
/// sweep treats as nothing to do.
pub fn list(harness: Harness, stores: &Stores) -> Result<Option<Vec<SessionFile>>, String> {
    let root = stores.root(harness);
    if !root.is_dir() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    walk(root, &mut paths)?;
    paths.retain(|path| wanted(harness, path));
    paths.sort();
    let files = match harness {
        Harness::Claude => claude_files(root, &paths),
        Harness::Codex => codex_files(root, &paths),
        Harness::Grok => grok_files(root, &paths),
        Harness::Antigravity => antigravity_files(root, &paths),
    };
    Ok(Some(files))
}

/// The files that belong to `session`, or every file when no session is
/// named; then the files modified at or after `since`.
#[must_use]
pub fn select(
    files: Vec<SessionFile>,
    session: Option<&str>,
    since: Option<SystemTime>,
) -> Vec<SessionFile> {
    files
        .into_iter()
        .filter(|file| session.is_none_or(|id| file.session.as_deref() == Some(id)))
        .filter(|file| since.is_none_or(|since| modified_at_or_after(&file.path, since)))
        .collect()
}

/// Whether a file of the store is a session file the harness keeps.
fn wanted(harness: Harness, path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match harness {
        Harness::Claude => has_any_suffix(name, &[".jsonl", ".json"]),
        Harness::Codex => has_any_suffix(name, &[".jsonl", ".jsonl.zst", ".json"]),
        Harness::Grok => has_any_suffix(name, &[".jsonl", ".json", ".txt"]),
        Harness::Antigravity => has_any_suffix(name, &DATABASE_SUFFIXES),
    }
}

/// Claude Code: `projects/<encoded cwd>/<session>.jsonl`, the session's
/// own folder `<session>/` beside it with subagent files, and `*.json`.
/// The working folder comes from the session file's lines.
fn claude_files(root: &Path, paths: &[PathBuf]) -> Vec<SessionFile> {
    let mut cwd_by_session: BTreeMap<PathBuf, Option<PathBuf>> = BTreeMap::new();
    paths
        .iter()
        .map(|path| {
            let relative = relative_to(root, path);
            let mut components = relative.components();
            let project = components.next().map(|c| PathBuf::from(c.as_os_str()));
            let second = components.next().map(|c| c.as_os_str().to_os_string());
            let rest = components.next().is_some();
            let session = second.as_ref().and_then(|second| {
                let second = second.to_string_lossy();
                if rest {
                    Some(second.into_owned())
                } else {
                    second
                        .strip_suffix(".jsonl")
                        .or_else(|| second.strip_suffix(".json"))
                        .map(str::to_string)
                }
            });
            let cwd = match (&project, &session) {
                (Some(project), Some(session)) => {
                    let key = project.join(session);
                    cwd_by_session
                        .entry(key)
                        .or_insert_with(|| {
                            let main = root.join(project).join(format!("{session}.jsonl"));
                            exclude::cwd_in_file(&main).or_else(|| exclude::cwd_in_file(path))
                        })
                        .clone()
                }
                _ => exclude::cwd_in_file(path),
            };
            SessionFile {
                path: path.clone(),
                relative,
                session,
                cwd,
            }
        })
        .collect()
}

/// Codex: `sessions/YYYY/MM/DD/rollout-<stamp>-<uuid>.jsonl`, compressed
/// to `.jsonl.zst` when older. The uuid is the session id, and the
/// `session_meta` line names the working folder. A compressed file is
/// not read here: see [`SessionFile::excluded`].
fn codex_files(root: &Path, paths: &[PathBuf]) -> Vec<SessionFile> {
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stem = name.split('.').next().unwrap_or(name);
            let session = codex_session_id(stem);
            let cwd = (kind_of(path) != Kind::Compressed)
                .then(|| exclude::cwd_in_file(path))
                .flatten();
            SessionFile {
                path: path.clone(),
                relative: relative_to(root, path),
                session: Some(session),
                cwd,
            }
        })
        .collect()
}

/// The uuid at the end of a Codex rollout name, else the whole stem.
fn codex_session_id(stem: &str) -> String {
    if stem.len() > 36 {
        let (_, tail) = stem.split_at(stem.len() - 36);
        if looks_like_uuid(tail) {
            return tail.to_string();
        }
    }
    stem.to_string()
}

fn looks_like_uuid(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(part, len)| part.len() == len && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Grok: `sessions/<percent-encoded cwd>/<session>/chat_history.jsonl`
/// and its siblings. The folder name is the working folder.
fn grok_files(root: &Path, paths: &[PathBuf]) -> Vec<SessionFile> {
    paths
        .iter()
        .map(|path| {
            let relative = relative_to(root, path);
            let mut components = relative.components();
            let encoded = components
                .next()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
            let second = components
                .next()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
            let rest = components.next().is_some();
            let session = rest.then_some(second).flatten();
            let cwd = encoded
                .as_deref()
                .and_then(exclude::percent_decode)
                .filter(|decoded| decoded.starts_with('/'))
                .map(PathBuf::from);
            SessionFile {
                path: path.clone(),
                relative,
                session,
                cwd,
            }
        })
        .collect()
}

/// Antigravity: `conversations/<conversation>.db` with its `-wal` and
/// `-shm` files. The conversation id is the session; no working folder.
fn antigravity_files(root: &Path, paths: &[PathBuf]) -> Vec<SessionFile> {
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            SessionFile {
                path: path.clone(),
                relative: relative_to(root, path),
                session: antigravity_session(name).map(str::to_string),
                cwd: None,
            }
        })
        .collect()
}

/// The conversation id in the name of an Antigravity database file or of
/// its `-wal` or `-shm` file. The daily backup reads the same id, so one
/// exclusion covers the native copy and the backup.
#[must_use]
pub fn antigravity_session(name: &str) -> Option<&str> {
    name.split(".db").next().filter(|id| !id.is_empty())
}

fn relative_to(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}

/// Every regular file under `root`. Symlinks are skipped: the copy reads
/// what the harness wrote, never what a link points at.
pub(super) fn walk(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(root)
        .map_err(|error| format!("cannot read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", root.display()))?;
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            walk(&path, out)?;
        } else if metadata.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Second granularity: a filesystem may keep no finer time, and the
/// harness's clock may trail the runner's by less than that.
fn modified_at_or_after(path: &Path, since: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| whole_seconds(modified) >= whole_seconds(since))
}

fn whole_seconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
