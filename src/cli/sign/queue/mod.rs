//! The signing queue: a session queues a validated head, the owner signs
//! from the queue base first. Nothing here pushes.

mod ceremony;
pub(super) mod gh;
mod ledger;
mod open;
pub(super) mod repo;
mod store;
#[cfg(test)]
mod tests;

use crate::cli::sign::seal::{MergeSeal, Seal};
use ceremony::Attempt;
use repo::{Head, Repo, Signing, Verdict};
use rune::error::{Error, ErrorKind};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

pub(crate) use open::{adopt, open, repo_slug};
use store::{Claim, Kind, Receipt, Request, Status, Store};

const SIGN_ATTEMPTS: usize = 3;

/// The derived state of a request, computed from the live repository every
/// time it is listed or signed; nothing but `signed` and `failed` is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum State {
    Current,
    Stale,
    Blocked,
    Claimed,
    /// The head carries a signature the queue has not verified: a signer
    /// that died after `jj sign`, or a signature made outside the queue.
    Unverified,
    Signed,
    Failed,
}

#[derive(Serialize)]
struct Listed<'a> {
    state: State,
    #[serde(flatten)]
    request: &'a Request,
}

/// A request with its bookmark's head as observed now, in the repository
/// the request names.
struct Live {
    request: Request,
    head: Option<Head>,
    /// `None` when the request's workspace is gone: the request is stale
    /// and stays listable, prunable, and droppable.
    repo: Option<Rc<Repo>>,
}

pub(crate) fn queue(
    bookmark: Option<&str>,
    receipt: Option<&Path>,
    repository: Option<&Path>,
    prune: bool,
    json: bool,
) -> Result<i32, Error> {
    match bookmark {
        Some(bookmark) => queue_head(bookmark, receipt, repository, json),
        None if prune => prune_requests(json),
        None => list(json),
    }
}

/// `rune sign queue <bookmark>`: the CLI-0041 request, the recorded head
/// signed in place by the owner.
fn queue_head(
    bookmark: &str,
    receipt: Option<&Path>,
    repository: Option<&Path>,
    json: bool,
) -> Result<i32, Error> {
    let repo = Repo::open(&start_path(repository)?)?;
    let (head, receipt) = qualify_head(&repo, bookmark, receipt, Kind::Head)?;
    let request = Request {
        id: store::new_request_id(&repo.key(), bookmark),
        repository: repo.key(),
        workspace: repo.workspace.to_string_lossy().into_owned(),
        bookmark: bookmark.to_string(),
        head: head.identity,
        receipt: Some(receipt),
        requested_at: store::now(),
        status: Status::Queued,
        kind: Kind::Head,
        open: None,
        coverage: None,
        signed_commit: None,
        claim: None,
        failure: None,
    };
    record_request(&repo, &request, json)
}

/// `rune sign submit <bookmark>`: admission to the merge-seal. Beyond the
/// receipt, the ledger the controller published on the pull request must
/// clear the head at its current generation.
pub(crate) fn submit(
    bookmark: &str,
    receipt: Option<&Path>,
    repository: Option<&Path>,
    json: bool,
) -> Result<i32, Error> {
    let repo = Repo::open(&start_path(repository)?)?;
    let (head, receipt) = qualify_head(&repo, bookmark, receipt, Kind::Merge)?;
    let slug = open::repo_slug(&repo, "origin")?;
    let mut pull_requests = gh::open_pull_requests(&slug, Some(bookmark))?;
    let pull_request = match pull_requests.len() {
        1 => pull_requests.remove(0),
        0 => {
            return Err(Error::new(
                ErrorKind::Config,
                format!("no open pull request has head {bookmark}"),
            ));
        }
        _ => {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} open pull requests have head {bookmark}",
                    pull_requests.len()
                ),
            ));
        }
    };
    if pull_request.is_draft {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "pull request #{} is a draft: run `rune sign open {bookmark}` first",
                pull_request.number
            ),
        ));
    }
    let (line, ledger) = ledger::fetch(&slug, &head.identity.commit_id)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!(
                "head {} has no ledger from {}: wait for the controller",
                &head.identity.commit_id[..12],
                gh::CONTROLLER_APP
            ),
        )
    })?;
    let checks = gh::required_checks(&slug, pull_request.number)?;
    let coverage = ledger::admit(
        &ledger,
        &line,
        &slug,
        pull_request.number,
        &pull_request.base_ref_name,
        &head.identity.commit_id,
        &checks,
    )
    .map_err(|reason| {
        Error::new(
            ErrorKind::Config,
            format!("{bookmark} is refused: {reason}"),
        )
    })?;
    let request = Request {
        id: store::new_request_id(&repo.key(), bookmark),
        repository: repo.key(),
        workspace: repo.workspace.to_string_lossy().into_owned(),
        bookmark: bookmark.to_string(),
        head: head.identity,
        receipt: Some(receipt),
        requested_at: store::now(),
        status: Status::Queued,
        kind: Kind::Merge,
        open: None,
        coverage: Some(coverage),
        signed_commit: None,
        claim: None,
        failure: None,
    };
    record_request(&repo, &request, json)
}

