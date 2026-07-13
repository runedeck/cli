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

#[test]
fn aggregate_operations_visit_both_domains_in_order() {
    let deck = support::deck_fixture();
    let target = tempfile::tempdir().unwrap();

    let validate = rune()
        .args(["validate", "--source", deck.to_str().unwrap()])
        .assert()
        .failure();
    assert_domain_order(&validate.get_output().stdout);

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
            "version: 1\nsources:\n  deck:\n    local: {}\nartifacts:\n  deck:\n    cast: science\n",
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

    rune()
        .args([
            "drift",
            "--source",
            deck.path().to_str().unwrap(),
            "--target",
            consumer.path().to_str().unwrap(),
        ])
        .assert()
        .success();

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
