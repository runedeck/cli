//! Integration tests for source-side `rune provenance`.
//!
//! A source repository carries `.provenance/*.yaml` sidecars next to the
//! artifacts they describe. `rune provenance --target <repo|subdir>` walks
//! those sidecars, resolves each subject back to a repo-relative file, and
//! recomputes its SHA-256 (plus any in-repo dependency digests).

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn scaffold_adopted_skill(repo: &Path, body: &str, subject_digest: &str) {
    fs::write(repo.join("module.yaml"), "name: fixture\n").unwrap();
    let skill_dir = repo.join("skills/Adopted");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), body).unwrap();

    let provenance_dir = skill_dir.join(".provenance");
    fs::create_dir_all(&provenance_dir).unwrap();
    let yaml = format!(
        "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: skills/Adopted/SKILL.md\n          digest:\n              sha256: {subject_digest}\n    predicate:\n        buildDefinition:\n            buildType: https://github.com/runedeck/rune/adopt/v1\n            externalParameters:\n                upstream_url: https://example.test/upstream\n            resolvedDependencies:\n                - name: upstream\n                  uri: https://example.test/upstream\n                  digest:\n                      sha256: deadbeef\n"
    );
    fs::write(provenance_dir.join("SKILL.yaml"), yaml).unwrap();
}

#[test]
fn provenance_source_verifies_matching_subject() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));

    rune()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn provenance_source_fails_on_stale_subject() {
    let repo = tempfile::tempdir().unwrap();
    // Sidecar records the digest of the original body; the file on disk differs.
    scaffold_adopted_skill(
        repo.path(),
        "edited after adoption\n",
        &sha256("original\n"),
    );

    rune()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicates::str::contains(
            "skills/Adopted/SKILL.md → stale (content changed since adoption)",
        ));
}

#[test]
fn provenance_source_scopes_to_subdirectory() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));

    rune()
        .args([
            "provenance",
            "--target",
            repo.path().join("skills/Adopted").to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn provenance_source_json_reports_subject_and_source() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));

    let output = rune()
        .args([
            "provenance",
            "--target",
            repo.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).into_owned();
    assert!(
        stdout.contains("\"subject\": \"skills/Adopted/SKILL.md\""),
        "json must name the subject: {stdout}"
    );
    assert!(
        stdout.contains("https://example.test/upstream"),
        "json must carry the resolved source: {stdout}"
    );
}

#[test]
fn provenance_source_fails_on_stale_subject_name() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));
    // A hand `mv` leaves the sidecar naming the old holder.
    fs::rename(
        repo.path().join("skills/Adopted"),
        repo.path().join("skills/Moved"),
    )
    .unwrap();

    rune()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicates::str::contains(
            "subject name disagrees with holder",
        ));
}