/// The receipt and the head every request needs: the bookmark exists, is
/// unsigned, and the receipt names it with a passing exit line.
fn qualify_head(
    repo: &Repo,
    bookmark: &str,
    receipt: Option<&Path>,
    kind: Kind,
) -> Result<(Head, Receipt), Error> {
    let receipt_path = receipt.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "a signing request needs --receipt <FILE>: the check log that ends with the exit line",
        )
    })?;
    let head = repo.head(bookmark)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("no bookmark {bookmark} in {}", repo.workspace.display()),
        )
    })?;
    // A merge-seal is a new commit above the reviewed head, so that head
    // may carry the owner's open-seal already. Signing in place may not.
    if head.signed && kind == Kind::Head {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{bookmark} is already signed at {}",
                head.identity.commit_id
            ),
        ));
    }
    let receipt = qualify_receipt(receipt_path, &head.identity.commit_id)?;
    Ok((head, receipt))
}

fn record_request(repo: &Repo, request: &Request, json: bool) -> Result<i32, Error> {
    let store = Store::open()?;
    // Under the repository lock, so two submissions cannot both pass the
    // duplicate check, and no signer is between its checks and its record.
    let _lock = store.lock_repository(&repo.key())?;
    let live = observe(&store)?;
    for existing in &live {
        if existing.request.repository == repo.key()
            && existing.request.bookmark == request.bookmark
            && derive(existing, &live)? != State::Stale
        {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} already has request {}: drop it or let it go stale",
                    request.bookmark, existing.request.id
                ),
            ));
        }
    }
    store.save(request)?;
    let coverage = request
        .coverage
        .as_ref()
        .map(|coverage| coverage.verdict.clone());
    if json {
        println!(
            "{}",
            serde_json::json!({
                "queued": true,
                "kind": request.kind,
                "id": request.id,
                "bookmark": request.bookmark,
                "commit": request.head.commit_id,
                "coverage": coverage,
                "next": "rune sign next",
            })
        );
    } else {
        match coverage {
            Some(coverage) => println!(
                "queued {} {} at {} ({coverage}): run `rune sign next` at the key",
                request.id,
                request.bookmark,
                short(&request.head.commit_id)
            ),
            None => println!(
                "queued {} {} at {}: run `rune sign next` at the key",
                request.id,
                request.bookmark,
                short(&request.head.commit_id)
            ),
        }
    }
    notify(&format!(
        "{} needs your signature: run rune sign next",
        request.bookmark
    ));
    Ok(0)
}

/// The receipt qualifies the head when its last line is an exit line with
/// status zero and it names the commit it checked.
fn qualify_receipt(path: &Path, commit_id: &str) -> Result<Receipt, Error> {
    let body = fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read receipt {}: {error}", path.display()),
        )
    })?;
    let last = body
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim();
    let Some(status) = exit_status(last) else {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "receipt {} does not end with an exit line such as `exit=0`",
                path.display()
            ),
        ));
    };
    if status != 0 {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "receipt {} reports `{last}`: the head is not clean",
                path.display()
            ),
        ));
    }
    if !names_commit(&body, commit_id) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "receipt {} does not name commit {}: it checked a different head",
                path.display(),
                short(commit_id)
            ),
        ));
    }
    // The owner signs from another directory, so the record keeps the
    // absolute path the session named.
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(Receipt {
        path: absolute.to_string_lossy().into_owned(),
        digest: store::file_digest(path)?,
        exit_line: last.to_string(),
    })
}

