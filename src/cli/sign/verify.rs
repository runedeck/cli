//! `rune sign --verify --seal <ref>`: the check the `owner-seal` workflow
//! runs. It reads the repository through git alone, so a plain checkout
//! verifies, reads `KEYS` from the protected branch as `origin` has it, and
//! reads the forge through `gh` for the pull request under check.

use super::SealCheck;
use super::queue::repo::{Repo, Signing, Verdict};
use super::queue::{gh, repo_slug};
use super::seal::{self, MergeSeal, OpenSeal, Seal};
use rune::error::{Error, ErrorKind};

/// How many commits between the base and the head are searched for the
/// open-seal.
const SEARCH_DEPTH: usize = 2000;
const REMOTE: &str = "origin";

/// Exit 0 when every seal check holds, 1 when one fails, and an error
/// when the repository or the forge cannot be read.
pub(crate) fn seals(check: &SealCheck<'_>) -> Result<i32, Error> {
    let cwd = std::env::current_dir()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read cwd: {error}")))?;
    let repo = Repo::open_for_reading(&cwd)?;
    let reference = check.reference;
    let head = repo.rev_parse(reference)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("cannot resolve {reference} to a commit"),
        )
    })?;
    let keys_ref = match check.keys_ref {
        Some(keys_ref) => keys_ref.to_string(),
        None => repo.protected_ref(REMOTE)?,
    };
    let keys = super::keys_fingerprints_at(&repo, &keys_ref)?;
    let signing = if repo.jj {
        Signing::owner(&repo.workspace)?
    } else {
        Signing {
            program: "gpg".to_string(),
            key: None,
        }
    };
    let slug = repo_slug(&repo, REMOTE)?;
    let mut report = Report::default();
    report.lines.push(format!("     KEYS read from {keys_ref}"));
    let head_identity = repo.commit(&head)?;
    let reviewed = match seal::parse(&head_identity.description) {
        Some(Seal::Merge(merge)) => {
            check_merge(&repo, &slug, &head, &merge, &signing, &keys, &mut report)?;
            merge.reviewed_sha
        }
        _ => head.clone(),
    };
    if let Some(pull_request) = under_check(&slug, check.pull_request, &head, &mut report)? {
        check_open(
            &repo,
            &head,
            &reviewed,
            &slug,
            &pull_request,
            &signing,
            &keys,
            &mut report,
        )?;
    }
    for line in &report.lines {
        println!("{line}");
    }
    Ok(i32::from(report.failed))
}

#[derive(Default)]
struct Report {
    failed: bool,
    lines: Vec<String>,
}

impl Report {
    fn check(&mut self, holds: bool, what: &str) {
        self.lines
            .push(format!("{} {what}", if holds { "ok  " } else { "FAIL" }));
        self.failed |= !holds;
    }

    fn fail(&mut self, what: &str) {
        self.check(false, what);
    }
}

/// The pull request the seal must bind: the one named, or the one open
/// pull request whose head is the ref. Two at the same head, as one head
/// under two bases, cannot be told apart without the number.
fn under_check(
    slug: &str,
    number: Option<u64>,
    head: &str,
    report: &mut Report,
) -> Result<Option<gh::PullRequest>, Error> {
    if let Some(number) = number {
        let pull_request = gh::pull_request(slug, number)?;
        report.check(
            pull_request.state == "OPEN",
            &format!("pull request #{number} is open"),
        );
        return Ok(Some(pull_request));
    }
    let mut at_head: Vec<gh::PullRequest> = gh::open_pull_requests(slug, None)?
        .into_iter()
        .filter(|pull_request| pull_request.head_ref_oid == head)
        .collect();
    if at_head.len() != 1 {
        report.fail(&format!(
            "exactly one open pull request has head {} ({} found): pass --pull-request",
            short(head),
            at_head.len()
        ));
        return Ok(None);
    }
    Ok(Some(at_head.remove(0)))
}

fn signed_by_owner(
    repo: &Repo,
    commit: &str,
    signing: &Signing,
    keys: &[String],
) -> Result<bool, Error> {
    Ok(match repo.verify(commit, signing)? {
        Verdict::Good(prints) => prints.iter().any(|print| keys.contains(print)),
        Verdict::Rejected(_) => false,
    })
}

