//! The owner's key ceremony as one command each: sealing a reviewed
//! branch with an empty signed commit, signing release tags, and
//! verifying either against the repository's `KEYS` file.
// A facet of the CLI root: the parent owns the module wiring and the
// shared helpers, and each facet uses them freely.
#[allow(clippy::wildcard_imports)]
use super::*;

use rune::error::ErrorKind;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SEAL_SUBJECT: &str = "seal: approve";

pub(crate) fn execute(
    amend: bool,
    tag: Option<&str>,
    commit: Option<&str>,
    verify: Option<&str>,
) -> Result<i32, Error> {
    if let Some(reference) = verify {
        return verify_reference(reference);
    }
    if let Some(name) = tag {
        return sign_tag(name, tag_target(commit));
    }
    if amend {
        return amend_head();
    }
    seal_branch()
}

fn seal_branch() -> Result<i32, Error> {
    let branch = current_branch()?;
    ensure_index_clean()?;
    run_signing_git(&["commit", "--allow-empty", "-S", "-m", SEAL_SUBJECT])?;
    run_interactive_git(&["verify-commit", "HEAD"])?;
    let remote = branch_push_remote(&branch)?;
    let refspec = branch_push_refspec(&branch);
    run_interactive_git(&["push", &remote, &refspec])?;
    let head = git_stdout(&["rev-parse", "HEAD"])?;
    println!("sealed {branch} @ {head}");
    Ok(0)
}

/// A signature is part of the commit object, so signing an existing commit
/// rewrites it: the head keeps its message and author, gains the owner's
/// signature, and replaces its pushed predecessor under a lease.
fn amend_head() -> Result<i32, Error> {
    let branch = current_branch()?;
    ensure_index_clean()?;
    run_signing_git(&["commit", "--amend", "--no-edit", "--allow-empty", "-S"])?;
    run_interactive_git(&["verify-commit", "HEAD"])?;
    let remote = branch_push_remote(&branch)?;
    let refspec = branch_push_refspec(&branch);
    run_interactive_git(&["push", "--force-with-lease", &remote, &refspec])?;
    let head = git_stdout(&["rev-parse", "HEAD"])?;
    println!("re-signed {branch} @ {head}");
    Ok(0)
}

fn ensure_index_clean() -> Result<(), Error> {
    if index_is_clean(None)? {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Config,
        "staged changes are present: unstage them before signing",
    ))
}

pub(crate) fn index_is_clean(repository: Option<&Path>) -> Result<bool, Error> {
    let mut command = Command::new("git");
    if let Some(repository) = repository {
        // An explicit repository must win over inherited hook context, where
        // GIT_DIR and friends would silently redirect -C to the outer repo.
        command
            .arg("-C")
            .arg(repository)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE");
    }
    let status = command
        .args(["diff", "--cached", "--quiet", "--exit-code"])
        .status()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if status.success() {
        return Ok(true);
    }
    if status.code() == Some(1) {
        return Ok(false);
    }
    Err(Error::new(
        ErrorKind::Io,
        "git could not inspect the staged changes",
    ))
}

fn sign_tag(name: &str, commit: &str) -> Result<i32, Error> {
    run_signing_git(&["tag", "--sign", "--message", name, name, commit])?;
    run_interactive_git(&["verify-tag", name])?;
    let remote = tag_push_remote()?;
    let refspec = tag_push_refspec(name);
    run_interactive_git(&["push", &remote, &refspec])?;
    println!("signed tag {name} pushed to {remote}");
    Ok(0)
}

fn verify_reference(reference: &str) -> Result<i32, Error> {
    let allowed = keys_fingerprints(&repository_keys_path()?)?;
    let raw_status = if is_tag(reference) {
        git_raw_verification(&["verify-tag", "--raw", reference])
    } else {
        git_raw_verification(&["verify-commit", "--raw", reference])
    };
    let Some(raw_status) = raw_status? else {
        println!("{reference}: unsigned or invalid signature");
        return Ok(1);
    };
    let Some(fingerprints) = signature_fingerprints(&raw_status) else {
        println!("{reference}: unsigned or invalid signature");
        return Ok(1);
    };
    if fingerprints.iter().any(|print| allowed.contains(print)) {
        println!("{reference}: valid signature from a KEYS key");
        return Ok(0);
    }
    println!(
        "{reference}: foreign signature {}",
        fingerprints.last().map_or("", String::as_str)
    );
    Ok(1)
}

fn current_branch() -> Result<String, Error> {
    git_stdout(&["symbolic-ref", "--short", "HEAD"]).map_err(|_| {
        Error::new(
            ErrorKind::Config,
            "HEAD is detached: seal a branch, not a commit",
        )
    })
}

