//! The pure parts of the wrapper: the card, the key, the slot, the
//! markers, the lock, and the switch's refusal. The jj flow runs against
//! the real binary in `tests/hook_wrap.rs`.

use super::*;

fn payload(event: &str, cwd: Option<&Path>) -> HandlerPayload {
    HandlerPayload {
        v: 1,
        harness: Harness::Claude,
        event: event.into(),
        native_event: "Stop".into(),
        session_id: Some("s".into()),
        turn_id: None,
        cwd: cwd.map(|p| p.display().to_string()),
        transcript_path: None,
        model: None,
        tool: None,
        prompt: None,
        stop_active: None,
        raw: serde_json::json!({}),
    }
}

/// A `tool.after` of a Write to `file`, fired from `cwd`.
fn write_payload(cwd: &Path, file: &Path) -> HandlerPayload {
    HandlerPayload {
        event: "tool.after".into(),
        native_event: "PostToolUse".into(),
        tool: Some(super::super::payload::ToolPayload {
            name: "Write".into(),
            id: None,
            input: serde_json::json!({ "file_path": file.display().to_string() }),
            response: None,
        }),
        ..payload("tool.after", Some(cwd))
    }
}

/// A `tool.after` of a Bash tool that ran `command` from `cwd`.
fn bash_payload(cwd: &Path, command: &str) -> HandlerPayload {
    HandlerPayload {
        event: "tool.after".into(),
        native_event: "PostToolUse".into(),
        tool: Some(super::super::payload::ToolPayload {
            name: "Bash".into(),
            id: None,
            input: serde_json::json!({ "command": command }),
            response: None,
        }),
        ..payload("tool.after", Some(cwd))
    }
}

/// A slot of `keys` with no record of who they are.
fn slot_of(keys: &[&str]) -> Slot {
    Slot {
        v: SLOT_VERSION,
        keys: keys.iter().map(|k| (*k).to_string()).collect(),
        commit: None,
        who: BTreeMap::new(),
    }
}

/// A workspace root with a `.jj/` directory and the switch on.
fn armed_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".jj")).unwrap();
    std::fs::write(dir.path().join(".jj").join(SWITCH), "on\n").unwrap();
    dir
}

/// The keys the slot under `root` holds, in order.
fn keys(root: &Path) -> Vec<String> {
    read_slot(&root.join(".jj").join(SLOT))
        .unwrap()
        .map(|slot| slot.keys.into_iter().collect())
        .unwrap_or_default()
}

#[test]
fn the_card_for_a_bare_change_says_how_to_undo_the_wrap_by_content() {
    let probe = Probe {
        empty: false,
        described: false,
        change: "kxyzwvut".into(),
        title: String::new(),
    };
    let text = card(Harness::Claude, &probe, "2 files changed, 3 insertions(+)");
    assert_eq!(
        text,
        "Turn wrapped: 2 files changed, 3 insertions(+) as kxyzwvut. Review: jj diff -r kxyzwvut. Undo: jj squash --from kxyzwvut --into @ -u, or /rewind; the undo conflicts when a later turn changed the same file."
    );
    assert!(!card(Harness::Codex, &probe, "1 file").contains("/rewind"));
    assert!(!text.contains("op revert"));
}

#[test]
fn the_card_for_a_described_change_keeps_the_owners_change_and_its_title() {
    let probe = Probe {
        empty: false,
        described: true,
        change: "kxyzwvut".into(),
        title: "feat: the owner's work".into(),
    };
    assert_eq!(
        card(Harness::Claude, &probe, "1 file changed"),
        "Turn added to kxyzwvut (feat: the owner's work): 1 file changed. Review: jj diff -r kxyzwvut. Continue in it: jj squash --from @ --into kxyzwvut -u. Undo: jj edit kxyzwvut, which drops the empty working copy."
    );
}

