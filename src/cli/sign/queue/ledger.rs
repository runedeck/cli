//! The controller's ledger as `rune sign submit` reads it: the `ledger`
//! check run on the reviewed head names the ledger artifact and its digest,
//! the artifact holds the full record, and the digest proves the one
//! against the other.

use super::gh::{self, Check, LedgerLine};
use super::store::{Coverage, Thread};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Lane statuses the controller records once a lane is done with a head.
const TERMINAL: [&str; 6] = [
    "completed",
    "completed-no-findings",
    "skipped",
    "ineligible",
    "failed",
    "rate-limited",
];

/// The coverage state the controller writes when the paid lane stood down.
const FREE_LANES_ONLY: &str = "free lanes only";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Ledger {
    pub reviewed_sha: String,
    pub generation: u64,
    #[serde(default)]
    pub verdict: Option<Verdict>,
    /// The stand-down state, `free lanes only: <reason>`, when no paid
    /// round judged this head and generation.
    #[serde(default)]
    pub coverage: Option<String>,
    #[serde(default)]
    pub lanes: BTreeMap<String, String>,
    #[serde(default)]
    pub threads: Vec<Thread>,
}

/// The paid lane's verdict as the controller records it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub(crate) struct Verdict {
    pub sha: String,
    pub generation: u64,
    /// `clean` or `findings`.
    pub verdict: String,
}

/// The ledger for `head`, proven: the controller's check run on the head
/// names the artifact and its digest, and the artifact hashes to it. `None`
/// when the controller has published nothing on this head.
pub(crate) fn fetch(
    repo: &str,
    head: &str,
) -> Result<Option<(LedgerLine, Ledger)>, rune::error::Error> {
    let Some(line) = gh::ledger_line(repo, head)? else {
        return Ok(None);
    };
    let bytes = gh::ledger_artifact(repo, line.artifact_id)?;
    let ledger = prove(&line, &bytes)
        .map_err(|reason| rune::error::Error::new(rune::error::ErrorKind::Parse, reason))?;
    Ok(Some((line, ledger)))
}

/// The ledger inside the artifact bytes, when they hash to the digest the
/// line names and describe the head the line names.
pub(crate) fn prove(line: &LedgerLine, bytes: &[u8]) -> Result<Ledger, String> {
    let digest = super::super::seal::hex(&Sha256::digest(bytes));
    if digest != line.digest {
        return Err(format!(
            "ledger artifact {} hashes to {} and the controller's line names {}: the artifact is not the ledger",
            line.artifact_id,
            short(&digest),
            short(&line.digest)
        ));
    }
    let ledger: Ledger = serde_json::from_slice(bytes)
        .map_err(|error| format!("the ledger is not valid JSON: {error}"))?;
    if ledger.reviewed_sha != line.reviewed_sha || ledger.generation != line.generation {
        return Err(format!(
            "the ledger artifact records {} at generation {} and the controller's line names {} at generation {}",
            short(&ledger.reviewed_sha),
            ledger.generation,
            short(&line.reviewed_sha),
            line.generation
        ));
    }
    Ok(ledger)
}

/// The admission rule of `rune sign submit`: the head is queued for a
/// merge-seal only when every condition holds, and the first failing one
/// is the refusal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admit(
    ledger: &Ledger,
    line: &LedgerLine,
    repo: &str,
    pull_request: u64,
    base: &str,
    head: &str,
    checks: &[Check],
) -> Result<Coverage, String> {
    if ledger.reviewed_sha != head {
        return Err(format!(
            "the ledger reviewed {} and the head is {}: wait for the controller",
            short(&ledger.reviewed_sha),
            short(head)
        ));
    }
    if line.pull_request != pull_request {
        return Err(format!(
            "the ledger belongs to pull request #{} and this is #{pull_request}",
            line.pull_request
        ));
    }
    let (state, reason) = match &ledger.verdict {
        Some(verdict) => {
            if verdict.sha != ledger.reviewed_sha || verdict.generation != ledger.generation {
                return Err(format!(
                    "the verdict binds to {} at generation {} and the ledger is at generation {}: re-adjudication is pending",
                    short(&verdict.sha),
                    verdict.generation,
                    ledger.generation
                ));
            }
            match verdict.verdict.as_str() {
                "clean" => ("clean", None),
                "findings" => {
                    return Err(
                        "the verdict is findings: address them and wait for the next round"
                            .to_string(),
                    );
                }
                other => {
                    return Err(format!("the verdict is {other}, not clean or findings"));
                }
            }
        }
        None => match ledger.coverage.as_deref().map(str::trim) {
            Some(coverage) if coverage.starts_with(FREE_LANES_ONLY) => {
                ("free-lanes-only", Some(coverage.to_string()))
            }
            _ => return Err("the ledger records no verdict on this head".to_string()),
        },
    };
    for (lane, status) in &ledger.lanes {
        if !TERMINAL.contains(&status.as_str()) {
            return Err(format!("lane {lane} is {status}, not terminal"));
        }
    }
    for thread in &ledger.threads {
        match thread.disposition.as_deref() {
            Some("fixed" | "rejected") => {}
            Some("owner") => {
                return Err(format!(
                    "thread {} is disposed owner: clear or reject it first",
                    thread.id
                ));
            }
            _ => {
                return Err(format!(
                    "thread {} is open without a disposition",
                    thread.id
                ));
            }
        }
    }
    if let Some(check) = checks.iter().find(|check| check.bucket != "pass") {
        return Err(format!("required check {} is {}", check.name, check.bucket));
    }
    Ok(Coverage {
        repo: repo.to_string(),
        pull_request,
        base: base.to_string(),
        reviewed_sha: ledger.reviewed_sha.clone(),
        generation: ledger.generation,
        digest: line.digest.clone(),
        verdict: state.to_string(),
        reason,
        lanes: ledger.lanes.clone(),
        threads: ledger.threads.clone(),
    })
}

