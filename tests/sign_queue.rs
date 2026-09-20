//! The signing queue against a real jj repository with a throwaway gpg key.
//! Skips when jj or gpg is missing, the way the init tests skip without jj.

use assert_cmd::Command;
use predicates::prelude::*;
use sha2::Digest;
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
/// retry that follows signs. On `--show-keys`, as the plain-checkout
/// verifier calls `gpg` from PATH, it lists the owner fingerprint.
const FAKE_GPG: &str = r#"#!/bin/sh
case " $* " in
  *" --show-keys "*)
    printf 'fpr:::::::::%s:\n' "${FAKE_GPG_KEY:-29DD2145CE7A818929459B2649F08103D3DA399E}"
    ;;
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
    // `main@origin` would make the base immutable to jj, and the tests sign
    // it in place; the fixture has no immutable commits.
    fs::write(
        workspace.join(".jjconfig.toml"),
        format!(
            "[signing]\nbackend = \"gpg\"\nkey = \"{OWNER_FINGERPRINT}\"\n[signing.backends.gpg]\nprogram = \"{}\"\n[revset-aliases]\n\"immutable_heads()\" = \"none()\"\n",
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
    // The body schema is committed at the base, so every branch's tree
    // carries it: `open` reads it from the head, not the working copy.
    install_schema(&fixture);
    run(jj(&fixture).args(["describe", "-m", "feat: base"]));
    run(jj(&fixture).args(["bookmark", "set", "change/base", "-r", "@"]));
    run(jj(&fixture).args(["new", "-m", "feat: top"]));
    fs::write(fixture.workspace.join("top.txt"), "top\n").expect("file");
    run(jj(&fixture).args(["bookmark", "set", "change/top", "-r", "@"]));
    run(jj(&fixture).args(["new"]));
    // The protected branch as origin has it, where KEYS is read from: the
    // base commit carries the repository's KEYS file.
    let base = head_commit(&fixture, "change/base");
    set_remote_ref(&fixture, "main", &base);
    Some(fixture)
}

/// Point `refs/remotes/origin/<name>` at a commit, as a fetch would.
fn set_remote_ref(fixture: &Fixture, name: &str, commit: &str) {
    run(git(fixture).args(["update-ref", &format!("refs/remotes/origin/{name}"), commit]));
}

fn git(fixture: &Fixture) -> Process {
    let mut command = Process::new("git");
    command.arg("-C").arg(&fixture.workspace);
    command
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
        .args(["sign", "queue", "change/base", "--receipt"])
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

/// Signing in place rewrites the commit. A head the remote already holds,
/// or one beneath the remote head, would come back as a force-push of a
/// published tip (2026-09-20: `4fa850b9` re-signed sideways to `64fc7e85`).
#[test]
fn a_request_refuses_a_head_the_remote_already_holds() {
    let Some(fixture) = fixture() else {
        eprintln!("skipped: jj or gpg is not installed");
        return;
    };
    let top = head_commit(&fixture, "change/top");
    // Outside the workspace: `jj new` below would otherwise snapshot the
    // receipt into one commit and drop it from the next working copy.
    let good = fixture.workspace.parent().expect("parent").join("top.log");
    fs::write(
        &good,
        format!("bookmark: x [move sideways from 0000 to {top}]\nexit=0\n"),
    )
    .expect("receipt");

    set_remote_ref(&fixture, "change/top", &top);
    rune(&fixture)
        .args(["sign", "queue", "change/top", "--receipt"])
        .arg(&good)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already published"))
        .stderr(predicate::str::contains("Sign before you push"));

    // The remote moved past the head: still published, still refused.
    run(jj(&fixture).args(["new", "change/top", "-m", "feat: later"]));
    let later = run(jj(&fixture).args(["log", "--no-graph", "-r", "@", "-T", "commit_id"]));
    set_remote_ref(&fixture, "change/top", &later);
    rune(&fixture)
        .args(["sign", "queue", "change/top", "--receipt"])
        .arg(&good)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already published"));

    // The remote holds an unrelated commit: the head is unpublished.
    let base = head_commit(&fixture, "change/base");
    set_remote_ref(&fixture, "change/top", &base);
    rune(&fixture)
        .args(["sign", "queue", "change/top", "--receipt"])
        .arg(&good)
        .assert()
        .success();
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

/// `gh` as the ceremony commands call it, answering from `FAKE_GH_DIR`:
/// `prs.json` for `pr list` and `pr view` (a `state` other than `OPEN`
/// hides the entry from `--state open`), `ledger.json` for the ledger: the
/// check-runs read answers with one `ledger` check run from the controller
/// app whose line names artifact 1 and the file's sha256, unless
/// `check-runs.json` overrides the runs, and the artifact read zips
/// `artifact-<id>.json`, or `ledger.json`, as `runeseer-ledger.json`.
/// `checks.json` answers `pr checks`, which refuses a call without
/// `--required`, and `ready-<n>` and `body-<n>` record `pr ready` and
/// `pr edit`. A `down` file makes every call fail, as a forge outage does.
const FAKE_GH: &str = r#"#!/usr/bin/env python3
import hashlib, io, json, os, shutil, sys, zipfile
d = os.environ["FAKE_GH_DIR"]
a = sys.argv[1:]
with open(os.path.join(d, "calls.log"), "a") as log:
    log.write(" ".join(a) + "\n")
if os.path.exists(os.path.join(d, "down")):
    sys.stderr.write("gh: connection refused\n")
    sys.exit(1)
def load(name, default):
    p = os.path.join(d, name)
    return json.load(open(p)) if os.path.exists(p) else default
def state(p):
    return p.get("state", "OPEN")
if a[:2] == ["pr", "list"]:
    prs = load("prs.json", [])
    if "--state" in a and a[a.index("--state") + 1] == "open":
        prs = [p for p in prs if state(p) == "OPEN"]
    if "--head" in a:
        head = a[a.index("--head") + 1]
        prs = [p for p in prs if p.get("headRefName") == head]
    print(json.dumps(prs))
elif a[:2] == ["pr", "view"]:
    found = [p for p in load("prs.json", []) if p["number"] == int(a[2])]
    if not found:
        sys.exit(1)
    found[0]["state"] = state(found[0])
    print(json.dumps(found[0]))
elif a[:2] == ["pr", "ready"]:
    open(os.path.join(d, "ready-" + a[2]), "w").close()
elif a[:2] == ["pr", "edit"]:
    shutil.copy(a[a.index("--body-file") + 1], os.path.join(d, "body-" + a[2]))
elif a[:2] == ["pr", "checks"]:
    if "--required" not in a:
        sys.exit(3)
    print(json.dumps(load("checks.json", [])))
elif a[0] == "api" and "/check-runs?check_name=ledger" in a[1]:
    runs = load("check-runs.json", None)
    p = os.path.join(d, "ledger.json")
    if runs is None:
        runs = []
        if os.path.exists(p):
            raw = open(p, "rb").read()
            ledger = json.loads(raw)
            line = {"artifact_id": 1, "digest": hashlib.sha256(raw).hexdigest(),
                    "generation": ledger["generation"], "pull_request": 7,
                    "reviewed_sha": ledger["reviewed_sha"]}
            runs = [{"id": 1, "name": "ledger", "app": {"slug": "runeseer"},
                     "output": {"text": "ledger: " + json.dumps(line, sort_keys=True, separators=(",", ":"))}}]
    print(json.dumps([{"check_runs": runs}]))
elif a[0] == "api" and "/actions/artifacts/" in a[1] and a[1].endswith("/zip"):
    artifact_id = a[1].rsplit("/", 2)[1]
    p = os.path.join(d, "artifact-" + artifact_id + ".json")
    if not os.path.exists(p):
        p = os.path.join(d, "ledger.json")
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.write(p, "runeseer-ledger.json")
    sys.stdout.buffer.write(buffer.getvalue())
elif a[0] == "api" and a[1].startswith("repos/"):
    print(json.dumps({"permissions": {"admin": True}}))
else:
    sys.exit(3)
"#;

/// A body that passes `schemas/PULL_REQUEST.mdschema`.
const BODY: &str = "Seal the ceremony.\n\n## Plan\n\nNone.\n\n## Changes\n\n- Add top.txt\n\n## Testing\n\n- cargo test\n\n## Release Notes\n\n- N/A\n";

/// The ceremony fixture: the queue fixture plus a `main` bookmark at the
/// base, an `origin` whose URL names the forge slug and whose push URL is
/// a bare repository beside the workspace, the pull request schema, a git
/// identity for the seal commits, the fake `gh` on PATH, and a hook the
/// session controls, which records a call in `pushed` if anything runs it.
struct Ceremony {
    fixture: Fixture,
    gh_dir: PathBuf,
    bin: PathBuf,
    /// The bare repository `origin` pushes go to.
    remote: PathBuf,
}

fn ceremony() -> Option<Ceremony> {
    let fixture = fixture()?;
    if !tool_present("python3") {
        return None;
    }
    let home = fixture.workspace.parent().expect("home").to_path_buf();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fs::write(bin.join("gh"), FAKE_GH).expect("fake gh");
    let hooks = fixture.workspace.join(".githooks");
    fs::create_dir_all(&hooks).expect("hooks");
    for hook in ["jj-push", "pre-push"] {
        fs::write(
            hooks.join(hook),
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/../pushed\"\n",
        )
        .expect("hook");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            bin.join("gh"),
            hooks.join("jj-push"),
            hooks.join("pre-push"),
        ] {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
    }
    install_schema(&fixture);
    let remote = home.join("origin.git");
    run(Process::new("git")
        .args(["init", "--bare", "--quiet"])
        .arg(&remote));
    // Transport goes to the bare repository while the URL keeps naming the
    // slug, and the repository's own hooks would record a push that ran
    // them.
    let instead_of = format!("url.{}.insteadOf", remote.display());
    let hooks_path = hooks.to_string_lossy();
    for (key, value) in [
        ("user.name", "Owner Test"),
        ("user.email", "owner@example.com"),
        ("remote.origin.url", "https://github.com/acme/widgets.git"),
        (instead_of.as_str(), "https://github.com/acme/widgets.git"),
        ("core.hooksPath", hooks_path.as_ref()),
    ] {
        run(git(&fixture).args(["config", key, value]));
    }
    run(jj(&fixture).args(["bookmark", "set", "main", "-r", "change/base"]));
    // The session pushed its branch: that is how the draft came to exist.
    run(git(&fixture).args([
        "-c",
        "core.hooksPath=/dev/null",
        "push",
        "--quiet",
        "origin",
        "refs/heads/change/top:refs/heads/change/top",
    ]));
    let gh_dir = home.join("gh");
    fs::create_dir_all(&gh_dir).expect("gh dir");
    Some(Ceremony {
        fixture,
        gh_dir,
        bin,
        remote,
    })
}

/// The commit `origin` holds for a branch, or `None` when it has none.
fn remote_head(ceremony: &Ceremony, bookmark: &str) -> Option<String> {
    let output = Process::new("git")
        .arg("--git-dir")
        .arg(&ceremony.remote)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("refs/heads/{bookmark}"))
        .output()
        .expect("git");
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn calls(ceremony: &Ceremony) -> String {
    fs::read_to_string(ceremony.gh_dir.join("calls.log")).unwrap_or_default()
}

fn ceremony_rune(ceremony: &Ceremony) -> Command {
    let path = format!(
        "{}:{}",
        ceremony.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut command = rune(&ceremony.fixture);
    command
        .env("PATH", path)
        .env("FAKE_GH_DIR", &ceremony.gh_dir);
    command
}

fn pull_request(number: u64, head: &str, oid: &str, draft: bool, body: &str) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "isDraft": draft,
        "baseRefName": "main",
        "headRefName": head,
        "headRefOid": oid,
        "body": body,
        "url": format!("https://github.com/acme/widgets/pull/{number}"),
        "state": "OPEN",
    })
}

fn with(mut pull_request: serde_json::Value, field: &str, value: &str) -> serde_json::Value {
    pull_request[field] = serde_json::json!(value);
    pull_request
}

fn write_pull_requests(ceremony: &Ceremony, pull_requests: &[serde_json::Value]) {
    fs::write(
        ceremony.gh_dir.join("prs.json"),
        serde_json::to_string(pull_requests).expect("json"),
    )
    .expect("prs.json");
}

fn body_file(ceremony: &Ceremony) -> PathBuf {
    let path = ceremony.fixture.workspace.join("body.md");
    fs::write(&path, BODY).expect("body");
    path
}

fn commit_message(ceremony: &Ceremony, revision: &str) -> String {
    run(jj(&ceremony.fixture).args(["log", "--no-graph", "-r", revision, "-T", "description"]))
}

fn parent_commit(ceremony: &Ceremony, revision: &str) -> String {
    run(jj(&ceremony.fixture).args([
        "log",
        "--no-graph",
        "-r",
        &format!("parents({revision})"),
        "-T",
        "commit_id",
    ]))
}

/// The fields of an open-seal message, whatever order they were written in.
fn open_seal(message: &str) -> serde_json::Value {
    let json = message
        .trim()
        .strip_prefix("open-seal: ")
        .expect("open-seal");
    serde_json::from_str(json).expect("seal json")
}

fn nonce_of(message: &str) -> String {
    open_seal(message)["nonce"]
        .as_str()
        .expect("nonce")
        .to_string()
}

fn ledger(
    reviewed_sha: &str,
    generation: u64,
    verdict_generation: u64,
    thread: &serde_json::Value,
) -> String {
    let ledger = serde_json::json!({
        "reviewed_sha": reviewed_sha,
        "generation": generation,
        "verdict": {"sha": reviewed_sha, "generation": verdict_generation, "verdict": "clean"},
        "lanes": {"codex": "completed-no-findings", "runeseer": "completed"},
        "threads": [thread.clone()],
    });
    ledger.to_string()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The merge-seal message `rune sign next` writes for the ledger the fake
/// forge serves: its digest is the sha256 of `ledger.json`.
fn merge_seal_message(ceremony: &Ceremony, reviewed_sha: &str, generation: u64) -> String {
    let raw = fs::read(ceremony.gh_dir.join("ledger.json")).expect("ledger");
    let digest = hex(&sha2::Sha256::digest(&raw));
    format!(
        "merge-seal: {{\"reviewed_sha\":\"{reviewed_sha}\",\"generation\":{generation},\"digest\":\"{digest}\"}}"
    )
}

fn write_ledger(ceremony: &Ceremony, body: &str) {
    fs::write(ceremony.gh_dir.join("ledger.json"), body).expect("ledger");
    fs::write(
        ceremony.gh_dir.join("checks.json"),
        r#"[{"name": "quality", "bucket": "pass"}]"#,
    )
    .expect("checks");
}

/// Open-seal `change/top` through the whole flow; returns the seal commit.
fn open_top(ceremony: &Ceremony) -> String {
    let top = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(ceremony, &[pull_request(7, "change/top", &top, true, "")]);
    let body = body_file(ceremony);
    ceremony_rune(ceremony)
        .args(["sign", "open", "change/top", "--body-file"])
        .arg(&body)
        .assert()
        .success()
        .stdout(predicate::str::contains("opened change/top"));
    head_commit(&ceremony.fixture, "change/top")
}

#[test]
fn open_refuses_the_protected_branch_and_a_branch_without_a_draft() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let body = body_file(&ceremony);
    ceremony_rune(&ceremony)
        .args(["sign", "open", "main", "--body-file"])
        .arg(&body)
        .assert()
        .failure()
        .stderr(predicate::str::contains("protected branch"));
    write_pull_requests(&ceremony, &[]);
    ceremony_rune(&ceremony)
        .args(["sign", "open", "change/top", "--body-file"])
        .arg(&body)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no open pull request has head change/top",
        ));
    assert!(!head_is_signed(&ceremony.fixture, "change/top"));
    assert!(!ceremony.fixture.workspace.join("pushed").exists());
    assert!(calls(&ceremony).contains("pr list --repo acme/widgets --state open"));
}

