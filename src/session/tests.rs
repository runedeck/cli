use std::path::{Path, PathBuf};
use std::time::Duration;

use super::copy::{Action, Relation, complete, copy_file, relation};
use super::debt::{Closed, Record, Store};
use super::source::{Harness, Kind, Stores, kind_of};
use super::{Config, Exclude, Outcome, Request, Trigger, capture, exclude, grok, sweep};

struct Fixture {
    _root: tempfile::TempDir,
    home: PathBuf,
    config: Config,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("root");
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).expect("home");
    let stores = Stores::resolve(&home, &|_| None);
    let config = Config {
        data_dir: root.path().join("Data"),
        state_dir: root.path().join("state"),
        stores,
        exclude: Exclude::default(),
        lock_wait: Duration::from_millis(50),
        specstory: None,
        sqlite3: PathBuf::from("sqlite3"),
    };
    Fixture {
        _root: root,
        home,
        config,
    }
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(path, text).expect("write");
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn request(harness: Harness, session: Option<&str>) -> Request {
    Request {
        harness,
        session: session.map(str::to_string),
        cwd: None,
        status: None,
        trigger: Trigger::Hook,
        since: None,
    }
}

fn claude_line(cwd: &str, text: &str) -> String {
    format!("{{\"cwd\":\"{cwd}\",\"type\":\"user\",\"message\":\"{text}\"}}\n")
}

// --------------------------------------------------------------- copy ----

#[test]
fn relation_tells_equal_extended_and_diverged_apart() {
    let dir = tempfile::tempdir().expect("dir");
    let old = dir.path().join("old");
    let same = dir.path().join("same");
    let grown = dir.path().join("grown");
    let rewritten = dir.path().join("rewritten");
    let shorter = dir.path().join("shorter");
    write(&old, "one\ntwo\n");
    write(&same, "one\ntwo\n");
    write(&grown, "one\ntwo\nthree\n");
    write(&rewritten, "one\nTWO\n");
    write(&shorter, "one\n");
    assert_eq!(relation(&same, &old).unwrap(), Relation::Equal);
    assert_eq!(relation(&grown, &old).unwrap(), Relation::Extends);
    assert_eq!(relation(&rewritten, &old).unwrap(), Relation::Diverged);
    assert_eq!(relation(&shorter, &old).unwrap(), Relation::Diverged);
}

#[test]
fn a_grown_session_overwrites_its_copy_and_keeps_no_version() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("store/s.jsonl");
    let copy = dir.path().join("data/s.jsonl");
    let version = dir.path().join("versions/run/s.jsonl");
    write(&source, "a\n");
    assert_eq!(
        copy_file(&source, &copy, &version, Kind::Append).unwrap(),
        Action::Created
    );
    write(&source, "a\nb\n");
    assert_eq!(
        copy_file(&source, &copy, &version, Kind::Append).unwrap(),
        Action::Overwritten
    );
    assert_eq!(read(&copy), "a\nb\n");
    assert!(!version.exists());
    assert_eq!(
        copy_file(&source, &copy, &version, Kind::Append).unwrap(),
        Action::Unchanged
    );
}

#[test]
fn a_rewritten_session_moves_its_copy_into_the_versions() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("store/s.jsonl");
    let copy = dir.path().join("data/s.jsonl");
    let version = dir.path().join("versions/run/s.jsonl");
    write(&source, "a\nb\n");
    copy_file(&source, &copy, &version, Kind::Append).unwrap();
    write(&source, "a\n");
    assert_eq!(
        copy_file(&source, &copy, &version, Kind::Append).unwrap(),
        Action::Versioned
    );
    assert_eq!(read(&copy), "a\n");
    assert_eq!(read(&version), "a\nb\n");
}

#[test]
fn a_database_is_overwritten_without_a_version() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("store/c.db");
    let copy = dir.path().join("data/c.db");
    let version = dir.path().join("versions/run/c.db");
    write(&source, "page one");
    copy_file(&source, &copy, &version, kind_of(&source)).unwrap();
    write(&source, "page two");
    assert_eq!(
        copy_file(&source, &copy, &version, kind_of(&source)).unwrap(),
        Action::Overwritten
    );
    assert!(!version.exists());
    assert_eq!(kind_of(Path::new("x.jsonl.zst")), Kind::Compressed);
    assert_eq!(kind_of(Path::new("x.db-wal")), Kind::Database);
}

