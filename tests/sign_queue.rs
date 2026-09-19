//! The signing queue against a real jj repository with a throwaway gpg key.
//! Skips when jj or gpg is missing, the way the init tests skip without jj.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use tempfile::TempDir;

struct Fixture {
    _home: TempDir,
    state: PathBuf,
    workspace: PathBuf,
    gnupg: PathBuf,
    xdg_config: PathBuf,
    /// A file whose content makes the fake gpg fail once with that message.
    fail_once: PathBuf,
}

/// The owner's fingerprint from the repository's own KEYS file, which the
/// fake gpg reports in its VALIDSIG line so the KEYS check passes.
const OWNER_FINGERPRINT: &str = "29DD2145CE7A818929459B2649F08103D3DA399E";

/// jj signs with `gpg -abu <key>` over stdin; git verifies with
/// `--status-fd=1 --verify <sig> -` and reads the GOODSIG and VALIDSIG
/// status lines. This program honors both contracts without a gpg-agent,
/// which the test sandbox cannot start. It fails once on request to
/// exercise the retry rule. With `FAKE_GPG_WAIT` set to a directory holding
/// an `armed` file, one signing call blocks until `go` appears there and
/// then times out, so a test can act while the owner's card is "busy"; the
/// retry that follows signs.
const FAKE_GPG: &str = r#"#!/bin/sh
case " $* " in
  *" -abu "*)
    if [ -n "$FAKE_GPG_WAIT" ] && [ -e "$FAKE_GPG_WAIT/armed" ]; then
      rm -f "$FAKE_GPG_WAIT/armed"
      : > "$FAKE_GPG_WAIT/in-sign"
      tries=0
      while [ ! -e "$FAKE_GPG_WAIT/go" ] && [ "$tries" -lt 300 ]; do sleep 0.1; tries=$((tries + 1)); done
      rm -f "$FAKE_GPG_WAIT/go"
      echo "gpg: signing failed: Timeout" >&2; exit 2
    fi
    if [ -s "$FAKE_GPG_FAIL_ONCE" ]; then
      message=$(cat "$FAKE_GPG_FAIL_ONCE"); : > "$FAKE_GPG_FAIL_ONCE"
      echo "gpg: signing failed: $message" >&2; exit 2
    fi
    cat >/dev/null
    printf -- '-----BEGIN PGP SIGNATURE-----\nfake %s\n-----END PGP SIGNATURE-----\n' "${FAKE_GPG_KEY:-29DD2145CE7A818929459B2649F08103D3DA399E}"
    ;;
  *" --verify "*)
    cat >/dev/null
    key="${FAKE_GPG_KEY:-29DD2145CE7A818929459B2649F08103D3DA399E}"
    printf '[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG %s Owner Test <owner@example.com>\n[GNUPG:] VALIDSIG %s 2026-09-15 1789482678 0 4 0 1 10 00 %s\n' "$key" "$key" "$key"
    ;;
  *) exit 3 ;;
esac
"#;

fn tool_present(name: &str) -> bool {
    Process::new(name)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

fn run(command: &mut Process) -> String {
    let output = command.output().expect("command runs");
    assert!(
        output.status.success(),
        "{:?} failed:\n{}",
        command,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn jj(fixture: &Fixture) -> Process {
    let mut command = Process::new("jj");
    command
        .arg("-R")
        .arg(&fixture.workspace)
        .env("JJ_USER", "Model One")
        .env("JJ_EMAIL", "model-one@example.com")
        .env("JJ_CONFIG", fixture.workspace.join(".jjconfig.toml"))
        .env("XDG_CONFIG_HOME", &fixture.xdg_config)
        .env("FAKE_GPG_FAIL_ONCE", &fixture.fail_once);
    command
}

/// A colocated jj repository signed through the fake gpg, with the
/// repository's real KEYS file so the owner-key check runs against gpg.
fn fixture() -> Option<Fixture> {
    if !tool_present("jj") || !tool_present("gpg") {
        return None;
    }
    let home = TempDir::new().expect("tempdir");
    let fake_gpg = home.path().join("fake-gpg");
    fs::write(&fake_gpg, FAKE_GPG).expect("fake gpg");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_gpg, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let workspace = home.path().join("repo");
    run(Process::new("jj")
        .args(["git", "init", "--colocate"])
        .arg(&workspace));
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("KEYS"),
        workspace.join("KEYS"),
    )
    .expect("KEYS");
    fs::write(
        workspace.join(".jjconfig.toml"),
        format!(
            "[signing]\nbackend = \"gpg\"\nkey = \"{OWNER_FINGERPRINT}\"\n[signing.backends.gpg]\nprogram = \"{}\"\n",
            fake_gpg.display()
        ),
    )
    .expect("jj config");
    // The real gpg reads KEYS from an empty home, so the user's keyring
    // and its locks stay untouched.
    let gnupg = home.path().join("gnupg");
    fs::create_dir_all(&gnupg).expect("gnupg home");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&gnupg, fs::Permissions::from_mode(0o700)).expect("chmod");
    }
    let xdg_config = home.path().join("xdg-config");
    fs::create_dir_all(&xdg_config).expect("xdg config");
    let fixture = Fixture {
        state: home.path().join("state"),
        fail_once: home.path().join("fail-once"),
        gnupg,
        xdg_config,
        _home: home,
        workspace,
    };
    fs::write(&fixture.fail_once, "").expect("flag file");
    run(jj(&fixture).args(["describe", "-m", "feat: base"]));
    run(jj(&fixture).args(["bookmark", "set", "change/base", "-r", "@"]));
    run(jj(&fixture).args(["new", "-m", "feat: top"]));
    fs::write(fixture.workspace.join("top.txt"), "top\n").expect("file");
    run(jj(&fixture).args(["bookmark", "set", "change/top", "-r", "@"]));
    run(jj(&fixture).args(["new"]));
    Some(fixture)
}

