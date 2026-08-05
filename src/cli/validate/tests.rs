//! Validation through `rune validate`, in process.
//!
//! Two paths run here and the helper chosen decides which.
//! [`validate_skill_fixture`] plants no schema on disk and therefore always
//! takes the built-in subset. [`validate_skill_fixture_strictly`] plants the
//! real schema and therefore reaches the standalone `mdschema` binary.
//!
//! Rules with no built-in equivalent, section order chief among them, belong
//! in `tests/runeshell.rs` or in a strict fixture here. Asserting them through
//! the built-in helper would pass for the wrong reason.

use super::*;
use tempfile::TempDir;

macro_rules! fixture {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/input/",
            $name
        ))
    };
}

/// Validate a skill through the built-in subset.
///
/// No `.mdschema` reaches disk, so the schema resolves to the embedded
/// template, whose `MdschemaSource.path` is `None`. That forces the built-in
/// path regardless of whether the standalone binary is installed. Reach for
/// [`validate_skill_fixture_strictly`] to exercise the standalone checker.
fn validate_skill_fixture(directory_name: &str, content: &str) -> ValidationReport {
    let temporary_directory = TempDir::new().unwrap();
    let skill_directory = temporary_directory.path().join(directory_name);
    std::fs::create_dir_all(&skill_directory).unwrap();
    std::fs::write(skill_directory.join("SKILL.md"), content).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_directory, temporary_directory.path(), &mut report).unwrap();
    report
}

/// The standalone checker's wording for the marker field the on-disk test
/// schema requires and the embedded template does not. Seeing this message
/// proves the checker read the file this test planted: the embedded template
/// cannot produce it in any wording, and the built-in subset words a missing
/// field differently ("missing required frontmatter field").
const ON_DISK_MARKER_FINDING: &str = "Required frontmatter field 'strict-marker' is missing";

/// Validate a skill through the standalone `mdschema` binary.
///
/// Planting a schema on disk gives `MdschemaSource.path` a value, which routes
/// dispatch to the standalone checker directly. The planted schema is the
/// embedded template plus one extra required field, `strict-marker`, so a
/// finding about that field discriminates the on-disk route from the
/// materialized-embedded fallback, which would otherwise behave identically.
fn validate_skill_fixture_strictly(directory_name: &str, content: &str) -> ValidationReport {
    let embedded = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/schemas/skill.mdschema"
    ));
    let anchor = "frontmatter:\n    fields:\n";
    assert!(embedded.starts_with(anchor), "template layout changed");
    let schema = embedded.replacen(
        anchor,
        "frontmatter:\n    fields:\n        - name: strict-marker\n          type: string\n",
        1,
    );

    let temporary_directory = TempDir::new().unwrap();
    let skill_directory = temporary_directory.path().join(directory_name);
    std::fs::create_dir_all(&skill_directory).unwrap();
    std::fs::write(skill_directory.join("SKILL.md"), content).unwrap();
    std::fs::write(skill_directory.join(".mdschema"), schema).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_directory, temporary_directory.path(), &mut report).unwrap();
    report
}

#[test]
fn consumer_root_validates_without_module_structure_errors() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(
        temp_directory.path().join(".rune"),
        "version: 1\nsources: {}\nrunes: {}\n",
    )
    .unwrap();
    std::fs::create_dir_all(temp_directory.path().join(".claude")).unwrap();

    let report = validate(&temp_directory.path().to_string_lossy(), false).unwrap();

    assert!(
        report.result.errors.is_empty(),
        "consumer root must not fail module checks: {:?}",
        report.result.errors
    );
    assert!(
        report
            .items
            .iter()
            .any(|item| item.name == ".rune" && item.status == ValidationStatus::Passed),
        "expected a passing .rune item"
    );
    assert!(
        report
            .result
            .warnings
            .iter()
            .any(|warning| warning.contains(".claude/.manifest")),
        "expected a missing-manifest warning: {:?}",
        report.result.warnings
    );
}

#[test]
fn consumer_root_reports_unparseable_dotrune() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join(".rune"), "version: [broken\n").unwrap();

    let report = validate(&temp_directory.path().to_string_lossy(), false).unwrap();

    assert!(
        report
            .result
            .errors
            .iter()
            .any(|error| error.contains(".rune")),
        "expected a .rune parse error: {:?}",
        report.result.errors
    );
}

