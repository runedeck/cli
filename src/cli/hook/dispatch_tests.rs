//! The dispatcher against a fake runner: every scenario of the spec that
//! does not need a real process.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use rune::hooks::{Harness, Hooks};
use serde_json::{Value, json};

use super::encode::Effect;
use super::payload::HandlerPayload;
use super::run::{Runner, dispatch};

/// A fake handler: what it prints, how it exits, how long it takes.
#[derive(Clone)]
struct Fake {
    stdout: String,
    code: i32,
    sleep_ms: u64,
}

fn fake(stdout: &str) -> Fake {
    Fake {
        stdout: stdout.to_string(),
        code: 0,
        sleep_ms: 0,
    }
}

/// A runner that answers from a table and records every payload it saw.
type Seen = Rc<RefCell<Vec<(String, HandlerPayload)>>>;

fn runner(fakes: HashMap<String, Fake>) -> (Runner, Seen) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let runner = Runner {
        record_dir: None,
        record_error: None,
        spawn: Box::new(move |handler, stdin, timeout, _env, _cwd| {
            let payload: HandlerPayload = serde_json::from_slice(stdin).expect("payload json");
            log.borrow_mut().push((handler.id.clone(), payload));
            let Some(fake) = fakes.get(&handler.id) else {
                return Err("cannot start: no such handler".to_string());
            };
            if Duration::from_millis(fake.sleep_ms) > timeout {
                // A real timeout spends the budget; the fake spends it too.
                std::thread::sleep(timeout);
                return Err(format!("timed out after {} ms", timeout.as_millis()));
            }
            Ok((Some(fake.code), fake.stdout.clone(), String::new()))
        }),
    };
    (runner, seen)
}

fn plan(yaml: &str) -> rune::hooks::Plan {
    serde_yaml::from_str::<Hooks>(yaml)
        .unwrap()
        .compile()
        .unwrap()
}

fn claude_tool(command: &str) -> Value {
    json!({"hook_event_name": "PreToolUse", "session_id": "s", "cwd": "/", "tool_name": "Bash", "tool_input": {"command": command}})
}

#[test]
fn a_denial_on_claude_tool_before_ends_the_gate() {
    let plan = plan(
        "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    order: 10\n    on_failure: deny\n  - id: later\n    exec: [later]\n    events: [tool.before]\n    order: 20\n",
    );
    let (runner, seen) = runner(HashMap::from([
        (
            "dcg".to_string(),
            fake(r#"{"effect":"deny","reason":"no rm"}"#),
        ),
        ("later".to_string(), fake("")),
    ]));
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PreToolUse",
        Ok(claude_tool("rm -rf /")),
        &runner,
    );
    let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(
        out["hookSpecificOutput"]["permissionDecisionReason"],
        "no rm"
    );
    assert_eq!(answer.exit, 0);
    assert_eq!(seen.borrow().len(), 1, "the second handler never ran");
}

#[test]
fn a_rewriter_before_the_guard_changes_what_the_guard_sees() {
    let plan = plan(
        "handlers:\n  - id: rtk\n    exec: [rtk]\n    events: [tool.before]\n    order: 10\n    rewrites: true\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    order: 20\n    on_failure: deny\n",
    );
    let (runner, seen) = runner(HashMap::from([
        (
            "rtk".to_string(),
            fake(r#"{"effect":"pass","input":{"command":"rtk git status"}}"#),
        ),
        ("dcg".to_string(), fake("")),
    ]));
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PreToolUse",
        Ok(claude_tool("git status")),
        &runner,
    );
    let seen = seen.borrow();
    let dcg_saw = &seen[1].1;
    assert_eq!(
        dcg_saw.tool.as_ref().unwrap().input["command"],
        "rtk git status"
    );
    assert_eq!(dcg_saw.raw["tool_input"]["command"], "git status");
    let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
    assert_eq!(
        out["hookSpecificOutput"]["updatedInput"]["command"],
        "rtk git status"
    );
}

