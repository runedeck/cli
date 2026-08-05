//! Specification validation: schema checks through the installed mdschema
//! bridge, parser diagnostics, cross-artifact evaluation, and the opaque
//! interop artifacts surfaced as warnings. One pipeline serves `validate`,
//! archive preflight, and doctor, so acceptance cannot differ by command.

use super::templates::{DELTA_SPEC_MDSCHEMA, SPEC_MDSCHEMA};
use super::{
    DiagnosticSeverity, PrefixMatch, SpecRoot, SpecViolation, changes_root,
    discover_capabilities_below, evaluate_change, load_with_override, parse_canonical,
    parse_diagnostics, prefix_match, print_json, read, read_directories, relative_display,
    resolve_spec_root, specs_root, validate_capability,
};
use crate::error::{Error, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdschemaDiagnostic {
    pub file: String,
    pub line: Option<usize>,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

pub type MdschemaCheck =
    fn(content: &str, file_path: &str, schema: &str) -> Vec<MdschemaDiagnostic>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValidationTarget {
    Tree,
    Change(String),
    Specification(String),
}

/// Validate the selected specification tree without printing.
pub fn validate_spec_tree(
    root: &Path,
    mdschema_check: MdschemaCheck,
) -> Result<Vec<SpecViolation>, Error> {
    validate_spec_target(root, None, mdschema_check)
}

/// Validate the selected specification tree or one resolved change or capability.
pub fn validate(
    source: &str,
    name: Option<&str>,
    json: bool,
    mdschema_check: MdschemaCheck,
) -> Result<i32, Error> {
    let diagnostics = validate_output(source, name, mdschema_check)?;
    let has_errors = diagnostics.iter().any(SpecViolation::is_error);
    if json {
        print_json(&diagnostics)?;
    } else if diagnostics.is_empty() {
        println!("specification validation passed");
    } else {
        print!("{}", render_diagnostics(&diagnostics));
    }
    Ok(i32::from(has_errors))
}

/// Validate and return diagnostics, without printing. The caller derives
/// the exit state via [`SpecViolation::is_error`].
pub fn validate_output(
    source: &str,
    name: Option<&str>,
    mdschema_check: MdschemaCheck,
) -> Result<Vec<SpecViolation>, Error> {
    validate_spec_target(Path::new(source), name, mdschema_check)
}

pub fn render_diagnostics(diagnostics: &[SpecViolation]) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
        };
        let location = diagnostic.line.map_or_else(
            || diagnostic.path.clone(),
            |line| format!("{}:{line}", diagnostic.path),
        );
        let _ = writeln!(
            rendered,
            "{severity}[{}]: {location}: {}",
            diagnostic.code, diagnostic.message
        );
    }
    rendered
}

pub(super) fn validate_spec_target(
    root: &Path,
    name: Option<&str>,
    mdschema_check: MdschemaCheck,
) -> Result<Vec<SpecViolation>, Error> {
    let spec_root = resolve_spec_root(root)?;
    let repository = spec_root.repository();
    let target = resolve_validation_target(root, name)?;
    let (spec_schema, _) = load_with_override(repository, "schemas/spec.mdschema", SPEC_MDSCHEMA)?;
    let (delta_schema, _) = load_with_override(
        repository,
        "schemas/delta-spec.mdschema",
        DELTA_SPEC_MDSCHEMA,
    )?;
    let mut diagnostics = Vec::new();

    match &target {
        ValidationTarget::Tree => {
            for (capability, spec_path) in discover_validation_capabilities(
                spec_root.specifications(),
                repository,
                None,
                &mut diagnostics,
            )? {
                validate_canonical(
                    repository,
                    &capability,
                    &spec_path,
                    &spec_schema,
                    mdschema_check,
                    &mut diagnostics,
                )?;
            }
            for change_dir in active_change_directories(spec_root.changes())? {
                validate_change(
                    &spec_root,
                    &change_dir,
                    &spec_schema,
                    &delta_schema,
                    false,
                    mdschema_check,
                    &mut diagnostics,
                )?;
            }
            append_opaque_artifacts(&spec_root, &mut diagnostics);
        }
        ValidationTarget::Change(change) => {
            validate_change(
                &spec_root,
                &spec_root.changes().join(change),
                &spec_schema,
                &delta_schema,
                true,
                mdschema_check,
                &mut diagnostics,
            )?;
        }
        ValidationTarget::Specification(capability) => {
            validate_canonical(
                repository,
                capability,
                &spec_root.specifications().join(capability).join("spec.md"),
                &spec_schema,
                mdschema_check,
                &mut diagnostics,
            )?;
        }
    }

    Ok(diagnostics)
}

