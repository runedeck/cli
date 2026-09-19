//! The controller's ledger as `rune sign submit` reads it: one JSON object
//! in a pull request comment that starts with the ledger marker, either as
//! a fenced `json` block or as the rest of the comment.

use super::gh::{Check, LEDGER_MARKER};
use super::store::{Coverage, Thread};
use serde::Deserialize;
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Ledger {
    pub reviewed_sha: String,
    pub generation: u64,
    #[serde(default)]
    pub verdict: Option<Verdict>,
    #[serde(default)]
    pub lanes: BTreeMap<String, String>,
    #[serde(default)]
    pub threads: Vec<Thread>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Verdict {
    pub sha: String,
    pub generation: u64,
    /// `clean` or `free-lanes-only`.
    pub state: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// The ledger inside a comment body, or the reason it cannot be read.
pub(crate) fn parse(body: &str) -> Result<Ledger, String> {
    let rest = body
        .trim_start()
        .strip_prefix(LEDGER_MARKER)
        .ok_or_else(|| "the comment does not start with the ledger marker".to_string())?;
    let json = fenced_json(rest).unwrap_or(rest);
    serde_json::from_str(json.trim())
        .map_err(|error| format!("the ledger is not valid JSON: {error}"))
}

fn fenced_json(text: &str) -> Option<&str> {
    let start = text.find("```json")? + "```json".len();
    let end = text[start..].find("```")? + start;
    Some(&text[start..end])
}

/// The admission rule of `rune sign submit`: the head is queued for a
/// merge-seal only when every condition holds, and the first failing one
/// is the refusal.
pub(crate) fn admit(
    ledger: &Ledger,
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
    let verdict = ledger
        .verdict
        .as_ref()
        .ok_or_else(|| "the ledger records no verdict on this head".to_string())?;
    if verdict.sha != ledger.reviewed_sha || verdict.generation != ledger.generation {
        return Err(format!(
            "the verdict binds to {} at generation {} and the ledger is at generation {}: re-adjudication is pending",
            short(&verdict.sha),
            verdict.generation,
            ledger.generation
        ));
    }
    match verdict.state.as_str() {
        "clean" => {}
        "free-lanes-only"
            if verdict
                .reason
                .as_deref()
                .is_some_and(|r| !r.trim().is_empty()) => {}
        "free-lanes-only" => {
            return Err("the verdict is free-lanes-only without a reason".to_string());
        }
        other => {
            return Err(format!(
                "the verdict is {other}, not clean or free-lanes-only"
            ));
        }
    }
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
        verdict: verdict.state.clone(),
        reason: verdict.reason.clone(),
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
                state: "clean".to_string(),
                reason: None,
            }),
            lanes: BTreeMap::from([
                ("codex".to_string(), "completed-no-findings".to_string()),
                ("runeseer".to_string(), "completed".to_string()),
            ]),
            threads: vec![Thread {
                id: "T1".to_string(),
                lane: "codex".to_string(),
                disposition: Some("fixed".to_string()),
                reason: None,
            }],
        }
    }

    fn checks() -> Vec<Check> {
        vec![Check {
            name: "quality".to_string(),
            bucket: "pass".to_string(),
        }]
    }

    #[test]
    fn a_ledger_comment_parses_from_a_fence_or_bare() {
        let body = format!(
            "{LEDGER_MARKER}\n```json\n{{\"reviewed_sha\":\"{SHA}\",\"generation\":1}}\n```\n"
        );
        assert_eq!(parse(&body).expect("fenced").generation, 1);
        let bare = format!("{LEDGER_MARKER} {{\"reviewed_sha\":\"{SHA}\",\"generation\":3}}");
        assert_eq!(parse(&bare).expect("bare").generation, 3);
        assert!(parse("a plain comment").is_err());
    }

    #[test]
    fn admission_refuses_each_missing_condition_and_records_coverage() {
        let clean = ledger(2, 2);
        let coverage = admit(&clean, "o/r", 7, "main", SHA, &checks()).expect("admitted");
        assert_eq!(coverage.generation, 2);
        assert_eq!(coverage.verdict, "clean");

        let error = admit(&ledger(3, 2), "o/r", 7, "main", SHA, &checks()).unwrap_err();
        assert!(error.contains("generation 3"), "{error}");

        let error = admit(&clean, "o/r", 7, "main", "ffff", &checks()).unwrap_err();
        assert!(error.contains("wait for the controller"), "{error}");

        let mut owner = clean.clone();
        owner.threads[0].disposition = Some("owner".to_string());
        assert!(
            admit(&owner, "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("disposed owner")
        );
        let mut open = clean.clone();
        open.threads[0].disposition = None;
        assert!(
            admit(&open, "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("open without a disposition")
        );
        let mut pending = clean.clone();
        pending
            .lanes
            .insert("cursor".to_string(), "pending".to_string());
        assert!(
            admit(&pending, "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("not terminal")
        );
        let failing = vec![Check {
            name: "quality".to_string(),
            bucket: "fail".to_string(),
        }];
        assert!(
            admit(&clean, "o/r", 7, "main", SHA, &failing)
                .unwrap_err()
                .contains("required check quality is fail")
        );
        let mut free = clean.clone();
        free.verdict.as_mut().unwrap().state = "free-lanes-only".to_string();
        assert!(
            admit(&free, "o/r", 7, "main", SHA, &checks())
                .unwrap_err()
                .contains("without a reason")
        );
        free.verdict.as_mut().unwrap().reason = Some("prose only".to_string());
        assert_eq!(
            admit(&free, "o/r", 7, "main", SHA, &checks())
                .expect("free lanes admitted")
                .reason
                .as_deref(),
            Some("prose only")
        );
    }
}