/// Every other refusal of `rune sign open`, in the order the command
/// checks them, each leaving the head unsigned and the remote untouched.
#[test]
fn open_refuses_a_ready_draft_a_stale_head_two_drafts_a_bad_body_and_a_foreign_branch_and_seals_above_an_owner_signed_head()
 {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let top = head_commit(&ceremony.fixture, "change/top");
    let body = body_file(&ceremony);
    let refused = |pull_requests: &[serde_json::Value], body: &Path, message: &str| {
        write_pull_requests(&ceremony, pull_requests);
        ceremony_rune(&ceremony)
            .args(["sign", "open", "change/top", "--body-file"])
            .arg(body)
            .assert()
            .failure()
            .stderr(predicate::str::contains(message));
        assert!(!head_is_signed(&ceremony.fixture, "change/top"));
        assert_eq!(remote_head(&ceremony, "change/top").as_deref(), Some(&*top));
    };
    refused(
        &[pull_request(7, "change/top", &top, false, "")],
        &body,
        "pull request #7 is already ready",
    );
    refused(
        &[pull_request(7, "change/top", "0123456789ab", true, "")],
        &body,
        "pull request #7 is at 0123456789ab and change/top is at",
    );
    refused(
        &[
            pull_request(7, "change/top", &top, true, ""),
            pull_request(8, "change/top", &top, true, ""),
        ],
        &body,
        "2 open pull requests have head change/top",
    );
    let short_body = ceremony.fixture.workspace.join("short.md");
    fs::write(&short_body, "Seal.\n\n## Plan\n\nNone.\n").expect("body");
    refused(
        &[pull_request(7, "change/top", &top, true, "")],
        &short_body,
        "does not pass schemas/PULL_REQUEST.mdschema",
    );
    // A branch whose remote head is not beneath it was not created by the
    // session that asks to open it.
    let other = forged_seal(
        &ceremony,
        "change/base",
        "feat: other",
        "change/other",
        true,
    );
    set_remote_ref(&ceremony.fixture, "change/top", &other);
    refused(
        &[pull_request(7, "change/top", &top, true, "")],
        &body,
        "change/top does not descend from origin/change/top",
    );
    set_remote_ref(&ceremony.fixture, "change/top", &top);
    // A head the owner signed first (`jj sign`, then `rune sign open`) is
    // accepted: the open-seal is a new commit above it. The remote still
    // holds the unsigned commit the session pushed, which is an ancestor
    // in content but not in history, so the fixture moves the remote ref
    // to the signed head the way a `jj git push` of it would.
    run(jj(&ceremony.fixture).args(["sign", "-r", "change/top"]));
    let signed = head_commit(&ceremony.fixture, "change/top");
    assert_ne!(signed, top);
    run(git(&ceremony.fixture).args([
        "-c",
        "core.hooksPath=/dev/null",
        "push",
        "--quiet",
        "--force",
        "origin",
        "refs/heads/change/top:refs/heads/change/top",
    ]));
    set_remote_ref(&ceremony.fixture, "change/top", &signed);
    // The rewrite checked out a fresh working copy: put the body back.
    let body = body_file(&ceremony);
    write_pull_requests(
        &ceremony,
        &[pull_request(7, "change/top", &signed, true, "")],
    );
    ceremony_rune(&ceremony)
        .args(["sign", "open", "change/top", "--body-file"])
        .arg(&body)
        .assert()
        .success()
        .stderr(predicate::str::contains("already owner-signed"))
        .stdout(predicate::str::contains("opened change/top"));
    assert!(ceremony.gh_dir.join("ready-7").exists());
    assert_eq!(parent_commit(&ceremony, "change/top"), signed);
}