#[test]
fn a_guard_timeout_denies_and_a_linter_crash_warns() {
    let plan = plan(
        "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    timeout_ms: 100\n    on_failure: deny\n  - id: lint\n    exec: [lint]\n    events: [tool.after]\n  - id: next\n    exec: [next]\n    events: [tool.after]\n    order: 200\n",
    );
    let (runner, seen) = runner(HashMap::from([
        (
            "dcg".to_string(),
            Fake {
                stdout: String::new(),
                code: 0,
                sleep_ms: 5_000,
            },
        ),
        (
            "lint".to_string(),
            Fake {
                stdout: String::new(),
                code: 3,
                sleep_ms: 0,
            },
        ),
        ("next".to_string(), fake("")),
    ]));
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PreToolUse",
        Ok(claude_tool("ls")),
        &runner,
    );
    assert!(
        answer
            .stdout
            .unwrap()
            .contains("\"permissionDecision\":\"deny\"")
    );
    assert!(answer.stderr[0].contains("timed out"));
    let after = json!({"hook_event_name": "PostToolUse", "session_id": "s", "cwd": "/", "tool_name": "Edit", "tool_input": {}});
    let answer = dispatch(&plan, Harness::Claude, "PostToolUse", Ok(after), &runner);
    assert_eq!(answer.stdout, None, "the edit stands");
    assert!(answer.stderr[0].contains("lint failed: exited 3 (warn, proceeding)"));
    assert_eq!(
        seen.borrow().last().unwrap().0,
        "next",
        "the next handler ran"
    );
}

#[test]
fn a_spent_budget_fails_the_pending_guard_without_spawning_it() {
    let plan = plan(
        "events:\n  tool.before: {budget_ms: 60}\nhandlers:\n  - id: slow\n    exec: [slow]\n    events: [tool.before]\n    order: 10\n    timeout_ms: 5000\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    order: 20\n    on_failure: deny\n",
    );
    let (runner, seen) = runner(HashMap::from([
        (
            "slow".to_string(),
            Fake {
                stdout: String::new(),
                code: 0,
                sleep_ms: 5_000,
            },
        ),
        ("dcg".to_string(), fake("")),
    ]));
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PreToolUse",
        Ok(claude_tool("ls")),
        &runner,
    );
    assert!(answer.stdout.unwrap().contains("deny"));
    assert!(
        answer.stderr.iter().any(|l| l.contains("budget")),
        "{:?}",
        answer.stderr
    );
    assert_eq!(seen.borrow().len(), 1, "dcg was never spawned");
}

#[test]
fn an_event_mismatch_under_a_guard_denies_and_runs_nothing() {
    let plan = plan(
        "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    on_failure: deny\n",
    );
    let (runner, seen) = runner(HashMap::from([("dcg".to_string(), fake(""))]));
    let raw = json!({"hook_event_name": "Stop", "session_id": "s"});
    let answer = dispatch(&plan, Harness::Claude, "PreToolUse", Ok(raw), &runner);
    assert!(answer.stdout.unwrap().contains("deny"));
    assert!(answer.stderr[0].contains("`Stop`") && answer.stderr[0].contains("`PreToolUse`"));
    assert_eq!(answer.exit, 0);
    assert!(seen.borrow().is_empty());
}

#[test]
fn a_missing_payload_on_a_passive_event_proceeds() {
    let plan = plan("handlers:\n  - id: cap\n    exec: [cap]\n    events: [session.end]\n");
    let (runner, _) = runner(HashMap::new());
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "SessionEnd",
        Err("payload is not JSON".into()),
        &runner,
    );
    assert_eq!(answer.stdout, None);
    assert_eq!(answer.exit, 0);
    assert!(answer.stderr[0].contains("proceeding"));
}

