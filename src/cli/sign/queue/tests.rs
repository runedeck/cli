use super::ceremony::range_lines;
use super::repo::{
    Head, Repo, SignOutcome, Signing, classify_sign_output, commit_id_revset, export_refused,
    has_signature, parse_commit, shadowing_alias, shadowing_alias_in, signed_exactly, user_value,
};
use super::repo::{helper_applies, helper_settings};
use super::shim;
use super::store::{
    Identity, Kind, RangeRequest, Receipt, Request, Status, Store, claim_is_dead, split_holder,
};
use super::{
    Live, State, derive, exit_status, head_matches, identities_match, names_commit,
    notifier_commands, range_identities_match, same_commit, topological,
};
use std::process::Command;
use std::rc::Rc;
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
        range: None,
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
    assert_eq!(
        store.load_all().expect("all"),
        [] as [crate::cli::sign::queue::store::Request; 0]
    );
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

#[test]
fn url_scoped_credential_helpers_are_trusted_alongside_the_bare_one() {
    // `gh auth setup-git` writes the URL-scoped form; a bare one and an
    // empty reset line are the other shapes `--get-regexp` prints.
    let listing = "credential.helper osxkeychain\ncredential.https://github.com.helper\ncredential.https://github.com.helper !/opt/homebrew/bin/gh auth git-credential\n";
    assert_eq!(
        helper_settings(listing),
        vec![
            "credential.helper=osxkeychain".to_string(),
            "credential.https://github.com.helper=".to_string(),
            "credential.https://github.com.helper=!/opt/homebrew/bin/gh auth git-credential"
                .to_string(),
        ]
    );
}

#[test]
fn a_helper_applies_by_scope_prefix_and_never_when_reset() {
    let url = "https://github.com/runedeck/cli.git";
    assert!(helper_applies("credential.helper=osxkeychain", url));
    assert!(helper_applies(
        "credential.https://github.com.helper=!gh auth git-credential",
        url
    ));
    assert!(!helper_applies(
        "credential.https://gitlab.example.helper=!glab auth git-credential",
        url
    ));
    assert!(!helper_applies(
        "credential.https://github.com.helper=",
        url
    ));
    assert!(!helper_applies("credential.helper=", url));
    assert!(!helper_applies("core.askPass=", url));
}

#[test]
fn notifiers_carry_the_message_and_the_mark_and_never_osascript() {
    let icon = std::path::Path::new("/tmp/rune/icon.png");
    let commands = notifier_commands(
        "main needs your signature",
        Some(icon),
        Some("com.mitchellh.ghostty"),
    );
    let programs: Vec<String> = commands
        .iter()
        .map(|command| command.get_program().to_string_lossy().into_owned())
        .collect();
    if cfg!(target_os = "macos") {
        assert_eq!(programs, ["terminal-notifier", "/bin/sh"]);
    } else {
        assert_eq!(programs, ["notify-send"]);
    }
    for command in &commands {
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.iter()
                .any(|arg| arg.contains("main needs your signature")),
            "{args:?}"
        );
        assert!(args.iter().any(|arg| arg.contains("rune")), "{args:?}");
    }
    let with_icon: Vec<&Command> = commands
        .iter()
        .filter(|command| command.get_args().any(|arg| arg == icon.as_os_str()))
        .collect();
    assert!(
        !with_icon.is_empty(),
        "at least one notifier takes the icon path"
    );
    assert!(
        notifier_commands("quiet", None, None)
            .iter()
            .all(|command| command.get_args().all(|arg| arg != icon.as_os_str()))
    );
}

#[test]
fn a_click_activates_the_terminal_the_owner_typed_in() {
    let commands = notifier_commands("signed", None, Some("com.mitchellh.ghostty"));
    if cfg!(target_os = "macos") {
        let args: Vec<Vec<String>> = commands
            .iter()
            .map(|command| {
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect()
            })
            .collect();
        assert!(
            args[0]
                .windows(2)
                .any(|pair| pair == ["-activate", "com.mitchellh.ghostty"])
        );
        assert!(args[1].iter().any(|arg| arg == "alerter"), "{:?}", args[1]);
        assert!(
            !args[1].iter().any(|arg| arg == "--sender"),
            "no impersonation"
        );
        assert!(args[1][1].contains("@CONTENTCLICKED") && args[1][1].contains("open -b"));
        // One banner at a time: the previous group member is ended first
        // and a watchdog ends this one after its timeout.
        assert!(args[1][1].starts_with("pkill -f \"^alerter .*--group rune-sign\""));
        assert!(args[1][1].contains("sleep 620"));
        let env: Vec<(String, String)> = commands[1]
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|value| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect();
        assert!(
            env.contains(&(
                "RUNE_NOTIFY_OPEN".to_string(),
                "com.mitchellh.ghostty".to_string()
            )),
            "{env:?}"
        );
    }
}