fn resolve_validation_target(root: &Path, name: Option<&str>) -> Result<ValidationTarget, Error> {
    let Some(name) = name else {
        return Ok(ValidationTarget::Tree);
    };
    validate_capability(name, "item name")?;
    let change_dir = changes_root(root)?.join(name);
    let specification = specs_root(root)?.join(name).join("spec.md");
    let change_exists = !name.contains('/') && change_dir.is_dir();
    match (change_exists, specification.is_file()) {
        (true, true) => Err(Error::new(
            ErrorKind::Config,
            format!("'{name}' is both an active change and a capability specification"),
        )),
        (true, false) => Ok(ValidationTarget::Change(name.to_string())),
        (false, true) => Ok(ValidationTarget::Specification(name.to_string())),
        (false, false) => match prefix_match(root, name)? {
            PrefixMatch::Change(change) => Ok(ValidationTarget::Change(change)),
            PrefixMatch::Specification(capability) => {
                Ok(ValidationTarget::Specification(capability))
            }
            PrefixMatch::Ambiguous(candidates) => Err(Error::new(
                ErrorKind::Config,
                format!(
                    "'{name}' matches more than one item: {}",
                    candidates.join(", ")
                ),
            )),
            PrefixMatch::None => Err(Error::new(
                ErrorKind::Config,
                format!("no active change or capability specification named '{name}'"),
            )),
        },
    }
}

fn active_change_directories(changes: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(read_directories(changes)?
        .into_iter()
        .filter(|path| path.file_name().is_none_or(|name| name != "archive"))
        .collect())
}

fn discover_validation_capabilities(
    base: &Path,
    repository: &Path,
    change: Option<&str>,
    diagnostics: &mut Vec<SpecViolation>,
) -> Result<Vec<(String, PathBuf)>, Error> {
    let root_specification = base.join("spec.md");
    if root_specification.is_file() {
        diagnostics.push(SpecViolation {
            code: "spec-root-invalid".to_string(),
            severity: DiagnosticSeverity::Error,
            path: relative_display(repository, &root_specification),
            line: None,
            column: None,
            message: "root-level specification has no capability identifier".to_string(),
            operation: None,
            capability: None,
            change: change.map(str::to_string),
        });
    }
    let mut capabilities = Vec::new();
    discover_capabilities_below(base, base, &mut capabilities)?;
    capabilities.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(capabilities)
}

#[derive(Clone, Copy)]
struct SchemaDiagnosticContext<'context> {
    repository: &'context Path,
    path: &'context Path,
    code: &'context str,
    capability: Option<&'context str>,
    change: Option<&'context str>,
}

fn validate_canonical(
    repository: &Path,
    capability: &str,
    path: &Path,
    schema: &str,
    mdschema_check: MdschemaCheck,
    diagnostics: &mut Vec<SpecViolation>,
) -> Result<(), Error> {
    let content = read(path)?;
    append_schema_diagnostics(
        SchemaDiagnosticContext {
            repository,
            path,
            code: "spec-schema-invalid",
            capability: Some(capability),
            change: None,
        },
        &content,
        schema,
        mdschema_check,
        diagnostics,
    );
    if let Err(issues) = parse_canonical(&content) {
        diagnostics.extend(parse_diagnostics(
            repository,
            path,
            "spec-parse-invalid",
            capability,
            None,
            issues,
        ));
    }
    Ok(())
}

