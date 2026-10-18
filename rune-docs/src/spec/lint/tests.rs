use super::*;
use tempfile::TempDir;

fn target<'target>(repository: &'target Path, path: &'target Path) -> LintTarget<'target> {
    LintTarget {
        repository,
        path,
        capability: Some("rune-name-search"),
        change: None,
    }
}

const CLEAN: &str = "# Search Specification\n\n## Purpose\n\nFind runes.\n\n## Requirements\n\n### Requirement: Existing\n\nThe system MUST retain this behavior.\n\n#### Scenario: Existing behavior\n\n- **WHEN** search runs\n- **THEN** results appear\n";

#[test]
fn shall_is_an_error_with_its_line() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let content = CLEAN.replace("MUST retain", "SHALL retain");
    let mut diagnostics = Vec::new();

    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "spec-shall-keyword");
    assert_eq!(diagnostics[0].line, Some(11));
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostics[0].path, "docs/specs/search/spec.md");
}

#[test]
fn shall_inside_a_fence_is_quoted_output_not_prose() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let content = format!(
        "{CLEAN}\nThe parser accepts `SHALL` and `MUST` as *keywords*, quoted here.\n\n```text\nThe tool SHALL echo.\n```\n"
    )
    .replace("*keywords*", "`*keywords*`");
    let mut diagnostics = Vec::new();

    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn canonical_over_the_limit_errors_and_delta_warns() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let padding = "\nMore prose.\n".repeat(MAX_SPEC_LINES);
    let content = format!("{CLEAN}{padding}");
    let mut diagnostics = Vec::new();

    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );
    lint_delta(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );

    let codes: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code.as_str(), diagnostic.severity))
        .collect();
    assert_eq!(
        codes,
        [
            ("spec-too-long", DiagnosticSeverity::Error),
            ("delta-too-long", DiagnosticSeverity::Warning),
        ]
    );
    assert!(diagnostics[0].message.contains("limit is 150"));
}

#[test]
fn exactly_the_limit_passes() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let body_lines = CLEAN.lines().count();
    let content = format!(
        "{CLEAN}{}",
        "\nMore prose.\n".repeat((MAX_SPEC_LINES - body_lines) / 2)
    );
    assert!(content.lines().count() <= MAX_SPEC_LINES);
    let mut diagnostics = Vec::new();

    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn italics_are_emphasis_and_need_no_term() {
    let root = TempDir::new().unwrap();
    let specs = root.path().join("docs/specs");
    std::fs::create_dir_all(&specs).unwrap();
    let path = specs.join("search/spec.md");
    let content = CLEAN.replace(
        "Find runes.",
        "Find runes through the *search index*, never a *scan*. *Rationale:* speed.",
    );

    let mut diagnostics = Vec::new();
    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::load(root.path(), &specs).unwrap(),
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn bold_scenario_keywords_are_not_terms() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let mut diagnostics = Vec::new();

    lint_canonical(
        target(root.path(), &path),
        CLEAN,
        &Terms::default(),
        &mut diagnostics,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn must_passes_the_house_rule_and_shall_fails_it() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let mut clean = Vec::new();
    lint_canonical(
        target(root.path(), &path),
        CLEAN,
        &Terms::default(),
        &mut clean,
    );
    assert!(clean.is_empty(), "MUST is the house keyword: {clean:?}");

    let mut flagged = Vec::new();
    lint_delta(
        target(root.path(), &path),
        "## ADDED Requirements\n\n### Requirement: Legacy\n\nThe tool SHALL keep parsing upstream artifacts.\n\n#### Scenario: Upstream artifact\n\n- **WHEN** an OpenSpec delta says SHALL\n- **THEN** the parser accepts it and this lint reports it\n",
        &Terms::default(),
        &mut flagged,
    );
    let lines: Vec<_> = flagged
        .iter()
        .map(|diagnostic| (diagnostic.code.as_str(), diagnostic.line))
        .collect();
    assert_eq!(
        lines,
        [
            ("delta-shall-keyword", Some(5)),
            ("delta-shall-keyword", Some(9))
        ]
    );
}

#[test]
fn a_requirement_over_the_word_cap_is_an_error_at_its_heading() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let wall = "The system MUST do one thing and then another thing. ".repeat(12);
    let content = CLEAN.replace("The system MUST retain this behavior.", wall.trim());
    let mut diagnostics = Vec::new();

    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "spec-requirement-too-long");
    assert_eq!(diagnostics[0].line, Some(9));
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(
        diagnostics[0].message.contains("120 words"),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn a_step_over_the_word_cap_is_an_error_and_inline_code_does_not_count() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/changes/x/specs/search/spec.md");
    let long_step = format!("- **THEN** {}", "a fact ".repeat(16));
    let content = CLEAN.replace("- **THEN** results appear", long_step.trim_end());
    let mut diagnostics = Vec::new();
    lint_delta(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "delta-step-too-long");
    assert_eq!(diagnostics[0].line, Some(16));

    // Code spans are blanked before counting, so a command-heavy step passes.
    let coded = format!("- **THEN** `{}` runs", "word ".repeat(40).trim_end());
    let content = CLEAN.replace("- **THEN** results appear", &coded);
    let mut diagnostics = Vec::new();
    lint_delta(
        target(root.path(), &path),
        &content,
        &Terms::default(),
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn short_capability_and_change_names_are_errors_by_the_default_rule() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/changes/add-x/specs/search/spec.md");
    let mut diagnostics = Vec::new();
    lint_delta(
        LintTarget {
            repository: root.path(),
            path: &path,
            capability: Some("search"),
            change: Some("add-x"),
        },
        CLEAN,
        &Terms::default(),
        &mut diagnostics,
    );
    let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"delta-capability-name-short"), "{codes:?}");
    assert!(codes.contains(&"change-name-short"), "{codes:?}");
    assert_eq!(name_words("sign-before-publishing-online"), 4);
    assert_eq!(name_words("a--b"), 2);
}

