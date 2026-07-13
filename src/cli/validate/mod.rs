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

#[derive(Debug, Default)]
struct ValidationReport {
    result: ActionResult,
    items: Vec<ValidationItem>,
}

impl ValidationReport {
    fn checkpoint(&self) -> (usize, usize) {
        (self.result.errors.len(), self.result.warnings.len())
    }

    fn record_since(&mut self, name: impl Into<String>, checkpoint: (usize, usize)) {
        let errors = self.result.errors[checkpoint.0..].join("; ");
        let warnings = self.result.warnings[checkpoint.1..].join("; ");
        if !errors.is_empty() {
            self.record(name, ValidationStatus::Failed, Some(errors));
        } else if !warnings.is_empty() {
            self.record(name, ValidationStatus::Warning, Some(warnings));
        } else {
            self.record(name, ValidationStatus::Passed, None);
        }
    }

    fn pass(&mut self, name: impl Into<String>) {
        self.record(name, ValidationStatus::Passed, None);
    }

    fn warn(&mut self, name: impl Into<String>, message: String) {
        self.result.warnings.push(message.clone());
        self.record(name, ValidationStatus::Warning, Some(message));
    }

    fn fail(&mut self, name: impl Into<String>, message: String) {
        self.result.errors.push(message.clone());
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
}

/// Validate module structure and content files against schemas, print the
/// selected output format, and return the process exit code.
///
/// Checks:
///   - Required/optional files from validation config
///   - agents/, rules/ — frontmatter against `.schema.yaml`, structure against `.mdschema`
///   - skills/ — recurses into subdirectories, checks `.mdschema`
pub fn execute(path: &str, json: bool) -> Result<i32, Error> {
    let report = validate(path)?;
    print_report(&report, json);
    Ok(i32::from(report.result.has_errors()))
}

fn validate(path: &str) -> Result<ValidationReport, Error> {
    let module_root = Path::new(path);
    if commands::deck::is_deck(module_root) {
        let deck = commands::deck::load(module_root)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
        let mut aggregate = ValidationReport::default();
        for deck_entry in deck.entries {
            let mut deck_entry_report = match validate_module(&deck_entry.root) {
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
            append_report(&mut aggregate, deck_entry_report);
        }
        return Ok(aggregate);
    }
    validate_module(module_root)
}

fn validate_module(module_root: &Path) -> Result<ValidationReport, Error> {
    let mut report = ValidationReport::default();

    check_module_structure(module_root, &mut report);
    let checkpoint = report.checkpoint();
    check_module_yaml(module_root, &mut report.result);
    if module_root.join("module.yaml").is_file() {
        report.record_since("module.yaml", checkpoint);
    }
    repository::check_template_drift(module_root, &mut report);

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

    plugin::check_plugin_scaffolding(module_root, &mut report);

    tools::run_external_checks(module_root, &mut report);

    Ok(report)
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

fn check_module_yaml(module_root: &Path, result: &mut ActionResult) {
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
        result.errors.push(format!(
            "{}: {} ({:?})",
            diagnostic.file, diagnostic.message, diagnostic.severity
        ));
    }
}

#[cfg(test)]
mod tests;