#[test]
fn every_commit_is_named_through_commit_id_and_an_alias_of_it_is_found() {
    let commits = vec!["abc123".to_string(), "def456".to_string()];
    assert_eq!(
        commit_id_revset(&commits),
        "commit_id(\"abc123\") | commit_id(\"def456\")"
    );
    assert_eq!(commit_id_revset(&[]), "");

    let listing = "revset-aliases.\"closest_bookmark(to)\"\u{1f}'heads(::to & bookmarks())'\u{1f}false\u{1e}revset-aliases.\"commit_id( x )\"\u{1f}'all()'\u{1f}false\u{1e}";
    assert_eq!(
        shadowing_alias(listing),
        Some("revset-aliases.\"commit_id( x )\"".to_string())
    );
    let single = "revset-aliases.'commit_id(id)'\u{1f}'all()'\u{1f}false\u{1e}";
    assert_eq!(
        shadowing_alias(single),
        Some("revset-aliases.'commit_id(id)'".to_string())
    );
    assert_eq!(
        shadowing_alias("revset-aliases.commit_id\u{1f}'all()'\u{1f}false\u{1e}"),
        None,
        "a symbol alias does not shadow the function"
    );
    assert_eq!(
        shadowing_alias("revset-aliases.\"commit_ids(x)\"\u{1f}'all()'\u{1f}false\u{1e}"),
        None
    );
    assert_eq!(shadowing_alias(""), None);
}

#[test]
fn a_commit_id_alias_is_found_in_a_config_file_root_and_in_a_conditional_scope() {
    let root: toml::Table =
        toml::from_str("[revset-aliases]\n\"commit_id( x )\" = \"all()\"\n").expect("toml");
    assert_eq!(
        shadowing_alias_in(&root),
        Some("revset-aliases.\"commit_id( x )\"".to_string())
    );
    let scoped: toml::Table = toml::from_str(
        "[user]\nname = \"x\"\n[[--scope]]\n--when.commands = [\"sign\"]\n[--scope.revset-aliases]\n\"commit_id(x)\" = \"all() ~ root()\"\n",
    )
    .expect("toml");
    assert_eq!(
        shadowing_alias_in(&scoped),
        Some("--scope.revset-aliases.\"commit_id(x)\"".to_string())
    );
    let harmless: toml::Table = toml::from_str(
        "[revset-aliases]\ncommit_id = \"all()\"\n\"commit_ids(x)\" = \"none()\"\n[[--scope]]\n--when.commands = [\"log\"]\n[--scope.revset-aliases]\n\"mine(x)\" = \"all()\"\n",
    )
    .expect("toml");
    assert_eq!(shadowing_alias_in(&harmless), None);
}

#[test]
fn a_commit_id_alias_is_found_in_a_scope_nested_inside_a_scope() {
    let nested: toml::Table = toml::from_str(
        "[[--scope]]\n--when.commands = [\"sign\"]\n[[--scope.--scope]]\n--when.commands = [\"sign\"]\n[--scope.--scope.revset-aliases]\n\"commit_id(x)\" = \"x | all()\"\n",
    )
    .expect("toml");
    assert_eq!(
        shadowing_alias_in(&nested),
        Some("--scope.--scope.revset-aliases.\"commit_id(x)\"".to_string())
    );
}