#[test]
fn module_root_with_dotrune_also_runs_consumer_checks() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join("module.yaml"), "name: demo\n").unwrap();
    std::fs::write(
        temp_directory.path().join(".rune"),
        "version: 1\nsources: {}\nrunes: {}\n",
    )
    .unwrap();

    let report = validate(&temp_directory.path().to_string_lossy(), false).unwrap();

    assert!(
        report
            .items
            .iter()
            .any(|item| item.name == ".rune" && item.status == ValidationStatus::Passed),
        "module + .rune root must validate both roles"
    );
    assert!(
        report
            .result
            .errors
            .iter()
            .any(|error| error.contains("defaults.yaml")),
        "module checks must still run: {:?}",
        report.result.errors
    );
}

#[test]
fn check_module_structure_reports_missing_required_files() {
    let temp_directory = TempDir::new().unwrap();
    let mut report = ValidationReport::default();

    check_module_structure(temp_directory.path(), &mut report);

    assert_eq!(report.result.errors.len(), REQUIRED_FILES.len());
}

#[test]
fn check_module_structure_passes_with_all_required_files() {
    let temp_directory = TempDir::new().unwrap();

    for filename in REQUIRED_FILES {
        std::fs::write(temp_directory.path().join(filename), "content").unwrap();
    }

    let mut report = ValidationReport::default();
    check_module_structure(temp_directory.path(), &mut report);

    assert!(report.result.errors.is_empty());
}

#[test]
fn check_module_yaml_validates_against_embedded_schema() {
    let temp_directory = TempDir::new().unwrap();
    let module_yaml = "name: test-module\nversion: 0.1.0\ndescription: test\nevents: []\n";
    std::fs::write(temp_directory.path().join("module.yaml"), module_yaml).unwrap();

    let mut report = ValidationReport::default();
    check_module_yaml(temp_directory.path(), &mut report);

    assert!(
        report.result.errors.is_empty(),
        "unexpected errors: {:?}",
        report.result.errors
    );
}

#[test]
fn check_module_yaml_skips_when_no_module_yaml() {
    let temp_directory = TempDir::new().unwrap();
    let mut report = ValidationReport::default();

    check_module_yaml(temp_directory.path(), &mut report);

    assert!(report.result.errors.is_empty());
}

#[test]
fn validate_source_returns_structured_broken_adr_violations_without_printing() {
    let root = TempDir::new().unwrap();
    for (name, content) in [
        (
            "module.yaml",
            "name: live-validation\nversion: 0.1.0\ndescription: test\nevents: []\n",
        ),
        ("defaults.yaml", "{}\n"),
        ("README.md", "# Test\n"),
        ("LICENSE", "test\n"),
    ] {
        std::fs::write(root.path().join(name), content).unwrap();
    }
    let decisions = root.path().join("docs/decisions");
    std::fs::create_dir_all(&decisions).unwrap();
    std::fs::write(
        decisions.join("ADR-0001.md"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/input/adr-missing-section.md"
        )),
    )
    .unwrap();

    let report = validate_source(root.path()).unwrap();

    assert!(report.checked > 0);
    assert!(report.violations.iter().any(|violation| {
        violation.artifact == "docs/decisions/ADR-0001.md"
            && violation.severity == ViolationSeverity::Error
    }));
}

#[cfg(feature = "spec")]
fn specification_schema_warning(
    root: &std::path::Path,
) -> Result<Vec<super::super::spec::SpecViolation>, Error> {
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("specification root is not a directory: {}", root.display()),
        ));
    }
    Ok(vec![super::super::spec::SpecViolation {
        code: "spec-schema-invalid".to_string(),
        severity: super::super::spec::DiagnosticSeverity::Warning,
        path: "docs/specs/search/spec.md".to_string(),
        line: Some(4),
        column: None,
        message: "schema advisory".to_string(),
        operation: None,
        capability: Some("search".to_string()),
        change: None,
    }])
}

#[cfg(feature = "spec")]
#[test]
fn top_level_validation_preserves_specification_schema_warnings() {
    let root = TempDir::new().unwrap();
    let mut report = ValidationReport::default();

    check_spec_lifecycle_with_validator(root.path(), &mut report, specification_schema_warning)
        .unwrap();

    assert!(report.result.errors.is_empty());
    assert_eq!(
        report.result.warnings,
        vec!["docs/specs/search/spec.md:4: schema advisory"]
    );
    assert!(report.violations.iter().any(|violation| {
        violation.artifact == "docs/specs/search/spec.md"
            && violation.line == Some(4)
            && violation.severity == ViolationSeverity::Warning
    }));
}

