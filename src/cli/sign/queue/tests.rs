use super::repo::{
    SignOutcome, Signing, classify_sign_output, export_refused, has_signature, parse_commit,
    user_value,
};
use super::store::{Identity, Kind, Receipt, Request, Status, Store, claim_is_dead, split_holder};
use super::{exit_status, names_commit, topological};
use tempfile::TempDir;

fn request(id: &str, repository: &str, at: &str) -> Request {
    Request {
        id: id.to_string(),
        repository: repository.to_string(),
        workspace: "/nowhere".to_string(),
        bookmark: format!("change/{id}"),
        head: Identity {
            change_id: format!("c{id}"),
            commit_id: format!("k{id}"),
            tree_id: format!("t{id}"),
            author: "Model <model@example.com>".to_string(),
            description: "feat: x".to_string(),
            parents: vec![],
        },
        receipt: Some(Receipt {
            path: "r.log".to_string(),
            digest: "d".to_string(),
            exit_line: "exit=0".to_string(),
        }),
        requested_at: at.to_string(),
        status: Status::Queued,
        kind: Kind::Head,
        open: None,
        coverage: None,
        signed_commit: None,
        claim: None,
        failure: None,
    }
}

#[test]
fn exit_line_accepts_bare_and_stage_prefixed_forms_only() {
    assert_eq!(exit_status("exit=0"), Some(0));
    assert_eq!(exit_status("dryrun-exit=0"), Some(0));
    assert_eq!(exit_status("push-exit=1"), Some(1));
    assert_eq!(exit_status("not-an-exit=0"), None);
    assert_eq!(exit_status("Exit=0"), None);
    assert_eq!(exit_status("exit=zero"), None);
    assert_eq!(exit_status("done"), None);
}

const COMMIT_OBJECT: &str = "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
parent 1111111111111111111111111111111111111111\n\
parent 2222222222222222222222222222222222222222\n\
author Ann Author <ann@example.com> 1789482678 +0200\n\
committer Model One <model-one@example.com> 1789482678 +0200\n\
change-id syllpkkotptpynksxkpkuknypqkyrpoz\n\
gpgsig -----BEGIN PGP SIGNATURE-----\n \n fake\n -----END PGP SIGNATURE-----\n\
\n\
feat: one\n\nbody\n";