fn short(commit: &str) -> &str {
    &commit[..12.min(commit.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "25d45bbb35a811f68ebce7a00910790aeef3e77c";

    fn ledger(generation: u64, verdict_generation: u64) -> Ledger {
        Ledger {
            reviewed_sha: SHA.to_string(),
            generation,
            verdict: Some(Verdict {
                sha: SHA.to_string(),
                generation: verdict_generation,
                verdict: "clean".to_string(),
            }),
            coverage: None,
            lanes: BTreeMap::from([
                ("codex".to_string(), "completed-no-findings".to_string()),
                ("runeseer".to_string(), "completed".to_string()),
            ]),
            threads: vec![Thread {
                id: "T1".to_string(),
                lane: Some("codex".to_string()),
                disposition: Some("fixed".to_string()),
                reason: None,
            }],
        }
    }

    fn line(generation: u64) -> LedgerLine {
        LedgerLine {
            artifact_id: 4242,
            digest: "ab".repeat(32),
            generation,
            pull_request: 7,
            reviewed_sha: SHA.to_string(),
        }
    }

    fn checks() -> Vec<Check> {
        vec![Check {
            name: "quality".to_string(),
            bucket: "pass".to_string(),
        }]
    }

    #[test]
    fn the_artifact_is_proven_by_the_line_it_hangs_from() {
        let bytes = format!(
            "{{\"reviewed_sha\":\"{SHA}\",\"generation\":2,\"threads\":[{{\"id\":\"T1\",\"lane\":null}}]}}"
        );
        let mut line = line(2);
        line.digest = super::super::super::seal::hex(&Sha256::digest(bytes.as_bytes()));
        let ledger = prove(&line, bytes.as_bytes()).expect("proven");
        assert_eq!(ledger.generation, 2);
        assert_eq!(ledger.threads[0].lane, None);
        let error = prove(&line, format!("{bytes}\n").as_bytes()).unwrap_err();
        assert!(error.contains("is not the ledger"), "{error}");
        line.generation = 3;
        let error = prove(&line, bytes.as_bytes()).unwrap_err();
        assert!(error.contains("at generation 3"), "{error}");
    }

    #[test]
    fn admission_refuses_each_missing_condition_and_records_coverage() {
        let clean = ledger(2, 2);
        let coverage = admit(&clean, &line(2), "o/r", 7, "main", SHA, &checks()).expect("admitted");
        assert_eq!(coverage.generation, 2);
        assert_eq!(coverage.verdict, "clean");
        assert_eq!(coverage.digest, "ab".repeat(32));

        let error = admit(&ledger(3, 2), &line(3), "o/r", 7, "main", SHA, &checks()).unwrap_err();
        assert!(error.contains("generation 3"), "{error}");

        let error = admit(&clean, &line(2), "o/r", 7, "main", "ffff", &checks()).unwrap_err();
        assert!(error.contains("wait for the controller"), "{error}");

        let error = admit(&clean, &line(2), "o/r", 8, "main", SHA, &checks()).unwrap_err();
        assert!(error.contains("belongs to pull request #7"), "{error}");

        let mut owner = clean.clone();
        owner.threads[0].disposition = Some("owner".to_string());
        assert!(
            admit(&owner, &line(2), "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("disposed owner")
        );
        let mut open = clean.clone();
        open.threads[0].disposition = None;
        assert!(
            admit(&open, &line(2), "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("open without a disposition")
        );
        let mut pending = clean.clone();
        pending
            .lanes
            .insert("cursor".to_string(), "pending".to_string());
        assert!(
            admit(&pending, &line(2), "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("not terminal")
        );
        let failing = vec![Check {
            name: "quality".to_string(),
            bucket: "fail".to_string(),
        }];
        assert!(
            admit(&clean, &line(2), "o/r", 7, "main", SHA, &failing)
                .unwrap_err()
                .contains("required check quality is fail")
        );
        let mut findings = clean.clone();
        findings.verdict.as_mut().unwrap().verdict = "findings".to_string();
        assert!(
            admit(&findings, &line(2), "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("the verdict is findings")
        );
        // A stand-down is a valid queue state: the free lanes covered the
        // head and the owner sees why the paid lane did not.
        let mut free = clean.clone();
        free.verdict = None;
        assert!(
            admit(&free, &line(2), "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("no verdict")
        );
        free.coverage = Some("free lanes only: prose only".to_string());
        let coverage =
            admit(&free, &line(2), "o/r", 7, "main", SHA, &checks()).expect("free lanes admitted");
        assert_eq!(coverage.verdict, "free-lanes-only");
        assert_eq!(
            coverage.reason.as_deref(),
            Some("free lanes only: prose only")
        );
    }
}
