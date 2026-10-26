//! The gpg program `jj sign` runs for the queue: the signature itself
//! checks the commit. jj hands every payload to the program named under
//! `signing.backends.gpg.program`, and the signer pins that setting, for
//! the one run, to a wrapper that runs `rune sign gpg-shim`. The shim reads
//! the payload as a git commit object, matches it against the identities
//! the signer wrote for the run, and runs the owner's gpg program with the
//! same arguments only for a match. Any other payload is refused before
//! the key, so `jj sign` fails and nothing outside the set is signed,
//! whatever a revset alias resolved to between the alias guard and the
//! run, and whatever another jj process did to the operation store
//! meanwhile. The guard stays as a second layer.
//!
//! On jj 0.46 the payload is not the stored commit. jj rewrites a commit
//! before it signs it, with a fresh committer line, and a later commit of
//! a range carries the signed rewrite of the earlier one as its parent. So
//! the payload's hash never equals a queued id. The shim matches the
//! identity fields instead, as `signed_exactly` does after a run, and maps
//! parents through the ids of the signed objects it has already produced:
//! the payload with the signature as a `gpgsig` header, hashed as git
//! does. `tests/sign_queue.rs` proves both facts against jj itself.

use super::repo::{name_commit, parse_commit, rewrites};
use super::store::{Identity, state_dir};
use rune::error::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What the signer writes before `jj sign` and the shim updates after
/// every payload: one file per run, under the state directory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct State {
    /// The owner's gpg program, as the user-scope jj config names it.
    pub program: String,
    /// The commits the owner saw and the signer named.
    pub named: Vec<Identity>,
    /// Named commit id to the id of its signed rewrite.
    pub signed: BTreeMap<String, String>,
    /// Why the shim refused a payload, when it did.
    pub refused: Option<String>,
}

/// The files of one run: the state and the wrapper jj runs as gpg.
pub(crate) struct Run {
    directory: PathBuf,
    state: PathBuf,
    program: PathBuf,
}

impl Run {
    /// Write the state and the wrapper for one run.
    pub(crate) fn create(program: &str, named: Vec<Identity>) -> Result<Self, Error> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let directory = state_dir()?
            .join("sign-queue")
            .join("shim")
            .join(format!("{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", directory.display()),
            )
        })?;
        let run = Self {
            state: directory.join("state.json"),
            program: directory.join("gpg"),
            directory,
        };
        write_state(
            &run.state,
            &State {
                program: program.to_string(),
                named,
                signed: BTreeMap::new(),
                refused: None,
            },
        )?;
        let rune = std::env::current_exe()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot locate rune: {error}")))?;
        let script = format!(
            "#!/bin/sh\nexec {} sign gpg-shim --state {} -- \"$@\"\n",
            crate::cli::shell_quote(&rune.to_string_lossy()),
            crate::cli::shell_quote(&run.state.to_string_lossy())
        );
        std::fs::write(&run.program, script).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", run.program.display()),
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&run.program, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!("cannot make {} executable: {error}", run.program.display()),
                    )
                })?;
        }
        Ok(run)
    }

    /// The wrapper's path, for `signing.backends.gpg.program`.
    pub(crate) fn program(&self) -> &Path {
        &self.program
    }

    /// Read what the shim recorded, then remove the run's files.
    pub(crate) fn finish(self) -> Result<State, Error> {
        let state = read_state(&self.state);
        let _ = std::fs::remove_dir_all(&self.directory);
        state
    }
}

