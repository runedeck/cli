use super::*;
use tempfile::TempDir;

const ORIGINAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/spec/archive-original.md"
));
const DELTA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/spec/archive-delta.md"
));
const MERGED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/spec/archive-merged.md"
));
const NESTED_ORIGINAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/openspec/v1.6.0/cases/round-trip/input/openspec/specs/payments/card/spec.md"
));
const NESTED_DELTA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/openspec/v1.6.0/cases/round-trip/input/openspec/changes/add-payment/specs/payments/card/spec.md"
));
const TRANSACTION_PROPOSAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/spec/transaction-proposal.md"
));
const INTEROP_PROJECT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interop/project.md"
));

fn write_change(root: &Path, id: &str, tasks: &str, delta: &str) {
    let change = changes_root(root).unwrap().join(id);
    fs::create_dir_all(change.join("specs/search")).unwrap();
    fs::write(change.join("proposal.md"), TRANSACTION_PROPOSAL).unwrap();
    fs::write(change.join("tasks.md"), tasks).unwrap();
    fs::write(change.join("specs/search/spec.md"), delta).unwrap();
}

#[test]
fn show_resolves_unambiguous_prefixes_and_rejects_ambiguity() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "add-widget", "- [ ] a\n", DELTA);
    write_change(root.path(), "add-gadget", "- [ ] a\n", DELTA);
    write_change(root.path(), "remove-legacy", "- [ ] a\n", DELTA);

    match prefix_match(root.path(), "rem").unwrap() {
        PrefixMatch::Change(id) => assert_eq!(id, "remove-legacy"),
        _ => panic!("expected a unique change match"),
    }
    match prefix_match(root.path(), "add").unwrap() {
        PrefixMatch::Ambiguous(candidates) => {
            assert_eq!(candidates, vec!["add-gadget", "add-widget"]);
        }
        _ => panic!("expected ambiguity"),
    }
    assert!(matches!(
        prefix_match(root.path(), "zz").unwrap(),
        PrefixMatch::None
    ));
}

#[test]
fn resolver_prefers_native_and_detects_openspec_roots() {
    let native = TempDir::new().unwrap();
    fs::create_dir_all(native.path().join("docs/changes")).unwrap();
    assert!(
        changes_root(native.path())
            .unwrap()
            .ends_with("docs/changes")
    );

    let openspec = TempDir::new().unwrap();
    fs::create_dir_all(openspec.path().join("openspec/changes")).unwrap();
    assert!(
        changes_root(openspec.path())
            .unwrap()
            .ends_with("openspec/changes")
    );
    assert!(
        specs_root(openspec.path())
            .unwrap()
            .ends_with("openspec/specs")
    );

    let empty = TempDir::new().unwrap();
    assert!(
        changes_root(empty.path())
            .unwrap()
            .ends_with("docs/changes")
    );
}

#[test]
fn configured_root_overrides_autodetect() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("docs/changes")).unwrap();
    fs::write(
        root.path().join("config.yaml"),
        "spec:\n    root: openspec\n",
    )
    .unwrap();

    // Mirrors the CLI's startup wiring: the hook reads `spec.root` from
    // the repo's config.yaml. Roots without one fall back to autodetect,
    // so the process-global hook cannot disturb the other tests.
    set_root_config_lookup(|repo| {
        let config_path = repo.join("config.yaml");
        let raw = match fs::read_to_string(&config_path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!("cannot read {}: {error}", config_path.display()));
            }
        };
        let value: serde_yaml::Value = serde_yaml::from_str(&raw)
            .map_err(|error| format!("cannot parse {}: {error}", config_path.display()))?;
        Ok(value
            .get("spec")
            .and_then(|spec| spec.get("root"))
            .and_then(|root| root.as_str())
            .map(str::to_string))
    });

    assert!(
        changes_root(root.path())
            .unwrap()
            .ends_with("openspec/changes")
    );
    assert!(specs_root(root.path()).unwrap().ends_with("openspec/specs"));
}

#[test]
fn propose_rejects_the_reserved_archive_change_identifier() {
    let root = TempDir::new().unwrap();

    let error = propose(&root.path().to_string_lossy(), "archive", &[], false, false).unwrap_err();

    assert_eq!(
        error.message(),
        "change id 'archive' is reserved for archived changes"
    );
    assert!(!root.path().join("docs/changes/archive").exists());
}

