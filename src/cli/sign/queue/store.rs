//! The request store: one JSON file per signing request under the user
//! state directory, a claim inside each request, and one lock file per
//! repository so two signers cannot work one repository at once.

use rune::error::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A claim older than this is reclaimable even when its process cannot be
/// checked, so a wedged signer never holds a request forever.
pub(crate) const CLAIM_MAX_AGE_SECONDS: i64 = 60 * 60;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Claim {
    pub pid: u32,
    pub started_at: String,
    /// The head's commit id as observed under the lock: what gets signed.
    /// The recorded head keeps the id the receipt names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub path: String,
    pub digest: String,
    pub exit_line: String,
}

/// The identity a request records for its head: everything a rewrite must
/// preserve except the commit id and the signature.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Identity {
    pub change_id: String,
    pub commit_id: String,
    pub tree_id: String,
    pub author: String,
    pub description: String,
    pub parents: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Queued,
    Signed,
    Failed,
}

/// What the owner's touch produces for a request.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// The recorded head is signed in place (CLI-0041).
    #[default]
    Head,
    /// An open-seal is signed above the head, pushed, and the draft pull
    /// request flips to ready with the nonce in its body.
    Open,
    /// A merge-seal is signed above the reviewed head. Nothing pushes.
    Merge,
}

/// What `rune sign open --queue` recorded for the owner to complete.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OpenRequest {
    pub repo: String,
    pub base: String,
    pub pull_request: u64,
    /// The validated body, kept beside the request so the flip appends the
    /// nonce to the text the session validated.
    pub body: String,
}

/// The ledger state `rune sign submit` observed and recorded on a request,
/// shown to the owner before the key.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Coverage {
    pub repo: String,
    pub pull_request: u64,
    pub base: String,
    pub reviewed_sha: String,
    pub generation: u64,
    /// `clean` or `free-lanes-only`.
    pub verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub lanes: std::collections::BTreeMap<String, String>,
    pub threads: Vec<Thread>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Thread {
    pub id: String,
    #[serde(default)]
    pub lane: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Request {
    pub id: String,
    pub repository: String,
    pub workspace: String,
    pub bookmark: String,
    pub head: Identity,
    /// Absent only for an `open` request, which the draft and the body
    /// qualify instead of a check log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
    pub requested_at: String,
    pub status: Status,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<OpenRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Coverage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

pub(crate) struct Store {
    root: PathBuf,
}

impl Store {
    /// `RUNE_STATE_DIR` wins, then `XDG_STATE_HOME`, then `~/.local/state`.
    pub(crate) fn open() -> Result<Self, Error> {
        let root = state_dir()?.join("sign-queue");
        fs::create_dir_all(root.join("requests"))
            .and_then(|()| fs::create_dir_all(root.join("locks")))
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", root.display()),
                )
            })?;
        Ok(Self { root })
    }

    #[cfg(test)]
    pub(crate) fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The ledger directory, named in every message about an empty queue,
    /// so a wrong `RUNE_STATE_DIR` never reads as nothing to sign.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn request_path(&self, id: &str) -> PathBuf {
        self.root.join("requests").join(format!("{id}.json"))
    }

    pub(crate) fn load_all(&self) -> Result<Vec<Request>, Error> {
        let directory = self.root.join("requests");
        let mut requests = Vec::new();
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(requests),
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("cannot read {}: {error}", directory.display()),
                ));
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|error| {
                    Error::new(ErrorKind::Io, format!("cannot list requests: {error}"))
                })?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                requests.push(read_request(&path)?);
            }
        }
        // Request time first, the id as a tie-break, so the order is the
        // same on every listing.
        requests.sort_by(|left, right| {
            (&left.requested_at, &left.id).cmp(&(&right.requested_at, &right.id))
        });
        Ok(requests)
    }

    pub(crate) fn load(&self, id: &str) -> Result<Request, Error> {
        read_request(&self.request_path(id))
    }

    /// Write through a temporary file so a reader never sees a torn record.
    pub(crate) fn save(&self, request: &Request) -> Result<(), Error> {
        let path = self.request_path(&request.id);
        let temporary = path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(request).map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot encode request: {error}"))
        })?;
        fs::write(&temporary, body)
            .and_then(|()| fs::rename(&temporary, &path))
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot write {}: {error}", path.display()),
                )
            })
    }

    pub(crate) fn remove(&self, id: &str) -> Result<(), Error> {
        let path = self.request_path(id);
        fs::remove_file(&path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot remove {}: {error}", path.display()),
            )
        })
    }

    /// One signer per repository, held as an OS file lock on the open file
    /// for as long as the signer runs, so a crash releases it and nothing
    /// can be stolen: a second signer reads the holder line and stops.
    pub(crate) fn lock_repository(&self, repository: &str) -> Result<RepositoryLock, Error> {
        let path = self
            .root
            .join("locks")
            .join(format!("{}.lock", short_digest(repository)));
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot open {}: {error}", path.display()),
                )
            })?;
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                let holder = fs::read_to_string(&path).unwrap_or_default();
                let (pid, started_at) = split_holder(&holder);
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("another signer holds {repository}: process {pid} since {started_at}"),
                ));
            }
            Err(fs::TryLockError::Error(error)) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("cannot lock {}: {error}", path.display()),
                ));
            }
        }
        let holder = format!("{} {}", std::process::id(), now());
        file.set_len(0)
            .and_then(|()| std::io::Write::write_all(&mut file, holder.as_bytes()))
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot write {}: {error}", path.display()),
                )
            })?;
        Ok(RepositoryLock { _file: file })
    }
}

