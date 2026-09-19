//! `rune sign open`: the owner puts their name on a draft pull request. The
//! branch must be the session's own, the draft must exist, the body must
//! pass the pull request schema, and the owner sees all of it before the
//! key. `rune sign adopt` admits an outside pull request the same way, on
//! an owner branch: the seal comes first, and only the sealed branch is
//! published.

use super::ceremony::{Attempt, acknowledge, seal_above, view};
use super::gh::{self, PullRequest};
use super::repo::{Head, PROTECTED, Repo, Signing};
use super::store::{Kind, OpenRequest, Request, Status};
use super::{Live, short, start_path};
use crate::cli::sign::seal::{self, OpenSeal, Seal};
use rune::error::{Error, ErrorKind};
use std::path::Path;

/// The remote the push targets and the seal names.
const REMOTE: &str = "origin";
const DRAFT_ATTEMPTS: u32 = 10;

/// The checked facts of a branch a session asks to open.
pub(crate) struct Candidate {
    pub repo: String,
    pub head: Head,
    pub pull_request: PullRequest,
    pub body: String,
}

pub(crate) fn open(
    bookmark: &str,
    body_file: Option<&Path>,
    queue: bool,
    repository: Option<&Path>,
    json: bool,
) -> Result<i32, Error> {
    let repo = Repo::open(&start_path(repository)?)?;
    let candidate = qualify(&repo, bookmark, body_file)?;
    let request = open_request(&repo, bookmark, &candidate);
    print!(
        "{}",
        view(&repo, &request, &candidate.pull_request.base_ref_name)?
    );
    if queue {
        return super::record_request(&repo, &request, json);
    }
    let signing = Signing::owner(&repo.workspace)?;
    let keys = owner_keys(&repo)?;
    match complete(&repo, &request, &signing, &keys)? {
        Attempt::Signed(sealed) => {
            println!("opened {bookmark} @ {} as ready", short(&sealed));
            Ok(0)
        }
        Attempt::NotSigned(reason) | Attempt::Failed(reason) => {
            Err(Error::new(ErrorKind::Config, reason))
        }
    }
}

/// The `KEYS` fingerprints from the protected branch as `origin` has it,
/// refreshed from the forge first when it answers.
pub(crate) fn owner_keys(repo: &Repo) -> Result<Vec<String>, Error> {
    let reference = repo.protected_ref(REMOTE)?;
    if let Err(reason) = repo.refresh_protected(REMOTE, &reference) {
        eprintln!("KEYS read from {reference} as last fetched: refresh failed ({reason})");
    }
    crate::cli::sign::keys_fingerprints_at(repo, &reference)
}

/// Every refusal of `rune sign open`, in the order the owner can act on
/// them: the branch, the draft, the body.
pub(crate) fn qualify(
    repo: &Repo,
    bookmark: &str,
    body_file: Option<&Path>,
) -> Result<Candidate, Error> {
    if PROTECTED.contains(&bookmark) {
        return Err(refusal(format!(
            "{bookmark} is the protected branch, not a session's own branch"
        )));
    }
    let head = repo.head(bookmark)?.ok_or_else(|| {
        refusal(format!(
            "no bookmark {bookmark} in {}",
            repo.workspace.display()
        ))
    })?;
    if head.signed {
        return Err(refusal(format!(
            "{bookmark} is already signed at {}",
            short(&head.identity.commit_id)
        )));
    }
    if let Some(remote_head) = repo.remote_head(REMOTE, bookmark)?
        && remote_head != head.identity.commit_id
        && !repo.is_ancestor(&remote_head, &head.identity.commit_id)?
    {
        return Err(refusal(format!(
            "{bookmark} does not descend from {REMOTE}/{bookmark}: the session did not create that branch"
        )));
    }
    let slug = repo_slug(repo, REMOTE)?;
    let mut drafts = gh::open_pull_requests(&slug, Some(bookmark))?;
    let pull_request = match drafts.len() {
        1 => drafts.remove(0),
        0 => {
            return Err(refusal(format!(
                "no open pull request has head {bookmark}: push the branch and let the app open the draft"
            )));
        }
        _ => {
            return Err(refusal(format!(
                "{} open pull requests have head {bookmark}",
                drafts.len()
            )));
        }
    };
    if !pull_request.is_draft {
        return Err(refusal(format!(
            "pull request #{} is already ready",
            pull_request.number
        )));
    }
    if pull_request.base_ref_name == bookmark {
        return Err(refusal(format!("{bookmark} is its own base")));
    }
    if pull_request.head_ref_oid != head.identity.commit_id {
        return Err(refusal(format!(
            "pull request #{} is at {} and {bookmark} is at {}: push first",
            pull_request.number,
            short(&pull_request.head_ref_oid),
            short(&head.identity.commit_id)
        )));
    }
    let body = read_body(repo, bookmark, body_file)?;
    validate_body(repo, &body)?;
    Ok(Candidate {
        repo: slug,
        head,
        pull_request,
        body,
    })
}