/// The receipt names the commit when one of its hexadecimal words is the
/// commit id or its twelve-character prefix, which is how the push hook
/// prints the head it checked. A prefix inside a longer word, or a hash of
/// something else that happens to share it, does not count.
pub(crate) fn names_commit(body: &str, commit_id: &str) -> bool {
    let prefix = short(commit_id);
    body.split(|c: char| !c.is_ascii_hexdigit())
        .any(|word| word == prefix || word == commit_id)
}

/// The receipt as recorded must still be on disk unchanged when the owner
/// signs: a request whose receipt was edited or replaced is refused.
fn receipt_still_holds(request: &Request) -> Result<(), String> {
    let Some(receipt) = &request.receipt else {
        return match request.kind {
            Kind::Open => Ok(()),
            Kind::Head | Kind::Merge => Err("the request records no receipt".to_string()),
        };
    };
    let path = Path::new(&receipt.path);
    let digest = store::file_digest(path).map_err(|error| error.to_string())?;
    if digest != receipt.digest {
        return Err(format!(
            "receipt {} changed since the request was queued",
            path.display()
        ));
    }
    qualify_receipt(path, &request.head.commit_id)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// `exit=0` and `dryrun-exit=0` qualify; `not-an-exit=0` does not, because
/// the name before `exit=` must be empty or one lowercase word and a hyphen.
pub(crate) fn exit_status(line: &str) -> Option<u32> {
    let (name, status) = line.split_once("exit=")?;
    let stage = name.strip_suffix('-').unwrap_or(name);
    if !(name.is_empty()
        || (name.ends_with('-')
            && !stage.is_empty()
            && stage.chars().all(|c| c.is_ascii_lowercase())))
    {
        return None;
    }
    status.parse().ok()
}

fn list(json: bool) -> Result<i32, Error> {
    let store = Store::open()?;
    let live = observe(&store)?;
    let states = live
        .iter()
        .map(|entry| derive(entry, &live))
        .collect::<Result<Vec<_>, _>>()?;
    if json {
        let listed: Vec<Listed<'_>> = live
            .iter()
            .zip(&states)
            .map(|(entry, state)| Listed {
                state: *state,
                request: &entry.request,
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&listed).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(0);
    }
    if live.is_empty() {
        println!("the signing queue at {} is empty", store.root().display());
        return Ok(0);
    }
    for (entry, state) in live.iter().zip(&states) {
        println!(
            "{:<8} {} {:<28} {} {}",
            state_label(*state),
            entry.request.id,
            entry.request.bookmark,
            short(&entry.request.head.commit_id),
            entry.request.workspace
        );
    }
    Ok(0)
}

/// Removal happens under each repository's lock, with the request and its
/// head observed again there, so a signer never loses a request it holds
/// and a state read before the lock is never acted on after it.
fn prune_requests(json: bool) -> Result<i32, Error> {
    let store = Store::open()?;
    let live = observe(&store)?;
    let mut locks: BTreeMap<String, store::RepositoryLock> = BTreeMap::new();
    let mut removed = Vec::new();
    for entry in &live {
        if !locks.contains_key(&entry.request.repository) {
            locks.insert(
                entry.request.repository.clone(),
                store.lock_repository(&entry.request.repository)?,
            );
        }
        let Ok(request) = store.load(&entry.request.id) else {
            continue;
        };
        let head = match &entry.repo {
            Some(repo) => repo.head(&request.bookmark)?,
            None => None,
        };
        let fresh = Live {
            head,
            request,
            repo: entry.repo.clone(),
        };
        let state = derive(&fresh, &live)?;
        if matches!(state, State::Stale | State::Signed | State::Failed) {
            store.remove(&fresh.request.id)?;
            removed.push((
                fresh.request.id.clone(),
                fresh.request.bookmark.clone(),
                state,
            ));
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string(&removed).unwrap_or_else(|_| "[]".to_string())
        );
    } else {
        for (id, bookmark, state) in &removed {
            println!("pruned {id} {bookmark} ({})", state_label(*state));
        }
    }
    Ok(0)
}

pub(crate) fn show(
    bookmark: Option<&str>,
    id: Option<&str>,
    repository: Option<&Path>,
    json: bool,
) -> Result<i32, Error> {
    let store = Store::open()?;
    let live = observe(&store)?;
    let entry = find_request(&live, bookmark, id, repository)?;
    let state = derive(entry, &live)?;
    let request = &entry.request;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&Listed { state, request })
                .unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("{:<12} {}", "state", state_label(state));
        println!("{:<12} {}", "kind", kind_label(request.kind));
        println!("{:<12} {}", "id", request.id);
        println!("{:<12} {}", "bookmark", request.bookmark);
        println!("{:<12} {}", "workspace", request.workspace);
        println!("{:<12} {}", "repository", request.repository);
        println!("{:<12} {}", "commit", request.head.commit_id);
        println!("{:<12} {}", "change", request.head.change_id);
        println!("{:<12} {}", "author", request.head.author);
        if let Some(receipt) = &request.receipt {
            println!("{:<12} {}", "receipt", receipt.path);
            println!("{:<12} {}", "exit", receipt.exit_line);
        }
        if let Some(open) = &request.open {
            println!("{:<12} #{} on {}", "pull", open.pull_request, open.repo);
            println!("{:<12} {}", "base", open.base);
        }
        if let Some(coverage) = &request.coverage {
            println!(
                "{:<12} #{} on {}",
                "pull", coverage.pull_request, coverage.repo
            );
            println!("{:<12} {}", "reviewed", coverage.reviewed_sha);
            println!("{:<12} {}", "generation", coverage.generation);
            match &coverage.reason {
                Some(reason) => println!("{:<12} {} ({reason})", "coverage", coverage.verdict),
                None => println!("{:<12} {}", "coverage", coverage.verdict),
            }
        }
        println!("{:<12} {}", "requested", request.requested_at);
        if let Some(signed) = &request.signed_commit {
            println!("{:<12} {signed}", "signed");
        }
        if let Some(failure) = &request.failure {
            println!("{:<12} {failure}", "failure");
        }
    }
    Ok(0)
}