/// The lock file stays on disk: deleting it would let a signer that opened
/// the old inode and one that created a new file hold the lock at once.
pub(crate) struct RepositoryLock {
    _file: fs::File,
}

fn read_request(path: &Path) -> Result<Request, Error> {
    let body = fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    serde_json::from_str(&body).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("{} is not a signing request: {error}", path.display()),
        )
    })
}

fn state_dir() -> Result<PathBuf, Error> {
    if let Some(dir) = std::env::var_os("RUNE_STATE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Some(dir) = std::env::var_os("XDG_STATE_HOME").filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir).join("rune"));
    }
    dirs::home_dir()
        .map(|home| home.join(".local/state/rune"))
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))
}

/// Millisecond precision, so two requests from one round keep their order.
pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub(crate) fn short_digest(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    hex_prefix(&digest, 16)
}

pub(crate) fn file_digest(path: &Path) -> Result<String, Error> {
    let bytes = fs::read(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    Ok(hex_prefix(&Sha256::digest(&bytes), 64))
}

fn hex_prefix(bytes: &[u8], length: usize) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex.truncate(length);
    hex
}

/// A request id: short, unique enough for one machine, and safe as a file
/// name whatever the bookmark is called.
pub(crate) fn new_request_id(repository: &str, bookmark: &str) -> String {
    short_digest(&format!(
        "{repository}\n{bookmark}\n{}\n{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        std::process::id()
    ))
}

pub(crate) fn split_holder(holder: &str) -> (u32, &str) {
    let mut fields = holder.split_whitespace();
    let pid = fields.next().and_then(|pid| pid.parse().ok()).unwrap_or(0);
    (pid, fields.next().unwrap_or(""))
}

/// A claim is dead when its process no longer exists or it has outlived
/// the maximum claim age. An unparseable start time counts as expired.
pub(crate) fn claim_is_dead(pid: u32, started_at: &str) -> bool {
    if pid == 0 || !process_alive(pid) {
        return true;
    }
    chrono::DateTime::parse_from_rfc3339(started_at)
        .map(|started| {
            chrono::Utc::now()
                .signed_duration_since(started)
                .num_seconds()
        })
        .is_ok_and(|age| age > CLAIM_MAX_AGE_SECONDS)
        || chrono::DateTime::parse_from_rfc3339(started_at).is_err()
}

fn process_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