fn refusal(message: String) -> Error {
    Error::new(ErrorKind::Config, message)
}

pub(crate) fn repo_slug(repo: &Repo, remote: &str) -> Result<String, Error> {
    let url = repo.remote_url(remote)?.ok_or_else(|| {
        refusal(format!(
            "no remote {remote} in {}: the seal names the repository it binds",
            repo.workspace.display()
        ))
    })?;
    Ok(seal::repo_slug(&url))
}

/// The body from the change the branch names, else from `--body-file`.
fn read_body(repo: &Repo, bookmark: &str, body_file: Option<&Path>) -> Result<String, Error> {
    let from_change = bookmark
        .strip_prefix("change/")
        .map(|id| {
            repo.workspace
                .join("docs/changes")
                .join(id)
                .join("pull-request.md")
        })
        .filter(|path| path.is_file());
    let path = match (from_change, body_file) {
        (Some(path), _) => path,
        (None, Some(path)) => path.to_path_buf(),
        (None, None) => {
            return Err(refusal(format!(
                "no docs/changes/<id>/pull-request.md for {bookmark}: pass --body-file <FILE>"
            )));
        }
    };
    std::fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read the body {}: {error}", path.display()),
        )
    })
}

/// The body against the repository's own `schemas/PULL_REQUEST.mdschema`,
/// through the standalone checker when it is on PATH and the built-in
/// subset otherwise.
fn validate_body(repo: &Repo, body: &str) -> Result<(), Error> {
    let schema_path = repo.workspace.join("schemas/PULL_REQUEST.mdschema");
    let schema = std::fs::read_to_string(&schema_path)
        .map_err(|error| refusal(format!("cannot read {}: {error}", schema_path.display())))?;
    let errors = match standalone_check(&schema_path, body)? {
        Some(errors) => errors,
        None => rune::validate::mdschema::check(body, "pull request body", &schema)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == rune::validate::Severity::Error)
            .map(|diagnostic| diagnostic.message)
            .collect(),
    };
    if errors.is_empty() {
        return Ok(());
    }
    Err(refusal(format!(
        "the body does not pass schemas/PULL_REQUEST.mdschema: {}",
        errors.join("; ")
    )))
}