/// `rune sign gpg-shim --state <file> -- <gpg arguments>`: the payload on
/// stdin, the signature on stdout, the exit status gpg's. A call that is
/// not a signing request, as a verification is, goes to the owner's
/// program unchanged.
pub(crate) fn run(state_path: &Path, args: &[String]) -> Result<i32, Error> {
    let mut state = read_state(state_path)?;
    let mut payload = Vec::new();
    std::io::stdin()
        .read_to_end(&mut payload)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read the payload: {error}")))?;
    if is_sign_request(args) {
        let id = object_id(&payload);
        let identity = parse_commit(&id, &String::from_utf8_lossy(&payload)).ok();
        let matched = identity
            .as_ref()
            .and_then(|identity| matched(&state, identity))
            .map(|named| named.commit_id.clone());
        let Some(named) = matched else {
            let reason = format!(
                "jj sign asked the key for {}, which is not a named commit: refused before the key",
                identity.as_ref().map_or_else(
                    || format!("an object that is not a commit ({})", super::short(&id)),
                    name_commit
                )
            );
            state.refused = Some(reason.clone());
            write_state(state_path, &state)?;
            eprintln!("rune: {reason}");
            return Ok(2);
        };
        let output = forward(&state.program, args, &payload)?;
        if output.status.success() {
            state
                .signed
                .insert(named, object_id(&signed_object(&payload, &output.stdout)));
            write_state(state_path, &state)?;
        }
        return relay(&output);
    }
    relay(&forward(&state.program, args, &payload)?)
}

/// Whether jj asked for a detached signature: it passes `-abu <key>`, or
/// `-ab` without a key, and nothing else does. (jj 0.46, `gpg_signing.rs`,
/// `run_sign_command`.)
pub(crate) fn is_sign_request(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "-ab" || arg == "-abu")
}

/// The named commit the payload is the rewrite of, under the rewrites the
/// shim has recorded: the same match as `signed_exactly` after a run, one
/// payload at a time. A named commit matches once.
pub(crate) fn matched<'a>(state: &'a State, payload: &Identity) -> Option<&'a Identity> {
    let rewrite: BTreeMap<&str, &str> = state
        .signed
        .iter()
        .map(|(named, signed)| (named.as_str(), signed.as_str()))
        .collect();
    state.named.iter().find(|named| {
        !state.signed.contains_key(&named.commit_id) && rewrites(named, payload, &rewrite)
    })
}

/// The id git gives a commit object with this content.
pub(crate) fn object_id(payload: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("commit {}\0", payload.len()).as_bytes());
    hasher.update(payload);
    hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// The commit object jj writes for a signed payload: the payload's headers,
/// then `gpgsig` with the signature's first line, every later line behind
/// one space, and the message. The signature's final newline ends the last
/// line and adds none. (jj 0.46 through gix, confirmed against the object
/// jj writes in `tests/sign_queue.rs`.)
pub(crate) fn signed_object(payload: &[u8], signature: &[u8]) -> Vec<u8> {
    let split = payload
        .windows(2)
        .position(|window| window == b"\n\n")
        .map_or(payload.len(), |at| at + 1);
    let (headers, message) = payload.split_at(split);
    let mut object = headers.to_vec();
    object.extend_from_slice(b"gpgsig ");
    let body = signature.strip_suffix(b"\n").unwrap_or(signature);
    for (index, line) in body.split(|byte| *byte == b'\n').enumerate() {
        if index > 0 {
            object.push(b' ');
        }
        object.extend_from_slice(line.strip_suffix(b"\r").unwrap_or(line));
        object.push(b'\n');
    }
    object.extend_from_slice(message);
    object
}

/// Run the owner's program with jj's arguments and the payload.
fn forward(program: &str, args: &[String], payload: &[u8]) -> Result<std::process::Output, Error> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run {program}: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A program that exits early closes its end; its status says why.
        let _ = stdin.write_all(payload);
    }
    child
        .wait_with_output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run {program}: {error}")))
}

/// Pass the program's output and status through to jj.
fn relay(output: &std::process::Output) -> Result<i32, Error> {
    let mut stderr = std::io::stderr();
    let _ = stderr.write_all(&output.stderr);
    let mut stdout = std::io::stdout();
    stdout
        .write_all(&output.stdout)
        .and_then(|()| stdout.flush())
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write the signature: {error}"),
            )
        })?;
    Ok(output.status.code().unwrap_or(1))
}

fn read_state(path: &Path) -> Result<State, Error> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("cannot parse {}: {error}", path.display()),
        )
    })
}

fn write_state(path: &Path, state: &State) -> Result<(), Error> {
    let body = serde_json::to_string_pretty(state).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot encode the shim state: {error}"),
        )
    })?;
    crate::cli::config::write_atomic(path, body)
}
