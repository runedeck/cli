//! Integration test for `rune drift --target`: verify a module's assembled
//! build against where it was deployed, scoped to the module's own files.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn scaffold_module(root: &Path) {
    fs::write(
        root.join("module.yaml"),
        "name: drift-fixture\nversion: 0.1.0\ndescription: drift target fixture\nevents: []\nrepository: https://github.com/example/drift-fixture\n",
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
    let rules = root.join("rules");
    fs::create_dir_all(&rules).unwrap();
    fs::write(
        rules.join("KeepDeployed.md"),
        "---\nname: KeepDeployed\ndescription: a rule\n---\n\nOriginal body.\n",
    )
    .unwrap();
}

fn install(module: &Path, target: &Path) {
    rune()
        .args([
            "install",
            "--source",
            module.to_str().unwrap(),
            "--target",
            target.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_target_clean_after_install() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());

    rune()
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_target_detects_edited_deployment() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());

    // Edit the deployed rule out from under the build.
    let deployed_rule = target.path().join(".claude/rules/KeepDeployed.md");
    let edited = fs::read_to_string(&deployed_rule).unwrap() + "\nUser appended a line.\n";
    fs::write(&deployed_rule, edited).unwrap();

    rune()
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stdout(predicates::str::contains("rules/KeepDeployed.md"))
        .stdout(predicates::str::contains("drifted"));
}

#[test]
fn drift_target_ignores_other_modules_in_deployment() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());

    // A foreign file with no provenance attribution to this module. Scoped
    // verify must not flag it; the result stays clean.
    fs::write(
        target.path().join(".claude/rules/Foreign.md"),
        "---\nname: Foreign\n---\nfrom another module\n",
    )
    .unwrap();

    rune()
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_without_a_mode_errors_when_no_provider_manifest_is_discoverable() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    rune()
        .current_dir(target.path())
        .args(["drift", "--source", module.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no provider target"));
}

#[test]
fn drift_without_a_mode_ignores_manifest_directories() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    fs::create_dir_all(target.path().join(".claude/.manifest")).unwrap();

    rune()
        .current_dir(target.path())
        .args(["drift", "--source", module.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no provider target"));
}

#[test]
fn drift_without_a_mode_discovers_and_reports_provider_targets_in_pwd() {
    let module = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), module.path());
    fs::remove_dir_all(module.path().join("build")).unwrap();

    let output = rune()
        .current_dir(module.path())
        .args(["drift", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    for provider in ["claude", "codex", "gemini", "opencode"] {
        assert!(
            stdout.contains(&format!("\"category\": \"{provider}\"")),
            "missing {provider} report: {stdout}"
        );
    }
}

#[test]
fn drift_without_a_mode_detects_modified_and_missing_files_without_build_output() {
    let module = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), module.path());
    fs::remove_dir_all(module.path().join("build")).unwrap();
    fs::write(
        module.path().join(".claude/rules/KeepDeployed.md"),
        "changed after install\n",
    )
    .unwrap();
    fs::remove_file(module.path().join(".codex/rules/KeepDeployed.md")).unwrap();

    let output = rune()
        .current_dir(module.path())
        .args(["drift", "--json"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("\"category\": \"claude\""), "{stdout}");
    assert!(stdout.contains("\"status\": \"BodyOnly\""), "{stdout}");
    assert!(stdout.contains("\"category\": \"codex\""), "{stdout}");
    assert!(stdout.contains("\"status\": \"UpstreamOnly\""), "{stdout}");
}

#[test]
fn drift_without_a_mode_reports_only_targets_with_regular_manifests() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());
    for provider_target in [".codex", ".gemini", ".opencode"] {
        fs::remove_file(target.path().join(provider_target).join(".manifest")).unwrap();
    }

    let output = rune()
        .current_dir(target.path())
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("\"category\": \"claude\""), "{stdout}");
    for provider in ["codex", "gemini", "opencode"] {
        assert!(
            !stdout.contains(&format!("\"category\": \"{provider}\"")),
            "unexpected {provider} report: {stdout}"
        );
    }
}
