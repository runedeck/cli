mod check;
mod mdschema_tool;
mod plugin;
mod repository;
mod schema;
pub(crate) mod templates;
mod tools;

use console::Style;
use rune::error::{Error, ErrorKind};
use rune::result::ActionResult;
use std::fs;
use std::path::Path;

/// Said once when strict structural checking did not run, whatever the reason.
///
/// Held as constants so a deck of many modules can recognise the notice a
/// child report already produced and keep only the first.
const MISSING_STANDALONE_CHECKER: &str = "standalone mdschema is unavailable, so only required-section presence, heading-level continuity, and maximum depth were checked. Section order, unexpected sections, permitted H3 placement, and heading uniqueness were NOT checked, and optional sections were skipped. Install it to check them: brew install jackchuka/tap/mdschema";
const UNUSABLE_STANDALONE_CHECKER: &str = "standalone mdschema is installed but could not be given a schema file, so only required-section presence, heading-level continuity, and maximum depth were checked. The temporary directory is the likely cause; the write error is on stderr.";
const REDUCED_CHECKING_ITEM: &str = "mdschema";

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

/// The one severity the CLI reasons in.
///
/// `rune::validate::Severity` is the library's finding severity, and this
/// alias keeps the CLI on it rather than maintaining a parallel copy that has
/// to be mapped at every boundary. The spec surface converts to
/// `rune_docs::spec::DiagnosticSeverity` when it prints, and nowhere else.
pub(crate) use rune::validate::Severity as ViolationSeverity;

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
    /// The standalone checker is missing for the whole run, not for one file,
    /// so its notice is emitted once. Repeating it per artifact buried the
    /// real findings and turned every passing item into a warning.
    reported_missing_standalone_checker: bool,
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

    /// Announce, once per run, that strict structural checking did not happen.
    ///
    /// Reported against the run rather than an artifact: a missing binary is a
    /// property of the machine, and naming a file invites the reader to think
    /// that file is at fault.
    fn report_missing_standalone_checker(&mut self) {
        self.report_reduced_checking_once(MISSING_STANDALONE_CHECKER);
    }

    /// The binary is installed but could not be handed a schema file.
    ///
    /// Distinct from the missing-binary notice: telling someone to install a
    /// tool they already have sends them looking in the wrong place.
    fn report_unusable_standalone_checker(&mut self) {
        self.report_reduced_checking_once(UNUSABLE_STANDALONE_CHECKER);
    }

    fn report_reduced_checking_once(&mut self, message: &str) {
        if self.reported_missing_standalone_checker {
            return;
        }
        self.reported_missing_standalone_checker = true;
        self.warn(REDUCED_CHECKING_ITEM, message.to_string());
    }

    fn diagnostic(&mut self, diagnostic: &rune::validate::Diagnostic) {
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
        let severity = diagnostic.severity;
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
pub fn execute(path: &str, json: bool, scan: bool, force: bool) -> Result<i32, Error> {
    let module_root = Path::new(path);
    let is_source = rune::deck::is_deck(module_root)
        || module_root.join("module.yaml").is_file()
        || module_root.join(".rune").is_file();
    if !is_source && !force {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{} is not a rune source (no deck.yaml, module.yaml, or .rune); pass --source <deck-or-module>, or --force to validate it anyway",
                module_root.display()
            ),
        ));
    }
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
    if rune::deck::is_deck(module_root) {
        let deck = rune::deck::load(module_root).map_err(Error::config)?;
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
        if module_root.join(".rune").exists() {
            let mut consumer_report = ValidationReport::default();
            consumer_checks(module_root, &mut consumer_report)?;
            append_report(&mut aggregate, consumer_report);
        }
        return Ok(aggregate);
    }
    let is_module = module_root.join("module.yaml").is_file();
    let is_consumer = module_root.join(".rune").exists();
    if is_consumer && !is_module {
        return validate_consumer(module_root, scan);
    }
    let mut report = validate_module(module_root, true, scan)?;
    if is_consumer {
        let mut consumer_report = ValidationReport::default();
        consumer_checks(module_root, &mut consumer_report)?;
        append_report(&mut report, consumer_report);
    }
    Ok(report)
}

/// Consumer roots (a `.rune` file, no deck or module manifest) hold deployed
/// provider trees, not source artifacts; module structure rules do not apply.
fn validate_consumer(consumer_root: &Path, scan: bool) -> Result<ValidationReport, Error> {
    let mut report = ValidationReport::default();
    consumer_checks(consumer_root, &mut report)?;
    tools::run_external_checks(consumer_root, scan, &mut report);
    Ok(report)
}