#[cfg(feature = "spec")]
#[test]
fn validate_source_reports_malformed_canonical_spec() {
    let root = TempDir::new().unwrap();
    for (name, content) in [
        (
            "module.yaml",
            "name: spec-validation\nversion: 0.1.0\ndescription: test\nevents: []\n",
        ),
        ("defaults.yaml", "{}\n"),
        ("README.md", "# Test\n"),
        ("LICENSE", "test\n"),
    ] {
        std::fs::write(root.path().join(name), content).unwrap();
    }
    let specs = root.path().join("docs/specs/search");
    std::fs::create_dir_all(&specs).unwrap();
    std::fs::write(specs.join("spec.md"), "# Search\n\n## Requirements\n").unwrap();

    let report = validate_source(root.path()).unwrap();

    assert!(report.violations.iter().any(|violation| {
        violation.artifact == "docs/specs/search/spec.md"
            && violation.severity == ViolationSeverity::Error
    }));
}

#[test]
fn skill_directory_accepts_minimal_runeshell() {
    let report = validate_skill_fixture("minimal-skill", fixture!("runeshell-minimal.md"));

    assert!(
        report.result.errors.is_empty(),
        "minimal RuneShell must validate: {:?}",
        report.result.errors
    );
}

#[test]
fn skill_directory_accepts_complete_runeshell() {
    let report = validate_skill_fixture("complete-skill", fixture!("runeshell-complete.md"));

    assert!(
        report.result.errors.is_empty(),
        "complete RuneShell must validate: {:?}",
        report.result.errors
    );
}

#[test]
fn skill_directory_rejects_provider_fields_in_canonical_source() {
    let report =
        validate_skill_fixture("provider-fields", fixture!("runeshell-provider-fields.md"));
    let errors = report.result.errors.join("; ");

    assert!(
        errors.contains("argument-hint"),
        "unexpected errors: {errors}"
    );
    assert!(
        errors.contains("disallowed-tools"),
        "unexpected errors: {errors}"
    );
}

#[test]
fn skill_directory_validates_user_override() {
    let root = TempDir::new().unwrap();
    let skill_directory = root.path().join("skills/minimal-skill");
    std::fs::create_dir_all(skill_directory.join("user")).unwrap();
    std::fs::write(
        skill_directory.join("SKILL.md"),
        fixture!("runeshell-minimal.md"),
    )
    .unwrap();
    std::fs::write(
        skill_directory.join("user/SKILL.md"),
        "# Missing frontmatter\n",
    )
    .unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_directory, root.path(), &mut report).unwrap();

    assert!(report.violations.iter().any(|violation| {
        violation.artifact == "skills/minimal-skill/user/SKILL.md"
            && violation.severity == ViolationSeverity::Error
    }));
}

#[test]
fn skill_directory_accepts_provider_specific_frontmatter() {
    let root = TempDir::new().unwrap();
    let skill_directory = root.path().join("skills/minimal-skill");
    std::fs::create_dir_all(skill_directory.join("claude")).unwrap();
    std::fs::write(
        skill_directory.join("SKILL.md"),
        fixture!("runeshell-minimal.md"),
    )
    .unwrap();
    std::fs::write(
        skill_directory.join("claude/SKILL.md"),
        fixture!("skill-variant-claude.md"),
    )
    .unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_directory, root.path(), &mut report).unwrap();

    assert!(
        report.result.errors.is_empty(),
        "provider frontmatter must validate after merging: {:?}",
        report.result.errors
    );
}

#[test]
fn skill_directory_rejects_malformed_provider_variant() {
    let root = TempDir::new().unwrap();
    let skill_directory = root.path().join("skills/minimal-skill");
    std::fs::create_dir_all(skill_directory.join("claude")).unwrap();
    std::fs::write(
        skill_directory.join("SKILL.md"),
        fixture!("runeshell-minimal.md"),
    )
    .unwrap();
    std::fs::write(
        skill_directory.join("claude/SKILL.md"),
        fixture!("variant-malformed-frontmatter.md"),
    )
    .unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_directory, root.path(), &mut report).unwrap();

    assert!(report.violations.iter().any(|violation| {
        violation.artifact == "skills/minimal-skill/claude/SKILL.md"
            && violation.severity == ViolationSeverity::Error
            && violation
                .message
                .contains("cannot parse variant frontmatter")
    }));
}

