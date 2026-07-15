mod check;
mod plugin;
mod repository;
mod schema;
pub(crate) mod templates;
mod tools;

use commands::error::{Error, ErrorKind};
use commands::result::ActionResult;
use console::Style;
use std::fs;
use std::path::Path;

const REQUIRED_FILES: &[&str] = &["module.yaml", "defaults.yaml", "README.md", "LICENSE"];
const OPTIONAL_FILES: &[&str] = &[
    "INSTALL.md",
    "CONTRIBUTING.md",
    "CODEOWNERS",
    "CHANGELOG.md",
    ".gitattributes",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ValidationStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Debug)]
struct ValidationItem {
    name: String,
    status: ValidationStatus,
    detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViolationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationViolation {
    pub(crate) artifact: String,
    pub(crate) line: Option<usize>,
    pub(crate) severity: ViolationSeverity,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SourceValidationReport {
    pub(crate) checked: usize,
    pub(crate) violations: Vec<ValidationViolation>,
}

#[derive(Debug, Default)]
struct ValidationReport {
    result: ActionResult,
    items: Vec<ValidationItem>,
    violations: Vec<ValidationViolation>,
}

impl ValidationReport {
    fn checkpoint(&self) -> (usize, usize) {
        (self.result.errors.len(), self.result.warnings.len())
    }

    fn record_since(&mut self, name: impl Into<String>, checkpoint: (usize, usize)) {
        let name = name.into();
        let new_errors = self.result.errors[checkpoint.0..].to_vec();
        let new_warnings = self.result.warnings[checkpoint.1..].to_vec();
        let errors = self.result.errors[checkpoint.0..].join("; ");
        let warnings = self.result.warnings[checkpoint.1..].join("; ");
        if !errors.is_empty() {
            self.record(&name, ValidationStatus::Failed, Some(errors));
        } else if !warnings.is_empty() {
            self.record(&name, ValidationStatus::Warning, Some(warnings));
        } else {
            self.record(&name, ValidationStatus::Passed, None);
        }
        for message in new_errors {
            self.push_violation_if_missing(&name, ViolationSeverity::Error, message, None);
        }
        for message in new_warnings {
            self.push_violation_if_missing(&name, ViolationSeverity::Warning, message, None);
        }
    }

    fn pass(&mut self, name: impl Into<String>) {
        self.record(name, ValidationStatus::Passed, None);
    }

    fn warn(&mut self, name: impl Into<String>, message: String) {
        let name = name.into();
        self.result.warnings.push(message.clone());
        self.push_violation_if_missing(&name, ViolationSeverity::Warning, message.clone(), None);
        self.record(name, ValidationStatus::Warning, Some(message));
    }

    fn fail(&mut self, name: impl Into<String>, message: String) {
        let name = name.into();
        self.result.errors.push(message.clone());
        self.push_violation_if_missing(&name, ViolationSeverity::Error, message.clone(), None);
        self.record(name, ValidationStatus::Failed, Some(message));
    }

    fn record(
        &mut self,
        name: impl Into<String>,
        status: ValidationStatus,
        detail: Option<String>,
    ) {
        let name = name.into();
        if let Some(item) = self.items.iter_mut().find(|item| item.name == name) {
            if status > item.status {
                item.status = status;
                item.detail = detail;
            } else if status == item.status
                && let Some(detail) = detail
                && item.detail.as_deref() != Some(&detail)
            {
                if let Some(current) = &mut item.detail {
                    current.push_str("; ");
                    current.push_str(&detail);
                } else {
                    item.detail = Some(detail);
                }
            }
            return;
        }
        self.items.push(ValidationItem {
            name,
            status,
            detail,
        });
    }

    fn push_violation_if_missing(
        &mut self,
        artifact: &str,
        severity: ViolationSeverity,
        message: String,
        line: Option<usize>,
    ) {
        if self
            .violations
            .iter()
            .any(|violation| violation.severity == severity && violation.message == message)
        {
            return;
        }
        self.violations.push(ValidationViolation {
            artifact: artifact.to_string(),
            line,
            severity,
            message,
        });
    }

    fn diagnostic(&mut self, diagnostic: &commands::validate::Diagnostic) {
        let detail = match diagnostic.line {
            Some(line) => format!(
                "{}:{line}: {} ({:?})",
                diagnostic.file, diagnostic.message, diagnostic.severity
            ),
            None => format!(
                "{}: {} ({:?})",
                diagnostic.file, diagnostic.message, diagnostic.severity
            ),
        };
        let severity = match diagnostic.severity {
            commands::validate::Severity::Error => ViolationSeverity::Error,
            commands::validate::Severity::Warning => ViolationSeverity::Warning,
        };
        match severity {
            ViolationSeverity::Error => self.result.errors.push(detail.clone()),
            ViolationSeverity::Warning => self.result.warnings.push(detail.clone()),
        }
        self.push_violation_if_missing(&diagnostic.file, severity, detail, diagnostic.line);
    }
}

/// Validate module structure and content files against schemas, print the
/// selected output format, and return the process exit code.
///
/// Checks:
///   - Required/optional files from validation config
///   - agents/, rules/ — frontmatter against `.schema.yaml`, structure against `.mdschema`
///   - skills/ — recurses into subdirectories, checks `.mdschema`
pub fn execute(path: &str, json: bool, scan: bool) -> Result<i32, Error> {
    let report = validate(path, scan)?;
    print_report(&report, json);
    Ok(i32::from(report.result.has_errors()))
}

/// Validate a source without printing, for live consumers such as the TUI.
/// Security scanners stay off: they belong to commit and push hooks.
pub(crate) fn validate_source(path: &Path) -> Result<SourceValidationReport, Error> {
    let report = validate(&path.to_string_lossy(), false)?;
    Ok(SourceValidationReport {
        checked: report.items.len(),
        violations: report.violations,
    })
}

fn validate(path: &str, scan: bool) -> Result<ValidationReport, Error> {
    let module_root = Path::new(path);
    if commands::deck::is_deck(module_root) {
        let deck = commands::deck::load(module_root)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
        let mut aggregate = ValidationReport::default();
        check_spec_lifecycle(module_root, &mut aggregate)?;
        for deck_entry in deck.entries {
            let mut deck_entry_report = match validate_module(&deck_entry.root, false, scan) {
                Ok(result) => result,
                Err(error) => {
                    aggregate.fail(&deck_entry.name, format!("{}: {error}", deck_entry.name));
                    continue;
                }
            };
            deck_entry_report.result.errors = deck_entry_report
                .result
                .errors
                .into_iter()
                .map(|error| format!("{}: {error}", deck_entry.name))
                .collect();
            for item in &mut deck_entry_report.items {
                item.name = format!("{}/{}", deck_entry.name, item.name);
            }
            for violation in &mut deck_entry_report.violations {
                violation.artifact = format!("{}/{}", deck_entry.name, violation.artifact);
            }
            append_report(&mut aggregate, deck_entry_report);
        }
        return Ok(aggregate);
    }
    validate_module(module_root, true, scan)
}

fn validate_module(
    module_root: &Path,
    check_deploy_baseline: bool,
    scan: bool,
) -> Result<ValidationReport, Error> {
    let mut report = ValidationReport::default();

    check_module_structure(module_root, &mut report);
    let checkpoint = report.checkpoint();
    check_module_yaml(module_root, &mut report);
    if module_root.join("module.yaml").is_file() {
        report.record_since("module.yaml", checkpoint);
    }
    if check_deploy_baseline {
        repository::check_template_drift(module_root, &mut report);
    }

    for kind in &["agents", "rules"] {
        let dir = module_root.join(kind);
        if dir.is_dir() {
            check::flat_directory(&dir, module_root, kind, &mut report)?;
        }
    }

    // ADR directory — validate against JSON schema if available
    let decisions_dir = module_root.join("docs").join("decisions");
    if decisions_dir.is_dir() {
        let json_schema = schema::load_json_schema(&decisions_dir);
        check::flat_directory_with_json_schema(
            &decisions_dir,
            module_root,
            "decisions",
            Some(&json_schema),
            &mut report,
        )?;
    }

    // Skills have subdirectories — iterate and validate each
    let skills_dir = module_root.join("skills");
    if skills_dir.is_dir() {
        let entries = fs::read_dir(&skills_dir).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {e}", skills_dir.display()),
            )
        })?;
        let mut entries = entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;
        entries.sort_by_key(std::fs::DirEntry::path);

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                check::skill_directory(&path, module_root, &mut report)?;
            }
        }
    }

    check_spec_lifecycle(module_root, &mut report)?;

    plugin::check_plugin_scaffolding(module_root, &mut report);

    tools::run_external_checks(module_root, scan, &mut report);

    Ok(report)
}