fn consumer_checks(consumer_root: &Path, report: &mut ValidationReport) -> Result<(), Error> {
    match crate::cli::dotrune::load(consumer_root) {
        Ok(_) => report.pass(".rune"),
        Err(error) => report.fail(".rune", format!(".rune: {error}")),
    }

    // The same provider set installation would use: the consumer's merged
    // config (custom targets, plugin: null) over the embedded defaults.
    let merged_config = crate::cli::config::load_merged_config(consumer_root)?;
    let providers = crate::cli::config::load_providers(&merged_config)?;
    let mut provider_targets: Vec<String> = providers
        .values()
        .flat_map(|provider| provider.target_roots().into_iter().map(ToString::to_string))
        .collect();
    provider_targets.sort();
    provider_targets.dedup();
    for target in provider_targets {
        let target_dir = consumer_root.join(&target);
        if !target_dir.is_dir() {
            continue;
        }
        if target_dir.join(".manifest").is_file() {
            report.pass(format!("{target}/.manifest"));
        } else {
            report.warn(
                format!("{target}/.manifest"),
                format!("{target}/.manifest: missing — run rune install to establish baseline"),
            );
        }
    }
    Ok(())
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
        let json_schema = schema::load_json_schema(&decisions_dir).map_err(Error::io)?;
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
        let entries = fs::read_dir(&skills_dir)
            .map_err(|error| Error::io(format!("cannot read {}: {error}", skills_dir.display())))?;
        let mut entries = entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| Error::io(format!("directory entry error: {error}")))?;
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
    let has_lifecycle = super::spec_root::specs_root(module_root)?.is_dir()
        || super::spec_root::changes_root(module_root)?.is_dir();
    if !has_lifecycle {
        return Ok(());
    }
    // A build without the spec feature must not silently pass a repo the full
    // build would fail: the same tree validating clean or dirty depending on
    // build flavor needs a visible reason.
    #[cfg(not(feature = "spec"))]
    {
        report.warn(
            "specifications",
            "spec tree present; lifecycle checks skipped (built without the spec feature)"
                .to_string(),
        );
        Ok(())
    }
    #[cfg(feature = "spec")]
    {
        check_spec_lifecycle_with_validator(module_root, report, super::spec::validate_spec_tree)
    }
}

#[cfg(feature = "spec")]
fn check_spec_lifecycle_with_validator(
    module_root: &Path,
    report: &mut ValidationReport,
    validator: fn(&Path) -> Result<Vec<super::spec::SpecViolation>, Error>,
) -> Result<(), Error> {
    let violations = validator(module_root)?;
    if violations.is_empty() {
        report.pass("specifications");
        return Ok(());
    }
    for violation in violations {
        let detail = violation.line.map_or_else(
            || violation.message.clone(),
            |line| format!("line {line}: {}", violation.message),
        );
        let rendered = format!(
            "{}{}: {}",
            violation.path,
            violation
                .line
                .map_or_else(String::new, |line| format!(":{line}")),
            violation.message
        );
        let (severity, status) = match violation.severity {
            super::spec::DiagnosticSeverity::Error => {
                report.result.errors.push(rendered);
                (ViolationSeverity::Error, ValidationStatus::Failed)
            }
            super::spec::DiagnosticSeverity::Warning => {
                report.result.warnings.push(rendered);
                (ViolationSeverity::Warning, ValidationStatus::Warning)
            }
        };
        report.push_violation_if_missing(
            &violation.path,
            severity,
            violation.message,
            violation.line,
        );
        report.record(violation.path, status, Some(detail));
    }
    Ok(())
}

fn append_report(aggregate: &mut ValidationReport, mut report: ValidationReport) {
    // A deck validates each module with its own report, so every module that
    // falls back would otherwise repeat the same machine-level notice. Keep the
    // first and drop the rest.
    if aggregate.reported_missing_standalone_checker {
        drop_reduced_checking_notice(&mut report);
    }

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
    aggregate.reported_missing_standalone_checker |= report.reported_missing_standalone_checker;
}

/// Remove a reduced-checking notice a child report produced.
///
/// Matches on the message rather than a flag because the notice has already
/// been written into three places by the time the report is merged.
fn drop_reduced_checking_notice(report: &mut ValidationReport) {
    let is_notice = |message: &str| {
        message.contains(MISSING_STANDALONE_CHECKER)
            || message.contains(UNUSABLE_STANDALONE_CHECKER)
    };

    report.result.warnings.retain(|warning| !is_notice(warning));
    report
        .violations
        .retain(|violation| !is_notice(&violation.message));
    report
        .items
        .retain(|item| !item.name.ends_with(REDUCED_CHECKING_ITEM));
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
    let diagnostics =
        rune::validate::validate_frontmatter(&yaml_as_frontmatter, module_schema, "module.yaml");

    for diagnostic in diagnostics {
        report.diagnostic(&diagnostic);
    }
}

#[cfg(test)]
mod tests;