fn branch_push_remote(branch: &str) -> Result<String, Error> {
    for key in [
        format!("branch.{branch}.pushRemote"),
        "remote.pushDefault".to_string(),
        format!("branch.{branch}.remote"),
    ] {
        if let Some(remote) = git_config_value(&key)? {
            return Ok(remote);
        }
    }
    default_push_remote()
}

fn tag_push_remote() -> Result<String, Error> {
    match git_stdout(&["symbolic-ref", "--short", "HEAD"]) {
        Ok(branch) => branch_push_remote(&branch),
        Err(_) => default_push_remote(),
    }
}

fn default_push_remote() -> Result<String, Error> {
    let configured = git_config_value("remote.pushDefault")?;
    let remote_output = git_stdout(&["remote"])?;
    let remotes: Vec<&str> = remote_output.lines().collect();
    select_default_remote(configured.as_deref(), &remotes).ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "no default push remote: configure remote.pushDefault or add origin",
        )
    })
}

fn git_config_value(key: &str) -> Result<Option<String>, Error> {
    let output = Command::new("git")
        .args(["config", "--get", key])
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if output.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ));
    }
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    Err(Error::new(
        ErrorKind::Io,
        format!(
            "git config failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    ))
}

pub(crate) fn select_default_remote(configured: Option<&str>, remotes: &[&str]) -> Option<String> {
    configured
        .map(str::to_string)
        .or_else(|| remotes.contains(&"origin").then(|| "origin".to_string()))
        .or_else(|| (remotes.len() == 1).then(|| remotes[0].to_string()))
}

pub(crate) fn branch_push_refspec(branch: &str) -> String {
    format!("refs/heads/{branch}:refs/heads/{branch}")
}

pub(crate) fn tag_push_refspec(name: &str) -> String {
    format!("refs/tags/{name}")
}

pub(crate) fn tag_target(commit: Option<&str>) -> &str {
    commit.unwrap_or("HEAD")
}

pub(crate) fn tag_reference(reference: &str) -> String {
    if reference.starts_with("refs/tags/") {
        reference.to_string()
    } else {
        format!("refs/tags/{reference}")
    }
}

fn is_tag(reference: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--quiet", "--verify"])
        .arg(tag_reference(reference))
        .output()
        .is_ok_and(|output| output.status.success())
}

fn repository_keys_path() -> Result<PathBuf, Error> {
    let top_level = git_stdout(&["rev-parse", "--show-toplevel"])?;
    let path = Path::new(&top_level).join("KEYS");
    if path.exists() {
        Ok(path)
    } else {
        Err(Error::new(
            ErrorKind::Config,
            "no KEYS file at the repository root: verification needs the committed owner keys",
        ))
    }
}

/// Every fingerprint (primary and subkey) published in `KEYS`, read via
/// gpg's stable colon-delimited listing.
fn keys_fingerprints(keys_path: &Path) -> Result<Vec<String>, Error> {
    let output = Command::new("gpg")
        .args(["--show-keys", "--with-colons"])
        .arg(keys_path)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run gpg: {error}")))?;
    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "gpg cannot read {}: {}",
                keys_path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(colon_listing_fingerprints(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

pub(crate) fn colon_listing_fingerprints(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            (fields.next() == Some("fpr")).then(|| fields.nth(8))?
        })
        .filter(|fingerprint| !fingerprint.is_empty())
        .map(str::to_string)
        .collect()
}

/// The signing-subkey and primary fingerprints from a complete good signature
/// status, or `None` when gpg reports an invalid, expired, or revoked key.
pub(crate) fn signature_fingerprints(raw_status: &str) -> Option<Vec<String>> {
    let status_words: Vec<&str> = raw_status.lines().filter_map(status_word).collect();
    if status_words
        .iter()
        .any(|word| matches!(*word, "EXPKEYSIG" | "REVKEYSIG" | "ERRSIG"))
        || !status_words.contains(&"GOODSIG")
    {
        return None;
    }
    let valid_signature = raw_status.lines().find(|line| {
        line.strip_prefix("[GNUPG:] ")
            .is_some_and(|status| status.starts_with("VALIDSIG "))
    })?;
    let fields: Vec<&str> = valid_signature.split_whitespace().skip(2).collect();
    let subkey = (*fields.first()?).to_string();
    let primary = (*fields.last()?).to_string();
    if subkey == primary {
        Some(vec![primary])
    } else {
        Some(vec![subkey, primary])
    }
}

fn status_word(line: &str) -> Option<&str> {
    line.strip_prefix("[GNUPG:] ")?.split_whitespace().next()
}

fn git_raw_verification(args: &[&str]) -> Result<Option<String>, Error> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    let status_lines = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(verified_status_output(
        output.status.success(),
        &status_lines,
    ))
}

pub(crate) fn verified_status_output(success: bool, raw_status: &str) -> Option<String> {
    success.then(|| raw_status.to_string())
}

/// Environment identity overrides (agent sessions export them) never apply
/// to a seal or tag: the signature is the owner's act, so the objects carry
/// the owner's configured identity.
const IDENTITY_OVERRIDES: [&str; 6] = [
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_AUTHOR_DATE",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_COMMITTER_DATE",
];

/// Verification and push inherit stdio so interactive prompts reach the owner.
fn run_interactive_git(args: &[&str]) -> Result<(), Error> {
    let status = interactive_git_status(args)?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Io,
            format!("git {} failed", args.first().unwrap_or(&"")),
        ))
    }
}