fn check_spec_lifecycle(module_root: &Path, report: &mut ValidationReport) -> Result<(), Error> {
    let has_lifecycle =
        module_root.join("docs/specs").is_dir() || module_root.join("docs/changes").is_dir();
    if !has_lifecycle {
        return Ok(());
    }
    let violations = super::spec::validate_spec_tree(module_root)?;
    if violations.is_empty() {
        report.pass("specifications");
        return Ok(());
    }
    for violation in violations {
        let detail = violation.line.map_or_else(
            || violation.message.clone(),
            |line| format!("line {line}: {}", violation.message),
        );
        report.result.errors.push(format!(
            "{}{}: {}",
            violation.path,
            violation
                .line
                .map_or_else(String::new, |line| format!(":{line}")),
            violation.message
        ));
        report.push_violation_if_missing(
            &violation.path,
            ViolationSeverity::Error,
            violation.message,
            violation.line,
        );
        report.record(violation.path, ValidationStatus::Failed, Some(detail));
    }
    Ok(())
}

fn append_report(aggregate: &mut ValidationReport, mut report: ValidationReport) {
    aggregate
        .result
        .installed
        .append(&mut report.result.installed);
    aggregate.result.skipped.append(&mut report.result.skipped);
    aggregate.result.pruned.append(&mut report.result.pruned);
    aggregate
        .result
        .warnings
        .append(&mut report.result.warnings);
    aggregate.result.errors.append(&mut report.result.errors);
    aggregate.items.append(&mut report.items);
    aggregate.violations.append(&mut report.violations);
}

