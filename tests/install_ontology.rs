//! `rune install` copies the deck's ontology into a consumer when `.rune`
//! names an `ontology` source, with a sidecar and a manifest, so `rune spec
//! validate` in the consumer reads the deck's terms from the default path.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

mod support;

fn rune() -> Command {
    support::rune()
}

fn fixture_deck() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/install-ontology/deck")
}

fn fixture_ontology() -> Vec<u8> {
    fs::read(fixture_deck().join("ontology/rune.ttl")).unwrap()
}

fn write_dotrune(consumer: &Path, source: &Path, ontology: Option<&str>) {
    let mut body = format!(
        "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    rules: [CiteTerms]\n",
        source.display()
    );
    if let Some(ontology) = ontology {
        body.push_str("ontology: ");
        body.push_str(ontology);
        body.push('\n');
    }
    fs::write(consumer.join(".rune"), body).unwrap();
}

fn install(consumer: &Path, extra: &[&str]) -> assert_cmd::assert::Assert {
    rune()
        .args(["install", "--source"])
        .arg(consumer)
        .arg("--target")
        .arg(consumer)
        .args(extra)
        .assert()
}

/// One change in the consumer whose delta specification links a term the
/// fixture ontology defines, through the term's glossary page.
fn write_change_citing_a_term(consumer: &Path) {
    let change = consumer.join("docs/changes/cite-the-deck-instance");
    fs::create_dir_all(change.join("specs/deployed-copy-records-provenance")).unwrap();
    fs::write(
        change.join("proposal.md"),
        "---\nstatus: proposed\n---\n\n# Cite the deck instance\n\n## Why\n\nA reader meets a term.\n\n## What Changes\n\n- Cite it.\n\n## Capabilities\n\n- deployed-copy-records-provenance (new)\n\n## Impact\n\n- The specification.\n",
    )
    .unwrap();
    fs::write(change.join("tasks.md"), "## 1. Work\n\n- [ ] 1.1 Cite\n").unwrap();
    fs::write(
        change.join("specs/deployed-copy-records-provenance/spec.md"),
        "## ADDED Requirements\n\n### Requirement: Copy carries a record\n\nEvery *[instance][INSTANCE]* MUST carry a record.\n\n#### Scenario: Copy lands\n\n- **WHEN** a copy lands\n- **THEN** its record exists\n\n[INSTANCE]: ../../../../specs/glossary/instance.md\n",
    )
    .unwrap();
}

#[test]
fn install_copies_the_ontology_with_a_sidecar_and_validate_reads_the_copy() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(consumer.path(), &fixture_deck(), Some("deck"));
    write_change_citing_a_term(consumer.path());

    // Without the copy the consumer defines no terms, so the link is an
    // ordinary link and nothing is checked.
    rune()
        .args(["spec", "validate", "cite-the-deck-instance", "--source"])
        .arg(consumer.path())
        .assert()
        .success();

    install(consumer.path(), &[])
        .success()
        .stdout(predicate::str::contains("ontology"));

    // With the copy the tag names the term, and its page is missing.
    rune()
        .args(["spec", "validate", "cite-the-deck-instance", "--source"])
        .arg(consumer.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "glossary page docs/specs/glossary/instance.md is missing",
        ));
    rune()
        .args(["spec", "glossary", "--write", "--source"])
        .arg(consumer.path())
        .assert()
        .success();

    let copy = consumer.path().join("ontology/rune.ttl");
    assert_eq!(fs::read(&copy).unwrap(), fixture_ontology());
    let sidecar = consumer.path().join("ontology/.provenance/rune.ttl.yaml");
    let statement = rune::manifest::provenance::read(&sidecar)
        .unwrap()
        .provenance;
    assert_eq!(statement.subject[0].name, "ontology/rune.ttl");
    assert_eq!(
        statement.subject[0].digest.sha256,
        rune::manifest::content_sha256_bytes(&fixture_ontology())
    );
    assert_eq!(
        statement.predicate.build_definition.resolved_dependencies[0].uri,
        "ontology/rune.ttl"
    );
    let manifest = fs::read_to_string(consumer.path().join("ontology/.manifest")).unwrap();
    let entries = rune::manifest::read(&manifest).unwrap();
    assert_eq!(
        entries["rune.ttl"].fingerprint,
        rune::manifest::content_sha256_bytes(&fixture_ontology())
    );
    assert_eq!(
        entries["rune.ttl"].provenance.as_deref(),
        Some(".provenance/rune.ttl.yaml")
    );

    rune()
        .args(["spec", "validate", "cite-the-deck-instance", "--source"])
        .arg(consumer.path())
        .assert()
        .success();
}

#[test]
fn install_without_the_key_deploys_no_ontology() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(consumer.path(), &fixture_deck(), None);

    install(consumer.path(), &[]).success();

    assert!(
        consumer.path().join(".claude/rules/CiteTerms.md").is_file(),
        "the requested rule still deploys"
    );
    assert!(!consumer.path().join("ontology").exists());
}

#[test]
fn an_ontology_key_naming_no_source_is_refused() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(consumer.path(), &support::deck_fixture(), Some("elsewhere"));

    install(consumer.path(), &[])
        .failure()
        .stderr(predicate::str::contains("ontology 'elsewhere'"))
        .stderr(predicate::str::contains("sources"));
    assert!(!consumer.path().join("ontology").exists());
}

#[test]
fn a_source_without_an_ontology_file_is_refused_before_anything_deploys() {
    let consumer = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    support::copy_deck_fixture(source.path());
    fs::write(
        consumer.path().join(".rune"),
        format!(
            "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    include: ['science/skills/OnlyScience']\nontology: deck\n",
            source.path().display()
        ),
    )
    .unwrap();

    install(consumer.path(), &[])
        .failure()
        .stderr(predicate::str::contains("ontology 'deck'"))
        .stderr(predicate::str::contains("source 'deck'"))
        .stderr(predicate::str::contains("ontology/rune.ttl"));
    assert!(!consumer.path().join("ontology").exists());
    assert!(
        !consumer.path().join(".claude/skills").exists(),
        "no provider tree is written when the ontology source is unusable"
    );
}

#[test]
fn a_hand_edited_copy_is_refused_without_force_and_drift_and_status_report_it() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotrune(consumer.path(), &fixture_deck(), Some("deck"));
    install(consumer.path(), &[]).success();
    let copy = consumer.path().join("ontology/rune.ttl");
    fs::write(&copy, "# edited by hand\n").unwrap();

    rune()
        .args(["drift"])
        .current_dir(consumer.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("rune.ttl"));
    rune()
        .args(["drift", "--source"])
        .arg(consumer.path())
        .arg("--target")
        .arg(consumer.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("ontology/rune.ttl"));
    let status = rune()
        .args(["status", "--json", "--source"])
        .arg(consumer.path())
        .output()
        .unwrap();
    let dashboard: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let target = dashboard["deploy_targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["name"] == "ontology")
        .expect("status lists the ontology as a deploy target");
    assert_eq!(target["stale"], 1);
    assert_eq!(target["ok"], 0);

    install(consumer.path(), &[])
        .success()
        .stdout(predicate::str::contains("user modified"));
    assert_eq!(fs::read_to_string(&copy).unwrap(), "# edited by hand\n");

    install(consumer.path(), &["--force"]).success();
    assert_eq!(fs::read(&copy).unwrap(), fixture_ontology());
    rune()
        .args(["drift"])
        .current_dir(consumer.path())
        .assert()
        .success();
}