const ONTOLOGY: &str = r#"@prefix rune: <https://runedeck.ai/ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

rune:Harness a rdfs:Class ;
    rdfs:label "harness" ;
    rdfs:comment "A coding agent that loads instructions." .

rune:Canon a rdfs:Class ;
    rdfs:label "canon" ;
    rdfs:comment "The authored source of one instruction." .

rune:Ghost a rdfs:Class ;
    rdfs:label "ghost" ;
    rune:status "draft" .
"#;

/// The definitions a specification under `docs/specs/<capability>/` writes.
const DEFINITIONS: &str = "\n\n[HARNESS]: ../glossary/harness.md\n[CANON]: ../glossary/canon.md\n[GHOST]: ../glossary/ghost.md";

/// A repository with the ontology and its generated glossary pages.
fn ontology_root() -> TempDir {
    let root = TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("ontology")).unwrap();
    std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
    std::fs::write(root.path().join("ontology/rune.ttl"), ONTOLOGY).unwrap();
    crate::spec::write_glossary_output(root.path().to_str().unwrap(), None).unwrap();
    root
}

fn lint_with_ontology(root: &TempDir, prose: &str) -> Vec<SpecViolation> {
    let path = root.path().join("docs/specs/search/spec.md");
    let content = CLEAN.replace("Find runes.", prose);
    let mut diagnostics = Vec::new();
    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::load(root.path(), &root.path().join("docs/specs")).unwrap(),
        &mut diagnostics,
    );
    diagnostics
}

fn codes(diagnostics: &[SpecViolation]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn an_emphasized_first_use_and_plain_later_uses_pass() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        &format!(
            "Every *[harness][HARNESS]* loads a *[canon][CANON]*. The [harnesses][HARNESS] share the [canon][CANON].{DEFINITIONS}"
        ),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn plain_italics_and_ordinary_links_are_not_terms() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "A *widget* is *not* a term, *Rationale:* neither, and [the readme][README] is a link.\n\n[README]: ../../README.md",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn a_plain_first_use_and_an_emphasized_later_use_are_errors() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        &format!(
            "A [harness][HARNESS] loads a *[canon][CANON]*. Again *[canon][CANON]*.{DEFINITIONS}"
        ),
    );
    assert_eq!(
        codes(&diagnostics),
        ["spec-term-first-use", "spec-term-first-use"],
        "{diagnostics:?}"
    );
    assert_eq!(diagnostics[0].line, Some(5));
    assert!(
        diagnostics[0]
            .message
            .contains("write `*[harness][HARNESS]*`")
    );
    assert!(
        diagnostics[1]
            .message
            .contains("later use of [canon][CANON]")
    );
    assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Error);
}

