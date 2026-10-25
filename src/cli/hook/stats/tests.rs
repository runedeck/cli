//! The listing and the aggregate view over a fixture record.

use serde_json::json;

use super::super::record::Record;
use super::{Filter, list, period, render, select, stats};

fn rec(
    ts: &str,
    session: &str,
    hook: &str,
    file: &str,
    asked: &str,
    rules: &[(&str, u64)],
    cause: &[&str],
) -> Record {
    Record {
        v: 1,
        ts: ts.to_string(),
        harness: "claude".into(),
        event: "tool.after".into(),
        native_event: "PostToolUse".into(),
        hook: hook.into(),
        session_id: Some(session.into()),
        tool_use_id: Some(format!("toolu_{ts}")),
        cwd: Some("/w".into()),
        file: Some(file.into()),
        duration_ms: Some(10),
        exit_status: Some(0),
        asked: asked.into(),
        answer: asked.into(),
        failure: None,
        metrics: Some(json!({
            "rules": rules.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
            "cause": cause,
        })),
        source: None,
        skipped: None,
    }
}

/// Eight runs over three weeks: the fixture every scenario reads.
fn fixture() -> Vec<Record> {
    vec![
        rec(
            "2026-10-05T10:00:00.000Z",
            "s1",
            "lint-on-write",
            "/w/a.md",
            "deny",
            &[("MD013", 3), ("MD036", 1)],
            &["MD013", "MD036"],
        ),
        rec(
            "2026-10-06T10:00:00.000Z",
            "s1",
            "lint-on-write",
            "/w/a.md",
            "deny",
            &[("MD013", 2)],
            &["MD013"],
        ),
        rec(
            "2026-10-07T10:00:00.000Z",
            "s1",
            "lint-on-write",
            "/w/a.md",
            "deny",
            &[("MD013", 2)],
            &["MD013"],
        ),
        rec(
            "2026-10-12T10:00:00.000Z",
            "s2",
            "lint-on-write",
            "/w/b.md",
            "allow",
            &[("STE.Semicolons", 4)],
            &[],
        ),
        rec(
            "2026-10-13T10:00:00.000Z",
            "s2",
            "dcg",
            "/w/b.md",
            "allow",
            &[],
            &[],
        ),
        rec(
            "2026-10-14T10:00:00.000Z",
            "s2",
            "lint-on-write",
            "/w/c.md",
            "deny",
            &[("MD036", 1), ("STE.Semicolons", 1)],
            &["MD036"],
        ),
        rec(
            "2026-10-15T23:30:00.000Z",
            "s3",
            "lint-on-write",
            "/w/d.md",
            "deny",
            &[("MD013", 1), ("Core.NoEmDash", 1)],
            &["Core.NoEmDash", "MD013"],
        ),
        rec(
            "2026-10-16T10:00:00.000Z",
            "s3",
            "lint-on-write",
            "/w/d.md",
            "allow",
            &[("STE.Semicolons", 2)],
            &[],
        ),
    ]
}

#[test]
fn a_week_and_a_date_are_inclusive_utc_periods() {
    let (start, end) = period("2026-W42").unwrap();
    assert_eq!(start.to_rfc3339(), "2026-10-12T00:00:00+00:00");
    assert_eq!(end.to_rfc3339(), "2026-10-18T23:59:59.999+00:00");
    let (start, end) = period("2026-10-15").unwrap();
    assert_eq!(start.to_rfc3339(), "2026-10-15T00:00:00+00:00");
    assert_eq!(end.to_rfc3339(), "2026-10-15T23:59:59.999+00:00");
    assert!(period("yesterday").is_err());
}

#[test]
fn a_filter_selects_one_hook_from_one_week_on_oldest_first() {
    let filter = Filter {
        hook: Some("lint-on-write".into()),
        since: Some("2026-W42".into()),
        ..Filter::default()
    };
    let kept = select(&fixture(), &filter).unwrap();
    assert_eq!(kept.len(), 4);
    assert!(kept.iter().all(|r| r.hook == "lint-on-write"));
    assert_eq!(kept[0].ts, "2026-10-12T10:00:00.000Z");
    assert!(kept.windows(2).all(|w| w[0].ts <= w[1].ts));
}