#[test]
fn propose_scaffolds_agent_consumable_change_tree() {
    let root = TempDir::new().unwrap();
    propose(
        &root.path().to_string_lossy(),
        "improve-search",
        &["search".to_string()],
        false,
        false,
    )
    .unwrap();

    let change = root.path().join("docs/changes/improve-search");
    assert!(change.join("proposal.md").is_file());
    assert!(change.join("tasks.md").is_file());
    let delta = fs::read_to_string(change.join("specs/search/spec.md")).unwrap();
    assert!(delta.contains("### Requirement: Search"));
    assert!(!delta.contains("${"));
}

#[test]
fn archive_refuses_unchecked_tasks() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "unfinished", "- [ ] finish it\n", DELTA);

    let error = archive(
        &root.path().to_string_lossy(),
        "unfinished",
        false,
        false,
        false,
    )
    .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::Validate);
    assert!(error.message().contains("finish it"));
    assert!(root.path().join("docs/changes/unfinished").is_dir());
}

#[test]
fn archive_refuses_an_empty_task_checklist_without_override() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "empty-tasks", "# Tasks\n", DELTA);

    let error = archive(
        &root.path().to_string_lossy(),
        "empty-tasks",
        false,
        false,
        false,
    )
    .unwrap_err();

    assert!(error.message().contains("no checklist tasks"));
}

#[test]
fn task_progress_ignores_checkboxes_inside_code_fences() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("tasks.md");
    fs::write(&path, "- [x] real\n```md\n- [ ] example\n```\n").unwrap();

    let status = read_tasks(&path).unwrap();

    assert_eq!(status.completed, 1);
    assert_eq!(status.total, 1);
}

#[test]
fn archive_merges_added_modified_and_removed_requirements() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();
    write_change(root.path(), "merge-search", "- [x] complete\n", DELTA);

    archive(
        &root.path().to_string_lossy(),
        "merge-search",
        false,
        false,
        false,
    )
    .unwrap();

    let merged = fs::read_to_string(canonical).unwrap();
    assert_eq!(merged, MERGED);
    assert!(!root.path().join("docs/changes/merge-search").exists());
    assert!(
        root.path()
            .join("docs/changes/archive")
            .read_dir()
            .unwrap()
            .next()
            .is_some()
    );
}

#[test]
fn nested_capabilities_are_discovered_validated_and_archived() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/payments/card/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, NESTED_ORIGINAL).unwrap();
    let change = root.path().join("docs/changes/add-payment");
    let delta = change.join("specs/payments/card/spec.md");
    fs::create_dir_all(delta.parent().unwrap()).unwrap();
    fs::write(change.join("proposal.md"), "# Change\n").unwrap();
    fs::write(change.join("tasks.md"), "- [x] complete\n").unwrap();
    fs::write(&delta, NESTED_DELTA).unwrap();

    let specifications = scan_specifications(root.path()).unwrap();
    assert_eq!(specifications[0].capability, "payments/card");
    assert_eq!(specifications[0].requirements, 1);
    let context = commands::load_context_output(root.path(), "add-payment").unwrap();
    assert_eq!(context.deltas[0].capability, "payments/card");
    assert_eq!(
        show(&root.path().to_string_lossy(), "payments/card", false).unwrap(),
        0
    );
    assert!(
        validate_spec_tree(root.path(), no_schema_check)
            .unwrap()
            .is_empty()
    );

    archive(
        &root.path().to_string_lossy(),
        "add-payment",
        false,
        false,
        false,
    )
    .unwrap();

    let merged = fs::read_to_string(canonical).unwrap();
    assert!(merged.contains("### Requirement: Existing card payment"));
    assert!(merged.contains("### Requirement: Card payment"));
}

#[test]
fn abandon_stamps_frontmatter_and_does_not_merge() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "drop-search", "- [ ] unfinished\n", DELTA);

    archive(
        &root.path().to_string_lossy(),
        "drop-search",
        false,
        true,
        false,
    )
    .unwrap();

    assert!(!root.path().join("docs/specs/search/spec.md").exists());
    let archived = read_directories(&root.path().join("docs/changes/archive"))
        .unwrap()
        .pop()
        .unwrap();
    assert!(
        read(&archived.join("proposal.md"))
            .unwrap()
            .contains("status: abandoned")
    );
}

