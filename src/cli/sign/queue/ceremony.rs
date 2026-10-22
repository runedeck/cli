//! What the two seals share: an empty commit written above a parent, the
//! bookmark moved onto it, the owner's key on it through the pinned
//! `jj sign`, and the result read back through git before anything else
//! trusts it. Plus the one acknowledgment the owner gives before the key.

use super::repo::{Repo, SignOutcome, Signing};
use super::store::{Coverage, Identity, RangeRequest, Request};
use super::{SIGN_ATTEMPTS, owner_signature, short};
use crate::cli::sign::seal::{self, Seal};
use rune::error::{Error, ErrorKind};
use std::io::{BufRead, IsTerminal, Write};

/// The terminal the acknowledgment is read from. Tests point it at a file.
const TTY_VARIABLE: &str = "RUNE_SIGN_TTY";

/// What one run at the key produced for a request.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Attempt {
    /// The signed commit id; the request is recorded as signed.
    Signed(String),
    /// Nothing was signed: a refusal, a cancellation, or a moved head. The
    /// request stays current for the next run.
    NotSigned(String),
    /// The key touched but the result does not hold; the request is
    /// recorded as failed and never signed over.
    Failed(String),
}

/// Write an empty commit above `parent` with `message`, move `bookmark`
/// onto it, sign it, and return the signed commit id. A failure before the
/// signature moves the bookmark back, so nothing unsigned is left on it.
pub(crate) fn seal_above(
    repo: &Repo,
    bookmark: &str,
    parent: &str,
    expected: &Seal,
    message: &str,
    signing: &Signing,
    keys: &[String],
) -> Result<Attempt, Error> {
    let commit = repo.empty_commit(parent, message)?;
    repo.move_bookmark(bookmark, &commit, parent)?;
    if let Err(reason) = sign_with_retry(
        repo,
        bookmark,
        &commit,
        std::slice::from_ref(&commit),
        signing,
    )? {
        repo.move_bookmark(bookmark, parent, &commit)?;
        return Ok(Attempt::NotSigned(reason));
    }
    let Some(head) = repo.head(bookmark)? else {
        return Ok(Attempt::Failed(format!(
            "bookmark {bookmark} vanished after signing"
        )));
    };
    let signed = head.identity.commit_id.clone();
    if let Err(reason) = check_seal(repo, &signed, parent, expected)? {
        return Ok(Attempt::Failed(reason));
    }
    if !head.signed {
        return Ok(Attempt::Failed(
            "jj sign returned without a signature".to_string(),
        ));
    }
    Ok(match owner_signature(repo, &signed, signing, keys)? {
        Ok(()) => Attempt::Signed(signed),
        Err(reason) => Attempt::Failed(reason),
    })
}

/// The seal at `commit` is an empty child of `parent` with the expected
/// message, so the signature covers exactly what was shown.
pub(crate) fn check_seal(
    repo: &Repo,
    commit: &str,
    parent: &str,
    expected: &Seal,
) -> Result<Result<(), String>, Error> {
    let identity = repo.commit(commit)?;
    if identity.parents != [parent.to_string()] {
        return Ok(Err(format!(
            "seal {} does not have {} as its sole parent",
            short(commit),
            short(parent)
        )));
    }
    if identity.tree_id != repo.commit(parent)?.tree_id {
        return Ok(Err(format!(
            "seal {} changes the tree of {}",
            short(commit),
            short(parent)
        )));
    }
    if seal::parse(&identity.description).as_ref() != Some(expected) {
        return Ok(Err(format!(
            "seal {} carries a different message than the one signed",
            short(commit)
        )));
    }
    Ok(Ok(()))
}

/// Before every attempt the bookmark must still point at the commit
/// observed under the lock: a head that moved after the claim is stale
/// and nothing is signed.
/// The pinentry waits for the PIN and the card touch for a few seconds
/// only (the PIN cache is short by design). A touch while another window
/// has focus types a one-time password there instead.
pub(crate) const TIMEOUT_HINT: &str = "Keep the pinentry focused and touch the key as soon as it appears; a touch while another window has focus types an OTP into that window.";

/// `commits` is what gets signed: the observed head alone, or every commit
/// of a range ending at it.
pub(crate) fn sign_with_retry(
    repo: &Repo,
    bookmark: &str,
    observed: &str,
    commits: &[String],
    signing: &Signing,
) -> Result<Result<(), String>, Error> {
    for attempt in 1..=SIGN_ATTEMPTS {
        let Some(head) = repo.head(bookmark)? else {
            return Ok(Err(format!("bookmark {bookmark} vanished")));
        };
        if head.identity.commit_id != observed {
            return Ok(Err(format!(
                "stale: {bookmark} moved to {} before signing",
                short(&head.identity.commit_id)
            )));
        }
        match repo.sign(commits, signing)? {
            SignOutcome::Signed => return Ok(Ok(())),
            SignOutcome::Timeout(detail) if attempt < SIGN_ATTEMPTS => {
                eprintln!("signing timed out (attempt {attempt} of {SIGN_ATTEMPTS}): {detail}");
                eprintln!("{TIMEOUT_HINT}");
            }
            SignOutcome::Timeout(detail) => {
                return Ok(Err(format!("timed out: {detail}. {TIMEOUT_HINT}")));
            }
            SignOutcome::Cancelled(detail) => return Ok(Err(format!("cancelled: {detail}"))),
            SignOutcome::Immutable(detail) => {
                return Ok(Err(format!(
                    "refused: {bookmark} is immutable, which for a pushed bookmark means the head is published. Sign before you push. {detail}"
                )));
            }
            SignOutcome::Failed(detail) => return Ok(Err(format!("failed: {detail}"))),
        }
    }
    Ok(Err("timed out".to_string()))
}

