//! Every read of and write to the forge goes through the `gh` program on
//! PATH, with the repository named on each call. The tests stub it with a
//! script, so each call here is the contract the stub honors.

use rune::error::{Error, ErrorKind};
use serde::Deserialize;
use std::process::Command;

/// The lines a required check reports, with `bucket` as gh classifies the
/// state: `pass`, `fail`, `pending`, `skipping`, or `cancel`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Check {
    pub name: String,
    pub bucket: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequest {
    pub number: u64,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default)]
    pub base_ref_name: String,
    #[serde(default)]
    pub head_ref_name: String,
    #[serde(default)]
    pub head_ref_oid: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub url: String,
}

const PULL_REQUEST_FIELDS: &str = "number,isDraft,baseRefName,headRefName,headRefOid,body,url";

/// The comment marker the controller puts first in the ledger comment.
pub(crate) const LEDGER_MARKER: &str = "<!-- rune-ledger -->";

/// `gh pr list --state open [--head <branch>]`: every open pull request, or
/// the ones whose head is the branch.
pub(crate) fn open_pull_requests(
    repo: &str,
    head: Option<&str>,
) -> Result<Vec<PullRequest>, Error> {
    let mut args = vec![
        "pr",
        "list",
        "--repo",
        repo,
        "--state",
        "open",
        "--limit",
        "500",
        "--json",
        PULL_REQUEST_FIELDS,
    ];
    if let Some(head) = head {
        args.extend(["--head", head]);
    }
    parse(&run(&args)?, "gh pr list")
}

/// `gh pr view <number>`: one pull request by number.
pub(crate) fn pull_request(repo: &str, number: u64) -> Result<PullRequest, Error> {
    let number = number.to_string();
    parse(
        &run(&[
            "pr",
            "view",
            &number,
            "--repo",
            repo,
            "--json",
            PULL_REQUEST_FIELDS,
        ])?,
        "gh pr view",
    )
}

/// `gh pr ready <number>`: flip the draft.
pub(crate) fn ready(repo: &str, number: u64) -> Result<(), Error> {
    run(&["pr", "ready", &number.to_string(), "--repo", repo]).map(|_| ())
}

/// `gh pr edit <number> --body-file <file>`: replace the body.
pub(crate) fn edit_body(repo: &str, number: u64, body: &str) -> Result<(), Error> {
    let file = tempfile::NamedTempFile::new().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write the body to a temporary file: {error}"),
        )
    })?;
    std::fs::write(file.path(), body)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot write the body: {error}")))?;
    let path = file.path().to_string_lossy().into_owned();
    run(&[
        "pr",
        "edit",
        &number.to_string(),
        "--repo",
        repo,
        "--body-file",
        &path,
    ])
    .map(|_| ())
}

/// `gh api repos/<repo>/issues/<number>/comments --paginate`: the body of the
/// last comment that starts with the ledger marker, or `None`.
pub(crate) fn ledger_comment(repo: &str, number: u64) -> Result<Option<String>, Error> {
    #[derive(Deserialize)]
    struct Comment {
        #[serde(default)]
        body: String,
    }
    let path = format!("repos/{repo}/issues/{number}/comments");
    let output = run(&["api", &path, "--paginate", "--slurp"])?;
    // `--slurp` wraps every page in one outer array.
    let pages: Vec<Vec<Comment>> = parse(&output, "gh api")?;
    Ok(pages
        .into_iter()
        .flatten()
        .filter(|comment| comment.body.trim_start().starts_with(LEDGER_MARKER))
        .map(|comment| comment.body)
        .next_back())
}

/// `gh pr checks <number> --required`: the required checks and their
/// buckets. gh exits nonzero while checks fail or pend, so only the
/// output counts.
pub(crate) fn required_checks(repo: &str, number: u64) -> Result<Vec<Check>, Error> {
    let output = Command::new("gh")
        .args([
            "pr",
            "checks",
            &number.to_string(),
            "--repo",
            repo,
            "--required",
            "--json",
            "name,bucket",
        ])
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run gh: {error}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "gh pr checks reported nothing: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    parse(&stdout, "gh pr checks")
}

/// `gh api repos/<repo>`: whether the caller administers the repository,
/// which is what owner-only means to the forge.
pub(crate) fn is_admin(repo: &str) -> Result<bool, Error> {
    #[derive(Deserialize)]
    struct Repository {
        #[serde(default)]
        permissions: Permissions,
    }
    #[derive(Deserialize, Default)]
    struct Permissions {
        #[serde(default)]
        admin: bool,
    }
    let path = format!("repos/{repo}");
    let repository: Repository = parse(&run(&["api", &path])?, "gh api")?;
    Ok(repository.permissions.admin)
}

fn run(args: &[&str]) -> Result<String, Error> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run gh: {error}")))?;
    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "gh {} failed: {}",
                args.iter().take(2).copied().collect::<Vec<_>>().join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse<T: for<'de> Deserialize<'de>>(output: &str, what: &str) -> Result<T, Error> {
    serde_json::from_str(output).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("{what} returned something other than the expected JSON: {error}"),
        )
    })
}