/// The standalone checker's error lines, or `None` when it is not on PATH
/// and the built-in subset must decide.
fn standalone_check(schema_path: &Path, body: &str) -> Result<Option<Vec<String>>, Error> {
    let present = std::process::Command::new("mdschema")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !present {
        return Ok(None);
    }
    let file = tempfile::Builder::new()
        .suffix(".md")
        .tempfile()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot write the body: {error}")))?;
    std::fs::write(file.path(), body)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot write the body: {error}")))?;
    let output = std::process::Command::new("mdschema")
        .args(["check", "--schema"])
        .arg(schema_path)
        .arg(file.path())
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run mdschema: {error}")))?;
    if output.status.success() {
        return Ok(Some(Vec::new()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let errors: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('✗'))
        .map(str::trim)
        .filter(|line| !line.starts_with("Found "))
        .map(str::to_string)
        .collect();
    if errors.is_empty() {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "mdschema check failed without reporting findings: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(Some(errors))
}

fn open_request(repo: &Repo, bookmark: &str, candidate: &Candidate) -> Request {
    Request {
        id: super::store::new_request_id(&repo.key(), bookmark),
        repository: repo.key(),
        workspace: repo.workspace.to_string_lossy().into_owned(),
        bookmark: bookmark.to_string(),
        head: candidate.head.identity.clone(),
        receipt: None,
        requested_at: super::store::now(),
        status: Status::Queued,
        kind: Kind::Open,
        open: Some(OpenRequest {
            repo: candidate.repo.clone(),
            base: candidate.pull_request.base_ref_name.clone(),
            pull_request: candidate.pull_request.number,
            body: candidate.body.clone(),
        }),
        coverage: None,
        signed_commit: None,
        claim: None,
        failure: None,
    }
}

/// Seal, push, flip, and write the nonce: the part of `open` that needs the
/// key, run at once or from the queue. The push takes a lease on the head
/// the draft was at, which is the head that was sealed.
pub(crate) fn complete(
    repo: &Repo,
    request: &Request,
    signing: &Signing,
    keys: &[String],
) -> Result<Attempt, Error> {
    let Some(open) = &request.open else {
        return Ok(Attempt::Failed(
            "the request records no pull request to open".to_string(),
        ));
    };
    let (sealed, nonce) = match seal_and_push(
        repo,
        request,
        open,
        signing,
        keys,
        Some(&request.head.commit_id),
    )? {
        Ok(pushed) => pushed,
        Err(attempt) => return Ok(attempt),
    };
    Ok(finish(
        open,
        open.pull_request,
        &sealed,
        &nonce,
        &request.bookmark,
    ))
}

/// The open-seal above the recorded head, signed, verified, and pushed to
/// `origin`. Anything that fails after the signature is `Failed`, so the
/// owner finishes by hand and nothing is signed again.
fn seal_and_push(
    repo: &Repo,
    request: &Request,
    open: &OpenRequest,
    signing: &Signing,
    keys: &[String],
    lease: Option<&str>,
) -> Result<Result<(String, String), Attempt>, Error> {
    let nonce = seal::nonce()?;
    let seal = OpenSeal {
        repo: open.repo.clone(),
        base: open.base.clone(),
        pull_request: open.pull_request,
        tree: request.head.tree_id.clone(),
        nonce: nonce.clone(),
    };
    let message = seal.message();
    let sealed = match seal_above(
        repo,
        &request.bookmark,
        &request.head.commit_id,
        &Seal::Open(seal),
        &message,
        signing,
        keys,
    )? {
        Attempt::Signed(sealed) => sealed,
        other => return Ok(Err(other)),
    };
    if let Err(error) = repo.push(REMOTE, &sealed, &request.bookmark, lease) {
        return Ok(Err(Attempt::Failed(format!(
            "open-seal {} is signed on {} and not pushed: {error}",
            short(&sealed),
            request.bookmark
        ))));
    }
    Ok(Ok((sealed, nonce)))
}

/// Flip the draft and write the nonce into its body, after the sealed
/// branch is on the remote.
fn finish(open: &OpenRequest, draft: u64, sealed: &str, nonce: &str, bookmark: &str) -> Attempt {
    let done = gh::ready(&open.repo, draft)
        .and_then(|()| gh::edit_body(&open.repo, draft, &seal::body_with_nonce(&open.body, nonce)));
    match done {
        Ok(()) => Attempt::Signed(sealed.to_string()),
        Err(error) => Attempt::Failed(format!(
            "open-seal {} is pushed on {bookmark} and pull request #{draft} is not ready: {error}",
            short(sealed)
        )),
    }
}

/// `next` on an `open` request: the owner acknowledges the view, then the
/// flow completes.
pub(crate) fn complete_from_queue(
    repo: &Repo,
    live: &Live,
    signing: &Signing,
    keys: &[String],
) -> Result<Attempt, Error> {
    let request = &live.request;
    let Some(open) = &request.open else {
        return Ok(Attempt::Failed(
            "the request records no pull request to open".to_string(),
        ));
    };
    print!("{}", view(repo, request, &open.base)?);
    if !acknowledge(&format!("sign the open-seal for {}?", request.bookmark))? {
        return Ok(Attempt::NotSigned("declined".to_string()));
    }
    complete(repo, request, signing, keys)
}

/// `rune sign adopt <number>`: fetch the outside head onto `adopt/<number>`,
/// show it, seal it with the key, and only then push, wait for the app's
/// draft, and ready it. Nothing of the outside branch reaches the remote
/// before the owner's signature is on it. The seal names the outside pull
/// request, because the draft does not exist yet.
pub(crate) fn adopt(number: u64, repository: Option<&Path>, json: bool) -> Result<i32, Error> {
    let repo = Repo::open(&start_path(repository)?)?;
    let slug = repo_slug(&repo, REMOTE)?;
    if !gh::is_admin(&slug)? {
        return Err(refusal(format!(
            "adopt is owner-only and the gh login does not administer {slug}"
        )));
    }
    let outside = gh::pull_request(&slug, number)?;
    if outside.state != "OPEN" {
        return Err(refusal(format!(
            "pull request #{number} is {}",
            outside.state.to_lowercase()
        )));
    }
    let bookmark = format!("adopt/{number}");
    if repo.head(&bookmark)?.is_some() || repo.remote_head(REMOTE, &bookmark)?.is_some() {
        return Err(refusal(format!("{bookmark} already exists: drop it first")));
    }
    validate_body(&repo, &outside.body)?;
    let fetched = repo.fetch_pull_head(REMOTE, number)?;
    if fetched != outside.head_ref_oid {
        return Err(refusal(format!(
            "pull request #{number} is at {} and the fetched head is {}",
            short(&outside.head_ref_oid),
            short(&fetched)
        )));
    }
    repo.create_bookmark(&bookmark, &fetched)?;
    let head = repo.head(&bookmark)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Io,
            format!("{bookmark} vanished after it was created"),
        )
    })?;
    let candidate = Candidate {
        repo: slug.clone(),
        head,
        pull_request: outside.clone(),
        body: outside.body.clone(),
    };
    let request = open_request(&repo, &bookmark, &candidate);
    let open = request.open.as_ref().expect("set above");
    print!("{}", view(&repo, &request, &open.base)?);
    let signing = Signing::owner(&repo.workspace)?;
    let keys = owner_keys(&repo)?;
    let (sealed, nonce) = match seal_and_push(&repo, &request, open, &signing, &keys, None)? {
        Ok(pushed) => pushed,
        Err(Attempt::NotSigned(reason) | Attempt::Failed(reason)) => {
            return Err(Error::new(ErrorKind::Config, reason));
        }
        Err(Attempt::Signed(_)) => unreachable!("seal_and_push never errs with Signed"),
    };
    let draft = match wait_for_draft(&slug, &bookmark) {
        Ok(draft) => draft,
        Err(error) => {
            return Err(refusal(format!(
                "open-seal {} is pushed on {bookmark}: {error}; ready the draft by hand and add `Open-Seal-Nonce: {nonce}` to its body",
                short(&sealed)
            )));
        }
    };
    match finish(open, draft.number, &sealed, &nonce, &bookmark) {
        Attempt::Signed(_) => {}
        Attempt::NotSigned(reason) | Attempt::Failed(reason) => {
            return Err(Error::new(ErrorKind::Config, reason));
        }
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "adopted": number,
                "bookmark": bookmark,
                "draft": draft.number,
                "commit": sealed,
            })
        );
    } else {
        println!(
            "adopted #{number} ({}) onto {bookmark}: draft #{} is ready at {}",
            short(&fetched),
            draft.number,
            short(&sealed)
        );
    }
    Ok(0)
}

fn wait_for_draft(slug: &str, bookmark: &str) -> Result<PullRequest, Error> {
    for attempt in 1..=DRAFT_ATTEMPTS {
        let mut drafts = gh::open_pull_requests(slug, Some(bookmark))?;
        if drafts.len() == 1 {
            return Ok(drafts.remove(0));
        }
        if attempt < DRAFT_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    }
    Err(refusal(format!("no draft opened for {bookmark}")))
}