pub(crate) fn drop(
    bookmark: Option<&str>,
    id: Option<&str>,
    repository: Option<&Path>,
) -> Result<i32, Error> {
    let store = Store::open()?;
    let live = observe(&store)?;
    let entry = find_request(&live, bookmark, id, repository)?;
    // The repository lock keeps a signer from claiming it meanwhile.
    let _lock = store.lock_repository(&entry.request.repository)?;
    let request = store.load(&entry.request.id)?;
    if request.claim.as_ref().is_some_and(claim_is_live) {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} is claimed by a running signer", request.id),
        ));
    }
    store.remove(&request.id)?;
    println!("dropped {} {}", request.id, request.bookmark);
    Ok(0)
}

fn find_request<'a>(
    live: &'a [Live],
    bookmark: Option<&str>,
    id: Option<&str>,
    repository: Option<&Path>,
) -> Result<&'a Live, Error> {
    if let Some(id) = id {
        return live
            .iter()
            .find(|entry| entry.request.id == id)
            .ok_or_else(|| Error::new(ErrorKind::Config, format!("no signing request {id}")));
    }
    let bookmark = bookmark
        .ok_or_else(|| Error::new(ErrorKind::Config, "name a bookmark or pass --id <REQUEST>"))?;
    let key = Repo::open(&start_path(repository)?)?.key();
    live.iter()
        .find(|entry| entry.request.bookmark == bookmark && entry.request.repository == key)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("no signing request for {bookmark} in this repository"),
            )
        })
}

pub(crate) fn next(json: bool) -> Result<i32, Error> {
    sign_requests(false, json)
}

pub(crate) fn all(json: bool) -> Result<i32, Error> {
    sign_requests(true, json)
}

#[derive(Serialize)]
struct Outcome {
    id: String,
    bookmark: String,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
}

impl Outcome {
    fn skipped(request: &Request, state: State) -> Self {
        Self {
            id: request.id.clone(),
            bookmark: request.bookmark.clone(),
            result: format!("skipped: {}", state_label(state)),
            commit: None,
        }
    }

