//! The record writer: monthly files, the directory created, a failed or
//! late write as one line, metrics validation at its edges.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{TimeZone, Utc};
use serde_json::json;

use super::{METRICS_CAP, Record, load, path_for, validate_metrics, write_all, write_all_with};

fn record(ts: &str, hook: &str) -> Record {
    Record {
        v: 1,
        ts: ts.to_string(),
        harness: "claude".into(),
        event: "tool.after".into(),
        native_event: "PostToolUse".into(),
        hook: hook.into(),
        session_id: Some("s1".into()),
        tool_use_id: Some("toolu_1".into()),
        cwd: Some("/w".into()),
        file: Some("/w/a.md".into()),
        duration_ms: Some(12),
        exit_status: Some(0),
        asked: "deny".into(),
        answer: "deny".into(),
        failure: None,
        metrics: None,
        source: None,
    }
}

#[test]
fn two_records_land_in_run_order_and_the_directory_is_created() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("hooks").join("record");
    let errors = write_all(
        &dir,
        vec![
            record("2026-10-16T10:00:00Z", "dcg"),
            record("2026-10-16T10:00:01Z", "lint-on-write"),
        ],
        Duration::from_secs(2),
    );
    assert!(errors.is_empty(), "{errors:?}");
    let records = load(&dir).unwrap();
    assert_eq!(
        records.iter().map(|r| r.hook.as_str()).collect::<Vec<_>>(),
        ["dcg", "lint-on-write"]
    );
    assert!(dir.join("2026-10.jsonl").is_file());
}

#[test]
fn records_across_a_month_boundary_land_in_two_files() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().to_path_buf();
    let errors = write_all(
        &dir,
        vec![
            record("2026-10-31T23:59:59Z", "a"),
            record("2026-11-01T00:00:01Z", "b"),
        ],
        Duration::from_secs(2),
    );
    assert_eq!(errors, Vec::<String>::new());
    assert!(dir.join("2026-10.jsonl").is_file());
    assert!(dir.join("2026-11.jsonl").is_file());
    assert_eq!(
        path_for(&dir, &Utc.with_ymd_and_hms(2027, 1, 5, 0, 0, 0).unwrap()),
        dir.join("2027-01.jsonl")
    );
}

#[cfg(unix)]
#[test]
fn a_read_only_directory_costs_one_line_and_nothing_else() {
    use std::os::unix::fs::PermissionsExt as _;
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("record");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let errors = write_all(
        &dir,
        vec![record("2026-10-16T10:00:00Z", "dcg")],
        Duration::from_secs(2),
    );
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if nix_is_root() {
        return; // root writes anywhere; the scenario does not apply
    }
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains(&dir.display().to_string()));
    assert!(errors[0].contains("1 hook record"));
}

fn nix_is_root() -> bool {
    std::fs::metadata("/").is_ok() && std::env::var("USER").as_deref() == Ok("root")
}

#[allow(clippy::unnecessary_wraps)] // the writer type the record takes
fn stuck(_dir: &Path, _records: &[Record]) -> std::io::Result<BTreeSet<PathBuf>> {
    std::thread::sleep(Duration::from_millis(400));
    Ok(BTreeSet::new())
}

#[test]
fn a_writer_past_the_deadline_loses_the_records_and_returns_in_time() {
    let temp = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let errors = write_all_with(
        temp.path(),
        vec![record("2026-10-16T10:00:00Z", "dcg")],
        Duration::from_millis(50),
        stuck,
    );
    assert!(started.elapsed() < Duration::from_millis(300));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("did not finish within 50 ms"));
    assert!(errors[0].contains("1 record(s) lost"));
}

#[test]
fn nothing_to_write_costs_nothing() {
    let errors = write_all(
        Path::new("/nonexistent/x"),
        Vec::new(),
        Duration::from_millis(1),
    );
    assert_eq!(errors, Vec::<String>::new());
}

#[test]
fn conforming_metrics_pass_and_text_is_refused() {
    assert_eq!(
        validate_metrics(&json!({"rules": {"MD013": 2, "typos": 1}, "cause": ["MD013"], "n": 3})),
        Ok(())
    );
    let err = validate_metrics(&json!({"note": "the quick brown fox"})).unwrap_err();
    assert!(err.contains("not an integer"), "{err}");
    let err = validate_metrics(&json!({"a key": 1})).unwrap_err();
    assert!(err.contains("is not an identifier"), "{err}");
    let err = validate_metrics(&json!({"cause": ["the quick brown fox"]})).unwrap_err();
    assert!(err.contains("is not an identifier"), "{err}");
    let err = validate_metrics(&json!({"rules": {"MD013": "two"}})).unwrap_err();
    assert!(err.contains("not an integer"), "{err}");
    assert!(validate_metrics(&json!([1, 2])).is_err());
}

#[test]
fn metrics_at_the_cap_pass_and_one_byte_above_fail() {
    // {"k":[ "aaaa", ... ]} tuned to exactly the cap.
    let build = |target: usize| {
        let mut items = Vec::new();
        let mut value = json!({"k": items.clone()});
        while value.to_string().len() < target {
            items.push("a".repeat(60));
            value = json!({"k": items.clone()});
        }
        // Trim the last item so the length hits the target exactly.
        let over = value.to_string().len() - target;
        let last = items.pop().unwrap();
        items.push(last[..last.len() - over].to_string());
        json!({"k": items})
    };
    let at = build(METRICS_CAP);
    assert_eq!(at.to_string().len(), METRICS_CAP);
    assert_eq!(validate_metrics(&at), Ok(()));
    let above = build(METRICS_CAP + 1);
    assert_eq!(above.to_string().len(), METRICS_CAP + 1);
    assert!(validate_metrics(&above).unwrap_err().contains("4097 bytes"));
}