#[test]
fn a_copy_is_complete_when_it_holds_the_source() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("s.jsonl");
    let copy = dir.path().join("c.jsonl");
    write(&source, "a\nb\n");
    write(&copy, "a\nb\nc\n");
    assert!(complete(&source, &copy, Kind::Append));
    assert!(!complete(&source, &copy, Kind::Compressed));
    write(&copy, "a\n");
    assert!(!complete(&source, &copy, Kind::Append));
    assert!(!complete(&source, &dir.path().join("none"), Kind::Append));
}

// ------------------------------------------------------------ exclude ----

#[test]
fn exclude_matches_folders_by_glob_and_sessions_by_id() {
    let home = Path::new("/home/owner");
    let exclude = Exclude::resolve(
        &[
            "~/Developer/acme/**".to_string(),
            "/srv/client-*".to_string(),
        ],
        &["sess-1".to_string()],
        home,
    );
    assert!(exclude.folder(Path::new("/home/owner/Developer/acme")));
    assert!(exclude.folder(Path::new("/home/owner/Developer/acme/app/")));
    assert!(exclude.folder(Path::new("/srv/client-7")));
    assert!(!exclude.folder(Path::new("/srv/client-7/deep")));
    assert!(!exclude.folder(Path::new("/home/owner/Developer/other")));
    assert!(exclude.session("sess-1"));
    assert!(exclude.matches(Some("sess-1"), None));
    assert!(!exclude.matches(Some("sess-2"), Some(Path::new("/tmp/x"))));
}

#[test]
fn the_working_folder_is_read_from_the_first_cwd_member() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("s.jsonl");
    write(
        &file,
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"x\",\"cwd\":\"/home/owner/a b\\/c\"}}\n",
    );
    assert_eq!(
        exclude::cwd_in_file(&file),
        Some(PathBuf::from("/home/owner/a b/c"))
    );
    assert_eq!(exclude::cwd_in_text("{\"cwd\": 3}"), None);
    assert_eq!(
        exclude::percent_decode("%2FUsers%2Fowner%2Fwork"),
        Some("/Users/owner/work".to_string())
    );
    assert_eq!(exclude::percent_decode("%2"), None);
}

// --------------------------------------------------------------- grok ----

#[test]
fn grok_lines_render_as_sections() {
    let text = concat!(
        "{\"type\":\"user\",\"content\":\"hello\"}\n",
        "{\"content\":[{\"text\":\"a\"},{\"tool\":\"x\"}]}\n",
        "{\"type\":\"assistant\",\"content\":null}\n",
    );
    assert_eq!(
        grok::render(text),
        "## User\n\nhello\n\n## Message\n\na\n{\"tool\":\"x\"}\n\n## Assistant\n\n\n\n"
    );
}

// --------------------------------------------------------------- debt ----

#[test]
fn debt_reads_the_scripts_records_and_closes_into_the_closed_folder() {
    let state = tempfile::tempdir().expect("state");
    let store = Store::new(state.path());
    write(
        &state
            .path()
            .join("capture-debt/claude-20261001T000000Z-1.json"),
        "{\"provider\":\"claude\",\"session\":\"\",\"cwd\":\"/p\",\"source\":\"hook\",\"harness_status\":null,\"reason\":\"lock\",\"at\":\"2026-10-01T00:00:00Z\"}",
    );
    let open = store.open().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].record.harness, "claude");
    assert_eq!(open[0].record.session, None);
    let summary = super::debt::summary(state.path()).unwrap();
    assert_eq!(summary.open, 1);
    assert_eq!(summary.oldest.as_deref(), Some("2026-10-01"));

    store.close(&open[0], Closed::Lost).unwrap();
    assert_eq!(store.open().unwrap(), []);
    let closed = store.closed().unwrap();
    assert_eq!(closed[0].closed_reason.as_deref(), Some("lost"));
    assert!(closed[0].closed_at.is_some());
    assert_eq!(super::debt::summary(state.path()).unwrap().open, 0);
}

#[test]
fn two_records_in_one_run_get_different_names() {
    let state = tempfile::tempdir().expect("state");
    let store = Store::new(state.path());
    let record = Record {
        harness: "codex".to_string(),
        session: Some("s".to_string()),
        cwd: None,
        source: Some("hook".to_string()),
        harness_status: None,
        reason: "raw".to_string(),
        error: None,
        at: super::now(),
        closed_at: None,
        closed_reason: None,
    };
    let first = store.record(&record).unwrap();
    let second = store.record(&record).unwrap();
    assert_ne!(first, second);
    assert_eq!(store.open().unwrap().len(), 2);
}

// ------------------------------------------------------------ capture ----