#[test]
fn a_sign_run_is_exact_when_every_named_commit_has_one_signed_rewrite_and_no_other() {
    let base = identity("cbase", "kbase", &[]);
    let one = identity("c1", "k1", &["kbase"]);
    let two = identity("c2", "k2", &["k1"]);
    let one_signed = identity("c1", "s1", &["kbase"]);
    let two_signed = identity("c2", "s2", &["s1"]);
    let named = vec![one.clone(), two.clone()];
    let written = vec![(one_signed.clone(), true), (two_signed.clone(), true)];
    assert_eq!(signed_exactly(&named, &written), Ok(()));
    let top_rewritten_unsigned = identity("c3", "r3", &["s2"]);
    let with_descendant = vec![
        (one_signed.clone(), true),
        (two_signed.clone(), true),
        (top_rewritten_unsigned, false),
    ];
    assert_eq!(
        signed_exactly(&named, &with_descendant),
        Ok(()),
        "a descendant rewritten without a signature is what signing.behavior=drop leaves"
    );
    let base_signed = identity("cbase", "sbase", &[]);
    let overreach = vec![
        (base_signed, true),
        (one_signed.clone(), true),
        (two_signed, true),
    ];
    let error = signed_exactly(&named, &overreach).expect_err("extra commit");
    assert_eq!(error, "jj sign also signed sbase (change cbase)");
    assert!(
        !error.contains(&base.description),
        "no repository text in the reason"
    );
    let partial = vec![(one_signed, true)];
    let error = signed_exactly(&named, &partial).expect_err("missing rewrite");
    assert_eq!(error, "no signed rewrite of k2 (change c2)");
}

#[test]
fn the_shim_matches_each_named_commit_once_with_parents_mapped_through_its_signed_ids() {
    let one = identity("c1", "k1", &["kbase"]);
    let two = identity("c2", "k2", &["k1"]);
    let mut state = shim::State {
        program: "gpg".to_string(),
        named: vec![one.clone(), two.clone()],
        signed: std::collections::BTreeMap::new(),
        refused: None,
    };
    // jj hands the shim a rewrite of `one` under a new id, then a rewrite
    // of `two` whose parent is the signed `one`, which only the shim knows.
    let one_payload = identity("c1", "p1", &["kbase"]);
    let two_payload = identity("c2", "p2", &["s1"]);
    assert_eq!(shim::matched(&state, &one_payload), Some(&one));
    assert_eq!(
        shim::matched(&state, &two_payload),
        None,
        "the parent is not yet known as the signed rewrite of one"
    );
    state.signed.insert("k1".to_string(), "s1".to_string());
    assert_eq!(shim::matched(&state, &two_payload), Some(&two));
    assert_eq!(
        shim::matched(&state, &one_payload),
        None,
        "a named commit is signed once"
    );
    let side = identity("cside", "pside", &["kbase"]);
    assert_eq!(shim::matched(&state, &side), None);
    let mut moved = two_payload.clone();
    moved.tree_id = "tree-other".to_string();
    assert_eq!(shim::matched(&state, &moved), None);
    let mut reparented = two_payload;
    reparented.parents = vec!["kbase".to_string()];
    assert_eq!(shim::matched(&state, &reparented), None);
}

#[test]
fn the_shim_rebuilds_the_signed_object_the_way_jj_writes_it() {
    let payload = COMMIT_OBJECT.replace(
        "gpgsig -----BEGIN PGP SIGNATURE-----\n \n fake\n -----END PGP SIGNATURE-----\n",
        "",
    );
    let signature = "-----BEGIN PGP SIGNATURE-----\n\nfake\n-----END PGP SIGNATURE-----\n";
    let object = shim::signed_object(payload.as_bytes(), signature.as_bytes());
    assert_eq!(String::from_utf8_lossy(&object), COMMIT_OBJECT);
    let crlf = "-----BEGIN PGP SIGNATURE-----\r\n\r\nfake\r\n-----END PGP SIGNATURE-----\r\n";
    let object = shim::signed_object(payload.as_bytes(), crlf.as_bytes());
    assert_eq!(String::from_utf8_lossy(&object), COMMIT_OBJECT);
    assert!(shim::is_sign_request(&[
        "--status-fd".into(),
        "2".into(),
        "-abu".into(),
        "k".into()
    ]));
    assert!(shim::is_sign_request(&[
        "--status-fd".into(),
        "2".into(),
        "-ab".into()
    ]));
    assert!(!shim::is_sign_request(&[
        "--verify".into(),
        "sig".into(),
        "-".into()
    ]));
}