/// What the owner sees before the key: the branch, the base, what the
/// pull request introduces, the ledger's coverage or the body, the proof,
/// and the fields the signature authorizes.
pub(crate) fn view(repo: &Repo, request: &Request, base: &str) -> Result<String, Error> {
    let mut lines: Vec<String> = Vec::new();
    let head = request
        .coverage
        .as_ref()
        .map_or(request.head.commit_id.as_str(), |coverage| {
            coverage.reviewed_sha.as_str()
        });
    lines.push(format!("bookmark     {}", request.bookmark));
    lines.push(format!("base         {base}"));
    lines.push(format!("workspace    {}", request.workspace));
    lines.push(format!("diff stat    {base}...{}", short(head)));
    for line in repo.diff_stat(base, head)?.lines() {
        lines.push(format!("             {line}"));
    }
    if let Some(coverage) = &request.coverage {
        lines.extend(coverage_table(coverage));
    }
    if let Some(open) = &request.open {
        lines.push(format!(
            "pull request #{} on {}",
            open.pull_request, open.repo
        ));
        lines.push("body".to_string());
        for line in open.body.lines() {
            lines.push(format!("    {line}"));
        }
    }
    if let Some(receipt) = &request.receipt {
        lines.push(format!("proof        {}", receipt.path));
        lines.push(format!("exit         {}", receipt.exit_line));
    }
    lines.push("authorizing".to_string());
    if let Some(coverage) = &request.coverage {
        lines.push(format!("    repo          {}", coverage.repo));
        lines.push(format!("    bookmark      {}", request.bookmark));
        lines.push(format!("    reviewed_sha  {}", coverage.reviewed_sha));
        lines.push(format!("    generation    {}", coverage.generation));
    } else if let Some(open) = &request.open {
        lines.push(format!("    repo          {}", open.repo));
        lines.push(format!("    bookmark      {}", request.bookmark));
        lines.push(format!("    base          {}", open.base));
        lines.push(format!("    tree          {}", request.head.tree_id));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

/// What the owner sees before the key signs a range: the base, the head,
/// the count, each author with the commits they wrote, and one line for
/// every commit, oldest first. `commits` is the range as it is now.
pub(crate) fn range_view(request: &Request, range: &RangeRequest, commits: &[Identity]) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("bookmark     {}", request.bookmark));
    lines.push(format!("workspace    {}", request.workspace));
    lines.extend(range_lines(range, commits));
    if let Some(receipt) = &request.receipt {
        lines.push(format!("proof        {}", receipt.path));
        lines.push(format!("exit         {}", receipt.exit_line));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The range part of the view, which `rune sign show` prints too.
pub(crate) fn range_lines(range: &RangeRequest, commits: &[Identity]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let head = commits.last().map_or("", |last| last.commit_id.as_str());
    lines.push(format!(
        "range        {} at {}..{}",
        range.protected,
        short(&range.base),
        short(head)
    ));
    lines.push(format!("commits      {}", commits.len()));
    let mut authors: Vec<(&str, usize)> = Vec::new();
    for commit in commits {
        match authors
            .iter_mut()
            .find(|(author, _)| *author == commit.author)
        {
            Some((_, count)) => *count += 1,
            None => authors.push((&commit.author, 1)),
        }
    }
    authors.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    for (author, count) in authors {
        lines.push(format!("author       {author} {count}"));
    }
    for commit in commits {
        lines.push(format!(
            "             {}  {}  {}",
            short(&commit.commit_id),
            commit.author,
            commit.description.lines().next().unwrap_or("")
        ));
    }
    lines
}

fn coverage_table(coverage: &Coverage) -> Vec<String> {
    let mut lines = Vec::new();
    match coverage.reason.as_deref() {
        Some(reason) => lines.push(format!("coverage     {} ({reason})", coverage.verdict)),
        None => lines.push(format!("coverage     {}", coverage.verdict)),
    }
    for (lane, status) in &coverage.lanes {
        lines.push(format!("lane         {lane:<20} {status}"));
    }
    if coverage.threads.is_empty() {
        lines.push("threads      none".to_string());
    }
    for thread in &coverage.threads {
        let disposition = thread.disposition.as_deref().unwrap_or("open");
        match thread.reason.as_deref() {
            Some(reason) => lines.push(format!(
                "thread       {:<20} {disposition} ({reason})",
                thread.id
            )),
            None => lines.push(format!("thread       {:<20} {disposition}", thread.id)),
        }
    }
    lines
}

/// One `y` on the terminal, and nothing else, is the owner's acknowledgment.
pub(crate) fn acknowledge(prompt: &str) -> Result<bool, Error> {
    let path = std::env::var_os(TTY_VARIABLE)
        .filter(|value| !value.is_empty())
        .map_or_else(
            || std::path::PathBuf::from("/dev/tty"),
            std::path::PathBuf::from,
        );
    let reader = std::fs::File::open(&path).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                "cannot open {} for the acknowledgment: {error}",
                path.display()
            ),
        )
    })?;
    // The prompt goes to the terminal itself when the answer comes from
    // one, and to stderr when the answer comes from a file, so the file
    // is never overwritten before it is read.
    if reader.is_terminal() {
        if let Ok(mut terminal) = std::fs::OpenOptions::new().write(true).open(&path) {
            let _ = write!(terminal, "{prompt} [y/N] ");
            let _ = terminal.flush();
        }
    } else {
        eprint!("{prompt} [y/N] ");
    }
    let mut answer = String::new();
    std::io::BufReader::new(reader)
        .read_line(&mut answer)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read the acknowledgment: {error}"),
            )
        })?;
    Ok(matches!(answer.trim(), "y" | "Y"))
}
