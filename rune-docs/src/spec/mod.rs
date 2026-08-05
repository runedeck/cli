//! Native specification-change lifecycle under `docs/`.
//!
//! Module map, by concern:
//!
//! - Public command surface: [`commands`] (propose, list, context, archive,
//!   show), [`validate`], [`doctor`]; each command has an output-returning
//!   `*_output` form, a pure `render_*` form, and a printing wrapper.
//! - Internal pipeline: [`parse`] and [`model`] read and edit canonical
//!   Markdown, [`apply`] merges deltas, [`transaction`] makes archive and
//!   conversion crash-safe, [`root`] resolves the spec root (shared source
//!   with the CLI fallback), [`tasks`] reads checklists, [`templates`]
//!   embeds scaffolding.
//! - This file keeps the shared vocabulary (summaries, diagnostics) and
//!   the file/discovery plumbing every submodule uses.
//! - Tests: unit tests at the bottom of each module; the `OpenSpec` v1.6.0
//!   oracle suite is the test-only `openspec_oracle` module; golden output
//!   fixtures live under `tests/fixtures/output/`.
//!
//! Finding types, one per layer; they are deliberately separate and never
//! merge into one enum:
//!
//! - [`SpecViolation`]: the public validation diagnostic and its `--json`
//!   wire format (explicit nulls); produced by `validate` and consumed by
//!   doctor via `doctor::severity_label`.
//! - [`MdschemaDiagnostic`]: the bridge contract the CLI's mdschema checker
//!   fills in; converted to `SpecViolation` by
//!   `validate::append_schema_diagnostics`.
//! - `doctor::DoctorFinding`: the doctor report row (lowercase string
//!   severity on the wire, pinned by the doctor fixtures).
//! - `transaction::TransactionHealthFinding`: transaction-layer health,
//!   converted into doctor rows by [`doctor_output`].
//! - `model::ParseIssue`: parser-internal; converted to `SpecViolation` by
//!   `parse_diagnostics`.
//!
//! The module deliberately has no dependency on the `OpenSpec` executable
//! or on harness-specific instruction files.

use crate::error::{Error, ErrorKind};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

mod apply;
mod commands;
mod doctor;
mod model;
#[cfg(test)]
mod openspec_oracle;
mod parse;
mod root;
mod tasks;
mod templates;
pub(crate) mod transaction;
mod validate;

pub use commands::{
    ArchiveOutput, ContextOutput, ListOutput, ListSort, ProposeOutput, ShowChangeOutput,
    ShowOutput, ShowSpecOutput, SpecsOutput, archive, archive_output, context, context_output,
    list, propose, propose_output, render_archive, render_change_list, render_context, render_json,
    render_propose, render_show_change, render_show_specification, render_specification_list,
    scan_changes, scan_specifications, show, show_output, sorted_changes,
};
pub use doctor::{SpecDoctorOutput, doctor, doctor_output, render_doctor};
pub use validate::{
    MdschemaCheck, MdschemaDiagnostic, render_diagnostics, validate, validate_output,
    validate_spec_tree,
};

use apply::apply_delta;
#[cfg(test)]
use commands::ContextDelta;
use commands::{ContextTask, PrefixMatch, prefix_match, print_json};
use model::{CanonicalSpec, ParseIssue};
use parse::{parse_canonical, parse_delta};
pub use root::{LiveTrees, SpecLayout, SpecRoot};
use tasks::read_tasks;

/// Install the CLI's merged-config lookup once per process.
pub fn set_root_config_lookup(lookup: fn(&Path) -> Result<Option<String>, String>) -> bool {
    root::set_root_config_lookup(lookup)
}

pub fn resolve_spec_root(repository: &Path) -> Result<SpecRoot, Error> {
    root::resolve(repository)
}

/// Resolve against an explicit `spec.root` value, bypassing the installed
/// config lookup. For flows that must act before the value is persisted,
/// such as recording the root only after a successful migration.
pub fn resolve_spec_root_with(repository: &Path, configured: &str) -> Result<SpecRoot, Error> {
    root::resolve_with_config(repository, Some(configured))
}

/// Report which conventional roots hold a live spec tree, without resolving
/// or tie-breaking; callers that must distinguish "openspec only" from a
/// resolved answer (the migration offer) probe here.
pub fn live_trees(repository: &Path) -> LiveTrees {
    root::live_trees(repository)
}

