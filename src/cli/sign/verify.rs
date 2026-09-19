//! `rune sign --verify --seal <ref>`: the check the `owner-seal` workflow
//! runs. It reads the repository through git alone, so a plain checkout
//! verifies, and it reads the forge through `gh` for the nonce.

use super::queue::repo::{Repo, Signing, Verdict};
use super::queue::{gh, repo_slug};
use super::seal::{self, MergeSeal, OpenSeal, Seal};
use rune::error::{Error, ErrorKind};

/// How far beneath the head an open-seal is searched for.
const SEARCH_DEPTH: usize = 2000;

/// Exit 0 when every seal check holds, 1 when one fails, and an error
/// when the repository or the forge cannot be read.
pub(crate) fn seals(reference: &str) -> Result<i32, Error> {
    let cwd = std::env::current_dir()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read cwd: {error}")))?;
    let repo = Repo::open_for_reading(&cwd)?;
    let head = repo.rev_parse(reference)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("cannot resolve {reference} to a commit"),
        )
    })?;
    let keys = super::keys_fingerprints(&repo.workspace.join("KEYS"))?;
    let signing = if repo.jj {
        Signing::owner(&repo.workspace)?
    } else {
        Signing {
            program: "gpg".to_string(),
            key: None,
        }
    };
    let slug = repo_slug(&repo, "origin")?;
    let mut report = Report::default();
    let head_identity = repo.commit(&head)?;
    let reviewed = match seal::parse(&head_identity.description) {
        Some(Seal::Merge(merge)) => {
            check_merge(&repo, &head, &merge, &signing, &keys, &mut report)?;
            merge.reviewed_sha
        }
        _ => head.clone(),
    };
    check_open(&repo, &head, &reviewed, &slug, &signing, &keys, &mut report)?;
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
/// names, signed by the owner.
fn check_merge(
    repo: &Repo,
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
    report.lines.push(format!(
        "     generation {} as the seal names it",
        merge.generation
    ));
    Ok(())
}

/// The nearest open-seal beneath `reviewed` binds this repository, its own
/// tree, and exactly one open pull request whose head is `head`.
fn check_open(
    repo: &Repo,
    head: &str,
    reviewed: &str,
    slug: &str,
    signing: &Signing,
    keys: &[String],
    report: &mut Report,
) -> Result<(), Error> {
    let found = repo
        .subjects(reviewed, SEARCH_DEPTH)?
        .into_iter()
        .find_map(|(commit, subject)| match seal::parse(&subject) {
            Some(Seal::Open(open)) => Some((commit, open)),
            _ => None,
        });
    let Some((commit, open)) = found else {
        report.failed = true;
        report
            .lines
            .push(format!("FAIL no open-seal beneath {}", short(head)));
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
    check_nonce(head, &open, slug, report)
}

fn check_nonce(head: &str, open: &OpenSeal, slug: &str, report: &mut Report) -> Result<(), Error> {
    let carriers: Vec<gh::PullRequest> = gh::open_pull_requests(slug, None)?
        .into_iter()
        .filter(|pull_request| seal::body_nonce(&pull_request.body).as_deref() == Some(&open.nonce))
        .collect();
    report.check(
        carriers.len() == 1,
        &format!(
            "nonce is carried by exactly one open pull request ({} found)",
            carriers.len()
        ),
    );
    let Some(carrier) = carriers.first() else {
        return Ok(());
    };
    report.check(
        carrier.head_ref_oid == head,
        &format!(
            "pull request #{} carrying the nonce has head {} (seal verified at {})",
            carrier.number,
            short(&carrier.head_ref_oid),
            short(head)
        ),
    );
    report.check(
        carrier.base_ref_name == open.base,
        &format!(
            "pull request #{} targets {} as the seal names",
            carrier.number, open.base
        ),
    );
    Ok(())
}

fn short(commit: &str) -> &str {
    &commit[..12.min(commit.len())]
}