#[test]
fn the_shim_hashes_a_payload_as_git_hashes_a_commit_object() {
    let Ok(git) = Command::new("git").arg("--version").output() else {
        eprintln!("skipped: git is not installed");
        return;
    };
    if !git.status.success() {
        return;
    }
    let mut child = Command::new("git")
        .args(["hash-object", "-t", "commit", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("git");
    std::io::Write::write_all(
        &mut child.stdin.take().expect("stdin"),
        COMMIT_OBJECT.as_bytes(),
    )
    .expect("payload");
    let output = child.wait_with_output().expect("git");
    assert_eq!(
        shim::object_id(COMMIT_OBJECT.as_bytes()),
        String::from_utf8_lossy(&output.stdout).trim()
    );
}

#[test]
fn the_range_view_shows_control_and_format_characters_as_escapes() {
    let range = RangeRequest {
        protected: "origin/main\u{1b}[2K".to_string(),
        base: "kbase".to_string(),
        commits: Vec::new(),
    };
    let mut commit = identity("c1", "k1", &["kbase"]);
    commit.description = "feat: ok\u{1b}[2K\u{1b}[1Agone\nbody".to_string();
    commit.author = "Model\u{202e} <model@example.com>".to_string();
    let text = range_lines(&range, &[commit]).join("\n");
    assert!(
        text.contains("range        origin/main\\x1b[2K at"),
        "{text}"
    );
    assert!(text.contains("feat: ok\\x1b[2K\\x1b[1Agone"), "{text}");
    assert!(
        text.contains("author       Model\\u{202e} <model@example.com> 1"),
        "{text}"
    );
    assert!(
        !text.contains("body"),
        "only the first line of a message is shown"
    );
    assert!(
        text.chars()
            .all(|c| c == '\n' || (!c.is_control() && c != '\u{202e}')),
        "{text:?}"
    );
}

#[test]
fn a_seal_rewritten_under_a_new_id_keeps_its_identity_and_a_moved_one_does_not() {
    let sealed = identity("cseal", "kseal", &["c1", "cmain"]);
    let mut rewritten = sealed.clone();
    rewritten.commit_id = "kseal-rewritten".to_string();
    assert!(identities_match(&sealed, &rewritten));
    let mut moved = rewritten;
    moved.change_id = "cnext".to_string();
    assert!(!identities_match(&sealed, &moved));
}

#[test]
fn a_range_head_matches_without_a_recorded_change_id_and_a_head_request_does_not() {
    let mut request = request("1", "acme/widgets", "2026-09-15T10:00:00Z");
    request.head.change_id = String::new();
    let mut live = request.head.clone();
    live.change_id = "added-by-jj".to_string();
    let head = Head {
        identity: live,
        signed: true,
    };
    assert!(!head_matches(&request, &head));
    request.kind = Kind::Range;
    assert!(head_matches(&request, &head));
}

fn live(request: Request, head: Option<Head>) -> Live {
    Live {
        request,
        head,
        repo: Some(Rc::new(Repo {
            workspace: "/nowhere".into(),
            git_dir: "/nowhere/.git".into(),
            jj: true,
        })),
    }
}

#[test]
fn a_signed_request_rewritten_without_a_signature_is_current_again() {
    let mut request = request("1", "acme/widgets", "2026-09-15T10:00:00Z");
    request.status = Status::Signed;
    request.signed_commit = Some("k1".to_string());
    let at_signed = Head {
        identity: request.head.clone(),
        signed: true,
    };
    assert_eq!(
        derive(&live(request.clone(), Some(at_signed)), &[]).expect("derive"),
        State::Signed
    );
    let mut rewritten = request.head.clone();
    rewritten.commit_id = "k1-rewritten".to_string();
    let unsigned = Head {
        identity: rewritten.clone(),
        signed: false,
    };
    assert_eq!(
        derive(&live(request.clone(), Some(unsigned)), &[]).expect("derive"),
        State::Current,
        "the recorded identity under a new id, unsigned, needs the key again"
    );
    let signed = Head {
        identity: rewritten.clone(),
        signed: true,
    };
    assert_eq!(
        derive(&live(request.clone(), Some(signed)), &[]).expect("derive"),
        State::Unverified
    );
    let mut moved = rewritten;
    moved.description = "feat: more".to_string();
    let elsewhere = Head {
        identity: moved,
        signed: false,
    };
    assert_eq!(
        derive(&live(request.clone(), Some(elsewhere)), &[]).expect("derive"),
        State::Moved
    );
    assert_eq!(
        derive(&live(request, None), &[]).expect("derive"),
        State::Moved
    );
}

fn identity(change: &str, commit: &str, parents: &[&str]) -> Identity {
    Identity {
        change_id: change.to_string(),
        commit_id: commit.to_string(),
        tree_id: format!("tree-{change}"),
        author: "Model <model@example.com>".to_string(),
        description: format!("feat: {change}\n"),
        parents: parents.iter().map(|parent| (*parent).to_string()).collect(),
    }
}

#[test]
fn a_range_matches_by_identity_in_order_and_ignores_commit_ids() {
    let recorded = vec![
        identity("c1", "k1", &["cbase"]),
        identity("c2", "k2", &["c1"]),
    ];
    let signed = vec![
        identity("c1", "s1", &["cbase"]),
        identity("c2", "s2", &["c1"]),
    ];
    assert!(range_identities_match(&recorded, &signed));
    let reordered = vec![signed[1].clone(), signed[0].clone()];
    assert!(!range_identities_match(&recorded, &reordered));
    assert!(!range_identities_match(&recorded, &signed[..1]));
    let mut amended = signed.clone();
    amended[1].description = "feat: c2 amended\n".to_string();
    assert!(!range_identities_match(&recorded, &amended));

    // A commit object without a change-id header reads with an empty change
    // id and empty parent change ids, and matches on everything else.
    let mut from_git = identity("", "k1", &[""]);
    from_git.tree_id = "tree-c1".to_string();
    from_git.description = "feat: c1\n".to_string();
    assert!(same_commit(&from_git, &identity("c1", "s1", &["cbase"])));
    let mut retree = identity("c1", "s1", &["cbase"]);
    retree.tree_id = "tree-other".to_string();
    assert!(!same_commit(&from_git, &retree));
}

/// A scratch git repository of four commits on the empty tree: `a`, its
/// children `b` and `c`, and `m`, the merge of both. Every git reads the
/// scratch home alone. `None` when git is not installed.
fn merge_repository() -> Option<(TempDir, Repo, [String; 4])> {
    if Command::new("git").arg("--version").output().is_err() {
        return None;
    }
    let dir = TempDir::new().expect("tempdir");
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).expect("home");
    let workspace = dir.path().join("repo");
    let git_dir = workspace.join(".git");
    let git = |args: &[&str]| -> String {
        let output = Command::new("git")
            .args(args)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("GIT_CONFIG_GLOBAL", home.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Model")
            .env("GIT_AUTHOR_EMAIL", "model@example.com")
            .env("GIT_COMMITTER_NAME", "Model")
            .env("GIT_COMMITTER_EMAIL", "model@example.com")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    let repo = workspace.to_str().expect("utf-8 path").to_string();
    git(&["init", "--quiet", "--template=", &repo]);
    let store = git_dir.to_str().expect("utf-8 path").to_string();
    let tree = git(&["--git-dir", &store, "mktree"]);
    let a = git(&["--git-dir", &store, "commit-tree", &tree, "-m", "a"]);
    let b = git(&[
        "--git-dir",
        &store,
        "commit-tree",
        &tree,
        "-p",
        &a,
        "-m",
        "b",
    ]);
    let c = git(&[
        "--git-dir",
        &store,
        "commit-tree",
        &tree,
        "-p",
        &a,
        "-m",
        "c",
    ]);
    let m = git(&[
        "--git-dir",
        &store,
        "commit-tree",
        &tree,
        "-p",
        &b,
        "-p",
        &c,
        "-m",
        "m",
    ]);
    let repo = Repo {
        workspace,
        git_dir,
        jj: false,
    };
    Some((dir, repo, [a, b, c, m]))
}

#[test]
fn a_git_graft_never_hides_a_parent_from_the_range() {
    let Some((_dir, repo, [a, b, c, m])) = merge_repository() else {
        eprintln!("skipped: git is not installed");
        return;
    };
    let mut expected = vec![b.clone(), c.clone(), m.clone()];
    expected.sort();
    let mut listed = repo.range(&a, &m).expect("range");
    listed.sort();
    assert_eq!(listed, expected);
    // A graft gives `m` the parent `b` alone for every git read that honors
    // `info/grafts`, and `c` leaves the range that is shown and signed.
    std::fs::create_dir_all(repo.git_dir.join("info")).expect("info dir");
    std::fs::write(
        repo.git_dir.join("info").join("grafts"),
        format!("{m} {b}\n"),
    )
    .expect("grafts");
    let mut grafted = repo.range(&a, &m).expect("range");
    grafted.sort();
    assert_eq!(grafted, expected, "the graft hid {c} from the range");
}