pub fn changes_root(repository: &Path) -> Result<PathBuf, Error> {
    Ok(resolve_spec_root(repository)?.changes().to_path_buf())
}

pub fn specs_root(repository: &Path) -> Result<PathBuf, Error> {
    Ok(resolve_spec_root(repository)?
        .specifications()
        .to_path_buf())
}

pub fn spec_base(repository: &Path) -> Result<PathBuf, Error> {
    Ok(resolve_spec_root(repository)?.base().to_path_buf())
}

/// Lifecycle state derived solely from the task checklist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeState {
    Draft,
    Active,
    Complete,
}

/// Agent- and dashboard-consumable summary of one active change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangeSummary {
    pub id: String,
    pub state: ChangeState,
    pub completed: usize,
    pub total: usize,
}

impl ChangeSummary {
    pub fn completion_percent(&self) -> usize {
        self.completed
            .saturating_mul(100)
            .checked_div(self.total)
            .unwrap_or(0)
    }
}

/// Dashboard-consumable summary of one canonical capability spec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SpecificationSummary {
    pub capability: String,
    pub requirements: usize,
}

static SLUG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").expect("static slug regex is valid")
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A stable validation diagnostic for a specification artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SpecViolation {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub operation: Option<String>,
    pub capability: Option<String>,
    pub change: Option<String>,
}

impl SpecViolation {
    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct MergeSummary {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub renamed: usize,
}

impl MergeSummary {
    fn add(&mut self, other: Self) {
        self.added += other.added;
        self.modified += other.modified;
        self.removed += other.removed;
        self.renamed += other.renamed;
    }
}

#[derive(Deserialize)]
struct ProposalMetadata {
    status: Option<String>,
}

struct MergePlan {
    capability: String,
    destination: PathBuf,
    content: String,
    summary: MergeSummary,
    warnings: Vec<String>,
    changed: bool,
}

struct ChangeEvaluation {
    plans: Vec<MergePlan>,
    diagnostics: Vec<SpecViolation>,
    canonical_artifacts: Vec<(String, PathBuf)>,
}

fn title_case(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn substitute(template: &str, change_id: &str, capability: &str) -> String {
    template
        .replace("${CHANGE_ID}", change_id)
        .replace("${CHANGE_TITLE}", &title_case(change_id))
        .replace("${CAPABILITY}", capability)
        .replace("${CAPABILITY_TITLE}", &title_case(capability))
}

fn validate_change_identifier(value: &str) -> Result<(), Error> {
    validate_slug(value, "change id")?;
    if value == "archive" {
        return Err(Error::new(
            ErrorKind::Config,
            "change id 'archive' is reserved for archived changes".to_string(),
        ));
    }
    Ok(())
}

fn validate_slug(value: &str, label: &str) -> Result<(), Error> {
    if SLUG.is_match(value) {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Config,
            format!("{label} must be a non-empty kebab-case slug: {value}"),
        ))
    }
}

fn validate_capability(value: &str, label: &str) -> Result<(), Error> {
    if !value.is_empty() && value.split('/').all(|segment| SLUG.is_match(segment)) {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Config,
            format!("{label} must be a slash-separated kebab-case path: {value}"),
        ))
    }
}

fn io_error(action: &str, path: &Path, error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("cannot {action} {}: {error}", path.display()),
    )
}

fn read(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|error| io_error("read", path, error))
}

/// Resolve a spec template or schema: a file at the same relative path under
/// the source root wins over the embedded copy, so a repo can carry its own
/// spec artifacts and track upstream updates by replacing the files. The
/// boolean reports whether the repo-local override was used, so callers can
/// surface the substitution instead of applying it silently.
fn load_with_override(
    root: &Path,
    relative: &str,
    embedded: &'static str,
) -> Result<(String, bool), Error> {
    let path = root.join(relative);
    if !path.exists() {
        return Ok((embedded.to_string(), false));
    }
    let confined = crate::support::confine_existing(root, &path)
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    let content = read(&confined)?;
    Ok((content, true))
}

fn strip_frontmatter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---\n") else {
        return content;
    };
    rest.find("\n---\n")
        .map_or(content, |end| &rest[end + "\n---\n".len()..])
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_directories(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("read", directory, error)),
    };
    let mut directories = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| io_error("read", directory, error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    directories.retain(|path| path.is_dir());
    directories.sort();
    Ok(directories)
}