#[test]
fn skill_identity_reports_h1_mismatch() {
    let report = validate_skill_fixture("identity-skill", fixture!("runeshell-h1-mismatch.md"));
    let errors = report.result.errors.join("; ");

    assert!(errors.contains("stable shell identity"));
    assert!(errors.contains("frontmatter name 'identity-skill'"));
    assert!(errors.contains("H1 'wrong-heading'"));
    assert!(errors.contains("directory 'identity-skill'"));
}

#[test]
fn skill_identity_reports_frontmatter_name_mismatch() {
    let report = validate_skill_fixture("identity-skill", fixture!("runeshell-name-mismatch.md"));
    let errors = report.result.errors.join("; ");

    assert!(errors.contains("frontmatter name 'wrong-name'"));
    assert!(errors.contains("H1 'identity-skill'"));
    assert!(errors.contains("directory 'identity-skill'"));
}

#[test]
fn skill_identity_reports_directory_mismatch() {
    let report = validate_skill_fixture("different-directory", fixture!("runeshell-minimal.md"));
    let errors = report.result.errors.join("; ");

    assert!(errors.contains("frontmatter name 'minimal-skill'"));
    assert!(errors.contains("H1 'minimal-skill'"));
    assert!(errors.contains("directory 'different-directory'"));
}

#[test]
fn focused_instructions_do_not_warn() {
    let report = validate_skill_fixture("complete-skill", fixture!("runeshell-complete.md"));

    assert!(
        !report
            .result
            .warnings
            .iter()
            .any(|warning| warning.contains("direct H3 headings")),
        "focused instructions must not warn: {:?}",
        report.result.warnings
    );
}

#[test]
fn broad_instructions_warn_without_failing() {
    let report = validate_skill_fixture("broad-skill", fixture!("runeshell-broad-instructions.md"));

    assert!(
        report.result.warnings.iter().any(|warning| warning
            .contains("stable shell breadth: Instructions has more than 4 direct H3 headings")),
        "broad instructions must warn: {:?}",
        report.result.warnings
    );
    assert!(
        report.result.errors.is_empty(),
        "breadth warning must not fail validation: {:?}",
        report.result.errors
    );
}

#[test]
fn fenced_headings_do_not_affect_identity_or_breadth() {
    let report = validate_skill_fixture("fenced-skill", fixture!("runeshell-fenced-headings.md"));
    let findings = format!(
        "{}; {}",
        report.result.errors.join("; "),
        report.result.warnings.join("; ")
    );

    assert!(!findings.contains("stable shell identity"), "{findings}");
    assert!(!findings.contains("direct H3 headings"), "{findings}");
}

/// A deck validates each module with its own report, and every module that
/// falls back produces the same machine-level notice. Merging must keep the
/// first and drop the rest, or a deck of N modules warns N times.
#[test]
fn deck_merge_keeps_one_reduced_checking_notice() {
    let mut aggregate = ValidationReport::default();
    let mut first_module = ValidationReport::default();
    first_module.report_missing_standalone_checker();
    let mut second_module = ValidationReport::default();
    second_module.report_missing_standalone_checker();

    append_report(&mut aggregate, first_module);
    append_report(&mut aggregate, second_module);

    let notices = aggregate
        .result
        .warnings
        .iter()
        .filter(|warning| warning.contains("standalone mdschema is unavailable"))
        .count();
    assert_eq!(notices, 1, "warnings: {:?}", aggregate.result.warnings);
    let violation_notices = aggregate
        .violations
        .iter()
        .filter(|violation| {
            violation
                .message
                .contains("standalone mdschema is unavailable")
        })
        .count();
    assert_eq!(violation_notices, 1);
}

