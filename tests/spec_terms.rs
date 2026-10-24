//! `rune spec glossary`, `rune spec fix`, the term check `rune validate`
//! runs over decision records and runes when the root has an ontology,
//! and the archive refusal of a draft term.
mod support;
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

const ONTOLOGY: &str = r#"@prefix rune: <https://runedeck.ai/ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

rune:Harness a rdfs:Class ;
    rdfs:label "harness" ;
    rdfs:comment "A coding agent that loads instructions." .

rune:change a rdf:Property ;
    rdfs:label "change" ;
    rdfs:comment "The change a record belongs to." .

rune:Change a rdfs:Class ;
    rdfs:label "change" ;
    rdfs:comment "One spec-driven unit of work." .
"#;

const DRAFT_COMMENT: &str = "Draft term. The fixer added it and nobody has defined it yet. Archive refuses a change that links to it.";

fn rune() -> Command {
    support::rune()
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

/// A module with its glossary pages written.
fn module_with_pages(rule: &str) -> tempfile::TempDir {
    let module = tempfile::tempdir().unwrap();
    write_module(module.path(), rule);
    rune()
        .args(["spec", "glossary", "--write", "--source"])
        .arg(module.path())
        .assert()
        .success();
    module
}

#[test]
fn glossary_prints_terms_with_tags_and_page_definitions_from_the_ontology() {
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
            "- **change**: The change a record belongs to. [CHANGE-KEY]\n",
        ))
        .stdout(predicate::str::contains(
            "[HARNESS]: docs/specs/glossary/harness.md\n",
        ));

    let output = rune()
        .args(["spec", "glossary", "--json", "--source"])
        .arg(module.path())
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["source"], "ontology/rune.ttl");
    assert_eq!(value["glossary"], "docs/specs/glossary");
    assert_eq!(value["terms"][0]["tag"], "CHANGE");
    assert_eq!(value["terms"][1]["page"], "change-key.md");
    assert_eq!(value["terms"][2]["iri"], "https://runedeck.ai/ns#Harness");
    assert_eq!(value["terms"][2]["status"], "defined");
}