fn heading_stub_check(content: &str, file_path: &str, _schema: &str) -> Vec<MdschemaDiagnostic> {
    if content.contains("Specification") {
        Vec::new()
    } else {
        vec![MdschemaDiagnostic {
            file: file_path.to_string(),
            line: Some(1),
            severity: DiagnosticSeverity::Error,
            message: "expected '# <Capability> Specification'".to_string(),
        }]
    }
}

fn warning_schema_check(_content: &str, file_path: &str, _schema: &str) -> Vec<MdschemaDiagnostic> {
    vec![MdschemaDiagnostic {
        file: file_path.to_string(),
        line: Some(1),
        severity: DiagnosticSeverity::Warning,
        message: "schema advisory".to_string(),
    }]
}

fn no_schema_check(_content: &str, _file_path: &str, _schema: &str) -> Vec<MdschemaDiagnostic> {
    Vec::new()
}

#[test]
fn validation_rejects_malformed_spec_and_unknown_delta_target() {
    let root = TempDir::new().unwrap();
    let malformed = root.path().join("docs/specs/broken/spec.md");
    fs::create_dir_all(malformed.parent().unwrap()).unwrap();
    fs::write(&malformed, "# Broken\n\n## Requirements\n").unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();
    write_change(
        root.path(),
        "bad-delta",
        "- [x] done\n",
        "## REMOVED Requirements\n\n### Requirement: Unknown\n",
    );

    let violations = validate_spec_tree(root.path(), heading_stub_check).unwrap();

    // The stub reports one schema diagnostic per malformed heading; the
    // real mdschema wiring is asserted CLI-side where the schemas load.
    assert!(violations.iter().any(|violation| {
        violation
            .message
            .contains("expected '# <Capability> Specification'")
    }));
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("unknown requirement 'Unknown'"))
    );
}

#[test]
fn validation_reports_root_level_specifications_as_diagnostics() {
    let root = TempDir::new().unwrap();
    let root_specification = root.path().join("docs/specs/spec.md");
    fs::create_dir_all(root_specification.parent().unwrap()).unwrap();
    fs::write(root_specification, ORIGINAL).unwrap();

    let diagnostics = validate_spec_tree(root.path(), no_schema_check).unwrap();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "spec-root-invalid"
            && diagnostic.path == "docs/specs/spec.md"
            && diagnostic.capability.is_none()
    }));
}

#[test]
fn validation_identifies_opaque_artifacts_without_reading_their_content() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("openspec/specs")).unwrap();
    fs::write(root.path().join("openspec/project.md"), INTEROP_PROJECT).unwrap();

    let diagnostics = validate_spec_tree(root.path(), no_schema_check).unwrap();

    let opaque = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "opaque-artifact")
        .unwrap();
    assert_eq!(opaque.path, "project.md");
    assert_eq!(
        opaque.message,
        "opaque OpenSpec artifact classified as file"
    );
    assert_eq!(opaque.severity, DiagnosticSeverity::Warning);
}

#[test]
fn spec_validation_preserves_schema_warning_severity() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(canonical, ORIGINAL).unwrap();

    let diagnostics =
        validate::validate_spec_target(root.path(), Some("search"), warning_schema_check).unwrap();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "schema advisory"
            && diagnostic.severity == DiagnosticSeverity::Warning
            && diagnostic.path == "docs/specs/search/spec.md"
    }));
    assert!(!diagnostics.iter().any(SpecViolation::is_error));
}

#[test]
fn targeted_change_reports_malformed_canonical_with_change_context() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, "# Search\n").unwrap();
    write_change(root.path(), "change-search", "- [x] complete\n", DELTA);

    let diagnostics =
        validate::validate_spec_target(root.path(), Some("change-search"), warning_schema_check)
            .unwrap();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "spec-parse-invalid"
            && diagnostic.path == "docs/specs/search/spec.md"
            && diagnostic.capability.as_deref() == Some("search")
            && diagnostic.change.as_deref() == Some("change-search")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "spec-schema-invalid"
            && diagnostic.severity == DiagnosticSeverity::Warning
            && diagnostic.capability.as_deref() == Some("search")
            && diagnostic.change.as_deref() == Some("change-search")
    }));
}