/// Both reduced-checking reasons share the once-per-run flag, so a deck mixing
/// a missing-binary module with a write-failed module still states one notice.
#[test]
fn deck_merge_deduplicates_across_both_reduced_checking_reasons() {
    let mut aggregate = ValidationReport::default();
    let mut first_module = ValidationReport::default();
    first_module.report_missing_standalone_checker();
    let mut second_module = ValidationReport::default();
    second_module.report_unusable_standalone_checker();

    append_report(&mut aggregate, first_module);
    append_report(&mut aggregate, second_module);

    let notices = aggregate
        .result
        .warnings
        .iter()
        .filter(|warning| {
            warning.contains("standalone mdschema is unavailable")
                || warning.contains("standalone mdschema is installed but could not")
        })
        .count();
    assert_eq!(notices, 1, "warnings: {:?}", aggregate.result.warnings);
}

/// Drives the availability flag directly, because the machine running the
/// suite has the binary installed and would otherwise never take this path.
#[test]
fn fallback_reports_partial_validation_and_keeps_basic_checks() {
    let schema = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/schemas/skill.mdschema"
    ));
    let temporary_directory = TempDir::new().unwrap();
    let skill_file = temporary_directory.path().join("SKILL.md");
    std::fs::write(&skill_file, fixture!("runeshell-h4.md")).unwrap();

    let source = schema::MdschemaSource {
        content: schema.to_string(),
        path: None,
    };
    let mut report = ValidationReport::default();
    check::check_mdschema_with_availability(
        fixture!("runeshell-h4.md"),
        &skill_file,
        "skills/deep-skill/SKILL.md",
        Some(&source),
        false,
        &mut report,
    );

    let warnings = report.result.warnings.join("; ");
    let errors = report.result.errors.join("; ");

    assert!(
        warnings.contains("standalone mdschema is unavailable"),
        "fallback must announce itself: {warnings}"
    );
    assert!(
        warnings.contains("Section order, unexpected sections, permitted H3 placement, and heading uniqueness were NOT checked"),
        "fallback must name what it skipped: {warnings}"
    );
    assert!(
        warnings.contains("brew install jackchuka/tap/mdschema"),
        "fallback must say how to fix it: {warnings}"
    );
    assert!(
        errors.contains("exceeds max_depth 3"),
        "the built-in subset must still run: {errors}"
    );
}

/// Guards the dispatch itself, from both directions.
///
/// The absence of the fallback warning alone would pass if dispatch silently
/// did nothing, so this also demands a finding only the standalone checker can
/// produce: an unexpected H2 has no built-in equivalent.
#[test]
fn on_disk_schema_routes_to_the_standalone_checker() {
    let report = validate_skill_fixture_strictly(
        "unknown-section-skill",
        fixture!("runeshell-unknown-section.md"),
    );
    let warnings = report.result.warnings.join("; ");
    let errors = report.result.errors.join("; ");

    assert!(
        !warnings.contains("standalone mdschema is unavailable"),
        "an on-disk schema must not fall back: {warnings}"
    );
    assert!(
        errors.contains("Unexpected section"),
        "only the standalone checker rejects an unexpected H2, so its absence means dispatch did nothing: {errors}"
    );
    assert!(
        errors.contains(ON_DISK_MARKER_FINDING),
        "the marker finding proves the ON-DISK schema was read; without it this could be the materialized embedded template answering: {errors}"
    );
}

/// Ordering has no built-in equivalent, so reaching this error proves the
/// standalone checker ran rather than the subset.
#[test]
fn standalone_checker_rejects_sections_out_of_order_in_process() {
    let report =
        validate_skill_fixture_strictly("misordered-skill", fixture!("runeshell-misordered.md"));
    let errors = report.result.errors.join("; ");

    assert!(
        errors.contains("should appear after \"Instructions\" but appears before it"),
        "standalone ordering error must surface in the report: {errors}"
    );
}

#[test]
fn skill_lint_flags_reserved_names() {
    let report = validate_skill_fixture("claude-tools", fixture!("runeshell-reserved-name.md"));

    assert!(
        report
            .result
            .warnings
            .iter()
            .any(|warning| warning.contains("reserved word 'claude'")),
        "reserved names must warn: {:?}",
        report.result.warnings
    );
}

#[test]
fn skill_name_requires_lowercase_kebab_case() {
    for (directory_name, fixture_name, content, valid) in [
        (
            "minimal-skill",
            "runeshell-minimal.md",
            fixture!("runeshell-minimal.md"),
            true,
        ),
        (
            "PascalSkill",
            "runeshell-pascal-name.md",
            fixture!("runeshell-pascal-name.md"),
            false,
        ),
        (
            "snake_skill",
            "runeshell-snake-name.md",
            fixture!("runeshell-snake-name.md"),
            false,
        ),
    ] {
        let report = validate_skill_fixture(directory_name, content);
        let pattern_error = report
            .result
            .errors
            .iter()
            .any(|error| error.contains("does not match"));
        assert_eq!(
            pattern_error, !valid,
            "name in '{fixture_name}' validity mismatch: {:?}",
            report.result.errors
        );
    }
}