#[test]
fn a_link_to_no_term_is_undefined_and_names_the_fixer() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "A *[widget][WIDGET]* here.\n\n[WIDGET]: ../glossary/widget.md",
    );
    assert_eq!(
        codes(&diagnostics),
        ["spec-term-undefined"],
        "{diagnostics:?}"
    );
    assert_eq!(diagnostics[0].line, Some(5));
    assert!(
        diagnostics[0]
            .message
            .contains("[widget][WIDGET] names no term")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("run `rune spec fix --terms`")
    );
}

#[test]
fn a_tag_without_a_definition_is_a_dangling_link() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(&root, "A [phantom][PHANTOM] here.");
    assert_eq!(
        codes(&diagnostics),
        ["spec-term-undefined"],
        "{diagnostics:?}"
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(
        diagnostics[0]
            .message
            .contains("the tag has no definition in this file and no term has the tag")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("add `[PHANTOM]: ../glossary/phantom.md` at the end of the file")
    );
}

#[test]
fn a_draft_term_warns_once_per_file() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        &format!("A *[ghost][GHOST]* and the [ghost][GHOST] again.{DEFINITIONS}"),
    );
    assert_eq!(codes(&diagnostics), ["spec-term-draft"], "{diagnostics:?}");
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert!(diagnostics[0].message.contains("term 'ghost' is a draft"));
}

#[test]
fn a_missing_page_and_a_hand_edited_page_are_page_errors() {
    let root = ontology_root();
    std::fs::remove_file(root.path().join("docs/specs/glossary/canon.md")).unwrap();
    let missing = lint_with_ontology(&root, &format!("A *[canon][CANON]*.{DEFINITIONS}"));
    assert_eq!(codes(&missing), ["spec-term-page"], "{missing:?}");
    assert!(
        missing[0]
            .message
            .contains("glossary page docs/specs/glossary/canon.md is missing")
    );

    let page = root.path().join("docs/specs/glossary/harness.md");
    std::fs::write(
        &page,
        std::fs::read_to_string(&page)
            .unwrap()
            .replace("coding", "code"),
    )
    .unwrap();
    let edited = lint_with_ontology(
        &root,
        &format!("A *[harness][HARNESS]* and [harness][HARNESS].{DEFINITIONS}"),
    );
    assert_eq!(codes(&edited), ["spec-term-page"], "{edited:?}");
    assert!(
        edited[0]
            .message
            .contains("differs from the ontology render")
    );
}

#[test]
fn a_known_tag_without_a_definition_names_the_definition_to_add() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(&root, "A *[harness][HARNESS]* loads.");
    assert_eq!(codes(&diagnostics), ["spec-term-page"], "{diagnostics:?}");
    assert!(
        diagnostics[0]
            .message
            .contains("add `[HARNESS]: ../glossary/harness.md` at the end of the file")
    );
}

#[test]
fn the_legacy_form_and_an_iri_definition_warn_and_name_the_new_form() {
    let root = ontology_root();
    let legacy = lint_with_ontology(
        &root,
        "Every *harness* [HARNESS] loads.\n\n[HARNESS]: https://runedeck.ai/ns#Harness",
    );
    assert_eq!(codes(&legacy), ["spec-term-legacy"], "{legacy:?}");
    assert_eq!(legacy[0].severity, DiagnosticSeverity::Warning);
    assert!(legacy[0].message.contains(
        "write `*[harness][HARNESS]*` at first use, `[harness][HARNESS]` later, and define `[HARNESS]: ../glossary/harness.md`"
    ));

    let iri = lint_with_ontology(
        &root,
        "A *[canon][CANON]* and [canon][CANON].\n\n[CANON]: <https://runedeck.ai/ns#Canon>",
    );
    assert_eq!(codes(&iri), ["spec-term-legacy"], "{iri:?}");
    assert!(
        iri[0]
            .message
            .contains("point it at the page: `[CANON]: ../glossary/canon.md`")
    );
}