#[test]
fn filters_combine_and_an_empty_record_lists_nothing() {
    let filter = Filter {
        rule: Some("MD013".into()),
        asked: Some("deny".into()),
        until: Some("2026-10-15".into()),
        ..Filter::default()
    };
    let kept = select(&fixture(), &filter).unwrap();
    assert_eq!(kept.len(), 4);
    assert!(
        kept.iter()
            .all(|r| r.asked == "deny" && r.ts.as_str() <= "2026-10-15T23:59:59.999Z")
    );
    assert!(
        kept.iter()
            .all(|r| r.metrics.as_ref().unwrap()["rules"].get("MD013").is_some())
    );
    assert_eq!(list(&[], false), "");
    assert_eq!(list(&[], true), "");
}

#[test]
fn a_rule_among_the_causes_and_as_the_sole_cause_are_two_columns() {
    let s = stats(&fixture(), 25);
    let md013 = s.rules.iter().find(|r| r.rule == "MD013").unwrap();
    assert_eq!(md013.findings, 8);
    assert_eq!(md013.cause_runs, 4);
    assert_eq!(md013.sole_cause_runs, 2);
    assert_eq!(md013.advice_runs, 0);
    let semi = s.rules.iter().find(|r| r.rule == "STE.Semicolons").unwrap();
    assert_eq!(
        semi.advice_runs, 2,
        "advice counts only runs that asked for allow"
    );
    assert_eq!(semi.cause_runs, 0);
}

#[test]
fn repeat_denials_compare_with_the_denial_just_before() {
    let s = stats(&fixture(), 25);
    // s1 on a.md: denied three times, the second and third with causes
    // [MD013] after [MD013, MD036] then [MD013]: two repeats, one same.
    assert_eq!(s.repeat_denials, 2);
    assert_eq!(s.repeat_denials_same_causes, 1);
    assert_eq!(s.by_asked["deny"], 5);
    assert_eq!(s.by_asked["allow"], 3);
    assert_eq!(s.weeks["2026-W41"]["deny"], 3);
    assert_eq!(s.files[0], ("/w/a.md".to_string(), 3));
    assert_eq!(s.hooks[0], ("lint-on-write".to_string(), 7));
}

#[test]
fn top_cuts_a_tie_by_name() {
    let records = vec![
        rec(
            "2026-10-05T10:00:00.000Z",
            "s1",
            "h",
            "/w/x.md",
            "deny",
            &[("MD036", 4)],
            &["MD036"],
        ),
        rec(
            "2026-10-05T11:00:00.000Z",
            "s1",
            "h",
            "/w/y.md",
            "deny",
            &[("MD013", 4)],
            &["MD013"],
        ),
    ];
    let s = stats(&records, 1);
    assert_eq!(s.rules.len(), 1);
    assert_eq!(s.rules[0].rule, "MD013");
}

#[test]
fn json_carries_every_figure_of_the_table() {
    let s = stats(&fixture(), 25);
    let table = render(&s, false);
    let json: serde_json::Value = serde_json::from_str(&render(&s, true)).unwrap();
    assert_eq!(json["runs"], 8);
    assert_eq!(json["repeat_denials"], 2);
    assert_eq!(json["repeat_denials_same_causes"], 1);
    assert_eq!(json["by_asked"]["deny"], 5);
    for row in json["rules"].as_array().unwrap() {
        let line = format!(
            "{}\t{}\t{}\t{}\t{}",
            row["rule"].as_str().unwrap(),
            row["findings"],
            row["cause_runs"],
            row["sole_cause_runs"],
            row["advice_runs"]
        );
        assert!(table.contains(&line), "{line} missing from\n{table}");
    }
    assert!(table.contains("2026-W41\t0\t3\t0"));
    assert_eq!(json["weeks"]["2026-W41"]["deny"], 3);
}