#[test]
fn open_seals_pushes_flips_the_draft_and_appends_the_nonce() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let top = head_commit(&ceremony.fixture, "change/top");
    let sealed = open_top(&ceremony);
    assert_ne!(sealed, top);
    assert!(head_is_signed(&ceremony.fixture, "change/top"));
    assert_eq!(parent_commit(&ceremony, "change/top"), top);
    let message = commit_message(&ceremony, "change/top");
    let seal = open_seal(&message);
    assert_eq!(seal["repo"], "acme/widgets");
    assert_eq!(seal["base"], "main");
    assert_eq!(seal["pull_request"], 7);
    let nonce = nonce_of(&message);
    // The seal reached origin under a lease, and no hook of the repository
    // ran on the owner's side.
    assert_eq!(
        remote_head(&ceremony, "change/top").as_deref(),
        Some(&*sealed)
    );
    assert!(!ceremony.fixture.workspace.join("pushed").exists());
    assert!(ceremony.gh_dir.join("ready-7").exists());
    let body = fs::read_to_string(ceremony.gh_dir.join("body-7")).expect("body");
    assert!(body.starts_with(BODY.trim_end()));
    assert!(body.ends_with(&format!("\nOpen-Seal-Nonce: {nonce}\n")));
}

/// `open --queue` records the request; `next` shows the view, declines on
/// `n`, and completes the whole flow on `y`.
#[test]
fn open_queued_is_completed_by_next_after_the_acknowledgment() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let top = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(&ceremony, &[pull_request(7, "change/top", &top, true, "")]);
    let body = body_file(&ceremony);
    ceremony_rune(&ceremony)
        .args(["sign", "open", "change/top", "--queue", "--body-file"])
        .arg(&body)
        .assert()
        .success()
        .stdout(predicate::str::contains("queued"));
    assert!(!head_is_signed(&ceremony.fixture, "change/top"));
    ceremony_rune(&ceremony)
        .args(["sign", "show", "change/top"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("current").and(predicate::str::contains("#7 on acme/widgets")),
        );
    let tty = ceremony.fixture.workspace.join("tty");
    fs::write(&tty, "n\n").expect("tty");
    ceremony_rune(&ceremony)
        .args(["sign", "next"])
        .env("RUNE_SIGN_TTY", &tty)
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("pull request #7 on acme/widgets")
                .and(predicate::str::contains("## Release Notes"))
                .and(predicate::str::contains(format!(
                    "tree          {}",
                    tree_of(&ceremony, &top)
                )))
                .and(predicate::str::contains("declined change/top")),
        );
    assert!(!head_is_signed(&ceremony.fixture, "change/top"));
    fs::write(&tty, "y\n").expect("tty");
    ceremony_rune(&ceremony)
        .args(["sign", "next"])
        .env("RUNE_SIGN_TTY", &tty)
        .assert()
        .success()
        .stdout(predicate::str::contains("signed change/top"));
    let sealed = head_commit(&ceremony.fixture, "change/top");
    assert!(head_is_signed(&ceremony.fixture, "change/top"));
    assert_eq!(parent_commit(&ceremony, "change/top"), top);
    assert_eq!(
        remote_head(&ceremony, "change/top").as_deref(),
        Some(&*sealed)
    );
    assert!(ceremony.gh_dir.join("ready-7").exists());
    let nonce = nonce_of(&commit_message(&ceremony, "change/top"));
    let body = fs::read_to_string(ceremony.gh_dir.join("body-7")).expect("body");
    assert!(body.ends_with(&format!("\nOpen-Seal-Nonce: {nonce}\n")));
}