#[test]
fn a_claude_session_end_copies_the_session_and_its_subagents_and_the_plans() {
    let f = fixture();
    let project = f.home.join(".claude/projects/-home-owner-repo");
    write(
        &project.join("abc.jsonl"),
        &claude_line("/home/owner/repo", "hi"),
    );
    write(&project.join("abc/subagents/agent-1.jsonl"), "{\"x\":1}\n");
    write(
        &project.join("other.jsonl"),
        &claude_line("/home/owner/repo", "no"),
    );
    write(&f.home.join(".claude/plans/plan.md"), "# plan\n");

    let outcome = capture(&f.config, &request(Harness::Claude, Some("abc"))).unwrap();
    assert!(
        matches!(
            outcome,
            Outcome::Copied {
                files: 2,
                versions: 0,
                ..
            }
        ),
        "{outcome:?}"
    );
    let sessions = f.config.data_dir.join("Sessions/claude/-home-owner-repo");
    assert!(sessions.join("abc.jsonl").is_file());
    assert!(sessions.join("abc/subagents/agent-1.jsonl").is_file());
    assert!(!sessions.join("other.jsonl").exists());
    assert_eq!(
        read(&f.config.data_dir.join("Plans/claude/plan.md")),
        "# plan\n"
    );
    assert_eq!(
        read(&project.join("abc.jsonl")),
        claude_line("/home/owner/repo", "hi"),
        "the harness's file is unchanged"
    );
    assert_eq!(Store::new(&f.config.state_dir).open().unwrap(), []);
}

#[test]
fn a_session_without_a_file_records_no_debt() {
    let f = fixture();
    write(&f.home.join(".claude/projects/p/x.jsonl"), "{}\n");
    let outcome = capture(&f.config, &request(Harness::Claude, Some("probe"))).unwrap();
    assert_eq!(outcome, Outcome::NoSessionFile);
    assert_eq!(Store::new(&f.config.state_dir).open().unwrap(), []);
    assert_eq!(
        capture(&f.config, &request(Harness::Codex, Some("x"))).unwrap(),
        Outcome::NoStore
    );
}

#[test]
fn a_bad_session_id_is_a_usage_error() {
    let f = fixture();
    assert!(capture(&f.config, &request(Harness::Claude, Some("../x"))).is_err());
}

#[test]
fn an_excluded_folder_or_session_is_skipped_without_debt() {
    let mut f = fixture();
    f.config.exclude = Exclude::resolve(
        &["/home/owner/work/**".to_string()],
        &["secret-1".to_string()],
        &f.home,
    );
    let projects = f.home.join(".claude/projects");
    write(
        &projects.join("-home-owner-work-x/w1.jsonl"),
        &claude_line("/home/owner/work/x", "work"),
    );
    write(
        &projects.join("-home-owner-own/secret-1.jsonl"),
        &claude_line("/home/owner/own", "listed"),
    );
    write(
        &projects.join("-home-owner-own/open-1.jsonl"),
        &claude_line("/home/owner/own", "private"),
    );

    let hook = Request {
        cwd: Some(PathBuf::from("/home/owner/work/x")),
        ..request(Harness::Claude, Some("w1"))
    };
    assert_eq!(capture(&f.config, &hook).unwrap(), Outcome::Skipped);
    assert_eq!(
        capture(&f.config, &request(Harness::Claude, Some("secret-1"))).unwrap(),
        Outcome::Skipped
    );

    let report = sweep(&f.config);
    let claude = f.config.data_dir.join("Sessions/claude");
    assert!(claude.join("-home-owner-own/open-1.jsonl").is_file());
    assert!(!claude.join("-home-owner-own/secret-1.jsonl").exists());
    assert!(!claude.join("-home-owner-work-x").exists());
    assert_eq!(report.debt_open, 0, "{report:?}");
    assert_eq!(Store::new(&f.config.state_dir).open().unwrap(), []);
}