#[test]
fn a_git_commit_object_yields_the_identity_fields() {
    let identity = parse_commit("abc", COMMIT_OBJECT).expect("object parses");
    assert_eq!(identity.commit_id, "abc");
    assert_eq!(identity.tree_id, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    assert_eq!(identity.author, "Ann Author <ann@example.com>");
    assert_eq!(identity.change_id, "syllpkkotptpynksxkpkuknypqkyrpoz");
    assert_eq!(identity.description, "feat: one\n\nbody\n");
    // Parent commit ids at this stage; the caller maps them to change ids.
    assert_eq!(identity.parents.len(), 2);
    assert!(has_signature(COMMIT_OBJECT));
    let unsigned = COMMIT_OBJECT.replace("gpgsig ", "gpgsig-none ");
    assert!(!has_signature(&unsigned));
    assert!(parse_commit("abc", "not a commit").is_err());
}

#[test]
fn sign_output_is_classified_for_the_retry_rule() {
    assert_eq!(classify_sign_output(true, "anything"), SignOutcome::Signed);
    assert!(matches!(
        classify_sign_output(false, "gpg: signing failed: Timeout"),
        SignOutcome::Timeout(_)
    ));
    assert!(matches!(
        classify_sign_output(false, "gpg: signing failed: Operation cancelled"),
        SignOutcome::Cancelled(_)
    ));
    assert!(matches!(
        classify_sign_output(
            false,
            "Error: Commit 50a17fae6e02 is immutable\nHint: Could not modify commit"
        ),
        SignOutcome::Immutable(_)
    ));
    assert!(matches!(
        classify_sign_output(false, "Error: No signing backend configured"),
        SignOutcome::Failed(_)
    ));
}

#[test]
fn a_conflicted_bookmark_is_read_from_the_export_refusal() {
    let stderr = "Failed to export some bookmarks:\n  change/fork: conflicted bookmark\n  other (conflicted bookmark)\nDone\n";
    assert!(export_refused(stderr, "change/fork"));
    assert!(export_refused(stderr, "other"));
    assert!(!export_refused(stderr, "change/for"));
    assert!(!export_refused(stderr, "Done"));
    assert!(!export_refused("Nothing changed.\n", "change/fork"));
}

#[test]
fn the_user_scope_value_wins_over_what_a_repository_shadows() {
    // A user entry a repository overrides is listed as overridden; it is
    // still the one to restore. Two user entries: the unshadowed one wins.
    let listing = "signing.backends.gpg.program\u{1f}\"/opt/gpg\"\u{1f}true\u{1e}\n";
    assert_eq!(
        user_value(listing, "signing.backends.gpg.program"),
        Some("/opt/gpg".to_string())
    );
    let two =
        "signing.key\u{1f}\"old\"\u{1f}true\u{1e}\nsigning.key\u{1f}\"new\"\u{1f}false\u{1e}\n";
    assert_eq!(user_value(two, "signing.key"), Some("new".to_string()));
    assert_eq!(user_value("", "signing.key"), None);
    assert_eq!(
        user_value("Warning: No matching config key\n", "signing.key"),
        None
    );
}

#[test]
fn the_pinned_overrides_cover_backend_program_behavior_and_key() {
    let signing = Signing {
        program: "/opt/gpg".to_string(),
        key: Some("0xABCD".to_string()),
    };
    let overrides = signing.overrides();
    assert!(overrides.contains(&"signing.backend=\"gpg\"".to_string()));
    assert!(overrides.contains(&"signing.backends.gpg.program=\"/opt/gpg\"".to_string()));
    assert!(overrides.contains(&"signing.behavior=\"drop\"".to_string()));
    assert!(overrides.contains(&"signing.key=\"0xABCD\"".to_string()));
    let keyless = Signing {
        program: "gpg".to_string(),
        key: None,
    };
    assert!(
        !keyless
            .overrides()
            .iter()
            .any(|entry| entry.starts_with("signing.key="))
    );
}

#[test]
fn topological_order_places_ancestors_first_and_keeps_time_order_otherwise() {
    let requests = vec![
        request("top", "repo", "2026-09-15T10:00:00Z"),
        request("base", "repo", "2026-09-15T10:01:00Z"),
        request("other", "other-repo", "2026-09-15T10:02:00Z"),
    ];
    // index 0 (top) descends from index 1 (base).
    let before = vec![vec![1], vec![], vec![]];
    let ordered: Vec<String> = topological(requests, &before)
        .into_iter()
        .map(|request| request.id)
        .collect();
    assert_eq!(ordered, vec!["base", "top", "other"]);
}

#[test]
fn store_round_trips_requests_and_removes_them() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("requests")).expect("requests dir");
    let store = Store::at(dir.path().to_path_buf());
    let request = request("one", "repo", "2026-09-15T10:00:00Z");
    store.save(&request).expect("save");
    assert_eq!(store.load("one").expect("load"), request);
    assert_eq!(store.load_all().expect("all").len(), 1);
    store.remove("one").expect("remove");
    assert!(store.load_all().expect("all").is_empty());
}

#[test]
fn repository_lock_refuses_a_live_holder_and_releases_on_drop() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("locks")).expect("locks dir");
    let store = Store::at(dir.path().to_path_buf());
    let held = store.lock_repository("repo").expect("first lock");
    let error = store
        .lock_repository("repo")
        .err()
        .expect("second lock refused");
    assert!(error.to_string().contains("another signer holds repo"));
    assert!(
        error
            .to_string()
            .contains(&format!("process {}", std::process::id()))
    );
    drop(held);
    // A child another test thread is spawning inherits the descriptor
    // until it execs, and holds the lock that long; the release lands
    // within a moment, never later.
    let released = (0..50).any(|_| {
        store.lock_repository("repo").is_ok() || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        }
    });
    assert!(released, "the lock is released when its holder is dropped");
}

