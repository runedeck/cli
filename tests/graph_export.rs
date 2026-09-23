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
            "<https://runedeck.ai/id/DECK-0001> a rune:DecisionRecord",
        ))
        .stdout(predicate::str::contains(
            "dcterms:title \"Example Decision\"",
        ))
        .stdout(predicate::str::contains(
            "dcterms:relation <https://runedeck.ai/id/DECK-0002>",
        ))
        .stdout(predicate::str::contains(
            "rune:sourcePath \"docs/decisions/DECK-0002 Other.md\"",
        ))
        .stdout(predicate::str::contains(
            "rune:sourcePath \"docs/decisions/DECK-0002 Overlap.md\"",
        ))
        .stdout(predicate::str::contains(
            "<https://runedeck.ai/id/TestRule> a rune:Rule",
        ))
        .stdout(predicate::str::contains(
            "rune:verdict <https://runedeck.ai/id/TestRule.verdict>",
        ))
        .stdout(predicate::str::contains(
            "dcterms:identifier \"benchmarks/test-rule\"",
        ));
}

/// With `ontology/context.jsonld` present, every mapped frontmatter key
/// becomes a triple in the form the context states, and the lifecycle
/// nodes (change, draft record, capability, requirement, scenario) join
/// the graph with the IRIs a record's `upstream` reference resolves to.
/// A deck with a context, two records, one change with a delta spec and a
/// draft, and one canonical capability.
fn lifecycle_deck() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    fs::create_dir_all(root.join("ontology")).expect("ontology dir");
    fs::write(
        root.join("ontology/context.jsonld"),
        r#"{"@context": {
            "rune": "https://runedeck.ai/ns#",
            "schema": "https://schema.org/",
            "dcterms": "http://purl.org/dc/terms/",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "title": "dcterms:title",
            "status": {"@id": "rune:status", "@type": "@vocab"},
            "created": {"@id": "schema:dateCreated", "@type": "xsd:date"},
            "author": {"@id": "schema:author", "@type": "@id"},
            "related": {"@id": "dcterms:relation", "@type": "@id", "@container": "@set"},
            "upstream": {"@id": "schema:isBasedOn", "@type": "@id", "@container": "@set"},
            "change": {"@id": "rune:change", "@type": "@id", "@container": "@set"},
            "supersedes": {"@id": "dcterms:replaces", "@type": "@id", "@container": "@set"},
            "description": {"@id": "schema:description"},
            "accepted": "rune:accepted"
        }}"#,
    )
    .expect("context");
    let decisions = root.join("docs/decisions");
    fs::create_dir_all(&decisions).expect("decisions dir");
    fs::write(
        decisions.join("DECK-0002 New Way.md"),
        "---\ntitle: New Way\ndescription: One line\nstatus: accepted\ncreated: 2026-02-19\nauthor: \"@owner\"\nchange: sample-change\nsupersedes: [\"DECK-0001 Old Way\"]\nupstream:\n    - \"DECK-0001 Old Way\"\n    - \"sample-capability#thing-is-checked\"\n    - \"obsidian://open?vault=Atlas&file=Canvas%2FSketch\"\nignored: dropped\n---\n\n# New Way\n",
    )
    .expect("record");
    fs::write(
        decisions.join("DECK-0001 Old Way.md"),
        "---\ntitle: Old Way\nstatus: superseded\n---\n",
    )
    .expect("old record");
    let change = root.join("docs/changes/sample-change");
    fs::create_dir_all(change.join("specs/sample-capability")).expect("change dir");
    fs::write(
        change.join("proposal.md"),
        "---\nadr: \"docs/decisions/DECK-0002 New Way.md\"\ndecisions: [\"DECK-0002 New Way\", \"A draft by title\"]\narchived: 2026-09-20\n---\n\n# Sample\n",
    )
    .expect("proposal");
    fs::write(
        change.join("specs/sample-capability/spec.md"),
        "## ADDED Requirements\n\n### Requirement: Thing Is Checked\n\nBody MUST hold.\n\n```markdown\n### Requirement: Inside a fence\n```\n\n#### Scenario: Thing enters the graph\n\n- **WHEN** x\n- **THEN** y\n\n### Requirement: !!!\n\n#### Scenario: Orphan under an empty slug\n",
    )
    .expect("delta spec");
    fs::write(
        change.join("adr.md"),
        "---\ntitle: Draft decision\nstatus: proposed\n---\n\n# Draft\n",
    )
    .expect("draft");
    let archived = root.join("docs/changes/archive/sample-change");
    fs::create_dir_all(&archived).expect("archived change");
    fs::write(
        archived.join("proposal.md"),
        "---\narchived: 2026-01-01\n---\n",
    )
    .expect("archived proposal");
    fs::write(
        archived.join("adr.md"),
        "---\ntitle: Leftover draft\nstatus: proposed\n---\n",
    )
    .expect("leftover draft");
    fs::create_dir_all(root.join("docs/specs/canonical-capability")).expect("specs dir");
    fs::write(
        root.join("docs/specs/canonical-capability/spec.md"),
        "# Canonical\n\n## Purpose\n\nP.\n\n## Requirements\n\n### Requirement: Stays Canonical\n\nMUST.\n\n#### Scenario: Canonical scenario\n\n- **WHEN** a\n- **THEN** b\n",
    )
    .expect("canonical spec");

    dir
}