    fn attempted(&self) -> bool {
        !self.result.starts_with("skipped: ")
    }
}

/// `next` signs the first current request in stack order, `all` every
/// current request in that order until one fails. A stale or blocked
/// request is reported as skipped and does not stop the search; it does
/// make the exit status nonzero, because the owner expected it signed.
fn sign_requests(every: bool, json: bool) -> Result<i32, Error> {
    let store = Store::open()?;
    // Lock every repository the ledger names, then observe under the locks:
    // a request queued between a first look and the lock would otherwise be
    // ordered against a stale snapshot.
    let mut locks: BTreeMap<String, store::RepositoryLock> = BTreeMap::new();
    for request in store.load_all()? {
        lock_once(&store, &mut locks, &request.repository)?;
    }
    let live = observe(&store)?;
    for entry in &live {
        lock_once(&store, &mut locks, &entry.request.repository)?;
    }
    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut failed = false;
    // The owner's signing settings are read once, and only when a request
    // is about to be signed, so a stale or blocked queue never fails on
    // configuration it does not need.
    let mut signing: Option<Signing> = None;
    for entry in &live {
        let Some(repo) = &entry.repo else {
            let outcome = Outcome::skipped(&entry.request, State::Stale);
            failed = true;
            outcomes.push(outcome);
            continue;
        };
        // Observe this request again: signing an ancestor rewrote its head
        // since the list was built.
        let fresh = Live {
            request: store.load(&entry.request.id)?,
            head: repo.head(&entry.request.bookmark)?,
            repo: Some(Rc::clone(repo)),
        };
        let state = derive(&fresh, &live)?;
        let outcome = match state {
            State::Current => sign_one(&store, fresh, signing_for(&mut signing, &repo.workspace)?)?,
            State::Failed | State::Signed => continue,
            // Signed on disk but never recorded: the signer died between
            // `jj sign` and the store write, or the owner signed by hand.
            State::Unverified => {
                record_signed_head(&store, fresh, signing_for(&mut signing, &repo.workspace)?)?
            }
            State::Claimed if fresh.request.claim.as_ref().is_some_and(claim_is_live) => continue,
            State::Claimed => {
                let mut fresh = fresh;
                fresh.request.claim = None;
                sign_one(&store, fresh, signing_for(&mut signing, &repo.workspace)?)?
            }
            State::Stale | State::Blocked => Outcome::skipped(&fresh.request, state),
        };
        let attempted = outcome.attempted();
        let succeeded = outcome.result == "signed";
        failed |= !succeeded;
        outcomes.push(outcome);
        if attempted && (!every || !succeeded) {
            break;
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&outcomes).unwrap_or_else(|_| "[]".to_string())
        );
    } else if outcomes.is_empty() {
        println!("nothing to sign in {}", store.root().display());
    } else {
        for outcome in &outcomes {
            match &outcome.commit {
                Some(commit) => println!("{} {} {}", outcome.result, outcome.bookmark, commit),
                None => println!("{} {}", outcome.result, outcome.bookmark),
            }
        }
    }
    Ok(i32::from(failed))
}

fn lock_once(
    store: &Store,
    locks: &mut BTreeMap<String, store::RepositoryLock>,
    repository: &str,
) -> Result<(), Error> {
    if !locks.contains_key(repository) {
        locks.insert(repository.to_string(), store.lock_repository(repository)?);
    }
    Ok(())
}

fn signing_for<'a>(slot: &'a mut Option<Signing>, workspace: &Path) -> Result<&'a Signing, Error> {
    if slot.is_none() {
        *slot = Some(Signing::owner(workspace)?);
    }
    Ok(slot.as_ref().expect("set above"))
}

