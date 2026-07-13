use assert_cmd::Command;
use std::fs;

mod support;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn assert_domain_order(output: &[u8]) {
    let stdout = String::from_utf8_lossy(output);
    let science = stdout
        .find("== science ==")
        .expect("science domain heading");
    let writing = stdout
        .find("== writing ==")
        .expect("writing domain heading");
    assert!(science < writing, "domains must be lexicographic: {stdout}");
}

fn assert_domain_prefix_order(output: &[u8]) {
    let stdout = String::from_utf8_lossy(output);
    let science = stdout.find("science/").expect("science domain item");
    let writing = stdout.find("writing/").expect("writing domain item");
    assert!(science < writing, "domains must be lexicographic: {stdout}");
}

#[test]
fn aggregate_operations_visit_both_domains_in_order() {
    let deck = support::deck_fixture();
    let target = tempfile::tempdir().unwrap();

    let validate = rune()
        .args(["validate", "--source", deck.to_str().unwrap()])
        .assert()
        .failure();
    assert_domain_prefix_order(&validate.get_output().stdout);

    let provenance = rune()
        .args(["provenance", "--target", deck.to_str().unwrap()])
        .assert()
        .success();
    assert_domain_order(&provenance.get_output().stdout);

    let clean = rune()
        .args([
            "clean",
            "--source",
            deck.to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_domain_order(&clean.get_output().stdout);
}

#[test]
fn aggregate_drift_reports_all_domains_and_fails_for_one_domain() {
    let source = tempfile::tempdir().unwrap();
    let upstream = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(source.path());
    support::copy_deck_fixture(upstream.path());
    fs::write(
        upstream
            .path()
            .join("runes/science/skills/OnlyScience/SKILL.md"),
        "changed upstream\n",
    )
    .unwrap();

    let drift = rune()
        .args([
            "drift",
            "--source",
            source.path().to_str().unwrap(),
            "--upstream",
            upstream.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
    let stdout = String::from_utf8_lossy(&drift.get_output().stdout);
    let science = stdout.find("science/").expect("science domain report");
    let writing = stdout.find("writing/").expect("writing domain report");
    assert!(science < writing, "domains must be lexicographic: {stdout}");
}

#[test]
fn cast_subset_drift_is_clean_until_a_deployed_file_changes() {
    let deck = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(deck.path());
    fs::write(
        consumer.path().join(".rune"),
        format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: science\n",
            deck.path().display()
        ),
    )
    .unwrap();

    rune()
        .args([
            "install",
            "--source",
            consumer.path().to_str().unwrap(),
            "--target",
            consumer.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let clean_drift = rune()
        .args([
            "drift",
            "--source",
            deck.path().to_str().unwrap(),
            "--target",
            consumer.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&clean_drift.get_output().stderr);
    assert!(
        !stderr.contains("config key `<root>` has incompatible types"),
        "an absent deck config must merge silently: {stderr}"
    );

    let deployed = consumer.path().join(".claude/skills/OnlyScience/SKILL.md");
    fs::write(&deployed, "locally edited\n").unwrap();

    let drift = rune()
        .args([
            "drift",
            "--source",
            deck.path().to_str().unwrap(),
            "--target",
            consumer.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
    let stdout = String::from_utf8_lossy(&drift.get_output().stdout);
    assert!(
        stdout.contains("OnlyScience"),
        "real drift must be named: {stdout}"
    );
}

#[test]
fn empty_module_defaults_merge_without_a_warning() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    fs::write(
        module.path().join("module.yaml"),
        "name: empty-defaults\nversion: 0.1.0\ndescription: fixture\nevents: []\n",
    )
    .unwrap();
    fs::write(module.path().join("defaults.yaml"), "").unwrap();
    fs::create_dir(module.path().join("rules")).unwrap();
    fs::write(module.path().join("rules/Rule.md"), "Rule body.\n").unwrap();

    let install = rune()
        .args([
            "install",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&install.get_output().stderr);
    assert!(
        !stderr.contains("config key `<root>` has incompatible types"),
        "empty defaults must merge silently: {stderr}"
    );
}

#[test]
fn deck_release_requires_and_packages_one_domain() {
    let deck = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(deck.path());

    let missing = rune()
        .args(["release", "--source", deck.path().to_str().unwrap()])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&missing.get_output().stderr);
    assert!(
        stderr.contains("requires a domain argument"),
        "deck release must explain the missing domain: {stderr}"
    );

    rune()
        .args([
            "release",
            "science",
            "--source",
            deck.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let dist = deck.path().join("runes/science/dist");
    assert!(
        fs::read_dir(&dist).unwrap().flatten().any(|entry| entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "gz")),
        "the selected module must use the existing release packaging path"
    );
    assert!(
        !deck.path().join("runes/writing/dist").exists(),
        "unselected domains must not be released"
    );
}