fn head_commit(fixture: &Fixture, bookmark: &str) -> String {
    run(jj(fixture).args([
        "log",
        "--no-graph",
        "-r",
        &format!("bookmarks(exact:\"{bookmark}\")"),
        "-T",
        "commit_id",
    ]))
}

fn head_is_signed(fixture: &Fixture, bookmark: &str) -> bool {
    run(jj(fixture).args([
        "log",
        "--no-graph",
        "-r",
        &format!("bookmarks(exact:\"{bookmark}\")"),
        "-T",
        "if(signature, \"signed\", \"unsigned\")",
    ])) == "signed"
}

fn receipt(fixture: &Fixture, name: &str, commit: &str, exit: &str) -> PathBuf {
    let path = fixture.workspace.join(name);
    fs::write(
        &path,
        format!("bookmark: x [move sideways from 0000 to {commit}]\n{exit}\n"),
    )
    .expect("receipt");
    path
}

fn rune(fixture: &Fixture) -> Command {
    Command::from_std(rune_process(fixture))
}

/// The binary as a plain process, for a signer that must run in the
/// background while the test acts.
fn rune_process(fixture: &Fixture) -> Process {
    let mut command = Process::new(env!("CARGO_BIN_EXE_rune"));
    configure(&mut command, fixture);
    command
}

fn configure(command: &mut Process, fixture: &Fixture) {
    command
        .env("RUNE_STATE_DIR", &fixture.state)
        .env("RUNE_NO_NOTIFY", "1")
        .env("GNUPGHOME", &fixture.gnupg)
        .env("JJ_CONFIG", fixture.workspace.join(".jjconfig.toml"))
        .env("XDG_CONFIG_HOME", &fixture.xdg_config)
        .env("FAKE_GPG_FAIL_ONCE", &fixture.fail_once)
        .current_dir(&fixture.workspace);
}

