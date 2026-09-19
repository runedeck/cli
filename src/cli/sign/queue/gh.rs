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
    /// `OPEN`, `CLOSED`, or `MERGED` as gh reports it.
    #[serde(default)]
    pub state: String,
}

const PULL_REQUEST_FIELDS: &str =
    "number,isDraft,baseRefName,headRefName,headRefOid,body,url,state";

/// The check run the controller publishes on `reviewed_sha`, whose first
/// output line is the ledger line.
pub(crate) const LEDGER_CHECK: &str = "ledger";
pub(crate) const LEDGER_LINE_PREFIX: &str = "ledger: ";
/// The file inside the ledger artifact.
pub(crate) const LEDGER_FILE: &str = "runeseer-ledger.json";

/// The app that owns the ledger: the reviewing identity. A check run is
/// the ledger only when this app created it, because only the creating
/// app can edit a check run.
pub(crate) const CONTROLLER_APP: &str = "runeseer";

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

/// The controller's word on one reviewed head: the first output line of
/// its `ledger` check run. The merge-seal binds `reviewed_sha`,
/// `generation`, and `digest`, and `artifact_id` names the full ledger.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct LedgerLine {
    pub artifact_id: u64,
    pub digest: String,
    pub generation: u64,
    pub pull_request: u64,
    pub reviewed_sha: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct CheckRun {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub app: Option<App>,
    #[serde(default)]
    pub output: Output,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct Output {
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct App {
    #[serde(default)]
    pub slug: String,
}

#[derive(Deserialize)]
struct CheckRunsPage {
    #[serde(default)]
    check_runs: Vec<CheckRun>,
}

impl CheckRun {
    pub(crate) fn by_controller(&self) -> bool {
        self.name == LEDGER_CHECK
            && self
                .app
                .as_ref()
                .is_some_and(|app| app.slug == CONTROLLER_APP)
    }

    /// The ledger line, when the first line of the output text is one.
    pub(crate) fn ledger_line(&self) -> Option<LedgerLine> {
        let first = self.output.text.as_deref()?.lines().next()?.trim_end();
        serde_json::from_str(first.strip_prefix(LEDGER_LINE_PREFIX)?).ok()
    }
}

/// `gh api repos/<repo>/commits/<sha>/check-runs?check_name=ledger`: the
/// newest ledger check run the controller app created on the commit, or
/// `None`. A same-named check run from any other app is not the ledger.
pub(crate) fn ledger_line(repo: &str, sha: &str) -> Result<Option<LedgerLine>, Error> {
    let path =
        format!("repos/{repo}/commits/{sha}/check-runs?check_name={LEDGER_CHECK}&per_page=100");
    let output = run(&["api", &path, "--paginate", "--slurp"])?;
    // `--slurp` wraps every page in one outer array.
    let pages: Vec<CheckRunsPage> = parse(&output, "gh api")?;
    Ok(newest_ledger_line(
        pages.into_iter().flat_map(|page| page.check_runs),
    ))
}

pub(crate) fn newest_ledger_line(runs: impl Iterator<Item = CheckRun>) -> Option<LedgerLine> {
    runs.filter(CheckRun::by_controller)
        .max_by_key(|run| run.id)
        .and_then(|run| run.ledger_line())
}

/// `gh api repos/<repo>/actions/artifacts/<id>/zip`: the ledger file out of
/// the artifact, byte for byte, so the caller can prove it by the digest.
/// The archive is read by `unzip`, which every runner and workstation has.
pub(crate) fn ledger_artifact(repo: &str, artifact_id: u64) -> Result<Vec<u8>, Error> {
    let path = format!("repos/{repo}/actions/artifacts/{artifact_id}/zip");
    let archive = run_bytes(&["api", &path])?;
    let file = tempfile::NamedTempFile::new().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write the ledger artifact to a temporary file: {error}"),
        )
    })?;
    std::fs::write(file.path(), &archive).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write the ledger artifact: {error}"),
        )
    })?;
    let output = Command::new("unzip")
        .arg("-p")
        .arg(file.path())
        .arg(LEDGER_FILE)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run unzip: {error}")))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "ledger artifact {artifact_id} holds no {LEDGER_FILE}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(output.stdout)
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
    run_bytes(args).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn run_bytes(args: &[&str]) -> Result<Vec<u8>, Error> {
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
    Ok(output.stdout)
}

fn parse<T: for<'de> Deserialize<'de>>(output: &str, what: &str) -> Result<T, Error> {
    serde_json::from_str(output).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("{what} returned something other than the expected JSON: {error}"),
        )
    })
}
