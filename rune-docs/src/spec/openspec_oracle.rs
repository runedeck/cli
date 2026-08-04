//! The `OpenSpec` v1.6.0 oracle suite: golden fixture cases under
//! `tests/fixtures/openspec/` proving that artifacts the pinned upstream
//! release accepts load, apply, and round-trip here without semantic loss.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/openspec");
const MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/openspec/manifest.yaml"
));
const INTENTIONAL_DIFFERENCES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/openspec/intentional-differences.md"
));

#[derive(Debug, Deserialize)]
struct CompatibilityManifest {
    version: u32,
    oracle: Oracle,
    cases: Vec<CompatibilityCase>,
}

#[derive(Debug, Deserialize)]
struct Oracle {
    project: String,
    revision: String,
    release: String,
}

#[derive(Debug, Deserialize)]
struct CompatibilityCase {
    name: String,
    kind: CaseKind,
    sources: Vec<String>,
    input: FixtureInput,
    expected: FixtureExpectation,
    upstream: Outcome,
    rune: Outcome,
    intentional_difference: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum CaseKind {
    Apply,
    Reject,
    RoundTrip,
}

#[derive(Debug, Deserialize)]
struct FixtureInput {
    canonical: Option<String>,
    delta: Option<String>,
    tree: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureExpectation {
    canonical: Option<String>,
    tree: Option<String>,
    diagnostics: Vec<ExpectedDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct ExpectedDiagnostic {
    code: String,
    severity: ExpectedSeverity,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ExpectedSeverity {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Accepted,
    Rejected,
}

#[test]
fn compatibility_manifest_references_pinned_existing_fixtures() {
    let manifest: CompatibilityManifest =
        serde_yaml::from_str(MANIFEST).expect("compatibility manifest must deserialize");
    let fixture_root = Path::new(FIXTURE_ROOT)
        .canonicalize()
        .expect("fixture root must exist");

    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.oracle.project, "Fission-AI/OpenSpec");
    assert_eq!(manifest.oracle.revision, "v1.6.0");
    assert_eq!(
        manifest.oracle.release,
        "https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0"
    );

    let mut case_names = BTreeSet::new();
    for compatibility_case in &manifest.cases {
        assert!(
            case_names.insert(compatibility_case.name.as_str()),
            "duplicate compatibility case: {}",
            compatibility_case.name
        );
        assert!(!compatibility_case.sources.is_empty());
        for source_url in &compatibility_case.sources {
            assert!(
                source_url.contains("/v1.6.0/"),
                "source is not pinned to v1.6.0: {source_url}"
            );
        }

        for relative_path in referenced_paths(compatibility_case) {
            assert_fixture_path(&fixture_root, relative_path);
        }

        for diagnostic in &compatibility_case.expected.diagnostics {
            assert!(!diagnostic.code.trim().is_empty());
            assert!(matches!(
                diagnostic.severity,
                ExpectedSeverity::Warning | ExpectedSeverity::Error
            ));
        }

        match compatibility_case.kind {
            CaseKind::Apply => {
                assert!(compatibility_case.input.delta.is_some());
                assert!(compatibility_case.expected.canonical.is_some());
            }
            CaseKind::Reject => {
                assert!(compatibility_case.input.delta.is_some());
                assert!(!compatibility_case.expected.diagnostics.is_empty());
            }
            CaseKind::RoundTrip => {
                assert!(compatibility_case.input.tree.is_some());
                assert!(compatibility_case.expected.tree.is_some());
            }
        }

        if compatibility_case.upstream != compatibility_case.rune {
            let difference = compatibility_case
                .intentional_difference
                .as_deref()
                .expect("differing outcomes need an intentional-difference entry");
            let heading = difference.replace('-', " ");
            assert!(
                INTENTIONAL_DIFFERENCES
                    .to_ascii_lowercase()
                    .contains(&heading),
                "missing intentional-difference section for {difference}"
            );
        }
    }
}

#[test]
fn all_operations_match_the_golden_specification() {
    let mut canonical =
        super::parse_canonical(&fixture_text("v1.6.0/cases/all-operations/input/spec.md"))
            .expect("canonical fixture must parse");
    let operations =
        super::parse_delta(&fixture_text("v1.6.0/cases/all-operations/input/delta.md"))
            .expect("delta fixture must parse");

    let applied = super::apply_delta(&mut canonical, &operations, "search")
        .expect("fixture operations must apply");

    assert_eq!(
        canonical.render(),
        fixture_text("v1.6.0/cases/all-operations/expected/spec.md")
    );
    assert_eq!(applied.summary.added, 1);
    assert_eq!(applied.summary.modified, 1);
    assert_eq!(applied.summary.removed, 1);
    assert_eq!(applied.summary.renamed, 1);
    assert!(applied.warnings.is_empty());
}

#[test]
fn new_capability_ignores_removals_and_matches_the_golden_specification() {
    let mut canonical = super::CanonicalSpec::new("search", "launch-search");
    let operations =
        super::parse_delta(&fixture_text("v1.6.0/cases/new-capability/input/delta.md"))
            .expect("delta fixture must parse");

    let applied = super::apply_delta(&mut canonical, &operations, "search")
        .expect("new-capability fixture must apply");

    assert_eq!(
        canonical.render(),
        fixture_text("v1.6.0/cases/new-capability/expected/spec.md")
    );
    assert_eq!(applied.summary.added, 1);
    assert_eq!(applied.summary.removed, 0);
    assert!(
        applied
            .warnings
            .iter()
            .any(|warning| warning.contains("REMOVED requirement"))
    );
}

#[test]
fn scenario_multiplicity_rejects_a_removed_occurrence() {
    let mut canonical = super::parse_canonical(&fixture_text(
        "v1.6.0/cases/scenario-multiplicity/input/spec.md",
    ))
    .expect("canonical fixture must parse");
    let operations = super::parse_delta(&fixture_text(
        "v1.6.0/cases/scenario-multiplicity/input/delta.md",
    ))
    .expect("delta fixture must parse");

    let Err(issue) = super::apply_delta(&mut canonical, &operations, "search") else {
        panic!("a removed scenario occurrence must fail");
    };

    assert!(issue.message.contains("removes scenario occurrence"));
    assert_eq!(
        canonical.render(),
        fixture_text("v1.6.0/cases/scenario-multiplicity/expected/spec.md")
    );
}

#[test]
fn fenced_operation_markers_remain_inert() {
    let mut canonical =
        super::parse_canonical(&fixture_text("v1.6.0/cases/fenced-operation/input/spec.md"))
            .expect("canonical fixture must parse");
    let operations = super::parse_delta(&fixture_text(
        "v1.6.0/cases/fenced-operation/input/delta.md",
    ))
    .expect("delta fixture must parse");

    super::apply_delta(&mut canonical, &operations, "search")
        .expect("fenced-operation fixture must apply");

    assert_eq!(
        canonical.render(),
        fixture_text("v1.6.0/cases/fenced-operation/expected/spec.md")
    );
}

#[test]
fn round_trip_golden_tree_preserves_paths_and_bytes() {
    let manifest: CompatibilityManifest =
        serde_yaml::from_str(MANIFEST).expect("compatibility manifest must deserialize");
    let round_trip = manifest
        .cases
        .iter()
        .find(|compatibility_case| compatibility_case.kind == CaseKind::RoundTrip)
        .expect("round-trip fixture must exist");
    let input_tree = Path::new(FIXTURE_ROOT).join(
        round_trip
            .input
            .tree
            .as_deref()
            .expect("round-trip input tree must be configured"),
    );
    let expected_tree = Path::new(FIXTURE_ROOT).join(
        round_trip
            .expected
            .tree
            .as_deref()
            .expect("round-trip expected tree must be configured"),
    );

    assert_eq!(
        collect_tree(&input_tree).expect("input tree must be readable"),
        collect_tree(&expected_tree).expect("expected tree must be readable")
    );
}

fn fixture_text(relative_path: &str) -> String {
    let fixture_path = Path::new(FIXTURE_ROOT).join(relative_path);
    fs::read_to_string(&fixture_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", fixture_path.display()))
}

fn referenced_paths(compatibility_case: &CompatibilityCase) -> impl Iterator<Item = &str> {
    [
        compatibility_case.input.canonical.as_deref(),
        compatibility_case.input.delta.as_deref(),
        compatibility_case.input.tree.as_deref(),
        compatibility_case.expected.canonical.as_deref(),
        compatibility_case.expected.tree.as_deref(),
    ]
    .into_iter()
    .flatten()
}

fn assert_fixture_path(fixture_root: &Path, relative_path: &str) {
    let declared_path = Path::new(relative_path);
    assert!(!declared_path.is_absolute());
    assert!(
        declared_path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_))),
        "fixture path escapes the corpus: {relative_path}"
    );
    let canonical_path = fixture_root
        .join(declared_path)
        .canonicalize()
        .unwrap_or_else(|error| panic!("fixture path is missing: {relative_path}: {error}"));
    assert!(
        canonical_path.starts_with(fixture_root),
        "fixture path escapes through a symlink: {relative_path}"
    );
}

fn collect_tree(root_path: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let canonical_root = root_path
        .canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", root_path.display()))?;
    let mut files = BTreeMap::new();
    collect_tree_files(&canonical_root, &canonical_root, &mut files)?;
    Ok(files)
}

fn collect_tree_files(
    canonical_root: &Path,
    current_path: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), String> {
    let entries = fs::read_dir(current_path)
        .map_err(|error| format!("failed to read {}: {error}", current_path.display()))?;
    for entry_result in entries {
        let entry = entry_result
            .map_err(|error| format!("failed to read {} entry: {error}", current_path.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "fixture trees cannot contain symlinks: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_tree_files(canonical_root, &entry.path(), files)?;
            continue;
        }
        if file_type.is_file() {
            let relative_path = entry
                .path()
                .strip_prefix(canonical_root)
                .map_err(|error| format!("failed to relativize fixture path: {error}"))?
                .to_path_buf();
            let content = fs::read(entry.path())
                .map_err(|error| format!("failed to read {}: {error}", entry.path().display()))?;
            files.insert(relative_path, content);
        }
    }
    Ok(())
}