fn discover_capabilities(base: &Path) -> Result<Vec<(String, PathBuf)>, Error> {
    if base.join("spec.md").is_file() {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "root-level specification has no capability identifier: {}",
                base.join("spec.md").display()
            ),
        ));
    }

    let mut capabilities = Vec::new();
    discover_capabilities_below(base, base, &mut capabilities)?;
    capabilities.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(capabilities)
}

fn discover_capabilities_below(
    base: &Path,
    directory: &Path,
    capabilities: &mut Vec<(String, PathBuf)>,
) -> Result<(), Error> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error("read", directory, error)),
    };
    for entry_result in entries {
        let entry = entry_result.map_err(|error| io_error("read", directory, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| io_error("inspect", &entry.path(), error))?;
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }
        let capability_dir = entry.path();
        let spec_path = capability_dir.join("spec.md");
        match fs::symlink_metadata(&spec_path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                capabilities.push((capability_identifier(base, &capability_dir)?, spec_path));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error("inspect", &spec_path, error)),
        }
        discover_capabilities_below(base, &capability_dir, capabilities)?;
    }
    Ok(())
}

fn capability_identifier(base: &Path, capability_dir: &Path) -> Result<String, Error> {
    let relative = capability_dir.strip_prefix(base).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                "cannot derive capability from {} under {}: {error}",
                capability_dir.display(),
                base.display()
            ),
        )
    })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("invalid capability path: {}", relative.display()),
            ));
        };
        let segment = segment.to_str().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("capability path is not UTF-8: {}", relative.display()),
            )
        })?;
        segments.push(segment);
    }
    let capability = segments.join("/");
    validate_capability(&capability, "capability")?;
    Ok(capability)
}

fn archived_change_exists(root: &Path, id: &str) -> Result<bool, Error> {
    Ok(archived_change_path(&changes_root(root)?, id)?.is_some())
}

fn archived_change_path(changes: &Path, id: &str) -> Result<Option<PathBuf>, Error> {
    let mut matches = read_directories(&changes.join("archive"))?
        .into_iter()
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            let Some(date) = name.get(..10) else {
                return false;
            };
            name.as_bytes().get(10) == Some(&b'-')
                && name.get(11..) == Some(id)
                && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
        })
        .collect::<Vec<_>>();
    matches.sort();
    Ok(matches.pop())
}

fn archived_change_status(archive: &Path) -> Result<&'static str, Error> {
    let proposal_path = archive.join("proposal.md");
    if !proposal_path.is_file() {
        return Ok("merged");
    }
    let proposal = read(&proposal_path)?;
    let Some(frontmatter) = proposal.strip_prefix("---\n") else {
        return Ok("merged");
    };
    let Some(end) = frontmatter.find("\n---\n") else {
        return Ok("merged");
    };
    let metadata: ProposalMetadata =
        serde_yaml::from_str(&frontmatter[..end]).map_err(|error| {
            Error::new(
                ErrorKind::Validate,
                format!(
                    "cannot parse archived proposal frontmatter {}: {error}",
                    proposal_path.display()
                ),
            )
        })?;
    Ok(if metadata.status.as_deref() == Some("abandoned") {
        "abandoned"
    } else {
        "merged"
    })
}

fn state_label(state: ChangeState) -> &'static str {
    match state {
        ChangeState::Draft => "draft",
        ChangeState::Active => "active",
        ChangeState::Complete => "complete",
    }
}

fn build_merge_plans(root: &Path, change_dir: &Path) -> Result<Vec<MergePlan>, Error> {
    let spec_root = resolve_spec_root(root)?;
    let evaluation = evaluate_change(&spec_root, change_dir)?;
    if evaluation.diagnostics.iter().any(SpecViolation::is_error) {
        return Err(semantic_diagnostics_error(&evaluation.diagnostics));
    }
    Ok(evaluation.plans)
}