#[test]
fn a_collect_event_merges_context_in_order_and_keeps_a_denial() {
    let plan = plan(
        "handlers:\n  - id: a\n    exec: [a]\n    events: [tool.after]\n    order: 10\n  - id: b\n    exec: [b]\n    events: [tool.after]\n    order: 20\n  - id: c\n    exec: [c]\n    events: [tool.after]\n    order: 30\n",
    );
    let (runner, seen) = runner(HashMap::from([
        ("a".to_string(), fake(r#"{"context":["one"]}"#)),
        (
            "b".to_string(),
            fake(r#"{"effect":"deny","reason":"lint"}"#),
        ),
        ("c".to_string(), fake(r#"{"context":["three"]}"#)),
    ]));
    let after = json!({"hook_event_name": "PostToolUse", "session_id": "s", "cwd": "/", "tool_name": "Edit", "tool_input": {}});
    let answer = dispatch(&plan, Harness::Claude, "PostToolUse", Ok(after), &runner);
    assert_eq!(seen.borrow().len(), 3, "every collector ran");
    let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
    assert_eq!(out["decision"], "block");
    assert_eq!(out["reason"], "lint");
    assert_eq!(out["hookSpecificOutput"]["additionalContext"], "one\nthree");
}

#[test]
fn a_grok_payload_is_normalized_for_the_handler_and_the_answer_is_groks() {
    let plan = plan(
        "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    on_failure: deny\n",
    );
    let (runner, seen) = runner(HashMap::from([(
        "dcg".to_string(),
        fake(r#"{"effect":"deny","reason":"no"}"#),
    )]));
    let raw = json!({"hookEventName": "pre_tool_use", "hook_event_name": "PreToolUse", "sessionId": "g1", "cwd": "/", "toolName": "Bash", "toolInput": {"command": "x"}});
    let answer = dispatch(&plan, Harness::Grok, "PreToolUse", Ok(raw), &runner);
    let saw = &seen.borrow()[0].1;
    assert_eq!(saw.session_id.as_deref(), Some("g1"));
    assert_eq!(saw.tool.as_ref().unwrap().name, "Bash");
    let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
    assert_eq!(out["decision"], "deny");
}

#[test]
fn a_denial_on_a_passive_event_is_logged_not_applied() {
    let plan = plan("handlers:\n  - id: id\n    exec: [id]\n    events: [session.start]\n");
    let (runner, _) = runner(HashMap::from([(
        "id".to_string(),
        fake(r#"{"effect":"deny","reason":"x"}"#),
    )]));
    let raw = json!({"hook_event_name": "SessionStart", "session_id": "s"});
    let answer = dispatch(&plan, Harness::Claude, "SessionStart", Ok(raw), &runner);
    assert_eq!(answer.stdout, None);
    assert!(answer.stderr[0].contains("passive"));
    let _ = Effect::Pass;
}

#[test]
fn an_unknown_registration_runs_nothing() {
    let plan = plan("handlers:\n  - id: a\n    exec: [a]\n    events: [tool.before]\n");
    let (runner, seen) = runner(HashMap::new());
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "Notification",
        Ok(json!({})),
        &runner,
    );
    assert_eq!(answer.stdout, None);
    assert!(answer.stderr[0].contains("no handler subscribes"));
    assert!(seen.borrow().is_empty());
}

// ------------------------------------------------------------ the record ----

fn recording_runner(fakes: HashMap<String, Fake>, dir: &std::path::Path) -> Runner {
    let (mut runner, _seen) = runner(fakes);
    runner.record_dir = Some(dir.to_path_buf());
    runner
}

fn claude_write(path: &str) -> Value {
    json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": "/w", "tool_name": "Write", "tool_use_id": "toolu_9", "tool_input": {"file_path": path}, "tool_response": {}})
}

#[test]
fn two_hooks_on_one_event_leave_two_records_in_order_with_the_answer() {
    let temp = tempfile::tempdir().unwrap();
    let plan = plan(
        "handlers:\n  - id: dcg\n    exec: [dcg]\n    events: [tool.after]\n    order: 10\n  - id: lint-on-write\n    exec: [lint]\n    events: [tool.after]\n    order: 20\n",
    );
    let runner = recording_runner(
        HashMap::from([
            ("dcg".to_string(), fake("")),
            (
                "lint-on-write".to_string(),
                fake(
                    r#"{"effect":"deny","reason":"/w/a.md:3:52 error Core.NoEmDash: No em dash. The sentence \"quick brown fox\" is quoted.","metrics":{"rules":{"Core.NoEmDash":1},"cause":["Core.NoEmDash"]}}"#,
                ),
            ),
        ]),
        temp.path(),
    );
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PostToolUse",
        Ok(claude_write("/w/a.md")),
        &runner,
    );
    let out: Value = serde_json::from_str(answer.stdout.as_deref().unwrap()).unwrap();
    assert_eq!(out["decision"], "block");
    let records = super::record::load(temp.path()).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].hook, "dcg");
    assert_eq!(records[0].asked, "allow");
    assert_eq!(records[0].answer, "deny");
    assert_eq!(records[1].hook, "lint-on-write");
    assert_eq!(records[1].asked, "deny");
    assert_eq!(records[1].answer, "deny");
    assert_eq!(records[1].exit_status, Some(0));
    assert_eq!(records[1].file.as_deref(), Some("/w/a.md"));
    assert_eq!(records[1].tool_use_id.as_deref(), Some("toolu_9"));
    assert_eq!(records[1].session_id.as_deref(), Some("s1"));
    assert_eq!(
        records[1].metrics.as_ref().unwrap()["rules"]["Core.NoEmDash"],
        1
    );
    let text = std::fs::read_to_string(
        temp.path().join(
            super::record::path_for(temp.path(), &chrono::Utc::now())
                .file_name()
                .unwrap(),
        ),
    )
    .unwrap();
    assert!(
        !text.contains("quick brown fox"),
        "the record must hold no text"
    );
}

#[test]
fn a_budget_spent_before_a_hook_records_the_budget_failure() {
    let temp = tempfile::tempdir().unwrap();
    let plan = plan(
        "events:\n  tool.before: {budget_ms: 60}\nhandlers:\n  - id: slow\n    exec: [slow]\n    events: [tool.before]\n    order: 10\n    timeout_ms: 5000\n  - id: dcg\n    exec: [dcg]\n    events: [tool.before]\n    order: 20\n    on_failure: deny\n",
    );
    let runner = recording_runner(
        HashMap::from([
            (
                "slow".to_string(),
                Fake {
                    stdout: String::new(),
                    code: 0,
                    sleep_ms: 500,
                },
            ),
            ("dcg".to_string(), fake("")),
        ]),
        temp.path(),
    );
    let _ = dispatch(
        &plan,
        Harness::Claude,
        "PreToolUse",
        Ok(claude_tool("ls")),
        &runner,
    );
    let records = super::record::load(temp.path()).unwrap();
    let dcg = records
        .iter()
        .find(|r| r.hook == "dcg")
        .expect("dcg record");
    assert_eq!(dcg.failure.as_deref(), Some("budget"));
    assert_eq!(dcg.duration_ms, Some(0));
    assert_eq!(dcg.answer, "deny");
    let slow = records.iter().find(|r| r.hook == "slow").unwrap();
    assert_eq!(slow.failure.as_deref(), Some("timeout"));
}

#[test]
fn text_in_metrics_is_dropped_and_the_answer_is_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let plan = plan(
        "handlers:\n  - id: lint-on-write\n    exec: [lint]\n    events: [tool.after]\n    order: 20\n",
    );
    let runner = recording_runner(
        HashMap::from([(
            "lint-on-write".to_string(),
            fake(r#"{"effect":"deny","reason":"bad","metrics":{"note":"the quick brown fox"}}"#),
        )]),
        temp.path(),
    );
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PostToolUse",
        Ok(claude_write("/w/a.md")),
        &runner,
    );
    assert!(answer.stdout.as_deref().unwrap().contains("\"block\""));
    assert!(
        answer
            .stderr
            .iter()
            .any(|l| l.contains("lint-on-write") && l.contains("metrics"))
    );
    let records = super::record::load(temp.path()).unwrap();
    assert_eq!(records[0].metrics, None);
    assert_eq!(records[0].asked, "deny");
}

#[cfg(unix)]
#[test]
fn a_read_only_record_directory_leaves_the_answer_and_exit_code_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    if std::env::var("USER").as_deref() == Ok("root") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("record");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let plan = plan(
        "handlers:\n  - id: lint-on-write\n    exec: [lint]\n    events: [tool.after]\n    order: 20\n",
    );
    let fakes = HashMap::from([(
        "lint-on-write".to_string(),
        fake(r#"{"effect":"deny","reason":"bad"}"#),
    )]);
    let silent = recording_runner(fakes.clone(), &dir);
    let answer = dispatch(
        &plan,
        Harness::Claude,
        "PostToolUse",
        Ok(claude_write("/w/a.md")),
        &silent,
    );
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (bare, _) = runner(fakes);
    let expected = dispatch(
        &plan,
        Harness::Claude,
        "PostToolUse",
        Ok(claude_write("/w/a.md")),
        &bare,
    );
    assert_eq!(answer.stdout, expected.stdout);
    assert_eq!(answer.exit, expected.exit);
    let record_lines: Vec<_> = answer
        .stderr
        .iter()
        .filter(|l| l.contains("hook record"))
        .collect();
    assert_eq!(record_lines.len(), 1, "{:?}", answer.stderr);
    assert!(record_lines[0].contains(&dir.display().to_string()));
}