#[test]
fn the_shared_message_names_the_other_sessions_and_the_reset() {
    let keys = slot_of(&["claude-aaaa", "codex-bbbb"]);
    assert_eq!(
        shared(&keys, "claude-aaaa"),
        "nothing wrapped: this session and codex-bbbb touched the workspace since the last wrap; the edits stay in the working copy. Sort it out, then run rune hook wrap reset in it"
    );
    assert_eq!(
        shared(&keys, "gemini-cccc"),
        "nothing wrapped: claude-aaaa, codex-bbbb touched the workspace since the last wrap and this session did not register; the edits stay in the working copy. Sort it out, then run rune hook wrap reset in it"
    );
    assert!(shared(&slot_of(&[]), "claude-aaaa").contains("no session registered a touch here"));
    let with_unknown = slot_of(&["claude-aaaa", UNKNOWN]);
    assert!(
        shared(&with_unknown, "claude-aaaa").starts_with(
            "nothing wrapped: this session and a session without an id touched the workspace"
        ),
        "the unknown key is named by what it is"
    );
    assert!(
        shared(&with_unknown, UNKNOWN).starts_with(
            "nothing wrapped: the payload names no session, so its touch is nobody's, and claude-aaaa touched"
        )
    );
    // A key with a record is named by its harness and its first touch,
    // not by its hash.
    let mut known = slot_of(&["claude-aaaa", "codex-bbbb"]);
    known.who.insert(
        "codex-bbbb".into(),
        Touch {
            harness: Harness::Codex,
            since: "2026-10-26T09:30:00Z".into(),
        },
    );
    assert_eq!(
        shared(&known, "claude-aaaa"),
        "nothing wrapped: this session and a codex session (first touch 2026-10-26T09:30:00Z) touched the workspace since the last wrap; the edits stay in the working copy. Sort it out, then run rune hook wrap reset in it"
    );
}

#[test]
fn the_session_key_hashes_the_id_and_names_the_harness() {
    let key = session_key(Harness::Claude, Some("abc-123"));
    assert!(key.starts_with("claude-"), "{key}");
    assert_eq!(key.len(), "claude-".len() + 16);
    assert!(!key.contains("abc-123"));
    assert_eq!(key, session_key(Harness::Claude, Some("abc-123")));
    assert_ne!(key, session_key(Harness::Codex, Some("abc-123")));
    assert_eq!(session_key(Harness::Claude, None), UNKNOWN);
    assert_eq!(session_key(Harness::Claude, Some("")), UNKNOWN);
}

#[test]
fn the_slot_round_trips_and_a_broken_or_old_one_reads_as_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(SLOT);
    assert_eq!(read_slot(&path).unwrap(), None);
    let mut slot = slot_of(&["claude-0123456789abcdef", "codex-fedcba9876543210"]);
    slot.commit = Some("abc".into());
    slot.who.insert(
        "codex-fedcba9876543210".into(),
        Touch {
            harness: Harness::Codex,
            since: "2026-10-26T09:30:00Z".into(),
        },
    );
    write_slot(&path, &slot).unwrap();
    assert_eq!(read_slot(&path).unwrap(), Some(slot));
    assert!(
        !std::fs::read_dir(dir.path())
            .unwrap()
            .any(|e| e.unwrap().file_name() != SLOT),
        "no temporary file is left behind"
    );
    std::fs::write(&path, "{not json").unwrap();
    assert!(
        read_slot(&path)
            .unwrap_err()
            .contains("not a turn-wrap slot")
    );
    std::fs::write(
        &path,
        r#"{"v":1,"key":"claude-0123456789abcdef","op":"x","commit":"y"}"#,
    )
    .unwrap();
    assert!(read_slot(&path).unwrap_err().contains("version 1 slot"));
}

#[test]
fn the_state_folds_every_touch_marker_into_the_slot_and_removes_it() {
    let root = armed_root();
    touch(root.path(), Harness::Codex, "codex-bbbb").unwrap();
    touch(root.path(), Harness::Codex, "codex-bbbb").unwrap();
    touch(root.path(), Harness::Claude, "claude-aaaa").unwrap();
    let slot = read_state(root.path()).unwrap();
    assert_eq!(
        slot.keys.iter().cloned().collect::<Vec<_>>(),
        ["claude-aaaa", "codex-bbbb"]
    );
    assert_eq!(slot.commit, None, "a marker records no base");
    assert_eq!(
        slot.who["codex-bbbb"].harness,
        Harness::Codex,
        "the marker's record is folded in"
    );
    assert_eq!(slot.who["claude-aaaa"].harness, Harness::Claude);
    let touched = root.path().join(".jj").join(TOUCHED);
    assert_eq!(std::fs::read_dir(&touched).unwrap().count(), 0);
    assert_eq!(
        read_slot(&root.path().join(".jj").join(SLOT)).unwrap(),
        Some(slot),
        "the folded set is written"
    );
}

#[test]
fn a_malformed_slot_is_an_error_and_keeps_its_markers() {
    let root = armed_root();
    std::fs::write(root.path().join(".jj").join(SLOT), "{not json").unwrap();
    touch(root.path(), Harness::Codex, "codex-bbbb").unwrap();
    assert!(read_state(root.path()).is_err());
    assert!(
        root.path()
            .join(".jj")
            .join(TOUCHED)
            .join("codex-bbbb")
            .is_file()
    );
}