fn evaluate_change(spec_root: &SpecRoot, change_dir: &Path) -> Result<ChangeEvaluation, Error> {
    let change = change_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("change path has no identifier: {}", change_dir.display()),
            )
        })?;
    let capabilities = discover_semantic_capabilities(&change_dir.join("specs"))?;
    let mut evaluation = ChangeEvaluation {
        plans: Vec::new(),
        diagnostics: Vec::new(),
        canonical_artifacts: Vec::new(),
    };
    if capabilities.is_empty() {
        evaluation.diagnostics.push(SpecViolation {
            code: "delta-artifact-missing".to_string(),
            severity: DiagnosticSeverity::Error,
            path: relative_display(spec_root.repository(), &change_dir.join("specs")),
            line: None,
            column: None,
            message: "change has no delta specifications under specs/<capability>/spec.md"
                .to_string(),
            operation: None,
            capability: None,
            change: Some(change.to_string()),
        });
        return Ok(evaluation);
    }
    for (capability, delta_path) in capabilities {
        evaluate_delta(spec_root, change, &capability, &delta_path, &mut evaluation)?;
    }
    Ok(evaluation)
}

fn discover_semantic_capabilities(base: &Path) -> Result<Vec<(String, PathBuf)>, Error> {
    let mut capabilities = Vec::new();
    discover_capabilities_below(base, base, &mut capabilities)?;
    capabilities.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(capabilities)
}

fn evaluate_delta(
    spec_root: &SpecRoot,
    change: &str,
    capability: &str,
    delta_path: &Path,
    evaluation: &mut ChangeEvaluation,
) -> Result<(), Error> {
    let content = read(delta_path)?;
    let operations = match parse_delta(&content) {
        Ok(operations) => operations,
        Err(issues) => {
            evaluation.diagnostics.extend(parse_diagnostics(
                spec_root.repository(),
                delta_path,
                "delta-parse-invalid",
                capability,
                Some(change),
                issues,
            ));
            return Ok(());
        }
    };
    let destination = spec_root.specifications().join(capability).join("spec.md");
    let mut canonical = if destination.is_file() {
        evaluation
            .canonical_artifacts
            .push((capability.to_string(), destination.clone()));
        let canonical_content = read(&destination)?;
        match parse_canonical(&canonical_content) {
            Ok(specification) => specification,
            Err(issues) => {
                evaluation.diagnostics.extend(parse_diagnostics(
                    spec_root.repository(),
                    &destination,
                    "spec-parse-invalid",
                    capability,
                    Some(change),
                    issues,
                ));
                return Ok(());
            }
        }
    } else {
        CanonicalSpec::new(capability, change)
    };
    let applied = match apply_delta(&mut canonical, &operations, capability) {
        Ok(applied) => applied,
        Err(issue) => {
            let operation = issue.line.and_then(|line| {
                operations
                    .iter()
                    .find(|operation| operation.line() == line)
                    .map(|operation| operation.kind().heading().to_ascii_lowercase())
            });
            evaluation.diagnostics.push(SpecViolation {
                code: "delta-application-conflict".to_string(),
                severity: DiagnosticSeverity::Error,
                path: relative_display(spec_root.repository(), delta_path),
                line: issue.line,
                column: None,
                message: issue.message,
                operation,
                capability: Some(capability.to_string()),
                change: Some(change.to_string()),
            });
            return Ok(());
        }
    };
    evaluation.plans.push(MergePlan {
        capability: capability.to_string(),
        destination,
        content: canonical.render(),
        summary: applied.summary,
        warnings: applied.warnings,
        changed: canonical.changed(),
    });
    Ok(())
}

fn parse_diagnostics(
    repository: &Path,
    path: &Path,
    code: &str,
    capability: &str,
    change: Option<&str>,
    issues: Vec<ParseIssue>,
) -> Vec<SpecViolation> {
    issues
        .into_iter()
        .map(|issue| SpecViolation {
            code: code.to_string(),
            severity: DiagnosticSeverity::Error,
            path: relative_display(repository, path),
            line: issue.line,
            column: None,
            message: issue.message,
            operation: None,
            capability: Some(capability.to_string()),
            change: change.map(str::to_string),
        })
        .collect()
}

fn semantic_diagnostics_error(diagnostics: &[SpecViolation]) -> Error {
    let details = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| {
            diagnostic.line.map_or_else(
                || format!("{}: {}", diagnostic.path, diagnostic.message),
                |line| format!("{}:{line}: {}", diagnostic.path, diagnostic.message),
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Error::new(ErrorKind::Validate, details)
}
#[cfg(test)]
mod tests;