// --- tools.rs native checks ---

#[test]
fn is_excluded_matches_prefix_glob() {
    let module_root = std::path::Path::new("/project");
    let file_path = std::path::Path::new("/project/templates/statement.yaml");
    let patterns = vec!["templates/*".to_string()];
    assert!(tools::is_excluded(file_path, module_root, &patterns));
}

#[test]
fn is_excluded_rejects_non_matching_path() {
    let module_root = std::path::Path::new("/project");
    let file_path = std::path::Path::new("/project/schemas/agent.schema.yaml");
    let patterns = vec!["templates/*".to_string()];
    assert!(!tools::is_excluded(file_path, module_root, &patterns));
}

#[test]
fn is_excluded_matches_exact_path() {
    let module_root = std::path::Path::new("/project");
    let file_path = std::path::Path::new("/project/defaults.yaml");
    let patterns = vec!["defaults.yaml".to_string()];
    assert!(tools::is_excluded(file_path, module_root, &patterns));
}

// --- plugin scaffolding (CLI orchestration) ---

fn write_plugin_manifest(root: &std::path::Path, body: &str) {
    let dir = root.join(".claude-plugin");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.json"), body).unwrap();
}

fn write_hook_script(root: &std::path::Path, relative: &str, executable: bool) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho hi\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if executable { 0o755 } else { 0o644 };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = executable;
}

#[test]
fn plugin_scaffolding_skipped_without_manifest() {
    let temp = TempDir::new().unwrap();
    let mut report = ValidationReport::default();
    plugin::check_plugin_scaffolding(temp.path(), &mut report);
    assert!(report.result.errors.is_empty(), "no plugin, no errors");
}

#[test]
fn plugin_scaffolding_valid_tree_passes() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), r#"{"name": "my-plugin"}"#);
    std::fs::create_dir_all(temp.path().join("hooks")).unwrap();
    std::fs::write(
        temp.path().join("hooks/hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/guard.sh"}]}]}}"#,
    )
    .unwrap();
    write_hook_script(temp.path(), "hooks/guard.sh", true);

    let mut report = ValidationReport::default();
    plugin::check_plugin_scaffolding(temp.path(), &mut report);
    assert!(
        report.result.errors.is_empty(),
        "valid tree: {:?}",
        report.result.errors
    );
}

#[test]
fn plugin_scaffolding_corrupt_manifest_errors() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), "{ not valid json");

    let mut report = ValidationReport::default();
    plugin::check_plugin_scaffolding(temp.path(), &mut report);
    assert!(
        report
            .result
            .errors
            .iter()
            .any(|e| e.contains("invalid JSON")),
        "expected JSON error: {:?}",
        report.result.errors
    );
}

#[test]
fn plugin_scaffolding_missing_hook_script_errors() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), r#"{"name": "p"}"#);
    std::fs::create_dir_all(temp.path().join("hooks")).unwrap();
    std::fs::write(
        temp.path().join("hooks/hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/ghost.sh"}]}]}}"#,
    )
    .unwrap();

    let mut report = ValidationReport::default();
    plugin::check_plugin_scaffolding(temp.path(), &mut report);
    assert!(
        report.result.errors.iter().any(|e| e.contains("not found")),
        "expected missing-script error: {:?}",
        report.result.errors
    );
}

#[cfg(unix)]
#[test]
fn plugin_scaffolding_non_executable_hook_errors() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), r#"{"name": "p"}"#);
    std::fs::create_dir_all(temp.path().join("hooks")).unwrap();
    std::fs::write(
        temp.path().join("hooks/hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/guard.sh"}]}]}}"#,
    )
    .unwrap();
    write_hook_script(temp.path(), "hooks/guard.sh", false);

    let mut report = ValidationReport::default();
    plugin::check_plugin_scaffolding(temp.path(), &mut report);
    assert!(
        report
            .result
            .errors
            .iter()
            .any(|e| e.contains("not executable")),
        "expected non-executable error: {:?}",
        report.result.errors
    );
}