#[test]
fn repair_dry_run_json_is_pure_and_lists_the_pending_source_write() {
    // The adopt scanner reads reviewed `adopt/v1` sidecars named for the
    // file they cover, which is what a finalized session writes.
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("module.yaml"), "name: fixture\n").unwrap();
    let skill_dir = repo.path().join("skills/Adopted");
    fs::create_dir_all(skill_dir.join(".provenance")).unwrap();
    let body = "Adopted skill body.\n";
    fs::write(skill_dir.join("SKILL.md"), body).unwrap();
    let yaml = format!(
        "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: skills/Adopted/SKILL.md\n          digest:\n              sha256: {}\n    predicate:\n        buildDefinition:\n            buildType: adopt/v1\n            externalParameters:\n                upstream_url: https://example.test/upstream\n            resolvedDependencies:\n                - name: upstream\n                  uri: https://example.test/upstream\n                  digest:\n                      sha256: deadbeef\n        runDetails:\n            metadata:\n                review: reviewed\n",
        sha256(body)
    );
    fs::write(skill_dir.join(".provenance/SKILL.md.yaml"), yaml).unwrap();
    fs::rename(&skill_dir, repo.path().join("skills/Moved")).unwrap();

    let output = rune()
        .current_dir(repo.path())
        .args(["repair", "--root", ".", "--dry-run", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "a pending source fault is not clean"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout is not pure JSON ({error}):\n{stdout}"));
    let repairs = report["source_repairs"].as_array().unwrap();
    assert_eq!(repairs.len(), 1);
    assert_eq!(repairs[0]["action"], "would rename subject");
    assert_eq!(
        repairs[0]["sidecar"],
        "skills/Moved/.provenance/SKILL.md.yaml"
    );
    assert_eq!(repairs[0]["subject"], "skills/Adopted/SKILL.md");
    assert_eq!(repairs[0]["destination"], "skills/Moved/SKILL.md");
    let sidecar =
        fs::read_to_string(repo.path().join("skills/Moved/.provenance/SKILL.md.yaml")).unwrap();
    assert!(
        sidecar.contains("name: skills/Adopted/SKILL.md"),
        "a dry run rewrites nothing"
    );

    let human = rune()
        .current_dir(repo.path())
        .args(["repair", "--root", ".", "--dry-run"])
        .output()
        .unwrap();
    let text = String::from_utf8(human.stdout).unwrap();
    assert_eq!(
        text.matches("would rename subject").count(),
        1,
        "each write is listed once:\n{text}"
    );
}

#[test]
fn repair_reports_an_unresolved_digest_fault_and_exits_nonzero() {
    // Repair rewrites names and prunes orphans; it never endorses edited
    // bytes. A reviewed digest that no longer matches stays an error in the
    // repair report and in its exit, with the reseal command named.
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("module.yaml"), "name: fixture\n").unwrap();
    let skill_dir = repo.path().join("skills/Adopted");
    fs::create_dir_all(skill_dir.join(".provenance")).unwrap();
    let body = "Adopted skill body.\n";
    fs::write(skill_dir.join("SKILL.md"), body).unwrap();
    let yaml = format!(
        "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: skills/Adopted/SKILL.md\n          digest:\n              sha256: {}\n    predicate:\n        buildDefinition:\n            buildType: adopt/v1\n            externalParameters:\n                upstream_url: https://example.test/upstream\n            resolvedDependencies:\n                - name: upstream\n                  uri: https://example.test/upstream\n                  digest:\n                      sha256: deadbeef\n        runDetails:\n            metadata:\n                review: reviewed\n",
        sha256(body)
    );
    fs::write(skill_dir.join(".provenance/SKILL.md.yaml"), yaml).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "Edited after review.\n").unwrap();

    for dry_run in [true, false] {
        let mut args = vec!["repair", "--root", ".", "--json"];
        if dry_run {
            args.push("--dry-run");
        }
        let output = rune()
            .current_dir(repo.path())
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "an unresolved digest fault is never a clean repair (dry_run={dry_run})"
        );
        let report: serde_json::Value =
            serde_json::from_str(&String::from_utf8(output.stdout).unwrap()).unwrap();
        assert_eq!(report["source_repairs"].as_array().unwrap().len(), 0);
        let faults = report["source_faults"].as_array().unwrap();
        assert_eq!(faults.len(), 1);
        let fault = faults[0].as_str().unwrap();
        assert!(fault.contains("digest does not match"), "{fault}");
        assert!(fault.contains("rune adopt reseal"), "{fault}");
    }
}

#[test]
fn provenance_source_accepts_deployment_evidence_beside_sidecars() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));
    // `rune install` writes this JSON record beside sidecars under a
    // provider target; a module tree that carries one must still verify.
    let snapshot = repo.path().join("skills/.provenance/source-snapshot.json");
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    fs::write(
        &snapshot,
        format!(
            r#"{{"version":"rune-provider-source-snapshot/v1","source":{{"version":"rune-source-snapshot/v1","digest":"{d}","roots":["deck"]}},"selected_skills":{{"skills/Adopted":"{d}"}},"model_override":null}}"#,
            d = "c".repeat(64)
        ),
    )
    .unwrap();

    rune()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .success();

    fs::write(&snapshot, "{torn").unwrap();
    rune()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicates::str::contains("invalid source snapshot"));
}

#[test]
fn provenance_source_resolves_a_relative_target_from_the_parent_directory() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    let module = repo.path().join("module");
    fs::create_dir_all(&module).unwrap();
    scaffold_adopted_skill(&module, body, &sha256(body));

    // A relative `--target` from a parent directory must resolve the holder
    // file once, never re-join it onto the module root.
    rune()
        .current_dir(repo.path())
        .args(["provenance", "--target", "module"])
        .assert()
        .success()
        .stdout(predicates::str::contains("missing source file").not());
}