fn run_signing_git(args: &[&str]) -> Result<(), Error> {
    let mut command = owner_git_command(args);
    let terminal_stderr = std::io::stderr();
    let mut terminal_stderr = terminal_stderr.lock();
    run_signing_command(
        &mut command,
        args.first().unwrap_or(&""),
        &mut terminal_stderr,
    )
}

fn run_signing_command(
    command: &mut Command,
    operation: &str,
    terminal_stderr: &mut impl Write,
) -> Result<(), Error> {
    let (status, stderr) = signing_command_status(command, terminal_stderr)?;
    if status.success() {
        Ok(())
    } else {
        Err(signing_command_error(operation, &stderr))
    }
}

pub(crate) fn signing_command_error(operation: &str, stderr: &str) -> Error {
    if has_gpg_signing_failure(stderr) {
        signing_failure(operation)
    } else {
        git_stderr_failure(operation, stderr)
    }
}

fn has_gpg_signing_failure(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        if line.starts_with("error: gpg failed to sign the data") {
            return true;
        }
        let Some(status) = line.strip_prefix("[GNUPG:] ") else {
            return false;
        };
        let mut fields = status.split_whitespace();
        fields.next() == Some("FAILURE") && fields.next() == Some("sign")
    })
}

pub(crate) fn signing_failure(operation: &str) -> Error {
    Error::new(
        ErrorKind::Config,
        format!(
            "git {operation} signing failed: configure user.signingkey or a gpg key matching the committer identity"
        ),
    )
}

/// Signing stderr is relayed while captured so hardware-key notices stay
/// visible and failures can still be classified after Git exits.
fn signing_command_status(
    command: &mut Command,
    terminal_stderr: &mut impl Write,
) -> Result<(std::process::ExitStatus, String), Error> {
    let mut child = command
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    let mut child_stderr = child.stderr.take().ok_or_else(|| {
        Error::new(
            ErrorKind::Io,
            "cannot capture git stderr for signing failure attribution",
        )
    })?;
    let mut captured_stderr = Vec::new();
    let mut buffer = [0; 1024];
    let mut terminal_is_writable = true;
    loop {
        let bytes_read = child_stderr.read(&mut buffer).map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot read git stderr: {error}"))
        })?;
        if bytes_read == 0 {
            break;
        }
        captured_stderr.extend_from_slice(&buffer[..bytes_read]);
        if terminal_is_writable {
            terminal_is_writable = terminal_stderr
                .write_all(&buffer[..bytes_read])
                .and_then(|()| terminal_stderr.flush())
                .is_ok();
        }
    }
    let status = child
        .wait()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot wait for git: {error}")))?;
    Ok((
        status,
        String::from_utf8_lossy(&captured_stderr).into_owned(),
    ))
}

fn interactive_git_status(args: &[&str]) -> Result<std::process::ExitStatus, Error> {
    owner_git_command(args)
        .status()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))
}

fn owner_git_command(args: &[&str]) -> Command {
    let mut command = Command::new("git");
    for variable in IDENTITY_OVERRIDES {
        command.env_remove(variable);
    }
    command.args(args);
    command
}

fn git_stdout(args: &[&str]) -> Result<String, Error> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(git_stderr_failure(
            args.first().unwrap_or(&""),
            &String::from_utf8_lossy(&output.stderr),
        ))
    }
}

fn git_stderr_failure(operation: &str, stderr: &str) -> Error {
    let cause = stderr.trim();
    let message = if cause.is_empty() {
        format!("git {operation} failed")
    } else {
        format!("git {operation} failed: {cause}")
    };
    Error::new(ErrorKind::Io, message)
}

#[cfg(test)]
mod tests;