#[test]
fn the_lock_is_exclusive_and_waits_a_bounded_time() {
    let root = armed_root();
    let held = Lock::take(root.path(), Duration::ZERO).unwrap();
    let started = Instant::now();
    let why = Lock::take(root.path(), Duration::from_millis(60)).unwrap_err();
    assert!(started.elapsed() >= Duration::from_millis(60), "{why}");
    assert!(why.contains("another wrapper held"), "{why}");
    drop(held);
    // A short bounded wait, not zero: the lock is a `flock`, and a child
    // another test forks in the meantime shares the open file description
    // until it execs, so the lock can outlive the drop for a moment.
    Lock::take(root.path(), Duration::from_secs(2))
        .expect("the lock is free once its holder is dropped");
}

#[test]
fn an_open_that_cannot_take_the_lock_leaves_a_marker() {
    let root = armed_root();
    let _held = Lock::take(root.path(), Duration::ZERO).unwrap();
    // No jj runs here: the lock never comes, so the marker is the record.
    assert_eq!(
        open(root.path(), Harness::Claude, "claude-aaaa", true).unwrap(),
        HandlerResult::default()
    );
    let marker = root.path().join(".jj").join(TOUCHED).join("claude-aaaa");
    let record: Touch = serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
    assert_eq!(record.harness, Harness::Claude, "the marker says who");
    assert!(!root.path().join(".jj").join(SLOT).exists());
}

#[test]
fn a_finish_that_cannot_take_the_lock_wraps_nothing_and_says_so() {
    let root = armed_root();
    let _held = Lock::take(root.path(), Duration::ZERO).unwrap();
    let result = finish(Harness::Claude, root.path(), "claude-aaaa", None, "").unwrap();
    let message = result.message.unwrap();
    assert!(
        message.starts_with("nothing wrapped: another wrapper held"),
        "{message}"
    );
    assert!(
        root.path()
            .join(".jj")
            .join(TOUCHED)
            .join("claude-aaaa")
            .is_file(),
        "the stop is still a touch"
    );
}

#[test]
fn a_finish_on_a_malformed_slot_wraps_nothing_and_names_the_reset() {
    let root = armed_root();
    std::fs::write(root.path().join(".jj").join(SLOT), "{not json").unwrap();
    let result = finish(Harness::Claude, root.path(), "claude-aaaa", None, "").unwrap();
    let message = result.message.unwrap();
    assert!(message.starts_with("nothing wrapped: "), "{message}");
    assert!(message.contains("not a turn-wrap slot"), "{message}");
    assert!(message.ends_with("run rune hook wrap reset"), "{message}");
    assert!(
        root.path().join(".jj").join(SLOT).is_file(),
        "the slot is left for the owner"
    );
}

#[test]
fn a_finish_of_a_session_that_is_not_alone_wraps_nothing_and_runs_no_jj() {
    let root = armed_root();
    touch(root.path(), Harness::Codex, "codex-bbbb").unwrap();
    let result = finish(Harness::Claude, root.path(), "claude-aaaa", None, "").unwrap();
    let message = result.message.unwrap();
    assert!(
        message.contains("this session and a codex session (first touch 20"),
        "{message}"
    );
    assert!(
        message.contains("Z) touched the workspace since the last wrap"),
        "{message}"
    );
    assert_eq!(
        keys(root.path()),
        ["claude-aaaa", "codex-bbbb"],
        "the stop registered this session before it decided"
    );
    // `.jj/` here is a bare directory: a jj command would have failed.
    assert!(
        finish(
            Harness::Claude,
            root.path(),
            "claude-aaaa",
            Some("stop"),
            ""
        )
        .unwrap()
        .message
        .is_none(),
        "a continued turn says nothing"
    );
}

#[test]
fn a_finish_after_a_failed_handler_wraps_nothing_and_keeps_the_key() {
    let root = armed_root();
    // `.jj/` here is a bare directory: a jj command would have failed.
    let result = finish(
        Harness::Claude,
        root.path(),
        "claude-aaaa",
        None,
        "steer,tests",
    )
    .unwrap();
    assert_eq!(
        result.message.unwrap(),
        "nothing wrapped: steer, tests failed or timed out before the wrapper, so the turn may be unfinished; the edits stay in the working copy for this session's next turn"
    );
    assert_eq!(keys(root.path()), ["claude-aaaa"], "the next stop may wrap");
}