/// Claim, sign the head observed under the lock, verify, record. The head
/// is the one `derive` saw: it keeps the recorded identity, and its commit
/// id may differ from the recorded one when an ancestor was signed since.
/// The recorded head is never rewritten: the receipt names it, and a retry
/// on a later run must still find it there.
fn sign_one(store: &Store, live: Live, signing: &Signing) -> Result<Outcome, Error> {
    let Live {
        mut request,
        head,
        repo,
    } = live;
    let Some(repo) = repo else {
        return record_failure(store, request, "workspace is gone");
    };
    let Some(head) = head else {
        return record_failure(store, request, "bookmark vanished before signing");
    };
    if let Err(reason) = receipt_still_holds(&request) {
        return record_failure(store, request, &reason);
    }
    let observed = head.identity.commit_id.clone();
    request.claim = Some(Claim {
        pid: std::process::id(),
        started_at: store::now(),
        commit: Some(observed.clone()),
    });
    store.save(&request)?;
    let keys = open::owner_keys(&repo)?;
    let live = Live {
        request: request.clone(),
        head: Some(head),
        repo: Some(Rc::clone(&repo)),
    };
    let attempt = match request.kind {
        Kind::Head => {
            match ceremony::sign_with_retry(&repo, &request.bookmark, &observed, signing)? {
                Ok(()) => match verify_signed_head(&repo, &request, signing, &keys)? {
                    Ok(commit) => Attempt::Signed(commit),
                    Err(reason) => Attempt::Failed(reason),
                },
                Err(reason) => Attempt::NotSigned(reason),
            }
        }
        Kind::Open => open::complete_from_queue(&repo, &live, signing, &keys)?,
        Kind::Merge => merge_seal(&repo, &live, signing, &keys)?,
    };
    let outcome = match attempt {
        Attempt::Signed(commit) => {
            request.status = Status::Signed;
            request.signed_commit = Some(commit.clone());
            request.claim = None;
            store.save(&request)?;
            Outcome {
                id: request.id.clone(),
                bookmark: request.bookmark.clone(),
                result: "signed".to_string(),
                commit: Some(commit),
            }
        }
        Attempt::Failed(reason) => record_failure(store, request, &reason)?,
        Attempt::NotSigned(reason) => {
            request.claim = None;
            store.save(&request)?;
            Outcome {
                id: request.id.clone(),
                bookmark: request.bookmark.clone(),
                result: reason,
                commit: None,
            }
        }
    };
    if outcome.result == "signed" {
        notify(&format!("{} signed", outcome.bookmark));
    }
    Ok(outcome)
}

/// `next` on a `merge` request: the owner acknowledges the view, then the
/// merge-seal is written above the reviewed head. Nothing pushes.
fn merge_seal(
    repo: &Repo,
    live: &Live,
    signing: &Signing,
    keys: &[String],
) -> Result<Attempt, Error> {
    let request = &live.request;
    let Some(coverage) = &request.coverage else {
        return Ok(Attempt::Failed(
            "the request records no coverage".to_string(),
        ));
    };
    print!("{}", ceremony::view(repo, request, &coverage.base)?);
    if !ceremony::acknowledge(&format!(
        "sign the merge-seal for {} at {} generation {}?",
        request.bookmark,
        short(&coverage.reviewed_sha),
        coverage.generation
    ))? {
        return Ok(Attempt::NotSigned("declined".to_string()));
    }
    let seal = MergeSeal {
        reviewed_sha: coverage.reviewed_sha.clone(),
        generation: coverage.generation,
        digest: coverage.digest.clone(),
    };
    let message = seal.message();
    ceremony::seal_above(
        repo,
        &request.bookmark,
        &coverage.reviewed_sha,
        &Seal::Merge(seal),
        &message,
        signing,
        keys,
    )
}

fn record_failure(store: &Store, mut request: Request, reason: &str) -> Result<Outcome, Error> {
    request.status = Status::Failed;
    request.failure = Some(reason.to_string());
    request.claim = None;
    store.save(&request)?;
    Ok(Outcome {
        id: request.id.clone(),
        bookmark: request.bookmark.clone(),
        result: format!("failed: {reason}"),
        commit: None,
    })
}

