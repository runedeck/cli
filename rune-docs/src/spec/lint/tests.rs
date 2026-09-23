use super::*;
use crate::spec::terms::GLOSSARY_FILE;
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
fn defined_term_needs_a_glossary_entry() {
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
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(diagnostics.len(), 2, "{messages:?}");
    assert!(messages[0].contains("'search index' needs a glossary"));
    assert!(messages[0].contains("docs/specs/glossary.md"));
    assert!(messages[1].contains("'scan'"));
    assert!(
        !messages.iter().any(|message| message.contains("Rationale")),
        "labels ending in a colon are emphasis, not definitions"
    );

    std::fs::write(
        specs.join(GLOSSARY_FILE),
        "# Glossary\n\n- **Search Index**: the prebuilt term table.\n",
    )
    .unwrap();
    let mut diagnostics = Vec::new();
    lint_canonical(
        target(root.path(), &path),
        &content,
        &Terms::load(root.path(), &specs).unwrap(),
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "spec-term-undefined");
    assert!(diagnostics[0].message.contains("'scan' has no entry"));
    assert_eq!(diagnostics[0].line, Some(5));
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
fn a_plain_plural_finds_its_singular_entry() {
    let root = TempDir::new().unwrap();
    let specs = root.path().join("docs/specs");
    std::fs::create_dir_all(&specs).unwrap();
    std::fs::write(
        specs.join(GLOSSARY_FILE),
        "# Glossary\n\n- **trailer**: a key-value line in the commit footer.\n",
    )
    .unwrap();
    let path = specs.join("search/spec.md");
    let content = CLEAN.replace("Find runes.", "Attribute through *trailers*.");
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
    assert_eq!(name_words("sign-before-publish-guard"), 4);
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
"#;

fn ontology_root() -> TempDir {
    let root = TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("ontology")).unwrap();
    std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
    std::fs::write(root.path().join("ontology/rune.ttl"), ONTOLOGY).unwrap();
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

#[test]
fn an_ontology_replaces_the_glossary_as_the_term_source() {
    let root = ontology_root();
    std::fs::write(
        root.path().join("docs/specs").join(GLOSSARY_FILE),
        "# Glossary\n\n- **scan**: a full read.\n",
    )
    .unwrap();

    let diagnostics = lint_with_ontology(
        &root,
        "Every *harness* [HARNESS] runs a *scan*.\n\n[HARNESS]: https://runedeck.ai/ns#Harness",
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "spec-term-undefined");
    assert!(
        diagnostics[0]
            .message
            .contains("'scan' is not in ontology/rune.ttl")
    );
}

#[test]
fn a_cited_first_use_with_its_definition_passes_and_later_uses_need_no_tag() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "Every *harness* [HARNESS] loads a *canon* [CANON]. The *harnesses* share the canon.\n\n[HARNESS]: https://runedeck.ai/ns#Harness\n[CANON]: https://runedeck.ai/ns#Canon",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn a_first_use_without_a_tag_names_the_tag_and_the_definition_to_add() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(&root, "Every *harness* loads instructions.");

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "term-reference-missing");
    assert_eq!(diagnostics[0].line, Some(5));
    assert!(
        diagnostics[0].message.contains(
            "write `*harness* [HARNESS]` and define `[HARNESS]: https://runedeck.ai/ns#Harness`"
        ),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn a_tag_defined_to_another_iri_or_not_defined_is_unresolved() {
    let root = ontology_root();
    let wrong = lint_with_ontology(
        &root,
        "Every *harness* [HARNESS] loads a *canon* [CANON].\n\n[HARNESS]: https://runedeck.ai/ns#Canon",
    );
    let codes: Vec<_> = wrong.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(
        codes,
        ["term-reference-unresolved", "term-reference-unresolved"],
        "{wrong:?}"
    );
    assert!(
        wrong[0]
            .message
            .contains("[HARNESS] must be defined as `[HARNESS]: https://runedeck.ai/ns#Harness`")
    );
    assert!(wrong[1].message.contains("[CANON] must be defined as"));
}

#[test]
fn a_definition_under_the_namespace_must_name_a_term() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "No terms here.\n\n[GHOST]: https://runedeck.ai/ns#Ghost\n[OTHER]: https://example.org/elsewhere",
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "term-reference-unresolved");
    assert!(
        diagnostics[0]
            .message
            .contains("[GHOST] names https://runedeck.ai/ns#Ghost, which is not a term")
    );
}

#[test]
fn a_document_outside_the_tree_gets_reference_rules_only_and_no_capability() {
    let root = ontology_root();
    let path = root.path().join("docs/decisions/CLI-0001 Example.md");
    let mut diagnostics = Vec::new();
    lint_document(
        LintTarget {
            repository: root.path(),
            path: &path,
            capability: None,
            change: None,
        },
        "# Example\n\nThe *harness* and the *widget*.\n",
        &Terms::load(root.path(), &root.path().join("docs/specs")).unwrap(),
        &mut diagnostics,
    );
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["term-reference-missing"], "{diagnostics:?}");
    assert!(diagnostics.iter().all(|d| d.capability.is_none()));
}

#[test]
fn front_matter_and_plain_emphasis_are_not_terms_in_a_document() {
    let root = ontology_root();
    let path = root.path().join("runes/core/skills/Example/SKILL.md");
    let mut diagnostics = Vec::new();
    lint_document(
        LintTarget {
            repository: root.path(),
            path: &path,
            capability: None,
            change: None,
        },
        "---\nallowed-tools: Bash(git add:*), Bash(git diff:*)\n---\n\nDo *not* skip the *harness* [HARNESS] step.\n\n[HARNESS]: https://runedeck.ai/ns#Harness\n",
        &Terms::load(root.path(), &root.path().join("docs/specs")).unwrap(),
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn adjacent_italic_runs_escaped_asterisks_and_lowercase_tags_follow_markdown() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "*harness* [harness] *canon* [CANON] \\*not a term\\* here.\n\n[harness]: <https://runedeck.ai/ns#Harness>\n[CANON]: https://runedeck.ai/ns#Canon\n",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let missing_second = lint_with_ontology(
        &root,
        "*harness* [HARNESS] *canon* here.\n\n[HARNESS]: https://runedeck.ai/ns#Harness\n",
    );
    let codes: Vec<_> = missing_second.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["term-reference-missing"], "{missing_second:?}");
    assert!(missing_second[0].message.contains("*canon* [CANON]"));
}

#[test]
fn the_first_definition_wins_and_indented_definitions_count() {
    let root = ontology_root();
    let diagnostics = lint_with_ontology(
        &root,
        "A *harness* [HARNESS] runs.\n\n   [HARNESS]: https://runedeck.ai/ns#Canon\n[HARNESS]: https://runedeck.ai/ns#Harness\n",
    );
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["term-reference-unresolved"], "{diagnostics:?}");
    assert_eq!(diagnostics[0].line, Some(5));
}

#[test]
fn tilde_fences_and_closed_front_matter_are_skipped_but_an_unclosed_block_is_prose() {
    let root = ontology_root();
    let path = root.path().join("docs/decisions/X.md");
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
    assert!(lint("---\ntitle: *harness*\n...\n\n~~~\n*harness*\n~~~\n").is_empty());
    let unclosed = lint("---\ntitle: x\n\nThe *harness* here.\n");
    assert_eq!(unclosed.len(), 1, "{unclosed:?}");
    assert_eq!(unclosed[0].code, "term-reference-missing");
}