#[test]
fn stacked_requests_sign_base_first_and_the_descendant_stays_current() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let top = head_commit(&fixture, "change/top");
    let base_receipt = receipt(&fixture, "base.log", &base, "dryrun-exit=0");
    let top_receipt = receipt(&fixture, "top.log", &top, "exit=0");

    // The descendant is queued first on purpose: order must come from ancestry.
    rune(&fixture)
        .args(["sign", "queue", "change/top", "--receipt"])
        .arg(&top_receipt)
        .assert()
        .success()
        .stdout(predicate::str::contains("rune sign next"));
    rune(&fixture)
        .args(["sign", "submit", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();

    let listing = rune(&fixture)
        .args(["sign", "queue"])
        .output()
        .expect("list")
        .stdout;
    let listing = String::from_utf8_lossy(&listing);
    let base_line = listing
        .lines()
        .position(|line| line.contains("change/base"))
        .expect("base is listed");
    let top_line = listing
        .lines()
        .position(|line| line.contains("change/top"))
        .expect("top is listed");
    assert!(base_line < top_line, "base must list first:\n{listing}");
    assert!(
        listing
            .lines()
            .any(|line| line.starts_with("blocked") && line.contains("change/top")),
        "top is blocked behind base:\n{listing}"
    );

    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stdout(predicate::str::contains("signed change/base"));
    assert!(head_is_signed(&fixture, "change/base"));
    assert!(!head_is_signed(&fixture, "change/top"));

    // Signing the base rewrote the top; the request must still be current.
    let listing = rune(&fixture)
        .args(["sign", "queue"])
        .output()
        .expect("list")
        .stdout;
    let listing = String::from_utf8_lossy(&listing);
    assert!(
        listing
            .lines()
            .any(|line| line.starts_with("current") && line.contains("change/top")),
        "top stays current after its base is signed:\n{listing}"
    );

    rune(&fixture)
        .args(["sign", "all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("signed change/top"));
    assert!(head_is_signed(&fixture, "change/top"));

    rune(&fixture)
        .args(["sign", "queue", "--prune"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pruned").count(2));
}

#[test]
fn a_request_refuses_a_failing_receipt_a_foreign_receipt_and_a_signed_head() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let failing = receipt(&fixture, "failing.log", &base, "dryrun-exit=1");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&failing)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not clean"));
    let foreign = receipt(&fixture, "foreign.log", "ffffffffffff", "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&foreign)
        .assert()
        .failure()
        .stderr(predicate::str::contains("checked a different head"));
    let no_exit = fixture.workspace.join("noexit.log");
    fs::write(&no_exit, format!("{base}\ndone\n")).expect("receipt");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&no_exit)
        .assert()
        .failure()
        .stderr(predicate::str::contains("exit line"));

    run(jj(&fixture).args(["sign", "-r", "change/base"]));
    let signed = head_commit(&fixture, "change/base");
    let good = receipt(&fixture, "good.log", &signed, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&good)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already signed"));
}

#[test]
fn a_moved_head_is_stale_and_is_never_signed() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let base_receipt = receipt(&fixture, "base.log", &base, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();
    run(jj(&fixture).args(["describe", "-r", "change/base", "-m", "feat: base, amended"]));
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stale"));
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("skipped: stale"));
    assert!(!head_is_signed(&fixture, "change/base"));
    rune(&fixture)
        .args(["sign", "drop", "change/base"])
        .assert()
        .success();
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .failure();
}

#[test]
fn a_timed_out_pinentry_is_retried_and_a_cancelled_one_is_not() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let base_receipt = receipt(&fixture, "base.log", &base, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();

    fs::write(&fixture.fail_once, "Operation cancelled").expect("flag");
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("cancelled"));
    assert!(!head_is_signed(&fixture, "change/base"));
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .success()
        .stdout(predicate::str::contains("current"));

    fs::write(&fixture.fail_once, "Timeout").expect("flag");
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stderr(predicate::str::contains("timed out (attempt 1 of 3)"))
        .stdout(predicate::str::contains("signed change/base"));
    assert!(head_is_signed(&fixture, "change/base"));
}

#[test]
fn a_signature_from_a_key_outside_keys_is_a_failure() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let base_receipt = receipt(&fixture, "base.log", &base, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();
    rune(&fixture)
        .args(["sign", "next"])
        .env("FAKE_GPG_KEY", "0000000000000000")
        .assert()
        .failure()
        .stdout(predicate::str::contains("not in KEYS"));
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .success()
        .stdout(predicate::str::contains("failed"));
}

#[test]
fn a_repository_level_signing_program_is_never_run() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    // A session that controls the repository plants a hostile signing
    // program in the repository-scope config, which jj keeps under the
    // config directory keyed by the repository's config id. The owner's
    // user-scope program must win.
    let hostile = fixture.workspace.join("hostile-gpg");
    fs::write(
        &hostile,
        "#!/bin/sh\necho pwned > \"$(dirname \"$0\")/pwned\"\nexit 1\n",
    )
    .expect("hostile");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hostile, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    // jj creates the repository's config id on first use of its config path.
    run(jj(&fixture).args(["config", "path", "--repo"]));
    let config_id = fs::read_to_string(fixture.workspace.join(".jj/repo/config-id"))
        .expect("config id")
        .trim()
        .to_string();
    let repo_config = fixture.xdg_config.join("jj").join("repos").join(&config_id);
    fs::create_dir_all(&repo_config).expect("repo config dir");
    fs::write(
        repo_config.join("config.toml"),
        format!(
            "[signing.backends.gpg]\nprogram = \"{}\"\n",
            hostile.display()
        ),
    )
    .expect("repo config");
    let listed = run(jj(&fixture).args(["config", "list", "--repo", "signing"]));
    assert!(
        listed.contains("hostile-gpg"),
        "repo config is read:\n{listed}"
    );
    let base = head_commit(&fixture, "change/base");
    let base_receipt = receipt(&fixture, "base.log", &base, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stdout(predicate::str::contains("signed change/base"));
    assert!(
        !fixture.workspace.join("pwned").exists(),
        "the repository-level program ran"
    );
}

#[test]
fn queueing_with_json_emits_a_json_object() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let base_receipt = receipt(&fixture, "base.log", &base, "exit=0");
    let output = rune(&fixture)
        .args(["sign", "queue", "change/base", "--json", "--receipt"])
        .arg(&base_receipt)
        .output()
        .expect("runs");
    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON object");
    assert_eq!(value["queued"], true);
    assert_eq!(value["bookmark"], "change/base");
}