fn tree_of(ceremony: &Ceremony, commit: &str) -> String {
    run(git(&ceremony.fixture).args(["rev-parse", &format!("{commit}^{{tree}}")]))
}

#[test]
fn submit_refuses_an_open_thread_and_an_undisposed_generation() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let top = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(
        &ceremony,
        &[pull_request(7, "change/top", &top, false, BODY)],
    );
    let receipt = receipt(&ceremony.fixture, "top.log", &top, "exit=0");
    write_ledger(
        &ceremony,
        &ledger(
            &top,
            1,
            1,
            &serde_json::json!({"id": "T1", "lane": "codex"}),
        ),
    );
    ceremony_rune(&ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "thread T1 is open without a disposition",
        ));
    write_ledger(
        &ceremony,
        &ledger(
            &top,
            2,
            1,
            &serde_json::json!({"id": "T1", "lane": "codex", "disposition": "fixed"}),
        ),
    );
    ceremony_rune(&ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "generation 1 and the ledger is at generation 2",
        ));
    ceremony_rune(&ceremony)
        .args(["sign", "queue"])
        .assert()
        .success()
        .stdout(predicate::str::contains("empty"));
}

/// Queue `change/top` for the merge-seal with a clean ledger.
fn submit_top(ceremony: &Ceremony) -> (String, PathBuf) {
    let top = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(
        ceremony,
        &[pull_request(7, "change/top", &top, false, BODY)],
    );
    let receipt = receipt(&ceremony.fixture, "top.log", &top, "exit=0");
    write_ledger(
        ceremony,
        &ledger(
            &top,
            2,
            2,
            &serde_json::json!({"id": "T1", "lane": "codex", "disposition": "fixed"}),
        ),
    );
    ceremony_rune(ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .success()
        .stdout(predicate::str::contains("(clean)"));
    (top, receipt)
}

#[test]
fn next_shows_the_view_refuses_on_n_and_seals_on_y() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let (top, _receipt) = submit_top(&ceremony);
    let tty = ceremony.fixture.workspace.join("tty");
    fs::write(&tty, "n\n").expect("tty");
    ceremony_rune(&ceremony)
        .args(["sign", "next"])
        .env("RUNE_SIGN_TTY", &tty)
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("diff stat    main...")
                .and(predicate::str::contains("top.txt"))
                .and(predicate::str::contains("coverage     clean"))
                .and(predicate::str::contains("lane         codex"))
                .and(predicate::str::contains(
                    "thread       T1                   fixed",
                ))
                .and(predicate::str::contains("/top.log"))
                .and(predicate::str::contains(format!("reviewed_sha  {top}")))
                .and(predicate::str::contains("generation    2"))
                .and(predicate::str::contains("declined change/top")),
        );
    assert!(!head_is_signed(&ceremony.fixture, "change/top"));
    assert_eq!(head_commit(&ceremony.fixture, "change/top"), top);
    ceremony_rune(&ceremony)
        .args(["sign", "show", "change/top"])
        .assert()
        .success()
        .stdout(predicate::str::contains("current").and(predicate::str::contains("merge")));

    fs::write(&tty, "y\n").expect("tty");
    ceremony_rune(&ceremony)
        .args(["sign", "next"])
        .env("RUNE_SIGN_TTY", &tty)
        .assert()
        .success()
        .stdout(predicate::str::contains("signed change/top"));
    assert!(head_is_signed(&ceremony.fixture, "change/top"));
    assert_eq!(parent_commit(&ceremony, "change/top"), top);
    assert_eq!(
        commit_message(&ceremony, "change/top").trim(),
        merge_seal_message(&ceremony, &top, 2)
    );
    // Nothing pushed: origin still holds the reviewed head.
    assert_eq!(remote_head(&ceremony, "change/top").as_deref(), Some(&*top));
    assert!(!ceremony.fixture.workspace.join("pushed").exists());
    let calls = calls(&ceremony);
    assert!(calls.contains("pr checks 7 --repo acme/widgets --required"));
    assert!(calls.contains("pr list --repo acme/widgets --state open"));
}

