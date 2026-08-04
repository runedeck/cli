use super::*;
use crate::parse;

macro_rules! fixture {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/input/",
            $name
        ))
    };
}

macro_rules! expected {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/expected/",
            $name
        ))
    };
}

const RULE_WITH_REFS: &str = fixture!("rule-with-refs.md");
const EXPECTED_STRIPPED: &str = expected!("rule-stripped.md");
const EXPECTED_APPEND: &str = expected!("variant-append-result.md");
const EXPECTED_PREPEND: &str = expected!("variant-prepend-result.md");
const EXPECTED_KEPT: &str = expected!("frontmatter-kept.md");
const VARIANT_APPEND: &str = fixture!("variant-append.md");
const VARIANT_PREPEND: &str = fixture!("variant-prepend.md");
const VARIANT_REPLACE: &str = fixture!("variant-replace.md");
const VARIANT_FRONTMATTER_ONLY: &str = fixture!("variant-frontmatter-only.md");
const VARIANT_UNKNOWN_MODE: &str = fixture!("variant-unknown-mode.md");
const VARIANT_MALFORMED_FRONTMATTER: &str = fixture!("variant-malformed-frontmatter.md");
const VARIANT_MERGE_BASE: &str = fixture!("variant-merge-base.md");
const VARIANT_MERGE_FIELDS: &str = fixture!("variant-merge-fields.md");
const AGENT_BASIC: &str = fixture!("agent-basic.md");
const FRONTMATTER_SIMPLE: &str = fixture!("frontmatter-simple.md");
const NO_FRONTMATTER_BODY: &str = fixture!("no-frontmatter-body.md");
const RULE_BASE: &str = fixture!("rule-base.md");
const REFS_DEFINITION_ONLY: &str = fixture!("refs-definition-only.md");
const REFS_INLINE_AND_DEF: &str = fixture!("refs-inline-and-def.md");
const PLAIN_NO_REFS: &str = fixture!("plain-no-refs.md");

// --- strip_frontmatter ---

#[test]
fn strip_frontmatter_removes_all_when_no_keep_fields() {
    let result = strip_frontmatter(AGENT_BASIC, &[]);
    assert!(!result.contains("---"));
    assert!(!result.contains("name:"));
    assert!(result.contains("This is a test agent"));
}

#[test]
fn strip_frontmatter_keeps_specified_fields() {
    let result = strip_frontmatter(AGENT_BASIC, &["name", "model"]);
    assert!(result.contains("---"));
    assert!(result.contains("name: TestAgent"));
    assert!(result.contains("model: fast"));
    assert!(!result.contains("version:"));
    assert!(!result.contains("description:"));
}

#[test]
fn strip_frontmatter_strips_heading() {
    let result = strip_frontmatter(AGENT_BASIC, &[]);
    assert!(!result.contains("# TestAgent"));
}

#[test]
fn strip_frontmatter_returns_body_without_frontmatter() {
    let result = strip_frontmatter(NO_FRONTMATTER_BODY, &[]);
    assert_eq!(result.trim(), NO_FRONTMATTER_BODY.trim());
}

#[test]
fn strip_frontmatter_empty_keep_fields_strips_fences() {
    let result = strip_frontmatter(FRONTMATTER_SIMPLE, &[]);
    assert!(!result.contains("---"));
    assert!(result.contains("Body text."));
}

#[test]
fn strip_frontmatter_unmatched_keep_fields_strips_all() {
    let result = strip_frontmatter(AGENT_BASIC, &["nonexistent"]);
    assert!(!result.contains("---"));
}

#[test]
fn strip_frontmatter_keeps_specified_fields_case_insensitively() {
    let content = "---\nName: TestAgent\nVERSION: 0.1.0\n---";
    let result = strip_frontmatter(content, &["name"]);
    assert!(result.contains("Name: TestAgent"));
    assert!(!result.contains("VERSION:"));
}

#[test]
fn strip_frontmatter_preserves_block_list_value() {
    let content = concat!(
        "---\n",
        "name: Foo\n",
        "allowed-tools:\n",
        "    - Bash(pass *)\n",
        "    - Read\n",
        "sources: rune-core\n",
        "---\n",
        "Body.\n",
    );
    let result = strip_frontmatter(content, &["name", "allowed-tools"]);
    assert!(result.contains("allowed-tools:"), "key line kept");
    assert!(
        result.contains("- Bash(pass *)"),
        "first list item must survive: {result}"
    );
    assert!(result.contains("- Read"), "second list item must survive");
    assert!(!result.contains("sources:"), "unlisted key dropped");
}