/// The merge-seal at `head` is an empty child of the reviewed commit it
/// names, signed by the owner, and names the generation and digest the
/// controller's ledger check run carries on that commit.
fn check_merge(
    repo: &Repo,
    slug: &str,
    head: &str,
    merge: &MergeSeal,
    signing: &Signing,
    keys: &[String],
    report: &mut Report,
) -> Result<(), Error> {
    let identity = repo.commit(head)?;
    let sole_parent = identity.parents == [merge.reviewed_sha.clone()];
    report.check(
        sole_parent,
        &format!(
            "merge-seal {} has sole parent {}",
            short(head),
            short(&merge.reviewed_sha)
        ),
    );
    let same_tree = sole_parent && repo.commit(&merge.reviewed_sha)?.tree_id == identity.tree_id;
    report.check(
        same_tree,
        &format!("merge-seal {} keeps the reviewed tree", short(head)),
    );
    report.check(
        signed_by_owner(repo, head, signing, keys)?,
        &format!("merge-seal {} is signed by a KEYS key", short(head)),
    );
    match gh::ledger_line(slug, &merge.reviewed_sha)? {
        Some(line) => {
            report.check(
                line.generation == merge.generation,
                &format!(
                    "merge-seal {} names generation {} and the ledger is at {}",
                    short(head),
                    merge.generation,
                    line.generation
                ),
            );
            report.check(
                line.digest == merge.digest,
                &format!(
                    "merge-seal {} names the ledger digest {}",
                    short(head),
                    short(&line.digest)
                ),
            );
        }
        None => report.fail(&format!(
            "reviewed {} carries no ledger check run from {}",
            short(&merge.reviewed_sha),
            gh::CONTROLLER_APP
        )),
    }
    Ok(())
}

/// The nearest open-seal between the pull request's base and `reviewed`
/// binds this repository, its own tree, and this pull request: by number,
/// or by the `adopt/<number>` branch an adopted pull request rides on.
#[allow(clippy::too_many_arguments)]
fn check_open(
    repo: &Repo,
    head: &str,
    reviewed: &str,
    slug: &str,
    pull_request: &gh::PullRequest,
    signing: &Signing,
    keys: &[String],
    report: &mut Report,
) -> Result<(), Error> {
    let number = pull_request.number;
    report.check(
        pull_request.head_ref_oid == head,
        &format!(
            "pull request #{number} has head {} (seal verified at {})",
            short(&pull_request.head_ref_oid),
            short(head)
        ),
    );
    let base_ref = format!("refs/remotes/{REMOTE}/{}", pull_request.base_ref_name);
    let base = repo.rev_parse(&base_ref)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("cannot resolve {base_ref}: fetch the base before verifying"),
        )
    })?;
    let found = repo
        .range_subjects(&base, reviewed, SEARCH_DEPTH)?
        .into_iter()
        .find_map(|(commit, subject)| match seal::parse(&subject) {
            Some(Seal::Open(open)) => Some((commit, open)),
            _ => None,
        });
    let Some((commit, open)) = found else {
        report.fail(&format!(
            "no open-seal between {base_ref} and {}: a seal in merged history does not count",
            short(reviewed)
        ));
        return Ok(());
    };
    report.check(
        signed_by_owner(repo, &commit, signing, keys)?,
        &format!("open-seal {} is signed by a KEYS key", short(&commit)),
    );
    report.check(
        repo.commit(&commit)?.tree_id == open.tree,
        &format!("open-seal {} names its own tree", short(&commit)),
    );
    report.check(
        open.repo == slug,
        &format!("open-seal {} binds {slug}", short(&commit)),
    );
    check_binding(&commit, &open, pull_request, report);
    check_nonce(&open, slug, pull_request, report)
}

/// The seal names this pull request, or the outside pull request this one
/// adopted, in which case the head branch is `adopt/<that number>`.
fn check_binding(
    commit: &str,
    open: &OpenSeal,
    pull_request: &gh::PullRequest,
    report: &mut Report,
) {
    let number = pull_request.number;
    if open.pull_request == number {
        report.check(
            true,
            &format!("open-seal {} names pull request #{number}", short(commit)),
        );
    } else {
        let adopted = pull_request.head_ref_name == open.adopt_branch();
        report.check(
            adopted,
            &format!(
                "open-seal {} names pull request #{}, adopted as #{number} on {}",
                short(commit),
                open.pull_request,
                pull_request.head_ref_name
            ),
        );
    }
    report.check(
        open.base == pull_request.base_ref_name,
        &format!(
            "pull request #{number} targets {} as the seal names",
            open.base
        ),
    );
}

/// This pull request's body carries the seal's nonce. Another open body
/// that carries it is reported and fails nothing here: that pull request
/// fails its own check, because the seal does not name it.
fn check_nonce(
    open: &OpenSeal,
    slug: &str,
    pull_request: &gh::PullRequest,
    report: &mut Report,
) -> Result<(), Error> {
    let number = pull_request.number;
    report.check(
        seal::body_nonce(&pull_request.body).as_deref() == Some(&open.nonce),
        &format!("pull request #{number} carries the seal's nonce"),
    );
    let others: Vec<u64> = gh::open_pull_requests(slug, None)?
        .into_iter()
        .filter(|other| {
            other.number != number && seal::body_nonce(&other.body).as_deref() == Some(&open.nonce)
        })
        .map(|other| other.number)
        .collect();
    for other in others {
        report.lines.push(format!(
            "     nonce is also carried by open pull request #{other}, which does not fail #{number}"
        ));
    }
    Ok(())
}

fn short(commit: &str) -> &str {
    &commit[..12.min(commit.len())]
}