#[test]
fn a_definition_under_the_namespace_must_name_a_term() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "No terms here.\n\n[NOPE]: https://runedeck.ai/ns#Nope\n[OTHER]: https://example.org/elsewhere",
    );
    assert_eq!(
        codes(&diagnostics),
        ["spec-term-undefined"],
        "{diagnostics:?}"
    );
    assert_eq!(diagnostics[0].line, Some(7));
    assert!(
        diagnostics[0]
            .message
            .contains("[NOPE] names https://runedeck.ai/ns#Nope, which is not a term")
    );
}

#[test]
fn a_delta_reports_the_delta_twins() {
    let root = ontology_root();
    let path = root
        .path()
        .join("docs/changes/add-search-index/specs/search/spec.md");
    let content = CLEAN.replace(
        "Find runes.",
        "A [harness][HARNESS] and a *[ghost][GHOST]*.\n\n[HARNESS]: ../../../../specs/glossary/harness.md\n[GHOST]: ../../../../specs/glossary/ghost.md",
    );
    let mut diagnostics = Vec::new();
    lint_delta(
        LintTarget {
            repository: root.path(),
            path: &path,
            capability: Some("search-index-rules"),
            change: Some("add-search-index"),
        },
        &content,
        &Terms::load(root.path(), &root.path().join("docs/specs")).unwrap(),
        &mut diagnostics,
    );
    assert_eq!(
        codes(&diagnostics),
        ["delta-term-first-use", "delta-term-draft"],
        "{diagnostics:?}"
    );
}

#[test]
fn a_document_outside_the_tree_gets_the_term_rules_only_and_no_capability() {
    let root = ontology_root();
    let path = root.path().join("docs/decisions/CLI-0001 Example.md");
    let terms = Terms::load(root.path(), &root.path().join("docs/specs")).unwrap();
    let lint = |content: &str| {
        let mut diagnostics = Vec::new();
        lint_document(
            LintTarget {
                repository: root.path(),
                path: &path,
                capability: None,
                change: None,
            },
            content,
            &terms,
            &mut diagnostics,
        );
        diagnostics
    };
    let clean = lint(
        "# Example\n\nThe *[harness][HARNESS]* and the *widget*.\n\n[HARNESS]: ../specs/glossary/harness.md\n",
    );
    assert!(clean.is_empty(), "{clean:?}");
    let plain =
        lint("# Example\n\nThe [harness][HARNESS].\n\n[HARNESS]: ../specs/glossary/harness.md\n");
    assert_eq!(codes(&plain), ["spec-term-first-use"], "{plain:?}");
    assert!(plain.iter().all(|d| d.capability.is_none()));
}

#[test]
fn front_matter_fences_and_images_are_not_term_links() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        &format!(
            "![harness][HARNESS] then *[harness][HARNESS]* ok.\n\n```md\n[canon][CANON]\n```\n\nThe *[canon][CANON]*.{DEFINITIONS}"
        ),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let path = root.path().join("docs/decisions/X.md");
    let terms = Terms::load(root.path(), &root.path().join("docs/specs")).unwrap();
    let mut front = Vec::new();
    lint_document(
        LintTarget {
            repository: root.path(),
            path: &path,
            capability: None,
            change: None,
        },
        "---\ntitle: [harness][HARNESS]\n---\n\n*[harness][HARNESS]* first.\n\n[HARNESS]: ../specs/glossary/harness.md\n",
        &terms,
        &mut front,
    );
    assert!(front.is_empty(), "{front:?}");
}

#[test]
fn a_repository_without_an_ontology_checks_no_links() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("docs/specs/search/spec.md");
    let mut diagnostics = Vec::new();
    lint_canonical(
        target(root.path(), &path),
        &CLEAN.replace(
            "Find runes.",
            "A [harness][HARNESS].\n\n[HARNESS]: ../glossary/harness.md",
        ),
        &Terms::default(),
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