#[test]
fn a_stop_without_a_session_id_registers_the_unknown_session_and_wraps_nothing() {
    let root = armed_root();
    let payload = HandlerPayload {
        session_id: None,
        ..payload("turn.finish", Some(root.path()))
    };
    let result = adapter_in(&payload, None, "", None).unwrap();
    assert!(
        result.message.unwrap().starts_with(
            "nothing wrapped: the payload names no session, so its touch is nobody's;"
        )
    );
    assert_eq!(keys(root.path()), [UNKNOWN]);
    let result = finish(Harness::Claude, root.path(), "claude-aaaa", None, "").unwrap();
    assert!(
        result
            .message
            .unwrap()
            .contains("this session and a session without an id touched the workspace"),
        "a later session is not alone with the unknown one"
    );
}

#[test]
fn a_tool_event_registers_the_session_in_the_workspace_its_file_is_in() {
    let root = armed_root();
    let elsewhere = tempfile::tempdir().unwrap();
    let file = root.path().join("src").join("new.rs");
    // The file does not exist yet, as before a Write: the nearest existing
    // ancestor places it. No snapshot runs on `tool.after`, so no jj.
    let payload = write_payload(elsewhere.path(), &file);
    assert_eq!(
        adapter_in(&payload, None, "", None).unwrap(),
        HandlerResult::default()
    );
    assert_eq!(
        keys(root.path()),
        [session_key(Harness::Claude, Some("s"))],
        "the session worked here from another directory"
    );
    assert_eq!(
        armed_roots(&payload, None),
        [root.path().canonicalize().unwrap()].into_iter().collect()
    );
    let unrelated = write_payload(elsewhere.path(), &elsewhere.path().join("x"));
    assert!(armed_roots(&unrelated, None).is_empty());
}

#[test]
fn a_session_is_alone_with_its_own_key_or_an_empty_set_and_never_as_unknown() {
    let root = armed_root();
    assert!(alone(root.path(), "claude-aaaa"), "an empty set");
    touch(root.path(), Harness::Claude, "claude-aaaa").unwrap();
    assert!(alone(root.path(), "claude-aaaa"), "its own marker");
    read_state(root.path()).unwrap();
    assert!(alone(root.path(), "claude-aaaa"), "its own key in the slot");
    touch(root.path(), Harness::Codex, "codex-bbbb").unwrap();
    assert!(
        !alone(root.path(), "claude-aaaa"),
        "another session's marker"
    );
    assert!(!alone(root.path(), "codex-bbbb"), "another session's key");
    assert!(!alone(root.path(), UNKNOWN));
    std::fs::write(root.path().join(".jj").join(SLOT), "{not json").unwrap();
    assert!(!alone(root.path(), "claude-aaaa"), "a malformed slot");
    let mut payload = payload("turn.finish", Some(root.path()));
    assert_eq!(hold(&payload), Hold::Card, "the card still needs its time");
    let other = tempfile::tempdir().unwrap();
    payload.cwd = Some(other.path().display().to_string());
    assert_eq!(
        hold(&payload),
        Hold::Nothing,
        "outside a workspace with the switch on"
    );
    let fresh = armed_root();
    payload.cwd = Some(fresh.path().display().to_string());
    assert_eq!(hold(&payload), Hold::Turn);
    touch(fresh.path(), Harness::Codex, "codex-bbbb").unwrap();
    assert_eq!(hold(&payload), Hold::Card, "another session's marker");
}

#[test]
fn a_shell_command_registers_the_session_in_every_workspace_it_names() {
    let root = armed_root();
    let elsewhere = tempfile::tempdir().unwrap();
    let here = root.path().display().to_string();
    let only_root: BTreeSet<PathBuf> = [root.path().canonicalize().unwrap()].into_iter().collect();
    // A `cd` into the workspace, then an edit by a relative path. No
    // snapshot runs on `tool.after`, so no jj.
    let payload = bash_payload(
        elsewhere.path(),
        &format!("cd {here} && sd old new conf.txt"),
    );
    assert_eq!(
        adapter_in(&payload, None, "", None).unwrap(),
        HandlerResult::default()
    );
    assert_eq!(
        keys(root.path()),
        [session_key(Harness::Claude, Some("s"))],
        "the command worked here from another directory"
    );
    assert_eq!(armed_roots(&payload, None), only_root);
    // The other ways a command names a path in the workspace.
    let name = root
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let parent = root.path().parent().unwrap();
    for command in [
        format!("cd {name}/src && cargo test"),
        format!("cat {here}/README.md"),
        format!("jj -R {here} log"),
        format!("git -C {here} status"),
        format!("jj --repo={here} log"),
        format!("echo hi >{here}/notes.md"),
        format!("(cd {here}; make) 2>/dev/null"),
    ] {
        let cwd = if command.starts_with("cd ") && !command.contains(&here) {
            parent
        } else {
            elsewhere.path()
        };
        assert_eq!(
            armed_roots(&bash_payload(cwd, &command), None),
            only_root,
            "{command}"
        );
    }
    // A command that names no path in it touches nothing there.
    let unrelated = bash_payload(elsewhere.path(), "cargo test -- wrap");
    assert!(armed_roots(&unrelated, None).is_empty());
}