#[test]
fn the_ceremony_forms_and_the_queue_do_not_mix() {
    Command::cargo_bin("rune")
        .expect("binary")
        .args(["sign", "next", "--tag", "v1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tag"));
}

#[test]
fn a_dead_claim_on_a_head_the_owner_already_signed_is_recorded_not_resigned() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    let base_receipt = receipt(&fixture, "base.log", &base, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();
    // The signer died between `jj sign` and the store write.
    run(jj(&fixture).args(["sign", "-r", "change/base"]));
    let requests = fixture.state.join("sign-queue").join("requests");
    for entry in fs::read_dir(&requests).expect("requests") {
        let path = entry.expect("entry").path();
        let mut record: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("record")).expect("json");
        record["claim"] = serde_json::json!({"pid": 999_999, "started_at": "2026-09-15T00:00:00Z"});
        fs::write(&path, record.to_string()).expect("record");
    }
    // Until the owner's `next` verifies it, the head is unverified, and
    // `--prune` keeps it.
    rune(&fixture)
        .args(["sign", "queue"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("unverified"));
    rune(&fixture)
        .args(["sign", "queue", "--prune"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    // A second card prompt would fail loudly: the fake gpg refuses once.
    fs::write(&fixture.fail_once, "Operation cancelled").expect("fail once");
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("signed change/base"));
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .success()
        .stdout(predicate::str::contains("signed"));
}

/// Queue the base with a passing receipt.
fn queue_base(fixture: &Fixture) -> String {
    let base = head_commit(fixture, "change/base");
    let base_receipt = receipt(fixture, "base.log", &base, "exit=0");
    rune(fixture)
        .args(["sign", "queue", "change/base", "--receipt"])
        .arg(&base_receipt)
        .assert()
        .success();
    base
}

/// Wait for the fake gpg to report that it is inside the signing call.
fn wait_for_signing(wait: &Path) {
    for _ in 0..200 {
        if wait.join("in-sign").exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the fake gpg never entered the signing call");
}

#[test]
fn a_bookmark_moved_after_the_claim_is_stale_and_nothing_is_signed() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    queue_base(&fixture);
    let top = head_commit(&fixture, "change/top");
    let wait = fixture.workspace.join("wait");
    fs::create_dir_all(&wait).expect("wait dir");
    fs::write(wait.join("armed"), "").expect("arm");
    let mut signer = rune_process(&fixture)
        .args(["sign", "next"])
        .env("FAKE_GPG_WAIT", &wait)
        .spawn()
        .expect("signer starts");
    wait_for_signing(&wait);
    // The head moves while the owner's card is busy. A jj command would
    // wait for the signing transaction, so the move is made where the
    // queue reads it: the git ref, which jj imports on its next command.
    run(Process::new("git")
        .arg("--git-dir")
        .arg(fixture.workspace.join(".git"))
        .args(["update-ref", "refs/heads/change/base", &top]));
    fs::write(wait.join("go"), "").expect("release");
    let status = signer.wait().expect("signer exits");
    assert!(!status.success(), "a moved head is never signed");
    assert!(!head_is_signed(&fixture, "change/base"));
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stale"));
}

#[test]
fn a_second_signer_on_the_same_repository_is_refused_while_the_first_runs() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    queue_base(&fixture);
    let wait = fixture.workspace.join("wait");
    fs::create_dir_all(&wait).expect("wait dir");
    fs::write(wait.join("armed"), "").expect("arm");
    let mut first = rune_process(&fixture)
        .args(["sign", "next"])
        .env("FAKE_GPG_WAIT", &wait)
        .spawn()
        .expect("first signer starts");
    wait_for_signing(&wait);
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("another signer holds"));
    fs::write(wait.join("go"), "").expect("release");
    // The first signer's retry signs; the lock went with its process.
    let status = first.wait().expect("first signer exits");
    assert!(status.success(), "the first signer signs on its retry");
    assert!(head_is_signed(&fixture, "change/base"));
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing to sign"));
}