/// The view shows the coverage state the ledger recorded when Runeseer
/// stood down, with its reason, before the key.
#[test]
fn next_shows_free_lanes_only_with_its_reason() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let top = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(
        &ceremony,
        &[pull_request(7, "change/top", &top, false, BODY)],
    );
    let receipt = receipt(&ceremony.fixture, "top.log", &top, "exit=0");
    let ledger = serde_json::json!({
        "reviewed_sha": top,
        "generation": 1,
        "verdict": null,
        "coverage": "free lanes only: runeseer off",
        "lanes": {"codex": "completed-no-findings", "runeseer": "skipped"},
        "threads": [],
    });
    write_ledger(&ceremony, &ledger.to_string());
    ceremony_rune(&ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .success()
        .stdout(predicate::str::contains("(free-lanes-only)"));
    let tty = ceremony.fixture.workspace.join("tty");
    fs::write(&tty, "n\n").expect("tty");
    ceremony_rune(&ceremony)
        .args(["sign", "next"])
        .env("RUNE_SIGN_TTY", &tty)
        .assert()
        .failure()
        .stdout(
            predicate::str::is_match(
                r"coverage\s+free-lanes-only \(free lanes only: runeseer off\)",
            )
            .expect("regex")
            .and(predicate::str::is_match(r"lane\s+runeseer\s+skipped").expect("regex"))
            .and(predicate::str::is_match(r"threads\s+none").expect("regex")),
        );
}