#[test]
fn malformed_delta_has_one_acceptance_result_across_commands() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();
    write_change(
        root.path(),
        "remove-unknown",
        "- [x] complete\n",
        "## REMOVED Requirements\n\n### Requirement: Unknown\n",
    );

    let diagnostics =
        validate::validate_spec_target(root.path(), Some("remove-unknown"), no_schema_check)
            .unwrap();
    let semantic_diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "delta-application-conflict")
        .unwrap();
    assert_eq!(semantic_diagnostic.capability.as_deref(), Some("search"));
    assert_eq!(
        semantic_diagnostic.change.as_deref(),
        Some("remove-unknown")
    );

    let archive_error = archive(
        &root.path().to_string_lossy(),
        "remove-unknown",
        false,
        false,
        false,
    )
    .unwrap_err();
    assert_eq!(archive_error.kind(), ErrorKind::Validate);
    assert!(
        archive_error
            .message()
            .contains("unknown requirement 'Unknown'")
    );
    assert_eq!(fs::read_to_string(&canonical).unwrap(), ORIGINAL);
    assert!(root.path().join("docs/changes/remove-unknown").is_dir());

    let doctor = doctor_output(&root.path().to_string_lossy()).unwrap();
    assert!(doctor.findings.iter().any(|finding| {
        finding.severity == "error"
            && finding.path == "docs/changes/remove-unknown/specs/search/spec.md"
            && finding.message.contains("unknown requirement 'Unknown'")
    }));
}

#[test]
fn whole_tree_validation_deduplicates_canonical_parse_diagnostics() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, "# Search\n").unwrap();
    write_change(root.path(), "change-search", "- [ ] pending\n", DELTA);

    let diagnostics = validate_spec_tree(root.path(), no_schema_check).unwrap();
    let canonical_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "spec-parse-invalid"
                && diagnostic.path == "docs/specs/search/spec.md"
        })
        .collect::<Vec<_>>();

    assert_eq!(canonical_diagnostics.len(), 1);
}

#[test]
fn validation_diagnostics_serialize_every_nullable_field() {
    let diagnostic = SpecViolation {
        code: "spec-parse-invalid".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "docs/specs/search/spec.md".to_string(),
        line: None,
        column: None,
        message: "invalid specification".to_string(),
        operation: None,
        capability: Some("search".to_string()),
        change: None,
    };

    let value = serde_json::to_value(diagnostic).unwrap();

    assert!(value.get("line").is_some_and(serde_json::Value::is_null));
    assert!(value.get("column").is_some_and(serde_json::Value::is_null));
    assert!(
        value
            .get("operation")
            .is_some_and(serde_json::Value::is_null)
    );
    assert!(value.get("change").is_some_and(serde_json::Value::is_null));
    assert_eq!(value["capability"], "search");
}

#[test]
fn targeted_validation_resolves_nested_capability_prefixes() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/payments/card/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(canonical, NESTED_ORIGINAL).unwrap();

    let diagnostics =
        validate::validate_spec_target(root.path(), Some("payments/c"), no_schema_check).unwrap();

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn validation_accepts_well_formed_canonical_and_delta_specs() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();
    write_change(root.path(), "valid-delta", "- [ ] pending\n", DELTA);

    let violations = validate_spec_tree(root.path(), no_schema_check).unwrap();

    assert!(
        violations.is_empty(),
        "unexpected violations: {violations:?}"
    );
}

#[test]
fn changes_classifies_draft_active_and_complete() {
    let root = TempDir::new().unwrap();
    for (id, tasks) in [
        ("draft", "- [ ] first\n"),
        ("active", "- [x] first\n- [ ] second\n"),
        ("complete", "- [x] first\n"),
    ] {
        let directory = root.path().join("docs/changes").join(id);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("tasks.md"), tasks).unwrap();
    }

    let summaries = scan_changes(root.path()).unwrap();

    assert_eq!(summaries.len(), 3);
    assert_eq!(
        summaries
            .iter()
            .find(|change| change.id == "draft")
            .unwrap()
            .state,
        ChangeState::Draft
    );
    assert_eq!(
        summaries
            .iter()
            .find(|change| change.id == "active")
            .unwrap()
            .state,
        ChangeState::Active
    );
    assert_eq!(
        summaries
            .iter()
            .find(|change| change.id == "complete")
            .unwrap()
            .state,
        ChangeState::Complete
    );
}

#[test]
fn show_rejects_a_name_that_is_both_change_and_specification() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "search", "- [ ] task\n", DELTA);
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();

    let error = show(&root.path().to_string_lossy(), "search", false).unwrap_err();

    assert!(error.message().contains("both an active change"));
}