#[test]
fn the_paths_a_command_names_come_from_its_words_without_a_shell() {
    let home = Path::new("/home/owner");
    let base = Path::new("/work");
    let named = |command: &str| named_paths(command, Some(base), Some(home));
    assert_eq!(
        named("ls ~/x ~ --repo=~/y -R /abs rel 2>err <in >>out && (cd /r; make) || true"),
        [
            "/work/ls",
            "/home/owner/x",
            "/home/owner",
            "/home/owner/y",
            "/abs",
            "/work/rel",
            "/work/err",
            "/work/in",
            "/work/out",
            "/work/cd",
            "/r",
            "/work/make",
            "/work/true",
        ]
        .map(PathBuf::from),
        "a command word lands under the base too, which the event's directory registers already"
    );
    assert_eq!(
        named_paths("ls rel ~/x /abs", None, None),
        [PathBuf::from("/abs")],
        "no base places a relative word, and no home a tilde"
    );
    assert_eq!(named("-v --flag"), Vec::<PathBuf>::new());
}

#[test]
fn the_first_touch_writes_its_key_before_the_snapshot_it_may_lose() {
    let root = armed_root();
    // `.jj/` here is a bare directory, so the snapshot fails: the key is
    // down all the same, as it is when the dispatcher kills a slow one.
    let why = register(root.path(), Harness::Claude, "claude-aaaa", true).unwrap_err();
    assert!(why.contains("jj"), "{why}");
    assert_eq!(keys(root.path()), ["claude-aaaa"]);
    assert_eq!(
        read_slot(&root.path().join(".jj").join(SLOT))
            .unwrap()
            .unwrap()
            .commit,
        None
    );
}

#[test]
fn the_workspace_root_is_the_nearest_ancestor_with_a_jj_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(root.join(".jj")).unwrap();
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    assert_eq!(
        workspace_root(&root.join("src/deep")),
        Some(root.canonicalize().unwrap())
    );
    assert_eq!(workspace_root(dir.path()), None);
}

#[test]
fn the_wrapper_is_armed_by_the_payload_directory_or_the_project_directory() {
    let root = armed_root();
    let canonical = root.path().canonicalize().unwrap();
    assert_eq!(
        armed_in(&payload("turn.finish", Some(root.path())), None),
        Some(canonical.clone())
    );
    assert_eq!(
        armed_in(&payload("turn.finish", None), Some(root.path().as_os_str())),
        Some(canonical),
        "a payload without a directory falls back to CLAUDE_PROJECT_DIR"
    );
    assert_eq!(armed_in(&payload("turn.finish", None), None), None);
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        armed_in(
            &payload("turn.finish", Some(other.path())),
            Some(root.path().as_os_str())
        ),
        None,
        "the payload's directory wins over the project directory"
    );
}

#[test]
fn the_wrapper_does_nothing_outside_a_jj_workspace_or_with_the_switch_off() {
    let dir = tempfile::tempdir().unwrap();
    let payload = payload("turn.finish", Some(dir.path()));
    assert_eq!(
        adapter_in(&payload, None, "", None).unwrap(),
        HandlerResult::default()
    );
    std::fs::create_dir_all(dir.path().join(".jj")).unwrap();
    assert_eq!(
        adapter_in(&payload, None, "", None).unwrap(),
        HandlerResult::default()
    );
}

#[test]
fn the_legacy_script_is_named_only_where_it_exists() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(legacy_script(home.path()), None);
    let script = home.path().join(".config/just/claude/turn/wrap");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::write(&script, "#!/bin/sh\n").unwrap();
    assert_eq!(legacy_script(home.path()), Some(script));
}