/// A `ledger` check run from any app other than the controller is not the
/// ledger, a newer forged run does not win over the controller's, and an
/// artifact that does not hash to the controller's line is refused.
#[test]
fn submit_reads_the_ledger_from_the_controller_alone() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let top = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(
        &ceremony,
        &[pull_request(7, "change/top", &top, false, BODY)],
    );
    let receipt = receipt(&ceremony.fixture, "top.log", &top, "exit=0");
    let clean = ledger(
        &top,
        1,
        1,
        &serde_json::json!({"id": "T1", "lane": "codex", "disposition": "fixed"}),
    );
    let open = ledger(&top, 1, 1, &serde_json::json!({"id": "T1", "lane": null}));
    let line = |artifact_id: u64, body: &str| {
        let digest = hex(&sha2::Sha256::digest(body.as_bytes()));
        format!(
            "ledger: {{\"artifact_id\":{artifact_id},\"digest\":\"{digest}\",\"generation\":1,\"pull_request\":7,\"reviewed_sha\":\"{top}\"}}"
        )
    };
    write_ledger(&ceremony, &clean);
    fs::write(ceremony.gh_dir.join("artifact-1.json"), &clean).expect("artifact");
    fs::write(ceremony.gh_dir.join("artifact-2.json"), &open).expect("artifact");
    fs::write(
        ceremony.gh_dir.join("check-runs.json"),
        serde_json::json!([
            {"id": 9, "name": "ledger", "app": {"slug": "someone-else"}, "output": {"text": line(1, &clean)}},
        ])
        .to_string(),
    )
    .expect("check runs");
    ceremony_rune(&ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no ledger from runeseer"));
    fs::write(
        ceremony.gh_dir.join("check-runs.json"),
        serde_json::json!([
            {"id": 5, "name": "ledger", "app": {"slug": "runeseer"}, "output": {"text": line(2, &open)}},
            {"id": 9, "name": "ledger", "app": {"slug": "someone-else"}, "output": {"text": line(1, &clean)}},
        ])
        .to_string(),
    )
    .expect("check runs");
    ceremony_rune(&ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "thread T1 is open without a disposition",
        ));
    // The controller's line names the clean artifact, but the artifact
    // served under that id is another: the digest refuses it.
    fs::write(
        ceremony.gh_dir.join("check-runs.json"),
        serde_json::json!([
            {"id": 5, "name": "ledger", "app": {"slug": "runeseer"}, "output": {"text": line(2, &clean)}},
        ])
        .to_string(),
    )
    .expect("check runs");
    ceremony_rune(&ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not the ledger"));
    ceremony_rune(&ceremony)
        .args(["sign", "queue"])
        .assert()
        .success()
        .stdout(predicate::str::contains("empty"));
}

/// The full ceremony on `change/top`: open-seal, a clean ledger, the
/// merge-seal. Returns the opened head, the merged head, and the body that
/// carries the nonce.
fn seal_pair(ceremony: &Ceremony) -> (String, String, String) {
    let opened = open_top(ceremony);
    let nonce = nonce_of(&commit_message(ceremony, "change/top"));
    let body = format!("{BODY}\nOpen-Seal-Nonce: {nonce}\n");
    // The controller reviewed the sealed head; the owner seals the merge.
    write_pull_requests(
        ceremony,
        &[pull_request(7, "change/top", &opened, false, &body)],
    );
    let receipt = receipt(&ceremony.fixture, "top.log", &opened, "exit=0");
    write_ledger(
        ceremony,
        &ledger(
            &opened,
            1,
            1,
            &serde_json::json!({"id": "T1", "disposition": "rejected", "reason": "by design"}),
        ),
    );
    ceremony_rune(ceremony)
        .args(["sign", "submit", "change/top", "--receipt"])
        .arg(&receipt)
        .assert()
        .success();
    let tty = ceremony.fixture.workspace.join("tty");
    fs::write(&tty, "y\n").expect("tty");
    ceremony_rune(ceremony)
        .args(["sign", "next"])
        .env("RUNE_SIGN_TTY", &tty)
        .assert()
        .success();
    let merged = head_commit(&ceremony.fixture, "change/top");
    write_pull_requests(
        ceremony,
        &[pull_request(7, "change/top", &merged, false, &body)],
    );
    (opened, merged, body)
}

/// A hand-made signed commit above `parent` with `message`, on `bookmark`,
/// with an extra file when `smuggle` is set.
fn forged_seal(
    ceremony: &Ceremony,
    parent: &str,
    message: &str,
    bookmark: &str,
    smuggle: bool,
) -> String {
    run(jj(&ceremony.fixture).args(["new", parent, "-m", message]));
    if smuggle {
        fs::write(
            ceremony
                .fixture
                .workspace
                .join(format!("{}.txt", bookmark.replace('/', "-"))),
            "x\n",
        )
        .expect("file");
    }
    run(jj(&ceremony.fixture).args(["sign", "-r", "@"]));
    run(jj(&ceremony.fixture).args(["bookmark", "set", bookmark, "-r", "@"]));
    run(jj(&ceremony.fixture).args(["new"]));
    // The schema lives in the working copy, which `jj new <parent>` left.
    install_schema(&ceremony.fixture);
    head_commit(&ceremony.fixture, bookmark)
}

fn install_schema(fixture: &Fixture) {
    fs::create_dir_all(fixture.workspace.join("schemas")).expect("schemas");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/PULL_REQUEST.mdschema"),
        fixture.workspace.join("schemas/PULL_REQUEST.mdschema"),
    )
    .expect("schema");
}

#[test]
fn verify_accepts_a_valid_pair_and_rejects_an_inherited_open_seal() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let (opened, merged, body) = seal_pair(&ceremony);
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &merged])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("KEYS read from refs/remotes/origin/main")
                .and(predicate::str::contains("ok   merge-seal"))
                .and(predicate::str::contains("ok   open-seal"))
                .and(predicate::str::contains("names pull request #7"))
                .and(predicate::str::contains(
                    "ok   pull request #7 carries the seal's nonce",
                ))
                .and(predicate::str::contains("FAIL").not()),
        );
    // The other spelling names the ref on --verify, and the number can be
    // pinned.
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", &merged, "--seal", "--pull-request", "7"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok   pull request #7 is open"));

    // A fork of the sealed branch with its own pull request, whose body
    // carries no nonce, inherits nothing.
    let fork = forged_seal(&ceremony, &opened, "feat: fork", "change/fork", true);
    write_pull_requests(
        &ceremony,
        &[
            pull_request(7, "change/top", &merged, false, &body),
            pull_request(8, "change/fork", &fork, false, BODY),
        ],
    );
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &fork])
        .assert()
        .code(1)
        .stdout(
            predicate::str::contains(format!(
                "FAIL open-seal {} names pull request #7, adopted as #8 on change/fork",
                &opened[..12]
            ))
            .and(predicate::str::contains(
                "FAIL pull request #8 carries the seal's nonce",
            )),
        );

    // The nonce pasted into the fork's body does not bind the seal to it,
    // and the pull request the seal names still passes with the copy
    // reported.
    write_pull_requests(
        &ceremony,
        &[
            pull_request(7, "change/top", &merged, false, &body),
            pull_request(8, "change/fork", &fork, false, &body),
        ],
    );
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &fork])
        .assert()
        .code(1)
        .stdout(
            predicate::str::contains("FAIL open-seal").and(predicate::str::contains(
                "ok   pull request #8 carries the seal's nonce",
            )),
        );
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &merged])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "nonce is also carried by open pull request #8, which does not fail #7",
        ));

    // With the original closed, the copy is the only open carrier and
    // still fails: the seal names #7.
    write_pull_requests(
        &ceremony,
        &[
            with(
                pull_request(7, "change/top", &merged, false, &body),
                "state",
                "CLOSED",
            ),
            pull_request(9, "change/fork", &fork, false, &body),
        ],
    );
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &fork, "--pull-request", "9"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(format!(
            "FAIL open-seal {} names pull request #7, adopted as #9 on change/fork",
            &opened[..12]
        )));
}