#[test]
fn show_reports_an_archived_change_by_name() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "done", "- [x] task\n", DELTA);
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();
    archive(&root.path().to_string_lossy(), "done", false, false, false).unwrap();

    let error = show(&root.path().to_string_lossy(), "done", false).unwrap_err();

    assert!(error.message().contains("already archived"));
}

#[test]
fn archive_retry_reports_the_existing_dated_archive() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "retry", "- [x] task\n", DELTA);
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();
    archive(&root.path().to_string_lossy(), "retry", false, false, false).unwrap();

    let code = archive(&root.path().to_string_lossy(), "retry", false, false, false).unwrap();

    assert_eq!(code, 0);
    assert_eq!(fs::read_to_string(canonical).unwrap(), MERGED);
}

#[test]
fn archive_retry_infers_status_from_archived_proposal() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "merged-retry", "- [x] task\n", DELTA);
    write_change(root.path(), "abandoned-retry", "- [ ] task\n", DELTA);
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();

    archive(
        &root.path().to_string_lossy(),
        "merged-retry",
        false,
        false,
        false,
    )
    .unwrap();
    archive(
        &root.path().to_string_lossy(),
        "abandoned-retry",
        false,
        true,
        false,
    )
    .unwrap();

    let changes = changes_root(root.path()).unwrap();
    let merged_archive = archived_change_path(&changes, "merged-retry")
        .unwrap()
        .unwrap();
    let abandoned_archive = archived_change_path(&changes, "abandoned-retry")
        .unwrap()
        .unwrap();
    assert_eq!(archived_change_status(&merged_archive).unwrap(), "merged");
    assert_eq!(
        archived_change_status(&abandoned_archive).unwrap(),
        "abandoned"
    );

    assert_eq!(
        archive(
            &root.path().to_string_lossy(),
            "merged-retry",
            false,
            true,
            false,
        )
        .unwrap(),
        0
    );
    assert_eq!(
        archive(
            &root.path().to_string_lossy(),
            "abandoned-retry",
            false,
            false,
            false,
        )
        .unwrap(),
        0
    );
}

#[test]
fn show_renders_an_active_change() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "improve-search", "- [x] a\n- [ ] b\n", DELTA);

    let code = show(&root.path().to_string_lossy(), "improve-search", false).unwrap();

    assert_eq!(code, 0);
}

#[cfg(unix)]
#[test]
fn openspec_advisory_distinguishes_process_outcomes() {
    use std::time::Duration;

    let root = TempDir::new().unwrap();
    assert_eq!(
        doctor::run_openspec_advisory(
            root.path(),
            "definitely-not-an-openspec-executable",
            &[],
            Duration::from_millis(100),
        ),
        doctor::OpenSpecAdvisory::Unavailable
    );
    assert_eq!(
        doctor::run_openspec_advisory(
            root.path(),
            "/usr/bin/true",
            &[],
            Duration::from_millis(100),
        ),
        doctor::OpenSpecAdvisory::Successful
    );
    assert_eq!(
        doctor::run_openspec_advisory(
            root.path(),
            "/usr/bin/false",
            &[],
            Duration::from_millis(100),
        ),
        doctor::OpenSpecAdvisory::ValidationFailed(
            "validation failed without diagnostic output".to_string()
        )
    );
    assert_eq!(
        doctor::run_openspec_advisory(
            root.path(),
            "/bin/sh",
            &["-c", "sleep 1"],
            Duration::from_millis(20),
        ),
        doctor::OpenSpecAdvisory::TimedOut
    );
}

#[cfg(unix)]
#[test]
fn openspec_advisory_drains_output_after_retention_limit() {
    use std::time::Duration;

    let root = TempDir::new().unwrap();
    let outcome = doctor::run_openspec_advisory(
        root.path(),
        "/bin/sh",
        &[
            "-c",
            "yes stdout | head -c 1048576; yes stderr | head -c 1048576 >&2; exit 1",
        ],
        Duration::from_secs(2),
    );

    let doctor::OpenSpecAdvisory::ValidationFailed(summary) = outcome else {
        panic!("expected validation failure, got {outcome:?}");
    };
    assert!(summary.len() <= 16 * 1024);
    assert!(summary.contains("stdout"));
}

#[test]
fn spec_doctor_flags_missing_proposal_and_delta() {
    let root = TempDir::new().unwrap();
    let change = root.path().join("docs/changes/broken");
    fs::create_dir_all(&change).unwrap();
    fs::write(change.join("tasks.md"), "- [ ] task\n").unwrap();

    let code = doctor(&root.path().to_string_lossy(), false).unwrap();

    assert_eq!(code, 1);
}