/// A queued request whose head carries a signature the queue never wrote:
/// the owner's key on the recorded identity is recorded as signed, anything
/// else as failed, because the queue never signs over a signature it does
/// not recognise.
fn record_signed_head(store: &Store, live: Live, signing: &Signing) -> Result<Outcome, Error> {
    let Live {
        mut request,
        head,
        repo,
    } = live;
    let Some(repo) = repo else {
        return record_failure(store, request, "workspace is gone");
    };
    let Some(head) = head else {
        return record_failure(store, request, "bookmark vanished before recording");
    };
    if request.kind != Kind::Head {
        // A seal is written by the queue alone: a signature that appeared
        // on an open or merge request's head came from elsewhere.
        return record_failure(store, request, "the head was signed outside the queue");
    }
    let keys = open::owner_keys(&repo)?;
    if let Err(reason) = owner_signature(&repo, &head.identity.commit_id, signing, &keys)? {
        return record_failure(store, request, &reason);
    }
    request.status = Status::Signed;
    request.signed_commit = Some(head.identity.commit_id.clone());
    request.claim = None;
    store.save(&request)?;
    Ok(Outcome {
        id: request.id.clone(),
        bookmark: request.bookmark.clone(),
        result: "signed".to_string(),
        commit: Some(head.identity.commit_id),
    })
}

/// After signing, the bookmark must point at a commit that keeps the
/// recorded identity and carries a signature from a key in `KEYS`.
fn verify_signed_head(
    repo: &Repo,
    request: &Request,
    signing: &Signing,
    keys: &[String],
) -> Result<Result<String, String>, Error> {
    let Some(head) = repo.head(&request.bookmark)? else {
        return Ok(Err(format!(
            "bookmark {} vanished after signing",
            request.bookmark
        )));
    };
    if !identity_matches(request, &head) {
        return Ok(Err(format!(
            "{} changed identity while signing",
            request.bookmark
        )));
    }
    if !head.signed {
        return Ok(Err("jj sign returned without a signature".to_string()));
    }
    Ok(
        owner_signature(repo, &head.identity.commit_id, signing, keys)?
            .map(|()| head.identity.commit_id),
    )
}

/// The same check as `rune sign --verify`: gpg accepts the signature and
/// one of its fingerprints is published in `KEYS`.
fn owner_signature(
    repo: &Repo,
    commit: &str,
    signing: &Signing,
    keys: &[String],
) -> Result<Result<(), String>, Error> {
    Ok(match repo.verify(commit, signing)? {
        Verdict::Good(fingerprints) if fingerprints.iter().any(|print| keys.contains(print)) => {
            Ok(())
        }
        Verdict::Good(fingerprints) => Err(format!(
            "signature from key {} is not in KEYS",
            fingerprints.last().map_or("", String::as_str)
        )),
        Verdict::Rejected(status) => Err(format!("signature rejected: {status}")),
    })
}

pub(crate) fn identity_matches(request: &Request, head: &Head) -> bool {
    let recorded = &request.head;
    let live = &head.identity;
    recorded.change_id == live.change_id
        && recorded.tree_id == live.tree_id
        && recorded.author == live.author
        && recorded.description == live.description
        && recorded.parents == live.parents
}

/// Failed is stored; everything else comes from the live head. Blocked is
/// checked before claimed, so a request with a dead claim is never
/// recovered ahead of its unsigned base, and against the other requests'
/// live heads, so a rewritten ancestor still blocks its descendant.
fn derive(entry: &Live, others: &[Live]) -> Result<State, Error> {
    let request = &entry.request;
    if request.status == Status::Failed {
        return Ok(State::Failed);
    }
    let (Some(repo), Some(head)) = (&entry.repo, &entry.head) else {
        return Ok(State::Stale);
    };
    // A seal is a new commit above the recorded head, so a signed request
    // is current only while the bookmark still points at what was signed.
    if request.status == Status::Signed {
        return Ok(
            if request.signed_commit.as_deref() == Some(&head.identity.commit_id) {
                State::Signed
            } else {
                State::Stale
            },
        );
    }
    if !identity_matches(request, head) {
        return Ok(State::Stale);
    }
    // A merge-seal binds the reviewed commit id itself, not an identity a
    // rewrite may keep: the ledger reviewed that exact commit.
    if request.kind == Kind::Merge
        && request
            .coverage
            .as_ref()
            .is_none_or(|coverage| coverage.reviewed_sha != head.identity.commit_id)
    {
        return Ok(State::Stale);
    }
    if head.signed && request.kind != Kind::Merge {
        return Ok(State::Unverified);
    }
    for other in others {
        if other.request.id == request.id
            || other.request.repository != request.repository
            || other.request.status == Status::Signed
        {
            continue;
        }
        if let Some(other_head) = &other.head
            && repo.is_ancestor(&other_head.identity.commit_id, &head.identity.commit_id)?
        {
            return Ok(State::Blocked);
        }
    }
    if request.claim.is_some() {
        return Ok(State::Claimed);
    }
    Ok(State::Current)
}