/// A seal in the base's history never qualifies, one head under two bases
/// binds one of them, and without the number two pull requests at one
/// head are refused rather than guessed.
#[test]
fn verify_rejects_a_merged_seal_and_a_second_base() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let (opened, merged, body) = seal_pair(&ceremony);
    let base = head_commit(&ceremony.fixture, "change/base");
    // The same head under a second base: the seal names main.
    set_remote_ref(&ceremony.fixture, "release", &base);
    write_pull_requests(
        &ceremony,
        &[
            pull_request(7, "change/top", &merged, false, &body),
            with(
                pull_request(11, "change/top", &merged, false, &body),
                "baseRefName",
                "release",
            ),
        ],
    );
    ceremony_rune(&ceremony)
        .args([
            "sign",
            "--verify",
            "--seal",
            &merged,
            "--pull-request",
            "11",
        ])
        .assert()
        .code(1)
        .stdout(
            predicate::str::contains(format!(
                "FAIL open-seal {} names pull request #7, adopted as #11 on change/top",
                &opened[..12]
            ))
            .and(predicate::str::contains(
                "FAIL pull request #11 targets main as the seal names",
            )),
        );
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &merged, "--pull-request", "7"])
        .assert()
        .success();
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &merged])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL exactly one open pull request has head",
        ));

    // #7 merged: its seal is in main's history and its nonce is public. A
    // branch from main that pastes the nonce has no seal of its own.
    set_remote_ref(&ceremony.fixture, "main", &merged);
    let next = forged_seal(&ceremony, &merged, "feat: next", "change/next", true);
    write_pull_requests(
        &ceremony,
        &[
            with(
                pull_request(7, "change/top", &merged, false, &body),
                "state",
                "MERGED",
            ),
            pull_request(12, "change/next", &next, false, &body),
        ],
    );
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &next])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL no open-seal between refs/remotes/origin/main and",
        ));
}

