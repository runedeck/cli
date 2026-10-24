//! The Claude import over fixture transcripts: two transcripts twice, a
//! live record already holding the tool use, a grown transcript, a tool
//! use before the offset, and a half-written last line.

use std::io::Write as _;
use std::time::Duration;

use super::super::record::{self, Record};
use super::{checkpoint_tools, import};

fn tool_use(ts: &str, id: &str, path: &str) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"{ts}","sessionId":"s1","message":{{"content":[{{"type":"tool_use","id":"{id}","name":"Write","input":{{"file_path":"{path}"}}}}]}}}}"#
    )
}

fn block(ts: &str, id: &str, text: &str) -> String {
    block_from("/Users/x/.sd/hook", ts, id, text)
}

/// A lint denial as the dispatcher at `hook` reported it.
fn block_from(hook: &str, ts: &str, id: &str, text: &str) -> String {
    format!(
        r#"{{"type":"attachment","timestamp":"{ts}","sessionId":"s1","cwd":"/w","attachment":{{"type":"hook_blocking_error","hookEvent":"PostToolUse","hookName":"PostToolUse:Write","toolUseID":"{id}","blockingError":{{"command":"sh {hook} PostToolUse","blockingError":"[sh {hook} PostToolUse]: {text}"}}}}}}"#
    )
}

#[test]
fn a_denial_from_the_just_dispatcher_is_imported() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    let dir = temp.path().join("record");
    let file = store.join("proj").join("one.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        [
            tool_use("2026-10-24T09:00:00.000Z", "toolu_9", "/w/a.md"),
            block_from(
                "/Users/x/.config/just/hook",
                "2026-10-24T09:00:01.000Z",
                "toolu_9",
                "a.md:7:121: [MD013] Line length 127 exceeds 120 characters",
            ),
        ]
        .join("\n")
            + "\n",
    )
    .unwrap();
    assert_eq!(import(&store, &dir).unwrap().added, 1);
}

fn advice(ts: &str, id: &str, text: &str) -> String {
    format!(
        r#"{{"type":"attachment","timestamp":"{ts}","sessionId":"s1","attachment":{{"type":"hook_additional_context","hookEvent":"PostToolUse","hookName":"PostToolUse:Edit","toolUseID":"{id}","content":["Vale advice for the file you wrote, not a block.\n{text}"]}}}}"#
    )
}

fn store_with_two_transcripts(root: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let a = root.join("proj-a").join("one.jsonl");
    let b = root.join("proj-b").join("sub").join("two.jsonl");
    std::fs::create_dir_all(a.parent().unwrap()).unwrap();
    std::fs::create_dir_all(b.parent().unwrap()).unwrap();
    std::fs::write(
        &a,
        [
            tool_use("2026-09-06T23:23:00.000Z", "toolu_1", "/w/design.md"),
            block("2026-09-06T23:23:37.440Z", "toolu_1", "design.md:7:121: [MD013] Line length 127 exceeds 120 characters\\ndesign.md:9:121: [MD013] Line length 130 exceeds 120 characters"),
            tool_use("2026-09-06T23:24:00.000Z", "toolu_2", "/w/a.md"),
            advice("2026-09-06T23:24:40.000Z", "toolu_2", "/w/a.md:3:91 warning STE.Semicolons: Do not use semicolons in prose."),
            r#"{"type":"user","timestamp":"2026-09-06T23:25:00.000Z","message":{"content":"hi"}}"#.to_string(),
        ]
        .join("\n")
            + "\n",
    )
    .unwrap();
    std::fs::write(
        &b,
        [
            tool_use("2026-09-07T10:00:00.000Z", "toolu_3", "/w/b.md"),
            block("2026-09-07T10:00:01.000Z", "toolu_3", "/w/b.md:3:52 error Core.NoEmDash: No em dash. \\\"the quick brown fox\\\" is quoted.\\nerror: `refernce` should be `reference`"),
            tool_use("2026-09-07T10:01:00.000Z", "toolu_4", "/w/c.md"),
            advice("2026-09-07T10:01:01.000Z", "toolu_4", "/w/c.md:1:1 suggestion STE.StrictWords: avoid 'since'."),
        ]
        .join("\n")
            + "\n",
    )
    .unwrap();
    (a, b)
}