#[test]
fn strip_frontmatter_block_value_of_dropped_key_does_not_leak() {
    let content = concat!(
        "---\n",
        "name: Foo\n",
        "sources:\n",
        "    - rune-core\n",
        "    - rune-dev\n",
        "version: 0.1.0\n",
        "---\n",
        "Body.\n",
    );
    let result = strip_frontmatter(content, &["name", "version"]);
    assert!(result.contains("name: Foo"));
    assert!(result.contains("version: 0.1.0"));
    assert!(!result.contains("sources:"), "dropped key gone");
    assert!(
        !result.contains("rune-dev"),
        "dropped key's list items must not leak: {result}"
    );
}

#[test]
fn map_field_finds_name_after_other_fields() {
    let content = "---\ndescription: test\nname: TestAgent\n---";
    let result = map_field(content, "name", str::to_lowercase);
    assert!(result.contains("name: testagent"));
    assert!(result.contains("description: test"));
}

#[test]
fn map_field_handles_double_quoted_value() {
    let content = "---\nname: \"SecurityArchitect\"\n---";
    let result = map_field(content, "name", str::to_lowercase);
    assert!(
        result.contains("name: securityarchitect"),
        "quoted value should be unwrapped before mapping: {result}"
    );
}

#[test]
fn map_field_handles_single_quoted_value() {
    let content = "---\nname: 'SecurityArchitect'\n---";
    let result = map_field(content, "name", str::to_lowercase);
    assert!(
        result.contains("name: securityarchitect"),
        "single-quoted value should be unwrapped before mapping: {result}"
    );
}

#[test]
fn map_field_returns_unchanged_when_field_missing() {
    let content = "---\ndescription: test\n---\nBody.";
    let result = map_field(content, "name", str::to_lowercase);
    assert_eq!(result, content);
}

// --- references::strip ---

#[test]
fn strip_removes_inline_markers() {
    let result = references::strip(
        "Text with a ref [1] and another [2].\n\n[1]: https://a\n[2]: https://b\n",
    );
    assert_eq!(result, "Text with a ref and another.\n");
}

#[test]
fn strip_keeps_bracketed_prose_without_definitions() {
    let content = "Use [optional] flags and read [1] carefully.\n\n[1]: https://a\n";
    let result = references::strip(content);
    assert_eq!(result, "Use [optional] flags and read carefully.\n");
}

#[test]
fn strip_keeps_callout_after_reference_block() {
    let content = "Body. [1]\n\n[1]: https://a\n\n[!NOTE] survives\n";
    let result = references::strip(content);
    assert_eq!(result, "Body.\n\n[!NOTE] survives\n");
}

#[test]
fn strip_removes_definition_lines() {
    let result = references::strip(REFS_DEFINITION_ONLY);
    assert_eq!(result, "Body text.\n");
}

#[test]
fn strip_removes_both_markers_and_definitions() {
    let result = references::strip(REFS_INLINE_AND_DEF);
    assert_eq!(result, "Paragraph here.\n");
}

#[test]
fn strip_preserves_content_without_refs() {
    let result = references::strip(PLAIN_NO_REFS);
    assert_eq!(result, PLAIN_NO_REFS);
}

// --- references::extract ---

#[test]
fn extract_returns_urls() {
    let urls = references::extract(RULE_WITH_REFS);
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0], "https://example.com/source-one");
    assert_eq!(urls[1], "https://example.com/source-two");
}

#[test]
fn extract_returns_empty_for_no_refs() {
    let urls = references::extract("No refs here.");
    assert!(urls.is_empty());
}

const REFS_MNEMONIC: &str = fixture!("refs-mnemonic.md");

#[test]
fn strip_removes_mnemonic_inline_markers() {
    let result = references::strip(
        "Text with a ref [MADR] and another [OWASP].\n\n[MADR]: https://a\n[OWASP]: https://b\n",
    );
    assert_eq!(result, "Text with a ref and another.\n");
}

