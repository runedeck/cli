use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

/// The exporter emits records, relations, rules, verdicts, and one
/// source path per input file, so duplicate identifiers stay visible.
#[test]
fn graph_export_emits_the_artifact_graph() {
    let dir = tempfile::tempdir().expect("tempdir");
    let decisions = dir.path().join("docs/decisions");
    fs::create_dir_all(&decisions).expect("decisions dir");
    fs::write(
        decisions.join("DECK-0001 Example.md"),
        "---\ntitle: Example Decision\nrelated:\n    - \"DECK-0002 Other\"\n---\n\n# Example\n",
    )
    .expect("record one");
    fs::write(
        decisions.join("DECK-0002 Other.md"),
        "---\ntitle: Other\n---\n\n# Other\n",
    )
    .expect("record two");
    fs::write(
        decisions.join("DECK-0002 Overlap.md"),
        "---\ntitle: Overlap\n---\n\n# Overlap\n",
    )
    .expect("duplicate record");

    let rules = dir.path().join("runes/core/rules");
    fs::create_dir_all(&rules).expect("rules dir");
    fs::write(
        rules.join("TestRule.md"),
        "---\ntitle: Test Rule\nmetadata:\n    verdict: benchmarks/test-rule\n---\n\nBody.\n",
    )
    .expect("rule");

    Command::cargo_bin("rune")
        .expect("binary")
        .args(["graph", "export", "--source"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "<https://runedeck.dev/id/DECK-0001> a rune:DecisionRecord",
        ))
        .stdout(predicate::str::contains(
            "dcterms:title \"Example Decision\"",
        ))
        .stdout(predicate::str::contains(
            "dcterms:relation <https://runedeck.dev/id/DECK-0002>",
        ))
        .stdout(predicate::str::contains(
            "rune:sourcePath \"docs/decisions/DECK-0002 Other.md\"",
        ))
        .stdout(predicate::str::contains(
            "rune:sourcePath \"docs/decisions/DECK-0002 Overlap.md\"",
        ))
        .stdout(predicate::str::contains(
            "<https://runedeck.dev/id/TestRule> a rune:Rule",
        ))
        .stdout(predicate::str::contains(
            "rune:verdict <https://runedeck.dev/id/TestRule.verdict>",
        ))
        .stdout(predicate::str::contains(
            "dcterms:identifier \"benchmarks/test-rule\"",
        ));
}