#[test]
fn codex_copies_compressed_files_as_they_are() {
    let f = fixture();
    let day = f.home.join(".codex/sessions/2026/10/01");
    let compressed =
        day.join("rollout-2026-10-01T10-00-00-0f3c2a1e-1111-4222-8333-444444444444.jsonl.zst");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::write(&compressed, [0x28, 0xb5, 0x2f, 0xfd, b'c', b'z']).unwrap();
    write(
        &day.join("rollout-2026-10-01T11-00-00-0f3c2a1e-1111-4222-8333-555555555555.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/home/owner/repo\"}}\n",
    );
    let outcome = capture(
        &f.config,
        &request(Harness::Codex, Some("0f3c2a1e-1111-4222-8333-444444444444")),
    )
    .unwrap();
    assert!(
        matches!(outcome, Outcome::Copied { files: 1, .. }),
        "{outcome:?}"
    );
    let copy = f.config.data_dir.join(
        "Sessions/codex/2026/10/01/rollout-2026-10-01T10-00-00-0f3c2a1e-1111-4222-8333-444444444444.jsonl.zst",
    );
    assert_eq!(
        std::fs::read(&copy).unwrap(),
        std::fs::read(&compressed).unwrap()
    );
}

#[test]
fn grok_sessions_are_copied_and_rendered_and_excluded_by_their_folder() {
    let mut f = fixture();
    f.config.exclude = Exclude::resolve(&["/Users/owner/work".to_string()], &[], &f.home);
    let sessions = f.home.join(".grok/sessions");
    write(
        &sessions.join("%2FUsers%2Fowner%2Fown/g1/chat_history.jsonl"),
        "{\"type\":\"user\",\"content\":\"hello\"}\n",
    );
    write(
        &sessions.join("%2FUsers%2Fowner%2Fwork/g2/chat_history.jsonl"),
        "{\"type\":\"user\",\"content\":\"work\"}\n",
    );
    let outcome = capture(&f.config, &request(Harness::Grok, Some("g1"))).unwrap();
    assert!(
        matches!(
            outcome,
            Outcome::Copied {
                files: 1,
                readable_error: None,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        read(
            &f.config
                .data_dir
                .join("Transcripts/grok/%2FUsers%2Fowner%2Fown/g1.md")
        ),
        "## User\n\nhello\n\n"
    );
    let report = sweep(&f.config);
    assert!(
        !f.config
            .data_dir
            .join("Sessions/grok/%2FUsers%2Fowner%2Fwork")
            .exists()
    );
    assert_eq!(report.transcripts, 1);
}

#[test]
fn antigravity_databases_are_copied_without_versions_and_backed_up_once_a_day() {
    let f = fixture();
    let conversations = f.home.join(".gemini/antigravity-cli/conversations");
    write(&conversations.join("conv-1.db"), "v1");
    write(&conversations.join("conv-1.db-wal"), "");
    let first = sweep(&f.config);
    write(&conversations.join("conv-1.db"), "v2");
    let second = sweep(&f.config);
    assert_eq!(
        read(&f.config.data_dir.join("Sessions/antigravity/conv-1.db")),
        "v2"
    );
    let versions = f.config.data_dir.join("SessionVersions/antigravity");
    let run_folders = std::fs::read_dir(&versions).map_or(0, |entries| {
        entries
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().ends_with("-daily"))
            .count()
    });
    assert_eq!(run_folders, 0, "no version per sweep for a database");
    if super::render::find_in_path("sqlite3", std::env::var_os("PATH").as_deref()).is_some() {
        // The file is not a database: sqlite3 refuses it, which is a
        // failure of one backup, never a missing tool.
        assert!(first.backup_error.is_some() || second.backup_error.is_some());
    }
    assert!(
        Store::new(&f.config.state_dir)
            .open()
            .unwrap()
            .iter()
            .all(|entry| entry.record.reason != "backup-tool-missing")
    );
}

#[test]
fn a_real_database_gets_one_backup_per_day() {
    let Some(sqlite3) = super::render::find_in_path("sqlite3", std::env::var_os("PATH").as_deref())
    else {
        return;
    };
    let f = fixture();
    let conversations = f.home.join(".gemini/antigravity-cli/conversations");
    std::fs::create_dir_all(&conversations).unwrap();
    let database = conversations.join("conv-9.db");
    let status = std::process::Command::new(&sqlite3)
        .arg(&database)
        .arg("create table t(x); insert into t values (1);")
        .status()
        .expect("sqlite3");
    assert!(status.success());
    let versions = f.config.data_dir.join("SessionVersions/antigravity");
    let day = super::today();
    let first = super::backup::daily(&conversations, &versions, &day, &sqlite3);
    assert_eq!((first.backed_up, first.current), (1, 0), "{first:?}");
    let second = super::backup::daily(&conversations, &versions, &day, &sqlite3);
    assert_eq!((second.backed_up, second.current), (0, 1), "{second:?}");
    assert!(
        super::backup::day_dir(&versions, &day)
            .join("conv-9.db")
            .is_file()
    );
}

#[test]
fn a_failed_copy_records_debt_that_a_later_success_clears() {
    let f = fixture();
    let project = f.home.join(".claude/projects/p");
    write(&project.join("s1.jsonl"), &claude_line("/p", "x"));
    // The data folder is a file: every copy fails.
    write(&f.config.data_dir, "not a folder");
    let failed = capture(&f.config, &request(Harness::Claude, Some("s1"))).unwrap();
    assert!(
        matches!(failed, Outcome::Failed { debt: Some(_), .. }),
        "{failed:?}"
    );
    assert_eq!(failed.exit_code(), 1);
    let store = Store::new(&f.config.state_dir);
    let open = store.open().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].record.session.as_deref(), Some("s1"));
    assert_eq!(open[0].record.source.as_deref(), Some("hook"));

    std::fs::remove_file(&f.config.data_dir).unwrap();
    let copied = capture(&f.config, &request(Harness::Claude, Some("s1"))).unwrap();
    assert_eq!(copied.exit_code(), 0);
    assert_eq!(store.open().unwrap(), []);
    assert_eq!(
        store.closed().unwrap()[0].closed_reason.as_deref(),
        Some("copied")
    );
}

