//! `rune spec glossary` and the term check `rune validate` runs over
//! decision records and runes when the root has an ontology.
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

const ONTOLOGY: &str = r#"@prefix rune: <https://runedeck.ai/ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

rune:Harness a rdfs:Class ;
    rdfs:label "harness" ;
    rdfs:comment "A coding agent that loads instructions." .

rune:change a rdf:Property ;
    rdfs:label "change key" ;
    rdfs:comment "The change a record belongs to." .

rune:Change a rdfs:Class ;
    rdfs:label "change" ;
    rdfs:comment "One spec-driven unit of work." .
"#;

fn rune() -> Command {
    Command::cargo_bin("rune").unwrap()
}

fn write_module(root: &Path, rule: &str) {
    std::fs::write(
        root.join("module.yaml"),
        "name: terms\nversion: 0.1.0\ndescription: test module\nevents: []\n",
    )
    .unwrap();
    std::fs::write(root.join("defaults.yaml"), "{}\n").unwrap();
    std::fs::write(root.join("README.md"), "# Terms\n").unwrap();
    std::fs::write(root.join("LICENSE"), "test license\n").unwrap();
    std::fs::write(root.join(".manifest"), "{}\n").unwrap();
    std::fs::create_dir_all(root.join("rules")).unwrap();
    std::fs::write(root.join("rules/Terms.md"), rule).unwrap();
    std::fs::create_dir_all(root.join("ontology")).unwrap();
    std::fs::write(root.join("ontology/rune.ttl"), ONTOLOGY).unwrap();
    std::fs::create_dir_all(root.join("docs/specs")).unwrap();
}

#[test]
fn glossary_prints_terms_and_reference_definitions_from_the_ontology() {
    let module = tempfile::tempdir().unwrap();
    write_module(module.path(), "A rule.\n");

    rune()
        .args(["spec", "glossary", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "- **change**: One spec-driven unit of work. [CHANGE]\n",
        ))
        .stdout(predicate::str::contains(
            "- **change key**: The change a record belongs to. [CHANGE-KEY]\n",
        ))
        .stdout(predicate::str::contains(
            "[HARNESS]: https://runedeck.ai/ns#Harness\n",
        ));

    let output = rune()
        .args(["spec", "glossary", "--json", "--source"])
        .arg(module.path())
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["source"], "ontology/rune.ttl");
    assert_eq!(value["terms"][0]["tag"], "CHANGE");
    assert_eq!(value["terms"][2]["iri"], "https://runedeck.ai/ns#Harness");
}

#[test]
fn validate_reports_an_uncited_term_and_ignores_plain_emphasis_in_a_rule() {
    let module = tempfile::tempdir().unwrap();
    write_module(
        module.path(),
        "Every *harness* loads this rule. A *widget* is emphasis, *not* a term.\n",
    );

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("rules/Terms.md"))
        .stdout(predicate::str::contains(
            "first use of *harness* has no reference tag",
        ))
        .stdout(predicate::str::contains("widget").not());
}

#[test]
fn validate_passes_a_rule_that_cites_its_terms() {
    let module = tempfile::tempdir().unwrap();
    write_module(
        module.path(),
        "Every *harness* [HARNESS] loads this rule.\n\n[HARNESS]: https://runedeck.ai/ns#Harness\n",
    );

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ terms"));
}

#[test]
fn validate_skips_the_term_check_without_an_ontology() {
    let module = tempfile::tempdir().unwrap();
    write_module(module.path(), "Every *harness* loads this rule.\n");
    std::fs::remove_file(module.path().join("ontology/rune.ttl")).unwrap();

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("terms").not());
}