#[test]
fn dead_and_expired_claims_are_reclaimable() {
    assert!(claim_is_dead(0, "2026-09-15T10:00:00Z"));
    assert!(claim_is_dead(std::process::id(), "2000-01-01T00:00:00Z"));
    assert!(claim_is_dead(std::process::id(), "not a time"));
    assert!(!claim_is_dead(
        std::process::id(),
        &chrono::Utc::now().to_rfc3339()
    ));
    assert_eq!(
        split_holder("42 2026-09-15T10:00:00Z"),
        (42, "2026-09-15T10:00:00Z")
    );
    assert_eq!(split_holder("garbage"), (0, ""));
}

#[test]
fn a_receipt_names_a_commit_by_a_whole_hexadecimal_word() {
    let commit = "25d45bbb35a811f68ebce7a00910790aeef3e77c";
    assert!(names_commit(
        "Validate change/x at 25d45bbb35a8 in a checkout\nexit=0\n",
        commit
    ));
    assert!(names_commit(
        &format!("bookmark: change/x [to {commit}]\n"),
        commit
    ));
    assert!(!names_commit(
        "digest 25d45bbb35a8ffff of another file\n",
        commit
    ));
    assert!(!names_commit("nothing here\n", commit));
}

#[test]
fn a_record_written_before_the_ceremony_reads_as_a_head_request() {
    let record = r#"{
        "id": "one", "repository": "repo", "workspace": "/nowhere", "bookmark": "change/one",
        "head": {"change_id": "c", "commit_id": "k", "tree_id": "t", "author": "M <m@example.com>",
                 "description": "feat: x", "parents": []},
        "receipt": {"path": "r.log", "digest": "d", "exit_line": "exit=0"},
        "requested_at": "2026-09-15T10:00:00Z", "status": "queued"
    }"#;
    let request: Request = serde_json::from_str(record).expect("old record parses");
    assert_eq!(request.kind, Kind::Head);
    assert!(request.open.is_none() && request.coverage.is_none());
    assert_eq!(
        request.receipt.as_ref().map(|r| r.path.as_str()),
        Some("r.log")
    );
}

#[test]
fn the_ledger_is_the_controller_apps_newest_ledger_check_run_and_no_other_app_counts() {
    use super::gh::{App, CheckRun, Output, newest_ledger_line};
    let sha = "25d45bbb35a811f68ebce7a00910790aeef3e77c";
    let line = |artifact_id: u64| {
        Some(format!(
            "ledger: {{\"artifact_id\":{artifact_id},\"digest\":\"{}\",\"generation\":1,\"pull_request\":7,\"reviewed_sha\":\"{sha}\"}}\nmore text",
            "ab".repeat(32)
        ))
    };
    let run = |id: u64, name: &str, slug: Option<&str>, text: Option<String>| CheckRun {
        id,
        name: name.to_string(),
        app: slug.map(|slug| App {
            slug: slug.to_string(),
        }),
        output: Output { text },
    };
    let older = run(5, "ledger", Some("runeseer"), line(1));
    let newer = run(8, "ledger", Some("runeseer"), line(2));
    let forged = run(9, "ledger", Some("someone-else"), line(3));
    let other_name = run(10, "quality", Some("runeseer"), line(4));
    let no_app = run(11, "ledger", None, line(5));
    let prose = run(
        12,
        "ledger",
        Some("runeseer"),
        Some("the ledger is elsewhere".to_string()),
    );
    let runs = vec![older.clone(), newer.clone(), forged, other_name, no_app];
    assert_eq!(
        newest_ledger_line(runs.into_iter()).map(|line| line.artifact_id),
        Some(2)
    );
    // A newer controller run whose text is not a line yields nothing: the
    // reader never falls back to an older run.
    assert_eq!(newest_ledger_line(vec![older, prose].into_iter()), None);
    assert_eq!(
        newest_ledger_line(vec![run(9, "ledger", Some("someone-else"), line(3))].into_iter()),
        None
    );
}
