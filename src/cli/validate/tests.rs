use super::*;
use tempfile::TempDir;

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
fn skill_directory_accepts_claude_code_optional_fields() {
    let temp_directory = TempDir::new().unwrap();
    let skill_dir = temp_directory.path().join("example-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = "---\n\
name: example-skill\n\
description: \"Test skill using Claude Code optional frontmatter fields.\"\n\
version: 0.1.0\n\
argument-hint: \"[year]\"\n\
allowed-tools: [Read, Bash]\n\
model: claude-opus-4-7\n\
effort: high\n\
when_to_use: \"When the user asks for X.\"\n\
---\n\
\n\
# example-skill\n\
\n\
Body content for the test skill.\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_dir, temp_directory.path(), &mut report).unwrap();

    assert!(
        report.result.errors.is_empty(),
        "Claude Code optional skill fields should validate cleanly: {:?}",
        report.result.errors
    );
}

#[test]
fn skill_directory_validates_user_override() {
    let root = TempDir::new().unwrap();
    let skill_dir = root.path().join("skills/override-skill");
    std::fs::create_dir_all(skill_dir.join("user")).unwrap();
    let valid = "---\nname: override-skill\ndescription: Valid base skill.\nversion: 0.1.0\n---\n\n# override-skill\n";
    std::fs::write(skill_dir.join("SKILL.md"), valid).unwrap();
    std::fs::write(skill_dir.join("user/SKILL.md"), "# Missing frontmatter\n").unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_dir, root.path(), &mut report).unwrap();

    assert!(report.violations.iter().any(|violation| {
        violation.artifact == "skills/override-skill/user/SKILL.md"
            && violation.severity == ViolationSeverity::Error
    }));
}

#[test]
fn skill_lint_warns_on_conformance_smells_without_blocking() {
    let temp_directory = TempDir::new().unwrap();
    let skill_dir = temp_directory.path().join("claude-helper");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = "---\nname: wrong-name\ndescription: \"A <helper> for things.\"\nversion: 0.1.0\n---\n\n# wrong-name\n\nShort.\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_dir, temp_directory.path(), &mut report).unwrap();

    let warnings = report.result.warnings.join("; ");
    assert!(warnings.contains("does not match its directory"));
    assert!(warnings.contains("angle-bracket pair"));
    assert!(warnings.contains("no trigger phrasing"));
    assert!(warnings.contains("too short to instruct"));
    assert!(
        report.result.errors.is_empty(),
        "lint findings must stay warnings: {:?}",
        report.result.errors
    );
}

#[test]
fn skill_lint_stays_quiet_on_a_conforming_skill() {
    let temp_directory = TempDir::new().unwrap();
    let skill_dir = temp_directory.path().join("tidy-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = "---\nname: tidy-skill\ndescription: \"Keeps things tidy. USE WHEN cleaning up, organizing files.\"\nversion: 0.1.0\n---\n\n# tidy-skill\n\nA body long enough to actually instruct the model about tidying things up carefully.\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_dir, temp_directory.path(), &mut report).unwrap();

    assert!(
        report.result.warnings.is_empty(),
        "a conforming skill must produce no lint warnings: {:?}",
        report.result.warnings
    );
}

#[test]
fn skill_lint_flags_reserved_names() {
    let temp_directory = TempDir::new().unwrap();
    let skill_dir = temp_directory.path().join("claude-tools");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = "---\nname: claude-tools\ndescription: \"Tooling. USE WHEN tooling questions arise.\"\nversion: 0.1.0\n---\n\n# claude-tools\n\nA body long enough to instruct the model about the tools in question here.\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_dir, temp_directory.path(), &mut report).unwrap();

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
fn skill_name_must_be_kebab_case() {
    let temp_directory = TempDir::new().unwrap();
    let skill_dir = temp_directory.path().join("PascalSkill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_md = "---\nname: PascalSkill\ndescription: Rejected by the agentskills name rule.\nversion: 0.1.0\n---\n\n# PascalSkill\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut report = ValidationReport::default();
    check::skill_directory(&skill_dir, temp_directory.path(), &mut report).unwrap();

    assert!(
        report
            .result
            .errors
            .iter()
            .any(|error| error.contains("does not match pattern")),
        "a PascalCase skill name must fail the kebab-case pattern: {:?}",
        report.result.errors
    );
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