#[test]
fn strip_removes_mnemonic_definitions() {
    let result = references::strip(REFS_MNEMONIC);
    assert!(!result.contains("[MADR]"));
    assert!(!result.contains("[OWASP]"));
    assert!(!result.contains("[keepachangelog]"));
    assert!(!result.contains("https://adr.github.io"));
}

#[test]
fn extract_returns_mnemonic_urls() {
    let urls = references::extract(REFS_MNEMONIC);
    assert_eq!(urls.len(), 3);
    assert_eq!(urls[0], "https://adr.github.io/madr/");
    assert_eq!(urls[1], "https://owasp.org/");
    assert_eq!(urls[2], "https://keepachangelog.com/");
}

// --- variants::merge_into_base ---

#[test]
fn apply_append_matches_golden_output() {
    let result = variants::merge_into_base(RULE_BASE, VARIANT_APPEND).unwrap();
    assert_eq!(result.mode, variants::BodyMergeMode::Append);
    assert_eq!(
        parse::frontmatter_body(&result.content).trim(),
        EXPECTED_APPEND.trim()
    );
}

#[test]
fn apply_prepend_matches_golden_output() {
    let result = variants::merge_into_base(RULE_BASE, VARIANT_PREPEND).unwrap();
    assert_eq!(result.mode, variants::BodyMergeMode::Prepend);
    assert_eq!(
        parse::frontmatter_body(&result.content).trim(),
        EXPECTED_PREPEND.trim()
    );
}

#[test]
fn apply_replaces_with_variant_body() {
    let result = variants::merge_into_base(RULE_BASE, VARIANT_REPLACE).unwrap();
    assert_eq!(result.mode, variants::BodyMergeMode::Replace);
    assert!(!result.content.contains("Base body."));
    assert!(result.content.contains("Replacement body."));
}

#[test]
fn apply_merges_provider_frontmatter_over_base() {
    let result = variants::merge_into_base(VARIANT_MERGE_BASE, VARIANT_MERGE_FIELDS).unwrap();

    assert_eq!(result.mode, variants::BodyMergeMode::Append);
    assert_eq!(
        parse::frontmatter_value(&result.content, "description").as_deref(),
        Some("Provider description.")
    );
    assert_eq!(
        parse::frontmatter_value(&result.content, "argument-hint").as_deref(),
        Some("<path>")
    );
    assert_eq!(
        parse::frontmatter_value(&result.content, "metadata.provider").as_deref(),
        Some("claude")
    );
    assert!(parse::frontmatter_value(&result.content, "metadata.version").is_none());
    assert!(parse::frontmatter_value(&result.content, "mode").is_none());
    assert_eq!(
        parse::frontmatter_body(&result.content),
        "Canonical body.\n"
    );
}

#[test]
fn apply_frontmatter_only_append_preserves_base_body() {
    let result = variants::merge_into_base(RULE_BASE, VARIANT_FRONTMATTER_ONLY).unwrap();

    assert_eq!(parse::frontmatter_body(&result.content), "Base body.\n");
    assert_eq!(
        parse::frontmatter_value(&result.content, "argument-hint").as_deref(),
        Some("<path>")
    );
}

#[test]
fn apply_rejects_unknown_mode() {
    let error = variants::merge_into_base(RULE_BASE, VARIANT_UNKNOWN_MODE).unwrap_err();
    assert_eq!(
        error,
        "unknown variant mode 'merge'; expected append, prepend, or replace"
    );
}

#[test]
fn apply_rejects_malformed_variant_frontmatter() {
    let error = variants::merge_into_base(RULE_BASE, VARIANT_MALFORMED_FRONTMATTER).unwrap_err();
    assert!(
        error.starts_with("cannot parse variant frontmatter:"),
        "unexpected error: {error}"
    );
}

// --- strip_frontmatter (golden output) ---

#[test]
fn strip_frontmatter_keep_name_matches_golden_output() {
    let result = strip_frontmatter(AGENT_BASIC, &["name"]);
    assert_eq!(result.trim(), EXPECTED_KEPT.trim());
}

// --- variants::resolve ---

#[test]
fn resolve_returns_none_for_missing_files() {
    let dir = std::path::Path::new("/nonexistent/path");
    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir, "rule.md", &qualifiers);
    assert!(result.is_none());
}