#[test]
fn graph_export_reads_the_context_and_the_lifecycle() {
    let dir = lifecycle_deck();
    let root = dir.path();
    let output = Command::cargo_bin("rune")
        .expect("binary")
        .args(["graph", "export", "--source"])
        .arg(root)
        .output()
        .expect("run");
    assert!(output.status.success());
    let turtle = String::from_utf8(output.stdout).expect("utf8");
    for expected in [
        "@prefix schema: <https://schema.org/> .",
        "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .",
        "<https://runedeck.ai/id/DECK-0002> a rune:DecisionRecord ;",
        "    schema:description \"One line\" ;",
        "    rune:status rune:accepted ;",
        "    schema:dateCreated \"2026-02-19\"^^xsd:date ;",
        "    schema:author <https://runedeck.ai/id/%40owner> ;",
        "    rune:change <https://runedeck.ai/id/change/sample-change> ;",
        "    dcterms:replaces <https://runedeck.ai/id/DECK-0001> ;",
        "    schema:isBasedOn <https://runedeck.ai/id/DECK-0001> ;",
        "    schema:isBasedOn <https://runedeck.ai/id/sample-capability#thing-is-checked> ;",
        "    schema:isBasedOn <obsidian://open?vault=Atlas&file=Canvas%2FSketch> ;",
        "<https://runedeck.ai/id/change/sample-change> a rune:Change ;",
        "    rune:archived \"2026-09-20\"^^xsd:date ;",
        "    rune:decisionRecord <https://runedeck.ai/id/DECK-0002> ;",
        "    rune:sourcePath \"docs/changes/sample-change/proposal.md\" .",
        "<https://runedeck.ai/id/change/sample-change#adr> a rune:DecisionRecord ;",
        "    rune:status rune:proposed ;",
        "    rune:change <https://runedeck.ai/id/change/sample-change> ;",
        "<https://runedeck.ai/id/sample-capability> a rune:Capability ;",
        "    rune:sourcePath \"docs/changes/sample-change/specs/sample-capability/spec.md\" .",
        "<https://runedeck.ai/id/sample-capability#thing-is-checked> a rune:Requirement ;",
        "    dcterms:isPartOf <https://runedeck.ai/id/sample-capability> .",
        "<https://runedeck.ai/id/sample-capability#thing-is-checked/thing-enters-the-graph> a rune:Scenario ;",
        "    dcterms:isPartOf <https://runedeck.ai/id/sample-capability#thing-is-checked> .",
        "<https://runedeck.ai/id/canonical-capability> a rune:Capability ;",
        "<https://runedeck.ai/id/canonical-capability#stays-canonical/canonical-scenario> a rune:Scenario ;",
    ] {
        assert!(
            turtle.contains(expected),
            "missing {expected:?} in:\n{turtle}"
        );
    }
    assert!(
        !turtle.contains("ignored"),
        "unmapped key leaked:\n{turtle}"
    );
    assert!(
        !turtle.contains("A draft by title"),
        "a title is not a record id:\n{turtle}"
    );
    assert_eq!(
        turtle.matches("a rune:DecisionRecord ;").count(),
        3,
        "{turtle}"
    );
}

/// Without a context file the output keeps today's shape and no lifecycle
/// node appears for a deck that has none.
#[test]
fn graph_export_without_a_context_is_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let decisions = dir.path().join("docs/decisions");
    fs::create_dir_all(&decisions).expect("decisions dir");
    fs::write(
        decisions.join("DECK-0001 Example.md"),
        "---\ntitle: Example\nstatus: accepted\ncreated: 2026-02-19\n---\n\n# Example\n",
    )
    .expect("record");
    let output = Command::cargo_bin("rune")
        .expect("binary")
        .args(["graph", "export", "--source"])
        .arg(dir.path())
        .output()
        .expect("run");
    let turtle = String::from_utf8(output.stdout).expect("utf8");
    assert_eq!(
        turtle,
        "@prefix rune:    <https://runedeck.ai/ns#> .\n@prefix dcterms: <http://purl.org/dc/terms/> .\n\n<https://runedeck.ai/id/DECK-0001> a rune:DecisionRecord ;\n    dcterms:identifier \"DECK-0001\" ;\n    dcterms:title \"Example\" ;\n    rune:sourcePath \"docs/decisions/DECK-0001 Example.md\" .\n\n"
    );

    // A change with an archived date declares xsd even without a context.
    let change = dir.path().join("docs/changes/dated");
    fs::create_dir_all(&change).expect("change");
    fs::write(
        change.join("proposal.md"),
        "---\narchived: 2026-09-20\n---\n",
    )
    .expect("proposal");
    let turtle = String::from_utf8(
        Command::cargo_bin("rune")
            .expect("binary")
            .args(["graph", "export", "--source"])
            .arg(dir.path())
            .output()
            .expect("run")
            .stdout,
    )
    .expect("utf8");
    assert!(
        turtle.contains("@prefix xsd: <http://www.w3.org/2001/XMLSchema#> ."),
        "{turtle}"
    );
    assert!(
        turtle.contains("<https://runedeck.ai/id/change/dated> a rune:Change ;"),
        "{turtle}"
    );
}
