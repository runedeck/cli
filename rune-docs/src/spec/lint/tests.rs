use super::*;
use tempfile::TempDir;

fn target<'target>(repository: &'target Path, path: &'target Path) -> LintTarget<'target> {
    LintTarget {
        repository,
        path,
        capability: "rune-name-search",
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
        &Glossary::default(),
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
        &Glossary::default(),
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
        &Glossary::default(),
        &mut diagnostics,
    );
    lint_delta(
        target(root.path(), &path),
        &content,
        &Glossary::default(),
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
        &Glossary::default(),
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
        &Glossary::load(root.path(), &specs),
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
        &Glossary::load(root.path(), &specs),
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
        &Glossary::default(),
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
        &Glossary::default(),
        &mut clean,
    );
    assert!(clean.is_empty(), "MUST is the house keyword: {clean:?}");

    let mut flagged = Vec::new();
    lint_delta(
        target(root.path(), &path),
        "## ADDED Requirements\n\n### Requirement: Legacy\n\nThe tool SHALL keep parsing upstream artifacts.\n\n#### Scenario: Upstream artifact\n\n- **WHEN** an OpenSpec delta says SHALL\n- **THEN** the parser accepts it and this lint reports it\n",
        &Glossary::default(),
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
        &Glossary::load(root.path(), &specs),
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
        &Glossary::default(),
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
        &Glossary::default(),
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
        &Glossary::default(),
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
            capability: "search",
            change: Some("add-x"),
        },
        CLEAN,
        &Glossary::default(),
        &mut diagnostics,
    );
    let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"delta-capability-name-short"), "{codes:?}");
    assert!(codes.contains(&"change-name-short"), "{codes:?}");
    assert_eq!(name_words("sign-before-publish-guard"), 4);
    assert_eq!(name_words("a--b"), 2);
}
