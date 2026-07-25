//! The owner's key ceremony as one command each: sealing a reviewed
//! branch with an empty signed commit, signing release tags, and
//! verifying either against the repository's `KEYS` file.
// A facet of the CLI root: the parent owns the module wiring and the
// shared helpers, and each facet uses them freely.
#[allow(clippy::wildcard_imports)]
use super::*;

use commands::error::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

const SEAL_SUBJECT: &str = "seal: approve";

pub(crate) fn execute(amend: bool, tag: Option<&str>, verify: Option<&str>) -> Result<i32, Error> {
    if let Some(reference) = verify {
        return verify_reference(reference);
    }
    if let Some(name) = tag {
        return sign_tag(name);
    }
    if amend {
        return amend_head();
    }
    seal_branch()
}

fn seal_branch() -> Result<i32, Error> {
    let branch = current_branch()?;
    require_signing_key()?;
    run_interactive_git(&["commit", "--allow-empty", "-S", "-m", SEAL_SUBJECT])?;
    run_interactive_git(&["verify-commit", "HEAD"])?;
    let remote = push_remote()?;
    run_interactive_git(&["push", &remote, "HEAD"])?;
    let head = git_stdout(&["rev-parse", "HEAD"])?;
    println!("sealed {branch} @ {head}");
    Ok(0)
}

/// A signature is part of the commit object, so signing an existing commit
/// rewrites it: the head keeps its message and author, gains the owner's
/// signature, and replaces its pushed predecessor under a lease.
fn amend_head() -> Result<i32, Error> {
    let branch = current_branch()?;
    require_signing_key()?;
    run_interactive_git(&["commit", "--amend", "--no-edit", "--allow-empty", "-S"])?;
    run_interactive_git(&["verify-commit", "HEAD"])?;
    let remote = push_remote()?;
    run_interactive_git(&["push", "--force-with-lease", &remote, "HEAD"])?;
    let head = git_stdout(&["rev-parse", "HEAD"])?;
    println!("re-signed {branch} @ {head}");
    Ok(0)
}

fn sign_tag(name: &str) -> Result<i32, Error> {
    require_signing_key()?;
    run_interactive_git(&["tag", "--sign", "--message", name, name])?;
    run_interactive_git(&["verify-tag", name])?;
    let remote = push_remote()?;
    run_interactive_git(&["push", &remote, &format!("refs/tags/{name}")])?;
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

/// The signing key must exist before any commit or tag is created, so a
/// misconfigured environment fails by naming the gap instead of leaving a
/// half-made seal behind.
fn require_signing_key() -> Result<(), Error> {
    let key = git_stdout(&["config", "--get", "user.signingkey"]).unwrap_or_default();
    if key.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            "no signing key configured: set user.signingkey to the owner's OpenPGP key",
        ));
    }
    Ok(())
}

fn current_branch() -> Result<String, Error> {
    git_stdout(&["symbolic-ref", "--short", "HEAD"]).map_err(|_| {
        Error::new(
            ErrorKind::Config,
            "HEAD is detached: seal a branch, not a commit",
        )
    })
}

fn push_remote() -> Result<String, Error> {
    let branch = current_branch()?;
    Ok(
        git_stdout(&["config", "--get", &format!("branch.{branch}.remote")])
            .unwrap_or_else(|_| "origin".to_string()),
    )
}

fn is_tag(reference: &str) -> bool {
    Command::new("git")
        .args([
            "rev-parse",
            "--quiet",
            "--verify",
            &format!("refs/tags/{reference}"),
        ])
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

/// The signing-subkey and primary fingerprints from a `VALIDSIG` status
/// line, or `None` when the verification produced no valid signature.
pub(crate) fn signature_fingerprints(raw_status: &str) -> Option<Vec<String>> {
    let valid_signature = raw_status
        .lines()
        .find(|line| line.starts_with("[GNUPG:] VALIDSIG "))?;
    let fields: Vec<&str> = valid_signature.split_whitespace().skip(2).collect();
    let subkey = (*fields.first()?).to_string();
    let primary = (*fields.last()?).to_string();
    if subkey == primary {
        Some(vec![primary])
    } else {
        Some(vec![subkey, primary])
    }
}

/// `git verify-commit --raw` exits nonzero for an unsigned object but
/// still reports the gpg status lines on stderr; both are the verdict.
fn git_raw_verification(args: &[&str]) -> Result<Option<String>, Error> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    let status_lines = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() || status_lines.contains("[GNUPG:]") {
        Ok(Some(status_lines))
    } else {
        Ok(None)
    }
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

/// Commit, tag, and push run with inherited stdio so pinentry prompts and
/// hardware-key touch notices reach the owner.
fn run_interactive_git(args: &[&str]) -> Result<(), Error> {
    let mut command = Command::new("git");
    for variable in IDENTITY_OVERRIDES {
        command.env_remove(variable);
    }
    let status = command
        .args(args)
        .status()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Io,
            format!("git {} failed", args.first().unwrap_or(&"")),
        ))
    }
}

fn git_stdout(args: &[&str]) -> Result<String, Error> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(Error::new(
            ErrorKind::Io,
            format!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}

#[cfg(test)]
mod tests;