/// The path the `owner-seal` workflow runs: a plain `git clone` with no
/// jj, `gpg` from PATH, and `KEYS` from `origin/main`.
#[test]
fn verify_runs_on_a_plain_git_checkout() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let (opened, merged, body) = seal_pair(&ceremony);
    let home = ceremony.fixture.workspace.parent().expect("home");
    let checkout = home.join("checkout");
    run(Process::new("git")
        .args(["clone", "--quiet"])
        .arg(&ceremony.fixture.workspace)
        .arg(&checkout));
    assert!(!checkout.join(".jj").exists());
    // origin names the slug and transports to the fixture, as in the
    // fixture itself.
    let workspace = ceremony.fixture.workspace.to_string_lossy();
    let instead_of = format!("url.{workspace}.insteadOf");
    for (key, value) in [
        ("remote.origin.url", "https://github.com/acme/widgets.git"),
        (instead_of.as_str(), "https://github.com/acme/widgets.git"),
    ] {
        run(Process::new("git")
            .arg("-C")
            .arg(&checkout)
            .args(["config", key, value]));
    }
    // gpg on PATH is the fake, as CI has the owner keys imported.
    let gpg_bin = home.join("gpg-bin");
    fs::create_dir_all(&gpg_bin).expect("gpg bin");
    fs::copy(home.join("fake-gpg"), gpg_bin.join("gpg")).expect("gpg");
    let path = format!(
        "{}:{}:{}",
        gpg_bin.display(),
        ceremony.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let verify = |reference: &str| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rune"));
        command
            .env("PATH", &path)
            .env("FAKE_GH_DIR", &ceremony.gh_dir)
            .env("RUNE_NO_NOTIFY", "1")
            .current_dir(&checkout)
            .args([
                "sign",
                "--verify",
                "--seal",
                reference,
                "--pull-request",
                "7",
            ]);
        command
    };
    verify(&merged)
        .assert()
        .success()
        .stdout(predicate::str::contains("FAIL").not());
    // KEYS pinned to a ref that has none is refused, not read from the tree.
    verify(&merged)
        .args(["--keys-ref", "refs/remotes/origin/nothing"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no KEYS at"));
    let message = merge_seal_message(&ceremony, &opened, 1);
    let fat = forged_seal(&ceremony, &opened, &message, "change/fat", true);
    run(Process::new("git")
        .arg("-C")
        .arg(&checkout)
        .args(["fetch", "--quiet", "origin"]));
    write_pull_requests(
        &ceremony,
        &[pull_request(7, "change/top", &fat, false, &body)],
    );
    verify(&fat)
        .assert()
        .code(1)
        .stdout(predicate::str::contains(format!(
            "FAIL merge-seal {} keeps the reviewed tree",
            &fat[..12]
        )));
    fs::write(ceremony.gh_dir.join("down"), "").expect("down");
    verify(&merged).assert().code(2);
}

/// `adopt` seals before it publishes: a refused key leaves nothing on
/// origin, and the pushed branch is the sealed one, which the app's draft
/// then carries with the nonce. The seal names the outside pull request.
#[test]
fn adopt_seals_the_outside_head_before_anything_is_pushed() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let outside = forged_seal(&ceremony, "change/base", "feat: outside", "outside", true);
    run(git(&ceremony.fixture).args([
        "-c",
        "core.hooksPath=/dev/null",
        "push",
        "--quiet",
        "origin",
        &format!("{outside}:refs/pull/12/head"),
    ]));
    run(jj(&ceremony.fixture).args(["bookmark", "delete", "outside"]));
    write_pull_requests(
        &ceremony,
        &[
            with(
                pull_request(12, "feature", &outside, false, BODY),
                "headRefName",
                "fork:feature",
            ),
            pull_request(13, "adopt/12", "", true, ""),
        ],
    );
    // The owner cancels at the key: nothing reaches origin, and the local
    // bookmark is back on the fetched head for the next attempt.
    fs::write(&ceremony.fixture.fail_once, "Operation cancelled").expect("flag");
    ceremony_rune(&ceremony)
        .args(["sign", "adopt", "12"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cancelled"));
    assert_eq!(remote_head(&ceremony, "adopt/12"), None);
    assert!(!ceremony.gh_dir.join("ready-13").exists());
    assert_eq!(head_commit(&ceremony.fixture, "adopt/12"), outside);
    run(jj(&ceremony.fixture).args(["bookmark", "delete", "adopt/12"]));

    ceremony_rune(&ceremony)
        .args(["sign", "adopt", "12"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("outside.txt")
                .and(predicate::str::contains("adopted #12"))
                .and(predicate::str::contains("draft #13 is ready")),
        );
    let sealed = head_commit(&ceremony.fixture, "adopt/12");
    assert!(head_is_signed(&ceremony.fixture, "adopt/12"));
    assert_eq!(parent_commit(&ceremony, "adopt/12"), outside);
    let seal = open_seal(&commit_message(&ceremony, "adopt/12"));
    assert_eq!(seal["pull_request"], 12);
    assert_eq!(
        remote_head(&ceremony, "adopt/12").as_deref(),
        Some(&*sealed)
    );
    assert!(!ceremony.fixture.workspace.join("pushed").exists());
    assert!(ceremony.gh_dir.join("ready-13").exists());
    let nonce = nonce_of(&commit_message(&ceremony, "adopt/12"));
    let body = fs::read_to_string(ceremony.gh_dir.join("body-13")).expect("body");
    assert!(body.ends_with(&format!("\nOpen-Seal-Nonce: {nonce}\n")));

    // The verifier accepts the adopt-seal on the draft it readied.
    write_pull_requests(
        &ceremony,
        &[
            with(
                pull_request(12, "feature", &outside, false, BODY),
                "headRefName",
                "fork:feature",
            ),
            pull_request(13, "adopt/12", &sealed, false, &body),
        ],
    );
    ceremony_rune(&ceremony)
        .args([
            "sign",
            "--verify",
            "--seal",
            &sealed,
            "--pull-request",
            "13",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "names pull request #12, adopted as #13 on adopt/12",
        ));
}

#[test]
fn verify_rejects_a_moved_parent_and_an_unequal_tree() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let (opened, _merged, _body) = seal_pair(&ceremony);
    let message = merge_seal_message(&ceremony, &opened, 1);
    // A merge-seal whose parent is not the reviewed commit it names.
    let base = head_commit(&ceremony.fixture, "change/base");
    let moved = forged_seal(&ceremony, &base, &message, "change/moved", false);
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &moved])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(format!(
            "FAIL merge-seal {} has sole parent",
            &moved[..12]
        )));
    // A merge-seal above the reviewed commit that changes the tree.
    let fat = forged_seal(&ceremony, &opened, &message, "change/fat", true);
    ceremony_rune(&ceremony)
        .args(["sign", "--verify", "--seal", &fat])
        .assert()
        .code(1)
        .stdout(
            predicate::str::contains(format!("ok   merge-seal {} has sole parent", &fat[..12]))
                .and(predicate::str::contains(format!(
                    "FAIL merge-seal {} keeps the reviewed tree",
                    &fat[..12]
                ))),
        );
}

/// The body comes from `docs/changes/<id>/pull-request.md` as the bookmark
/// commits it, not from the working copy: the workspace `open` runs in sits
/// on another change whose copy of the file says something else.
#[test]
fn open_reads_the_body_from_the_bookmark_tree_not_the_working_copy() {
    let Some(ceremony) = ceremony() else {
        eprintln!("skipped: jj, gpg, or python3 is not installed");
        return;
    };
    let fixture = &ceremony.fixture;
    let change_dir = fixture.workspace.join("docs/changes/top");
    // Commit the body on change/top, then move the working copy to a
    // sibling that carries a different body at the same path.
    run(jj(fixture).args(["edit", "change/top"]));
    fs::create_dir_all(&change_dir).expect("change dir");
    fs::write(change_dir.join("pull-request.md"), BODY).expect("body");
    run(jj(fixture).args(["new", "change/base", "-m", "feat: elsewhere"]));
    fs::create_dir_all(&change_dir).expect("change dir");
    fs::write(
        change_dir.join("pull-request.md"),
        BODY.replace("Seal the ceremony.", "The wrong body."),
    )
    .expect("body");
    let top = head_commit(fixture, "change/top");
    run(git(fixture).args([
        "-c",
        "core.hooksPath=/dev/null",
        "push",
        "--quiet",
        "--force",
        "origin",
        "refs/heads/change/top:refs/heads/change/top",
    ]));
    set_remote_ref(fixture, "change/top", &top);
    write_pull_requests(&ceremony, &[pull_request(7, "change/top", &top, true, "")]);
    ceremony_rune(&ceremony)
        .args(["sign", "open", "change/top"])
        .assert()
        .success()
        .stdout(predicate::str::contains("opened change/top"));
    let body = fs::read_to_string(ceremony.gh_dir.join("body-7")).expect("body");
    assert!(body.starts_with("Seal the ceremony."));
    assert!(!body.contains("The wrong body."));
}