#[test]
fn a_stale_request_is_skipped_and_the_next_current_one_is_signed() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    // Two unrelated bookmarks: side is queued first and then amended.
    run(jj(&fixture).args(["new", "change/base", "-m", "feat: side"]));
    run(jj(&fixture).args(["bookmark", "set", "change/side", "-r", "@"]));
    let side = head_commit(&fixture, "change/side");
    let side_receipt = receipt(&fixture, "side.log", &side, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/side", "--receipt"])
        .arg(&side_receipt)
        .assert()
        .success();
    let top = head_commit(&fixture, "change/top");
    let top_receipt = receipt(&fixture, "top.log", &top, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/top", "--receipt"])
        .arg(&top_receipt)
        .assert()
        .success();
    run(jj(&fixture).args(["describe", "-r", "change/side", "-m", "feat: side, amended"]));
    // Base is unsigned and unqueued, so top is current; side is stale.
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("skipped: stale change/side")
                .and(predicate::str::contains("signed change/top")),
        );
    assert!(head_is_signed(&fixture, "change/top"));
}

#[test]
fn a_repository_that_aliases_a_listing_keyword_is_refused() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    queue_base(&fixture);
    run(jj(&fixture).args(["config", "path", "--repo"]));
    let config_id = fs::read_to_string(fixture.workspace.join(".jj/repo/config-id"))
        .expect("config id")
        .trim()
        .to_string();
    let repo_config = fixture.xdg_config.join("jj").join("repos").join(&config_id);
    fs::create_dir_all(&repo_config).expect("repo config dir");
    fs::write(
        repo_config.join("config.toml"),
        "[template-aliases]\nvalue = '\"forged\"'\n",
    )
    .expect("repo config");
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("template-aliases.value"));
    assert!(!head_is_signed(&fixture, "change/base"));
}

#[test]
fn a_cancelled_attempt_on_a_rewritten_head_leaves_the_request_signable() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    // Queue top, then sign base by hand: top is rewritten and keeps its
    // identity, so its request is current under a new commit id.
    let top = head_commit(&fixture, "change/top");
    let top_receipt = receipt(&fixture, "top.log", &top, "exit=0");
    rune(&fixture)
        .args(["sign", "queue", "change/top", "--receipt"])
        .arg(&top_receipt)
        .assert()
        .success();
    run(jj(&fixture).args(["sign", "-r", "change/base"]));
    assert_ne!(
        head_commit(&fixture, "change/top"),
        top,
        "base signing rewrote top"
    );
    // The owner cancels the first attempt; the record must still name the
    // commit the receipt names, or the next run would refuse the receipt.
    fs::write(&fixture.fail_once, "Operation cancelled").expect("fail once");
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("cancelled"));
    rune(&fixture)
        .args(["sign", "show", "change/top"])
        .assert()
        .success()
        .stdout(predicate::str::contains("current").and(predicate::str::contains(&top)));
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("signed change/top"));
}

#[test]
fn an_empty_queue_names_the_ledger_it_read() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let ledger = fixture.state.join("sign-queue").display().to_string();
    rune(&fixture)
        .args(["sign", "queue"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&ledger).and(predicate::str::contains("empty")));
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("nothing to sign in ").and(predicate::str::contains(&ledger)),
        );
}

#[test]
fn a_relative_receipt_is_recorded_absolute_and_the_owner_signs_from_elsewhere() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let base = head_commit(&fixture, "change/base");
    receipt(&fixture, "base.log", &base, "exit=0");
    // The session names the receipt relative to its workspace.
    rune(&fixture)
        .args(["sign", "queue", "change/base", "--receipt", "base.log"])
        .assert()
        .success();
    rune(&fixture)
        .args(["sign", "show", "change/base"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            fixture.workspace.join("base.log").display().to_string(),
        ));
    // The owner signs from a different directory.
    rune(&fixture)
        .args(["sign", "next"])
        .current_dir(fixture.state.parent().expect("home"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("signed change/base"));
}

#[test]
fn a_request_whose_workspace_is_gone_is_stale_and_prunable_not_fatal() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    queue_base(&fixture);
    let requests = fixture.state.join("sign-queue").join("requests");
    for entry in fs::read_dir(&requests).expect("requests") {
        let path = entry.expect("entry").path();
        let mut record: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("record")).expect("json");
        record["workspace"] = serde_json::json!("/nowhere/that/exists");
        fs::write(&path, record.to_string()).expect("record");
    }
    rune(&fixture)
        .args(["sign", "queue"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("stale"));
    rune(&fixture)
        .args(["sign", "next"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("skipped: stale"));
    rune(&fixture)
        .args(["sign", "queue", "--prune"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pruned"));
}