#[test]
fn two_transcripts_import_once_and_a_second_run_adds_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    let dir = temp.path().join("record");
    store_with_two_transcripts(&store);
    let first = import(&store, &dir).unwrap();
    assert_eq!(first.transcripts, 2);
    assert_eq!(first.added, 4);
    let records = record::load(&dir).unwrap();
    assert_eq!(records.len(), 4);
    let md = records
        .iter()
        .find(|r| r.tool_use_id.as_deref() == Some("toolu_1"))
        .unwrap();
    assert_eq!(md.asked, "deny");
    assert_eq!(md.file.as_deref(), Some("/w/design.md"));
    assert_eq!(md.metrics.as_ref().unwrap()["rules"]["MD013"], 2);
    assert_eq!(md.source.as_deref(), Some("import-claude"));
    let adv = records
        .iter()
        .find(|r| r.tool_use_id.as_deref() == Some("toolu_2"))
        .unwrap();
    assert_eq!(adv.asked, "allow");
    assert_eq!(
        adv.metrics.as_ref().unwrap()["cause"],
        serde_json::json!([])
    );
    let text = std::fs::read_to_string(dir.join("2026-09.jsonl")).unwrap();
    assert!(!text.contains("quick brown fox"));
    assert!(!text.contains("refernce"));
    let second = import(&store, &dir).unwrap();
    assert_eq!(second.added, 0);
    assert_eq!(record::load(&dir).unwrap().len(), 4);
}

#[test]
fn a_live_record_that_holds_the_tool_use_is_not_imported_again() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    let dir = temp.path().join("record");
    store_with_two_transcripts(&store);
    let live = Record {
        v: 1,
        ts: "2026-09-07T10:00:01.000Z".into(),
        harness: "claude".into(),
        event: "tool.after".into(),
        native_event: "PostToolUse".into(),
        hook: "lint-on-write".into(),
        session_id: Some("s1".into()),
        tool_use_id: Some("toolu_3".into()),
        cwd: Some("/w".into()),
        file: Some("/w/b.md".into()),
        duration_ms: Some(50),
        exit_status: Some(0),
        asked: "deny".into(),
        answer: "deny".into(),
        failure: None,
        metrics: None,
        source: None,
    };
    assert_eq!(
        record::write_all(&dir, vec![live], Duration::from_secs(2)),
        Vec::<String>::new()
    );
    let report = import(&store, &dir).unwrap();
    assert_eq!(report.added, 3);
    assert_eq!(report.skipped, 1);
    let records = record::load(&dir).unwrap();
    let for_three: Vec<_> = records
        .iter()
        .filter(|r| r.tool_use_id.as_deref() == Some("toolu_3"))
        .collect();
    assert_eq!(for_three.len(), 1);
    assert_eq!(for_three[0].duration_ms, Some(50), "the live record stays");
}

#[test]
fn a_grown_transcript_adds_one_and_resolves_an_earlier_tool_use() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    let dir = temp.path().join("record");
    let (a, _) = store_with_two_transcripts(&store);
    // The tool use for toolu_5 sits before the first import's offset.
    let mut file = std::fs::OpenOptions::new().append(true).open(&a).unwrap();
    writeln!(
        file,
        "{}",
        tool_use("2026-09-06T23:26:00.000Z", "toolu_5", "/w/late.md")
    )
    .unwrap();
    drop(file);
    import(&store, &dir).unwrap();
    assert!(checkpoint_tools(&dir, &a).contains("toolu_5"));
    let mut file = std::fs::OpenOptions::new().append(true).open(&a).unwrap();
    writeln!(
        file,
        "{}",
        block(
            "2026-09-06T23:26:30.000Z",
            "toolu_5",
            "late.md:1:1: [MD041] First line"
        )
    )
    .unwrap();
    drop(file);
    let report = import(&store, &dir).unwrap();
    assert_eq!(report.added, 1);
    let records = record::load(&dir).unwrap();
    let late = records
        .iter()
        .find(|r| r.tool_use_id.as_deref() == Some("toolu_5"))
        .unwrap();
    assert_eq!(late.file.as_deref(), Some("/w/late.md"));
}

#[test]
fn a_half_written_last_line_waits_for_the_next_import() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    let dir = temp.path().join("record");
    let (a, _) = store_with_two_transcripts(&store);
    let partial = block(
        "2026-09-06T23:27:00.000Z",
        "toolu_6",
        "x.md:1:1: [MD041] First line",
    );
    let (head, tail) = partial.split_at(partial.len() / 2);
    let mut file = std::fs::OpenOptions::new().append(true).open(&a).unwrap();
    write!(file, "{head}").unwrap();
    drop(file);
    let first = import(&store, &dir).unwrap();
    assert_eq!(first.added, 4, "the half line is not a record yet");
    let mut file = std::fs::OpenOptions::new().append(true).open(&a).unwrap();
    writeln!(file, "{tail}").unwrap();
    drop(file);
    let second = import(&store, &dir).unwrap();
    assert_eq!(second.added, 1);
    assert!(
        record::load(&dir)
            .unwrap()
            .iter()
            .any(|r| r.tool_use_id.as_deref() == Some("toolu_6"))
    );
}