#[test]
fn resolve_finds_user_variant() {
    let dir = tempfile::tempdir().unwrap();
    let user_dir = dir.path().join("user");
    std::fs::create_dir(&user_dir).unwrap();
    std::fs::write(user_dir.join("rule.md"), "user variant").unwrap();

    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), user_dir.join("rule.md"));
}

#[test]
fn resolve_finds_provider_variant() {
    let dir = tempfile::tempdir().unwrap();
    let provider_dir = dir.path().join("anthropic");
    std::fs::create_dir(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("rule.md"), "provider variant").unwrap();

    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), provider_dir.join("rule.md"));
}

#[test]
fn resolve_user_takes_precedence_over_provider() {
    let dir = tempfile::tempdir().unwrap();

    let user_dir = dir.path().join("user");
    std::fs::create_dir(&user_dir).unwrap();
    std::fs::write(user_dir.join("rule.md"), "user variant").unwrap();

    let provider_dir = dir.path().join("anthropic");
    std::fs::create_dir(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("rule.md"), "provider variant").unwrap();

    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), user_dir.join("rule.md"));
}

#[test]
fn resolve_provider_model_takes_precedence_over_provider() {
    let dir = tempfile::tempdir().unwrap();

    let provider_dir = dir.path().join("anthropic");
    std::fs::create_dir(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("rule.md"), "provider variant").unwrap();

    let model_dir = provider_dir.join("sonnet");
    std::fs::create_dir(&model_dir).unwrap();
    std::fs::write(model_dir.join("rule.md"), "model variant").unwrap();

    let qualifiers = vec!["anthropic".to_string(), "sonnet".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), model_dir.join("rule.md"));
}

// --- assemble (pipeline) ---

#[test]
fn assemble_strips_frontmatter_and_refs() {
    let result = assemble(RULE_WITH_REFS, None, &[], true).unwrap();
    assert_eq!(result.trim(), EXPECTED_STRIPPED.trim());
}

#[test]
fn assemble_with_append_variant() {
    let result = assemble(RULE_WITH_REFS, Some(VARIANT_APPEND), &[], true).unwrap();
    assert!(result.contains("First paragraph with a reference."));
    assert!(result.contains("This content is appended"));
}

#[test]
fn assemble_with_prepend_variant() {
    let result = assemble(RULE_WITH_REFS, Some(VARIANT_PREPEND), &[], true).unwrap();
    assert!(result.contains("First paragraph with a reference."));
    assert!(result.contains("This content is prepended"));
    let prepend_pos = result.find("This content is prepended").unwrap();
    let body_pos = result.find("First paragraph").unwrap();
    assert!(prepend_pos < body_pos);
}

#[test]
fn assemble_keeps_specified_frontmatter_fields() {
    let result = assemble(AGENT_BASIC, None, &["name"], true).unwrap();
    assert!(result.contains("---"));
    assert!(result.contains("name: TestAgent"));
    assert!(!result.contains("version:"));
}

#[test]
fn assemble_filters_frontmatter_after_variant_merge() {
    let result = assemble(
        VARIANT_MERGE_BASE,
        Some(VARIANT_MERGE_FIELDS),
        &["name", "argument-hint", "allowed-tools"],
        false,
    )
    .unwrap();

    assert_eq!(
        parse::frontmatter_value(&result, "name").as_deref(),
        Some("test-skill")
    );
    assert_eq!(
        parse::frontmatter_value(&result, "argument-hint").as_deref(),
        Some("<path>")
    );
    assert_eq!(
        parse::frontmatter_value(&result, "allowed-tools").as_deref(),
        Some("Read")
    );
    assert!(parse::frontmatter_value(&result, "description").is_none());
    assert!(parse::frontmatter_value(&result, "metadata").is_none());
    assert!(parse::frontmatter_value(&result, "mode").is_none());
    assert_eq!(parse::frontmatter_body(&result), "Canonical body.\n");
}

#[test]
fn assemble_no_variant_no_keep_strips_everything() {
    let result = assemble(AGENT_BASIC, None, &[], true).unwrap();
    assert!(!result.contains("---"));
    assert!(!result.contains("# TestAgent"));
    assert!(result.contains("This is a test agent"));
}