#[test]
fn spec_doctor_flags_an_incomplete_transaction_directory() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("docs/.rune-transaction")).unwrap();

    let code = doctor(&root.path().to_string_lossy(), false).unwrap();

    assert_eq!(code, 1);
}

#[test]
fn spec_doctor_reports_archive_lock_contention() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("docs")).unwrap();
    let spec_root = resolve_spec_root(root.path()).unwrap();
    let lock_path = spec_root.base().join(".rune-archive.lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    lock.try_lock().unwrap();

    let findings = transaction::health_findings(&spec_root).unwrap();

    assert!(
        findings
            .iter()
            .any(|finding| finding.message.contains("holds the spec archive lock"))
    );
}

#[test]
fn spec_doctor_passes_a_healthy_tree() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "healthy", "- [ ] task\n", DELTA);
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();

    let code = doctor(&root.path().to_string_lossy(), false).unwrap();

    assert_eq!(code, 0);
}

#[test]
fn list_sort_progress_orders_least_complete_first() {
    let root = TempDir::new().unwrap();
    write_change(root.path(), "aa-done", "- [x] a\n- [x] b\n", DELTA);
    write_change(root.path(), "zz-fresh", "- [ ] a\n- [ ] b\n", DELTA);

    let mut summaries = scan_changes(root.path()).unwrap();
    summaries.sort_by(|left, right| {
        left.completion_percent()
            .cmp(&right.completion_percent())
            .then_with(|| left.id.cmp(&right.id))
    });

    assert_eq!(summaries[0].id, "zz-fresh");
    assert_eq!(summaries[1].id, "aa-done");
}

#[test]
fn propose_deduplicates_repeated_capabilities() {
    let root = TempDir::new().unwrap();
    propose(
        &root.path().to_string_lossy(),
        "dupes",
        &["alpha".to_string(), "beta".to_string(), "alpha".to_string()],
        false,
        false,
    )
    .unwrap();

    let proposal = fs::read_to_string(root.path().join("docs/changes/dupes/proposal.md")).unwrap();
    assert_eq!(
        proposal.matches("- alpha").count(),
        1,
        "duplicate capability flags must list once: {proposal}"
    );
    assert!(root.path().join("docs/changes/dupes/specs/alpha").is_dir());
    assert!(root.path().join("docs/changes/dupes/specs/beta").is_dir());
}

#[test]
fn propose_with_design_scaffolds_and_context_includes_it() {
    let root = TempDir::new().unwrap();
    propose(&root.path().to_string_lossy(), "designed", &[], true, false).unwrap();

    assert!(
        root.path()
            .join("docs/changes/designed/design.md")
            .is_file()
    );
    let output = commands::load_context_output(root.path(), "designed").unwrap();
    assert!(
        output
            .design
            .as_deref()
            .is_some_and(|design| design.contains("Design")),
        "context must include the design body"
    );
}

#[test]
fn propose_prefers_a_repo_local_template_override() {
    let root = TempDir::new().unwrap();
    let overrides = root.path().join("templates/spec");
    fs::create_dir_all(&overrides).unwrap();
    fs::write(
        overrides.join("proposal.md"),
        "# Custom ${CHANGE_TITLE}\n\nOverride template body for ${CAPABILITY}.\n",
    )
    .unwrap();

    propose(
        &root.path().to_string_lossy(),
        "custom-change",
        &["widgets".to_string()],
        false,
        false,
    )
    .unwrap();

    let proposal =
        fs::read_to_string(root.path().join("docs/changes/custom-change/proposal.md")).unwrap();
    assert!(proposal.starts_with("# Custom Custom Change"));
    assert!(proposal.contains("Override template body for widgets."));
}

#[test]
fn list_specs_counts_canonical_requirements() {
    let root = TempDir::new().unwrap();
    let canonical = root.path().join("docs/specs/search/spec.md");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::write(&canonical, ORIGINAL).unwrap();

    let summaries = scan_specifications(root.path()).unwrap();

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].capability, "search");
    assert_eq!(summaries[0].requirements, 2);
}

/// Golden captures of every output family, JSON and human. The JSON
/// bytes are the wire format scripts consume; the human bytes are what
/// terminals show. A failing test here means the observable output
/// changed: if that is intentional, regenerate the fixture with
/// `RUNE_UPDATE_FIXTURES=1 cargo test` and review the fixture diff.
mod output_fixtures {
    use super::super::*;