fn validate_change(
    spec_root: &SpecRoot,
    change_dir: &Path,
    spec_schema: &str,
    delta_schema: &str,
    validate_referenced_canonical_schema: bool,
    mdschema_check: MdschemaCheck,
    diagnostics: &mut Vec<SpecViolation>,
) -> Result<(), Error> {
    let Some(change) = change_dir.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    for (capability, delta_path) in discover_validation_capabilities(
        &change_dir.join("specs"),
        spec_root.repository(),
        Some(change),
        diagnostics,
    )? {
        let content = read(&delta_path)?;
        append_schema_diagnostics(
            SchemaDiagnosticContext {
                repository: spec_root.repository(),
                path: &delta_path,
                code: "delta-schema-invalid",
                capability: Some(&capability),
                change: Some(change),
            },
            &content,
            delta_schema,
            mdschema_check,
            diagnostics,
        );
    }
    let evaluation = evaluate_change(spec_root, change_dir)?;
    if validate_referenced_canonical_schema {
        for (capability, path) in &evaluation.canonical_artifacts {
            let content = read(path)?;
            append_schema_diagnostics(
                SchemaDiagnosticContext {
                    repository: spec_root.repository(),
                    path,
                    code: "spec-schema-invalid",
                    capability: Some(capability),
                    change: Some(change),
                },
                &content,
                spec_schema,
                mdschema_check,
                diagnostics,
            );
        }
    }
    append_unique_diagnostics(diagnostics, evaluation.diagnostics);
    Ok(())
}

fn append_schema_diagnostics(
    diagnostic_fields: SchemaDiagnosticContext<'_>,
    content: &str,
    schema: &str,
    mdschema_check: MdschemaCheck,
    diagnostics: &mut Vec<SpecViolation>,
) {
    let relative = relative_display(diagnostic_fields.repository, diagnostic_fields.path);
    diagnostics.extend(
        mdschema_check(content, &relative, schema)
            .into_iter()
            .map(|diagnostic| SpecViolation {
                code: diagnostic_fields.code.to_string(),
                severity: diagnostic.severity,
                path: diagnostic.file,
                line: diagnostic.line,
                column: None,
                message: diagnostic.message,
                operation: None,
                capability: diagnostic_fields.capability.map(str::to_string),
                change: diagnostic_fields.change.map(str::to_string),
            }),
    );
}

fn append_unique_diagnostics(diagnostics: &mut Vec<SpecViolation>, incoming: Vec<SpecViolation>) {
    for diagnostic in incoming {
        let duplicate = diagnostics.iter().any(|existing| {
            existing.code == diagnostic.code
                && existing.path == diagnostic.path
                && existing.line == diagnostic.line
                && existing.message == diagnostic.message
        });
        if !duplicate {
            diagnostics.push(diagnostic);
        }
    }
}

fn append_opaque_artifacts(spec_root: &SpecRoot, diagnostics: &mut Vec<SpecViolation>) {
    match crate::interop::opaque_artifacts(spec_root) {
        Ok(artifacts) => diagnostics.extend(artifacts.into_iter().map(|artifact| SpecViolation {
            code: "opaque-artifact".to_string(),
            severity: DiagnosticSeverity::Warning,
            path: artifact.path,
            line: None,
            column: None,
            message: format!(
                "opaque OpenSpec artifact classified as {}",
                artifact.classification.name()
            ),
            operation: None,
            capability: None,
            change: None,
        })),
        Err(error) => diagnostics.push(SpecViolation {
            code: "artifact-invalid".to_string(),
            severity: DiagnosticSeverity::Error,
            path: relative_display(
                spec_root.repository(),
                &spec_root.base().join(".interop/openspec/manifest.yaml"),
            ),
            line: None,
            column: None,
            message: error.message().to_string(),
            operation: None,
            capability: None,
            change: None,
        }),
    }
}