#[test]
fn the_sweep_reconciles_complete_and_lost_records() {
    let f = fixture();
    let project = f.home.join(".claude/projects/p");
    write(&project.join("done.jsonl"), &claude_line("/p", "x"));
    let store = Store::new(&f.config.state_dir);
    let record = |harness: &str, session: Option<&str>| Record {
        harness: harness.to_string(),
        session: session.map(str::to_string),
        cwd: None,
        source: Some("hook".to_string()),
        harness_status: None,
        reason: "raw".to_string(),
        error: None,
        at: "2026-10-01T00:00:00Z".to_string(),
        closed_at: None,
        closed_reason: None,
    };
    store.record(&record("claude", Some("done"))).unwrap();
    store.record(&record("claude", Some("gone"))).unwrap();
    store.record(&record("claude", None)).unwrap();
    store.record(&record("opencode", Some("o1"))).unwrap();
    store.record(&record("sweep", None)).unwrap();
    store.record(&record("codex", Some("c1"))).unwrap();

    let report = sweep(&f.config);
    assert_eq!((report.debt_closed, report.debt_open), (5, 1), "{report:?}");
    let closed = store.closed().unwrap();
    let reason = |session: Option<&str>, harness: &str| {
        closed
            .iter()
            .find(|r| r.harness == harness && r.session.as_deref() == session)
            .and_then(|r| r.closed_reason.clone())
    };
    assert_eq!(reason(Some("done"), "claude").as_deref(), Some("complete"));
    assert_eq!(reason(Some("gone"), "claude").as_deref(), Some("lost"));
    assert_eq!(reason(None, "claude").as_deref(), Some("complete"));
    assert_eq!(reason(Some("o1"), "opencode").as_deref(), Some("lost"));
    assert_eq!(reason(None, "sweep").as_deref(), Some("no-session-file"));
    let open = store.open().unwrap();
    assert_eq!(
        open[0].record.harness, "codex",
        "no codex store: the record waits"
    );
}

#[test]
fn a_harness_that_deletes_its_session_leaves_the_copy_in_place() {
    let f = fixture();
    let file = f.home.join(".codex/sessions/2026/10/01/rollout-a.jsonl");
    write(&file, "{\"x\":1}\n");
    sweep(&f.config);
    std::fs::remove_file(&file).unwrap();
    sweep(&f.config);
    assert!(
        f.config
            .data_dir
            .join("Sessions/codex/2026/10/01/rollout-a.jsonl")
            .is_file()
    );
}

#[test]
fn a_held_lock_defers_the_capture_without_debt() {
    let f = fixture();
    write(&f.home.join(".claude/projects/p/s.jsonl"), "{}\n");
    std::fs::create_dir_all(super::lock::dir(&f.config.state_dir, "claude")).unwrap();
    assert_eq!(
        capture(&f.config, &request(Harness::Claude, Some("s"))).unwrap(),
        Outcome::Deferred
    );
    assert_eq!(Store::new(&f.config.state_dir).open().unwrap(), []);
}

#[test]
fn the_runner_scope_is_the_files_changed_after_the_start() {
    let f = fixture();
    let project = f.home.join(".claude/projects/p");
    let old = project.join("old.jsonl");
    write(&old, "{}\n");
    let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(hour_ago)
        .unwrap();
    write(&project.join("new.jsonl"), "{}\n");
    let runner = Request {
        trigger: Trigger::Runner,
        since: Some(std::time::SystemTime::now() - Duration::from_secs(5)),
        ..request(Harness::Claude, None)
    };
    let outcome = capture(&f.config, &runner).unwrap();
    assert!(
        matches!(outcome, Outcome::Copied { files: 1, .. }),
        "{outcome:?}"
    );
    assert!(
        f.config
            .data_dir
            .join("Sessions/claude/p/new.jsonl")
            .is_file()
    );
    assert!(
        !f.config
            .data_dir
            .join("Sessions/claude/p/old.jsonl")
            .exists()
    );
}
