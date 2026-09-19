//! The two seals of the review ceremony as commit messages: an open-seal
//! binds one pull request by number to the repository, its base, its head
//! tree, and a single-use nonce; a merge-seal binds the reviewed head, the
//! ledger generation, and the digest of the ledger the owner authorized.

use rune::error::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use std::io::Read;

pub(crate) const OPEN_PREFIX: &str = "open-seal: ";
pub(crate) const MERGE_PREFIX: &str = "merge-seal: ";
/// The body line `rune sign open` appends when it flips the draft ready.
pub(crate) const NONCE_LINE: &str = "Open-Seal-Nonce: ";
const NONCE_BYTES: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OpenSeal {
    pub repo: String,
    pub base: String,
    /// The pull request the seal readies. `rune sign adopt` seals before
    /// the app opens the draft, so its seal names the outside pull request
    /// and the draft's head branch is `adopt/<number>`.
    pub pull_request: u64,
    pub tree: String,
    pub nonce: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MergeSeal {
    pub reviewed_sha: String,
    pub generation: u64,
    /// The sha256 of the ledger artifact the owner saw, as the controller's
    /// ledger line names it. A rebuilt ledger at the same generation has
    /// another digest and unseals.
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Seal {
    Open(OpenSeal),
    Merge(MergeSeal),
}

impl OpenSeal {
    pub(crate) fn message(&self) -> String {
        format!("{OPEN_PREFIX}{}", json(self))
    }

    /// The head branch a draft carries when this seal was made by
    /// `rune sign adopt` on the pull request it names.
    pub(crate) fn adopt_branch(&self) -> String {
        format!("adopt/{}", self.pull_request)
    }
}

impl MergeSeal {
    pub(crate) fn message(&self) -> String {
        format!("{MERGE_PREFIX}{}", json(self))
    }
}

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

/// The seal a commit message carries, or `None` for any other message. The
/// object must be the whole first line: a trailing word turns a seal into
/// prose and is not a seal.
pub(crate) fn parse(message: &str) -> Option<Seal> {
    let first = message.lines().next()?.trim_end();
    if let Some(body) = first.strip_prefix(OPEN_PREFIX) {
        let seal: OpenSeal = serde_json::from_str(body).ok()?;
        return (is_hex(&seal.nonce, NONCE_BYTES * 2) && is_hex(&seal.tree, 40))
            .then_some(Seal::Open(seal));
    }
    if let Some(body) = first.strip_prefix(MERGE_PREFIX) {
        let seal: MergeSeal = serde_json::from_str(body).ok()?;
        return (is_hex(&seal.reviewed_sha, 40) && is_hex(&seal.digest, 64))
            .then_some(Seal::Merge(seal));
    }
    None
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

/// Thirty-two random bytes from the operating system as lowercase hex. A
/// nonce is single-use: it is written once into a commit and once into a
/// pull request body, and the verifier requires exactly one open pull
/// request to carry it.
pub(crate) fn nonce() -> Result<String, Error> {
    let mut bytes = [0u8; NONCE_BYTES];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read random bytes for the seal nonce: {error}"),
            )
        })?;
    Ok(hex(&bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The nonce a pull request body names, when it carries exactly one nonce
/// line.
pub(crate) fn body_nonce(body: &str) -> Option<String> {
    let mut found = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix(NONCE_LINE))
        .map(str::trim)
        .filter(|value| is_hex(value, NONCE_BYTES * 2));
    let first = found.next()?.to_string();
    found.next().is_none().then_some(first)
}

/// The body with the nonce line appended after one blank line.
pub(crate) fn body_with_nonce(body: &str, nonce: &str) -> String {
    let trimmed = body.trim_end();
    format!("{trimmed}\n\n{NONCE_LINE}{nonce}\n")
}

/// The `owner/name` slug of a GitHub remote URL, or the URL itself for any
/// other remote, so a seal always names where it was made.
pub(crate) fn repo_slug(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let path = trimmed
        .strip_prefix("git@github.com:")
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("https://github.com/"))
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .map(|slug| slug.strip_suffix(".git").unwrap_or(slug));
    match path {
        Some(slug) if slug.split('/').count() == 2 => slug.to_string(),
        _ => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
    const SHA: &str = "25d45bbb35a811f68ebce7a00910790aeef3e77c";

    #[test]
    fn a_nonce_is_thirty_two_random_bytes_as_hex_and_never_repeats() {
        let first = nonce().expect("nonce");
        let second = nonce().expect("nonce");
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn seal_messages_round_trip_and_prose_is_not_a_seal() {
        let open = OpenSeal {
            repo: "runedeck/cli".to_string(),
            base: "main".to_string(),
            pull_request: 62,
            tree: TREE.to_string(),
            nonce: "ab".repeat(32),
        };
        let message = open.message();
        assert!(message.starts_with("open-seal: {"));
        let fields: serde_json::Value =
            serde_json::from_str(message.strip_prefix(OPEN_PREFIX).unwrap()).unwrap();
        assert_eq!(fields["pull_request"], 62);
        assert_eq!(fields["repo"], "runedeck/cli");
        assert_eq!(open.adopt_branch(), "adopt/62");
        assert_eq!(parse(&message), Some(Seal::Open(open.clone())));
        assert_eq!(
            parse(&format!("{message}\n\nbody\n")),
            Some(Seal::Open(open))
        );
        let merge = MergeSeal {
            reviewed_sha: SHA.to_string(),
            generation: 2,
            digest: "ab".repeat(32),
        };
        assert_eq!(parse(&merge.message()), Some(Seal::Merge(merge)));
        // A merge-seal without the ledger digest is the old shape and is
        // not a seal.
        assert_eq!(
            parse(&format!(
                "{MERGE_PREFIX}{{\"reviewed_sha\":\"{SHA}\",\"generation\":2}}"
            )),
            None
        );
        assert_eq!(parse("seal: approve"), None);
        assert_eq!(parse("open-seal: not json"), None);
        assert_eq!(parse(&format!("{message} trailing")), None);
        let short_nonce = OpenSeal {
            repo: "r".to_string(),
            base: "main".to_string(),
            pull_request: 1,
            tree: TREE.to_string(),
            nonce: "abcd".to_string(),
        };
        assert_eq!(parse(&short_nonce.message()), None);
        // A seal without the pull request number is the old shape and is
        // not a seal.
        assert_eq!(
            parse(&format!(
                "{OPEN_PREFIX}{{\"repo\":\"r\",\"base\":\"main\",\"tree\":\"{TREE}\",\"nonce\":\"{}\"}}",
                "ab".repeat(32)
            )),
            None
        );
        let short_sha = MergeSeal {
            reviewed_sha: "abc".to_string(),
            generation: 1,
            digest: "ab".repeat(32),
        };
        assert_eq!(parse(&short_sha.message()), None);
    }

    #[test]
    fn a_body_names_its_nonce_exactly_once() {
        let nonce = "cd".repeat(32);
        let body = body_with_nonce("## Plan\n\nx\n", &nonce);
        assert!(body.ends_with(&format!("\n\nOpen-Seal-Nonce: {nonce}\n")));
        assert_eq!(body_nonce(&body), Some(nonce.clone()));
        assert_eq!(body_nonce("## Plan\n"), None);
        assert_eq!(body_nonce(&body_with_nonce(&body, &"ef".repeat(32))), None);
        assert_eq!(body_nonce("Open-Seal-Nonce: short\n"), None);
    }

    #[test]
    fn a_github_remote_yields_its_slug_and_others_stay_verbatim() {
        assert_eq!(repo_slug("git@github.com:runedeck/cli.git"), "runedeck/cli");
        assert_eq!(repo_slug("https://github.com/runedeck/cli"), "runedeck/cli");
        assert_eq!(
            repo_slug("https://github.com/runedeck/cli.git/"),
            "runedeck/cli"
        );
        assert_eq!(repo_slug("/tmp/remote.git"), "/tmp/remote.git");
    }
}