fn claim_is_live(claim: &Claim) -> bool {
    !store::claim_is_dead(claim.pid, &claim.started_at)
}

/// Every request with its live head, in stack order: within one repository
/// an ancestor precedes its descendants, computed from the live heads.
fn observe(store: &Store) -> Result<Vec<Live>, Error> {
    let mut repos: BTreeMap<String, Option<Rc<Repo>>> = BTreeMap::new();
    let mut live = Vec::new();
    for request in store.load_all()? {
        let repo = if let Some(repo) = repos.get(&request.workspace) {
            repo.clone()
        } else {
            // A workspace that was removed or moved makes its requests
            // stale; it never disables the queue for everyone else.
            let repo = Repo::open(Path::new(&request.workspace)).ok().map(Rc::new);
            repos.insert(request.workspace.clone(), repo.clone());
            repo
        };
        let head = match &repo {
            Some(repo) => repo.head(&request.bookmark)?,
            None => None,
        };
        live.push(Live {
            request,
            head,
            repo,
        });
    }
    let mut before = vec![Vec::new(); live.len()];
    for (i, left) in live.iter().enumerate() {
        for (j, right) in live.iter().enumerate() {
            if i == j || left.request.repository != right.request.repository {
                continue;
            }
            if let (Some(repo), Some(ancestor), Some(descendant)) =
                (&left.repo, &left.head, &right.head)
                && repo.is_ancestor(&ancestor.identity.commit_id, &descendant.identity.commit_id)?
            {
                before[j].push(i);
            }
        }
    }
    Ok(topological(live, &before))
}

/// A stable topological order: `before[j]` lists the indexes that must
/// precede `j`; ties keep the input order (request time).
pub(crate) fn topological<T>(items: Vec<T>, before: &[Vec<usize>]) -> Vec<T> {
    let mut placed = vec![false; items.len()];
    let mut order = Vec::with_capacity(items.len());
    while order.len() < items.len() {
        let next = (0..items.len()).find(|&index| {
            !placed[index] && before[index].iter().all(|&dependency| placed[dependency])
        });
        match next {
            Some(index) => {
                placed[index] = true;
                order.push(index);
            }
            // A cycle cannot happen in a DAG; keep the rest in input order.
            None => {
                for (index, slot) in placed.iter_mut().enumerate() {
                    if !*slot {
                        *slot = true;
                        order.push(index);
                    }
                }
            }
        }
    }
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|index| slots[index].take())
        .collect()
}

fn start_path(repository: Option<&Path>) -> Result<PathBuf, Error> {
    match repository {
        Some(path) => Ok(path.to_path_buf()),
        None => std::env::current_dir()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read cwd: {error}"))),
    }
}

fn short(commit_id: &str) -> &str {
    &commit_id[..12.min(commit_id.len())]
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Head => "head",
        Kind::Open => "open",
        Kind::Merge => "merge",
    }
}

fn state_label(state: State) -> &'static str {
    match state {
        State::Current => "current",
        State::Stale => "stale",
        State::Blocked => "blocked",
        State::Claimed => "claimed",
        State::Unverified => "unverified",
        State::Signed => "signed",
        State::Failed => "failed",
    }
}

/// Best effort: the owner may be away from the terminal. A failure to
/// notify is never an error.
fn notify(message: &str) {
    if std::env::var_os("RUNE_NO_NOTIFY").is_some_and(|value| !value.is_empty()) {
        return;
    }
    let attempt = if cfg!(target_os = "macos") {
        Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "display notification \"{}\" with title \"rune sign\"",
                message.replace('"', "'")
            ))
            .status()
    } else {
        Command::new("notify-send")
            .arg("rune sign")
            .arg(message)
            .status()
    };
    let _ = attempt;
}