    fn assert_matches_fixture(rendered: &str, relative: &str) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        if std::env::var_os("RUNE_UPDATE_FIXTURES").is_some() {
            fs::write(&path, rendered).unwrap();
            return;
        }
        let fixture = fs::read_to_string(&path).unwrap();
        assert_eq!(
            rendered, fixture,
            "{relative} no longer matches; regenerate with RUNE_UPDATE_FIXTURES=1 if intentional"
        );
    }

    fn json_line(value: &impl Serialize) -> String {
        format!("{}\n", render_json(value).unwrap())
    }

    fn sample_propose() -> ProposeOutput {
        ProposeOutput {
            change: "add-widget".to_string(),
            capabilities: vec!["widget".to_string()],
            created: vec![
                "docs/changes/add-widget/proposal.md".to_string(),
                "docs/changes/add-widget/tasks.md".to_string(),
                "docs/changes/add-widget/specs/widget/spec.md".to_string(),
            ],
            template_overrides: vec!["templates/spec/proposal.md".to_string()],
            next_steps: vec![
                "Edit docs/changes/add-widget/proposal.md".to_string(),
                "Run rune spec validate add-widget".to_string(),
            ],
        }
    }

    fn sample_changes() -> Vec<ChangeSummary> {
        vec![
            ChangeSummary {
                id: "add-widget".to_string(),
                state: ChangeState::Draft,
                completed: 0,
                total: 4,
            },
            ChangeSummary {
                id: "extend-search".to_string(),
                state: ChangeState::Active,
                completed: 2,
                total: 4,
            },
            ChangeSummary {
                id: "retire-legacy-export".to_string(),
                state: ChangeState::Complete,
                completed: 4,
                total: 4,
            },
        ]
    }

    fn sample_specifications() -> Vec<SpecificationSummary> {
        vec![
            SpecificationSummary {
                capability: "widget".to_string(),
                requirements: 3,
            },
            SpecificationSummary {
                capability: "search/index".to_string(),
                requirements: 1,
            },
        ]
    }

    fn sample_context() -> ContextOutput {
        ContextOutput {
            id: "add-widget".to_string(),
            proposal: "Add a widget to the demo tool.\n".to_string(),
            design: Some("Widgets render inline.".to_string()),
            deltas: vec![ContextDelta {
                capability: "widget".to_string(),
                body: "## ADDED Requirements\n\n### Requirement: Widget rendering\n\nThe tool SHALL render widgets.\n".to_string(),
            }],
            tasks: vec![
                ContextTask {
                    text: "Scaffold the change".to_string(),
                    done: true,
                },
                ContextTask {
                    text: "Implement rendering".to_string(),
                    done: false,
                },
            ],
        }
    }

    fn sample_archive() -> ArchiveOutput {
        ArchiveOutput {
            change: "add-widget".to_string(),
            status: "merged",
            archived_to: "docs/changes/archive/2026-01-15-add-widget".to_string(),
            capabilities: vec!["widget".to_string()],
            merge: MergeSummary {
                added: 1,
                modified: 1,
                removed: 0,
                renamed: 1,
            },
            warnings: vec!["2 task(s) remain unchecked".to_string()],
        }
    }

    fn sample_diagnostics() -> Vec<SpecViolation> {
        vec![
            SpecViolation {
                code: "delta-missing-target".to_string(),
                severity: DiagnosticSeverity::Error,
                path: "docs/changes/add-widget/specs/widget/spec.md".to_string(),
                line: Some(7),
                column: Some(5),
                message: "MODIFIED requirement 'Widget rendering' does not exist".to_string(),
                operation: Some("modified".to_string()),
                capability: Some("widget".to_string()),
                change: Some("add-widget".to_string()),
            },
            SpecViolation {
                code: "scenario-missing".to_string(),
                severity: DiagnosticSeverity::Warning,
                path: "docs/specs/widget/spec.md".to_string(),
                line: None,
                column: None,
                message: "requirement has no scenarios".to_string(),
                operation: None,
                capability: None,
                change: None,
            },
        ]
    }

    fn sample_doctor() -> SpecDoctorOutput {
        SpecDoctorOutput {
            changes: 2,
            specs: 3,
            findings: vec![
                doctor::DoctorFinding {
                    severity: "error",
                    path: "docs/changes/add-widget/tasks.md".to_string(),
                    message: "tasks.md is missing".to_string(),
                },
                doctor::DoctorFinding {
                    severity: "warning",
                    path: "docs/changes/add-widget/design.md".to_string(),
                    message: "design.md has no content".to_string(),
                },
            ],
        }
    }

    fn sample_show_change() -> ShowChangeOutput {
        ShowChangeOutput {
            state: ChangeState::Active,
            completed: 1,
            total: 2,
            context: sample_context(),
        }
    }

    fn sample_show_specification() -> ShowSpecOutput {
        ShowSpecOutput {
            capability: "widget".to_string(),
            requirements: 1,
            content: "# Widget Specification\n\n## Requirements\n\n### Requirement: Widget rendering\n\nThe tool SHALL render widgets.\n".to_string(),
        }
    }

    #[test]
    fn propose_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_propose()),
            "tests/fixtures/output/propose.json",
        );
        assert_matches_fixture(
            &render_propose(&sample_propose()),
            "tests/fixtures/output/propose.txt",
        );
    }

    #[test]
    fn change_list_output_is_frozen() {
        let summaries = sample_changes();
        assert_matches_fixture(
            &json_line(&ListOutput {
                changes: summaries.clone(),
            }),
            "tests/fixtures/output/change-list.json",
        );
        let sheet = crate::sheet::Sheet::detect();
        assert_matches_fixture(
            &render_change_list(&summaries, &sheet),
            "tests/fixtures/output/change-list.txt",
        );
    }

    #[test]
    fn specification_list_output_is_frozen() {
        let summaries = sample_specifications();
        assert_matches_fixture(
            &json_line(&SpecsOutput {
                specs: summaries.clone(),
            }),
            "tests/fixtures/output/specification-list.json",
        );
        assert_matches_fixture(
            &render_specification_list(&summaries),
            "tests/fixtures/output/specification-list.txt",
        );
    }

    #[test]
    fn context_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_context()),
            "tests/fixtures/output/context.json",
        );
        assert_matches_fixture(
            &render_context(&sample_context()),
            "tests/fixtures/output/context.txt",
        );
    }

    #[test]
    fn context_without_tasks_is_frozen() {
        let mut context = sample_context();
        context.tasks.clear();
        assert_matches_fixture(
            &render_context(&context),
            "tests/fixtures/output/context-empty-tasks.txt",
        );
    }

    #[test]
    fn show_change_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_show_change()),
            "tests/fixtures/output/show-change.json",
        );
        assert_matches_fixture(
            &render_show_change(&sample_show_change()),
            "tests/fixtures/output/show-change.txt",
        );
    }

    #[test]
    fn show_specification_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_show_specification()),
            "tests/fixtures/output/show-specification.json",
        );
        assert_matches_fixture(
            &render_show_specification(&sample_show_specification(), "docs/specs/widget/spec.md"),
            "tests/fixtures/output/show-specification.txt",
        );
    }

    #[test]
    fn archive_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_archive()),
            "tests/fixtures/output/archive.json",
        );
        assert_matches_fixture(
            &render_archive(&sample_archive()),
            "tests/fixtures/output/archive.txt",
        );
    }

    #[test]
    fn abandoned_archive_output_is_frozen() {
        let mut output = sample_archive();
        output.status = "abandoned";
        assert_matches_fixture(
            &render_archive(&output),
            "tests/fixtures/output/archive-abandoned.txt",
        );
    }

    #[test]
    fn validate_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_diagnostics()),
            "tests/fixtures/output/validate.json",
        );
        assert_matches_fixture(
            &render_diagnostics(&sample_diagnostics()),
            "tests/fixtures/output/validate.txt",
        );
    }

    #[test]
    fn doctor_output_is_frozen() {
        assert_matches_fixture(
            &json_line(&sample_doctor()),
            "tests/fixtures/output/doctor.json",
        );
        assert_matches_fixture(
            &render_doctor(&sample_doctor()),
            "tests/fixtures/output/doctor.txt",
        );
    }

    #[test]
    fn healthy_doctor_output_is_frozen() {
        let output = SpecDoctorOutput {
            changes: 2,
            specs: 3,
            findings: Vec::new(),
        };
        assert_matches_fixture(
            &render_doctor(&output),
            "tests/fixtures/output/doctor-healthy.txt",
        );
    }
}