fn check_module_structure(module_root: &Path, report: &mut ValidationReport) {
    for filename in REQUIRED_FILES {
        if module_root.join(filename).is_file() {
            report.pass(*filename);
        } else {
            report.fail(*filename, format!("missing required file: {filename}"));
        }
    }

    for filename in OPTIONAL_FILES {
        if module_root.join(filename).is_file() {
            report.pass(*filename);
        }
    }
}

fn print_report(report: &ValidationReport, json: bool) {
    if json {
        match serde_json::to_string_pretty(&report.result) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize result: {error}"),
        }
        return;
    }

    let bold = Style::new().bold();
    let dim = Style::new().dim();
    let green = Style::new().green();
    let yellow = Style::new().yellow();
    let red = Style::new().red();

    println!();
    println!(" {}", bold.apply_to("validation"));
    for item in &report.items {
        match item.status {
            ValidationStatus::Passed => {
                println!("   {} {}", green.apply_to("✓"), dim.apply_to(&item.name));
            }
            ValidationStatus::Warning => {
                print_item_with_detail(item, &yellow, &dim, "⚡");
            }
            ValidationStatus::Failed => {
                print_item_with_detail(item, &red, &dim, "✗");
            }
        }
    }

    println!();
    let checked = report.items.len();
    let warnings = report.result.warnings.len();
    let errors = report.result.errors.len();
    println!(
        " {} {} checked  {} {} {}  {} {} {}",
        green.apply_to("✓"),
        checked,
        yellow.apply_to("⚡"),
        warnings,
        if warnings == 1 { "warning" } else { "warnings" },
        red.apply_to("✗"),
        errors,
        if errors == 1 { "error" } else { "errors" },
    );
    println!();
}

fn print_item_with_detail(item: &ValidationItem, style: &Style, dim: &Style, symbol: &str) {
    if let Some(detail) = &item.detail {
        println!(
            "   {} {} {} {}",
            style.apply_to(symbol),
            item.name,
            dim.apply_to("—"),
            style.apply_to(detail),
        );
    } else {
        println!("   {} {}", style.apply_to(symbol), item.name);
    }
}

fn check_module_yaml(module_root: &Path, report: &mut ValidationReport) {
    let module_yaml_path = module_root.join("module.yaml");
    if !module_yaml_path.is_file() {
        return;
    }

    let Some(module_schema) = schema::embedded_schema("module") else {
        return;
    };

    let Ok(content) = fs::read_to_string(&module_yaml_path) else {
        return;
    };

    let yaml_as_frontmatter = format!("---\n{content}---\n");
    let diagnostics = commands::validate::validate_frontmatter(
        &yaml_as_frontmatter,
        module_schema,
        "module.yaml",
    );

    for diagnostic in diagnostics {
        report.diagnostic(&diagnostic);
    }
}

#[cfg(test)]
mod tests;