#[test]
fn glossary_write_then_check_is_current_and_a_hand_edit_is_drift() {
    let module = tempfile::tempdir().unwrap();
    write_module(module.path(), "A rule.\n");
    std::fs::write(
        module.path().join("docs/specs/glossary.md"),
        "# Glossary\n\nold single file\n",
    )
    .unwrap();

    rune()
        .args(["spec", "glossary", "--write", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(
            "wrote docs/specs/glossary: 3 page(s) and the index, 4 file(s) changed\n  removed  docs/specs/glossary.md\n",
        );
    assert!(!module.path().join("docs/specs/glossary.md").exists());
    let page = module.path().join("docs/specs/glossary/harness.md");
    assert_eq!(
        std::fs::read_to_string(&page).unwrap(),
        "---\ntitle: harness\niri: https://runedeck.ai/ns#Harness\nkind: class\nstatus: defined\n---\n\n# harness\n\nA coding agent that loads instructions.\n\nGenerated from `ontology/rune.ttl`. Edit the ontology, not this page.\n"
    );
    assert_eq!(
        std::fs::read_to_string(module.path().join("docs/specs/glossary/index.md")).unwrap(),
        "# Glossary\n\nGenerated from `ontology/rune.ttl`. Edit the ontology, not these pages.\n\n- [change](change.md): One spec-driven unit of work.\n- [change](change-key.md): The change a record belongs to.\n- [harness](harness.md): A coding agent that loads instructions.\n"
    );

    rune()
        .args(["spec", "glossary", "--check", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout("docs/specs/glossary is current\n");

    let text = std::fs::read_to_string(&page).unwrap();
    std::fs::write(
        &page,
        text.replace("A coding agent that loads instructions.", "Edited."),
    )
    .unwrap();
    rune()
        .args(["spec", "glossary", "--check", "--source"])
        .arg(module.path())
        .assert()
        .code(1)
        .stdout(
            "docs/specs/glossary is stale: run `rune spec glossary --write`\n  changed  harness (harness.md)\n",
        );
}

#[test]
fn glossary_check_without_a_directory_exits_one_and_the_flags_do_not_combine() {
    let module = tempfile::tempdir().unwrap();
    write_module(module.path(), "A rule.\n");

    rune()
        .args(["spec", "glossary", "--check", "--source"])
        .arg(module.path())
        .assert()
        .code(1)
        .stdout("docs/specs/glossary is missing: run `rune spec glossary --write`\n");

    let output = rune()
        .args(["spec", "glossary", "--check", "--json", "--source"])
        .arg(module.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "missing");
    assert_eq!(value["path"], "docs/specs/glossary");

    rune()
        .args(["spec", "glossary", "--write", "--check", "--source"])
        .arg(module.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn validate_reports_a_plain_first_use_and_ignores_emphasis_in_a_rule() {
    let module = module_with_pages(
        "Every [harness][HARNESS] loads this rule. A *widget* is emphasis, *not* a term.\n\n[HARNESS]: ../docs/specs/glossary/harness.md\n",
    );

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("rules/Terms.md"))
        .stdout(predicate::str::contains(
            "first use of [harness][HARNESS] in this file is not emphasized",
        ))
        .stdout(predicate::str::contains("widget").not());
}

#[test]
fn validate_passes_a_rule_that_links_its_terms_and_warns_on_the_legacy_form() {
    let module = module_with_pages(
        "Every *[harness][HARNESS]* loads this rule, and every [harness][HARNESS] again.\n\n[HARNESS]: ../docs/specs/glossary/harness.md\n",
    );

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ terms"));

    std::fs::write(
        module.path().join("rules/Terms.md"),
        "Every *harness* [HARNESS] loads this rule.\n\n[HARNESS]: https://runedeck.ai/ns#Harness\n",
    )
    .unwrap();
    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("1 warning"))
        .stdout(predicate::str::contains(
            "`*harness* [HARNESS]` is the legacy form; write `*[harness][HARNESS]*` at first use, `[harness][HARNESS]` later, and define `[HARNESS]: ../docs/specs/glossary/harness.md`",
        ));
}

#[test]
fn validate_skips_the_term_check_without_an_ontology() {
    let module = tempfile::tempdir().unwrap();
    write_module(
        module.path(),
        "Every [harness][HARNESS] loads this rule.\n\n[HARNESS]: ../docs/specs/glossary/harness.md\n",
    );
    std::fs::remove_file(module.path().join("ontology/rune.ttl")).unwrap();

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("terms").not());
}

#[test]
fn fix_links_migrates_the_legacy_form_and_fix_terms_drafts_an_unknown_term() {
    let module = module_with_pages(
        "Every *harness* [HARNESS] loads a *[gadget][GADGET]*; the *harness* [HARNESS] again.\n\n[HARNESS]: https://runedeck.ai/ns#Harness\n[GADGET]: ../docs/specs/glossary/gadget.md\n",
    );

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "term link [gadget][GADGET] names no term: the page is missing and no term in ontology/rune.ttl has the tag; run `rune spec fix --terms` to add a draft",
        ));

    rune()
        .args(["spec", "fix", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(
            "rewrote rules/Terms.md\nadded draft 'gadget' as Gadget (docs/specs/glossary/gadget.md) from rules/Terms.md:1\nwrote ontology/rune.ttl and docs/specs/glossary\n",
        );
    assert_eq!(
        std::fs::read_to_string(module.path().join("rules/Terms.md")).unwrap(),
        "Every *[harness][HARNESS]* loads a *[gadget][GADGET]*; the [harness][HARNESS] again.\n\n[HARNESS]: ../docs/specs/glossary/harness.md\n[GADGET]: ../docs/specs/glossary/gadget.md\n"
    );
    let ontology = std::fs::read_to_string(module.path().join("ontology/rune.ttl")).unwrap();
    assert!(ontology.ends_with(
        "rune:Gadget a rdfs:Class ;\n    rdfs:label \"gadget\" ;\n    rune:status \"draft\" .\n"
    ));
    let page =
        std::fs::read_to_string(module.path().join("docs/specs/glossary/gadget.md")).unwrap();
    assert!(page.contains("status: draft\n"));
    assert!(page.contains(DRAFT_COMMENT));

    rune()
        .args(["validate", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "term 'gadget' is a draft; define it in ontology/rune.ttl before archive",
        ));

    rune()
        .args(["spec", "fix", "--source"])
        .arg(module.path())
        .assert()
        .success()
        .stdout("nothing to fix\n");
}

#[test]
fn archive_refuses_a_change_that_links_to_a_draft_term() {
    let module = module_with_pages("A rule.\n");
    let change = module.path().join("docs/changes/add-the-gadget-rules");
    std::fs::create_dir_all(change.join("specs/gadget-rules")).unwrap();
    std::fs::write(change.join("proposal.md"), "# Change\n").unwrap();
    std::fs::write(change.join("tasks.md"), "- [x] done\n").unwrap();
    std::fs::write(
        change.join("specs/gadget-rules/spec.md"),
        "## ADDED Requirements\n\n### Requirement: Gadget Rules\n\nA *[gadget][GADGET]* MUST spin, and a [phantom][PHANTOM] too.\n\n#### Scenario: Gadget spins\n\n- **WHEN** a gadget exists\n- **THEN** it spins\n\n[GADGET]: ../../../../specs/glossary/gadget.md\n",
    )
    .unwrap();
    rune()
        .args(["spec", "fix", "--terms", "--source"])
        .arg(module.path())
        .assert()
        .success();

    rune()
        .args(["spec", "archive", "add-the-gadget-rules", "--source"])
        .arg(module.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "change 'add-the-gadget-rules' links to terms that are not settled:",
        ))
        .stderr(predicate::str::contains(
            "draft 'gadget' as [gadget][GADGET]: docs/changes/add-the-gadget-rules/specs/gadget-rules/spec.md:5",
        ))
        .stderr(predicate::str::contains(
            "undefined [phantom][PHANTOM]: docs/changes/add-the-gadget-rules/specs/gadget-rules/spec.md:5",
        ));
    assert!(change.is_dir());
}
