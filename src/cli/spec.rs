//! Native specification-change lifecycle under `docs/`.
//!
//! The module owns change scaffolding, task-state scans, delta validation, and
//! archive-time merges. It deliberately has no dependency on the `OpenSpec`
//! executable or on harness-specific instruction files.

use chrono::Utc;
use commands::error::{Error, ErrorKind};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const PROPOSAL_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/proposal.md"
));
const TASKS_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/tasks.md"
));
const DELTA_SPEC_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/delta-spec.md"
));
const DESIGN_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/design.md"
));
const SPEC_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/spec.mdschema"
));
const DELTA_SPEC_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/delta-spec.mdschema"
));

/// One resolver answers where changes and specs live: the native
/// `docs/` root, or `openspec/` when `spec.root: openspec` is configured
/// or only an `openspec/` tree exists. Every consumer (commands, doctor,
/// completions) routes through these two functions.
pub(crate) fn changes_root(root: &Path) -> PathBuf {
    root.join(spec_base(root)).join("changes")
}

pub(crate) fn specs_root(root: &Path) -> PathBuf {
    root.join(spec_base(root)).join("specs")
}

fn spec_base(root: &Path) -> &'static str {
    let configured = crate::cli::config::load_merged_config(root)
        .ok()
        .and_then(|merged| commands::yaml::yaml_list(&merged, "spec.root"));
    match configured.as_deref() {
        Some("openspec") => "openspec",
        Some("docs") => "docs",
        Some(other) => {
            eprintln!(
                "warning: spec.root '{other}' is not supported (docs | openspec); using docs"
            );
            "docs"
        }
        None => {
            let native = root.join("docs").join("changes").is_dir()
                || root.join("docs").join("specs").is_dir();
            // A bare openspec/ (a project.md stub, an aborted export) is not
            // a spec tree; only its changes/specs dirs flip the base.
            let openspec = root.join("openspec").join("changes").is_dir()
                || root.join("openspec").join("specs").is_dir();
            if !native && openspec {
                "openspec"
            } else {
                "docs"
            }
        }
    }
}

static SLUG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").expect("static slug regex is valid")
});
static TASK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*[-*]\s+\[([ xX])\]\s*(.*)$").expect("static task regex is valid")
});

/// Lifecycle state derived solely from the task checklist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ChangeState {
    Draft,
    Active,
    Complete,
}

/// Agent- and dashboard-consumable summary of one active change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ChangeSummary {
    pub(crate) id: String,
    pub(crate) state: ChangeState,
    pub(crate) completed: usize,
    pub(crate) total: usize,
}

impl ChangeSummary {
    pub(crate) fn completion_percent(&self) -> usize {
        self.completed
            .saturating_mul(100)
            .checked_div(self.total)
            .unwrap_or(0)
    }
}

/// Dashboard-consumable summary of one canonical capability spec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SpecificationSummary {
    pub(crate) capability: String,
    pub(crate) requirements: usize,
}

/// A validation error associated with a lifecycle markdown file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpecViolation {
    pub(crate) path: String,
    pub(crate) line: Option<usize>,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
struct ProposeOutput {
    change: String,
    capabilities: Vec<String>,
    created: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    template_overrides: Vec<String>,
    next_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ListOutput {
    changes: Vec<ChangeSummary>,
}

#[derive(Debug, Serialize)]
struct SpecsOutput {
    specs: Vec<SpecificationSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ContextDelta {
    capability: String,
    body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ContextTask {
    text: String,
    done: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ContextOutput {
    id: String,
    proposal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    design: Option<String>,
    deltas: Vec<ContextDelta>,
    tasks: Vec<ContextTask>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct MergeSummary {
    pub(crate) added: usize,
    pub(crate) modified: usize,
    pub(crate) removed: usize,
}

impl MergeSummary {
    fn add(&mut self, other: Self) {
        self.added += other.added;
        self.modified += other.modified;
        self.removed += other.removed;
    }
}

#[derive(Debug, Serialize)]
struct ArchiveOutput {
    change: String,
    status: &'static str,
    archived_to: String,
    capabilities: Vec<String>,
    merge: MergeSummary,
    warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeltaKind {
    Added,
    Modified,
    Removed,
}

impl DeltaKind {
    fn heading(self) -> &'static str {
        match self {
            Self::Added => "ADDED",
            Self::Modified => "MODIFIED",
            Self::Removed => "REMOVED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Requirement {
    name: String,
    content: String,
    line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DeltaOperation {
    kind: DeltaKind,
    requirement: Requirement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalSpec {
    prefix: String,
    requirements: Vec<Requirement>,
}

impl CanonicalSpec {
    fn new(capability: &str) -> Self {
        let title = title_case(capability);
        Self {
            prefix: format!(
                "# {title} Specification\n\n## Purpose\n\nDescribe the current purpose of the {title} capability.\n\n## Requirements"
            ),
            requirements: Vec::new(),
        }
    }

    fn render(&self) -> String {
        let mut rendered = self.prefix.trim_end().to_string();
        for requirement in &self.requirements {
            rendered.push_str("\n\n");
            rendered.push_str(requirement.content.trim());
        }
        rendered.push('\n');
        rendered
    }
}

struct MergePlan {
    capability: String,
    destination: PathBuf,
    content: String,
    summary: MergeSummary,
}

/// Scaffold a native change folder with one delta per capability.
pub(crate) fn propose(
    source: &str,
    id: &str,
    capabilities: &[String],
    design: bool,
    json: bool,
) -> Result<i32, Error> {
    validate_slug(id, "change id")?;
    let mut capabilities: Vec<&str> = if capabilities.is_empty() {
        vec![id]
    } else {
        capabilities.iter().map(String::as_str).collect()
    };
    // Repeated flags are idempotent: one delta and one listing per
    // capability, first occurrence keeps its position.
    let mut seen = BTreeSet::new();
    capabilities.retain(|capability| seen.insert(*capability));
    for capability in &capabilities {
        validate_slug(capability, "capability")?;
    }

    let root = Path::new(source);
    let change_dir = changes_root(root).join(id);
    if change_dir.exists() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("change '{id}' already exists at {}", change_dir.display()),
        ));
    }

    let mut overrides = Vec::new();
    let mut template = |relative: &str, embedded: &'static str| -> Result<String, Error> {
        let (content, overridden) = load_with_override(root, relative, embedded)?;
        if overridden {
            overrides.push(relative.to_string());
        }
        Ok(content)
    };
    let capabilities_list = capabilities
        .iter()
        .map(|capability| format!("- {capability} (new or modified)"))
        .collect::<Vec<_>>()
        .join("\n");
    let proposal = substitute(
        &template("templates/spec/proposal.md", PROPOSAL_TEMPLATE)?,
        id,
        capabilities[0],
    )
    .replace("${CAPABILITIES}", &capabilities_list);
    let tasks = substitute(
        &template("templates/spec/tasks.md", TASKS_TEMPLATE)?,
        id,
        capabilities[0],
    );
    let delta_template = template("templates/spec/delta-spec.md", DELTA_SPEC_TEMPLATE)?;

    let mut files = vec![
        (change_dir.join("proposal.md"), proposal),
        (change_dir.join("tasks.md"), tasks),
    ];
    for capability in &capabilities {
        files.push((
            change_dir.join("specs").join(capability).join("spec.md"),
            substitute(&delta_template, id, capability),
        ));
    }
    if design {
        let design_content = substitute(
            &template("templates/spec/design.md", DESIGN_TEMPLATE)?,
            id,
            capabilities[0],
        );
        files.push((change_dir.join("design.md"), design_content));
    }

    for (path, content) in &files {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error("create", parent, error))?;
        }
        fs::write(path, content).map_err(|error| io_error("write", path, error))?;
    }

    let created = files
        .iter()
        .map(|(path, _)| relative_display(root, path))
        .collect::<Vec<_>>();
    let output = ProposeOutput {
        change: id.to_string(),
        capabilities: capabilities.iter().map(ToString::to_string).collect(),
        created,
        template_overrides: overrides,
        next_steps: vec![
            "Link proposal.md to the governing ADR and fill in scope.".to_string(),
            "Replace the delta spec placeholders with SHALL requirements and scenarios."
                .to_string(),
            "Implement tasks.md, checking items as executable checks pass.".to_string(),
            format!("Run `rune spec archive {id}` when every task is checked."),
        ],
    };
    print_propose(&output, json)?;
    Ok(0)
}

/// Sort order for `spec list`, mirroring `openspec list`'s `--sort`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ListSort {
    /// Alphabetical by change id.
    #[default]
    Name,
    /// Least-complete changes first, so active work surfaces on top.
    Progress,
}

/// List active changes and their task completion fractions, or the
/// canonical capability specifications with `--specs`.
pub(crate) fn list(source: &str, specs: bool, sort: ListSort, json: bool) -> Result<i32, Error> {
    if specs {
        return list_specifications(Path::new(source), json);
    }
    let mut summaries = scan_changes(Path::new(source))?;
    if sort == ListSort::Progress {
        summaries.sort_by(|left, right| {
            left.completion_percent()
                .cmp(&right.completion_percent())
                .then_with(|| left.id.cmp(&right.id))
        });
    }
    let summaries = summaries;
    if json {
        print_json(&ListOutput { changes: summaries })?;
        return Ok(0);
    }

    let sheet = crate::cli::style::Sheet::detect(false);
    if summaries.is_empty() {
        println!("{}", sheet.dim("No active changes."));
        return Ok(0);
    }
    for change in summaries {
        let label = state_label(change.state);
        let state = match label {
            "draft" => sheet.magenta(label),
            "active" => sheet.yellow(label),
            "complete" => sheet.green(label),
            other => sheet.dim(other),
        };
        let progress = format!("{}/{}", change.completed, change.total);
        let progress = if change.total > 0 && change.completed == change.total {
            sheet.green(&progress)
        } else if change.completed == 0 {
            sheet.dim(&progress)
        } else {
            sheet.yellow(&progress)
        };
        println!(
            "{state}{:<pad$} {} {progress}",
            "",
            sheet.bold(&format!("{:<32}", change.id)),
            pad = 10usize.saturating_sub(label.len()),
        );
    }
    Ok(0)
}

fn list_specifications(root: &Path, json: bool) -> Result<i32, Error> {
    let summaries = scan_specifications(root)?;
    if json {
        print_json(&SpecsOutput { specs: summaries })?;
        return Ok(0);
    }
    if summaries.is_empty() {
        println!("No canonical specifications.");
        return Ok(0);
    }
    for specification in summaries {
        println!(
            "{:<32} {} requirement(s)",
            specification.capability, specification.requirements
        );
    }
    Ok(0)
}

/// Emit an agent-ready work order for one active change.
pub(crate) fn context(source: &str, id: &str, json: bool) -> Result<i32, Error> {
    validate_slug(id, "change id")?;
    let output = load_context_output(Path::new(source), id)?;
    if json {
        print_json(&output)?;
    } else {
        print_context(&output);
    }
    Ok(0)
}

fn load_context_output(root: &Path, id: &str) -> Result<ContextOutput, Error> {
    let change_dir = changes_root(root).join(id);
    if !change_dir.is_dir() {
        if archived_change_exists(root, id)? {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "change '{id}' is already archived under {}",
                    changes_root(root).join("archive").display()
                ),
            ));
        }
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "active change '{id}' was not found at {}",
                change_dir.display()
            ),
        ));
    }

    let proposal = read(&change_dir.join("proposal.md"))?;
    let mut deltas = Vec::new();
    for capability_dir in read_directories(&change_dir.join("specs"))? {
        let Some(capability) = capability_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let delta_path = capability_dir.join("spec.md");
        if delta_path.is_file() {
            deltas.push(ContextDelta {
                capability: capability.to_string(),
                body: read(&delta_path)?,
            });
        }
    }
    if deltas.is_empty() {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "change '{id}' has no delta specifications under {}",
                change_dir.join("specs").display()
            ),
        ));
    }

    let tasks_content = read(&change_dir.join("tasks.md"))?;
    let design_path = change_dir.join("design.md");
    let design = if design_path.is_file() {
        Some(read(&design_path)?.trim().to_string())
    } else {
        None
    };
    Ok(ContextOutput {
        id: id.to_string(),
        proposal: strip_frontmatter(&proposal).trim().to_string(),
        design,
        deltas,
        tasks: parse_tasks(&tasks_content).tasks,
    })
}

/// Merge or explicitly abandon a change, then move it into the dated archive.
pub(crate) fn archive(
    source: &str,
    id: &str,
    yes: bool,
    abandon: bool,
    json: bool,
) -> Result<i32, Error> {
    validate_slug(id, "change id")?;
    let root = Path::new(source);
    let change_dir = changes_root(root).join(id);
    if !change_dir.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "active change '{id}' was not found at {}",
                change_dir.display()
            ),
        ));
    }

    let archive_name = format!("{}-{id}", Utc::now().format("%Y-%m-%d"));
    let archive_dir = changes_root(root).join("archive").join(&archive_name);
    if archive_dir.exists() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("archive target already exists: {}", archive_dir.display()),
        ));
    }

    if abandon {
        stamp_abandoned(&change_dir.join("proposal.md"))?;
        move_to_archive(&change_dir, &archive_dir)?;
        let output = ArchiveOutput {
            change: id.to_string(),
            status: "abandoned",
            archived_to: relative_display(root, &archive_dir),
            capabilities: Vec::new(),
            merge: MergeSummary::default(),
            warnings: Vec::new(),
        };
        print_archive(&output, json)?;
        return Ok(0);
    }

    let task_status = read_tasks(&change_dir.join("tasks.md"))?;
    let mut warnings = Vec::new();
    if task_status.total == 0 && !yes {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "change '{id}' has no checklist tasks; add tasks to tasks.md, rerun with -y to override, or use --abandon"
            ),
        ));
    }
    if task_status.total == 0 {
        warnings.push("overrode an empty or missing task checklist with -y".to_string());
    }
    if !task_status.unchecked.is_empty() && !yes {
        let items = task_status
            .unchecked
            .iter()
            .map(|task| format!("  - [ ] {task}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "change '{id}' has {} unchecked task(s):\n{items}\nrerun with -y to override, or use --abandon to archive without merging",
                task_status.unchecked.len()
            ),
        ));
    }
    if !task_status.unchecked.is_empty() {
        warnings.push(format!(
            "overrode {} unchecked task(s) with -y",
            task_status.unchecked.len()
        ));
    }

    // Parse and apply every delta in memory first. No canonical file is
    // touched until all capabilities have passed semantic validation.
    let plans = build_merge_plans(root, &change_dir)?;
    for plan in &plans {
        if let Some(parent) = plan.destination.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error("create", parent, error))?;
        }
        fs::write(&plan.destination, &plan.content)
            .map_err(|error| io_error("write", &plan.destination, error))?;
    }
    move_to_archive(&change_dir, &archive_dir)?;

    let mut merge = MergeSummary::default();
    for plan in &plans {
        merge.add(plan.summary);
    }
    let output = ArchiveOutput {
        change: id.to_string(),
        status: "merged",
        archived_to: relative_display(root, &archive_dir),
        capabilities: plans.iter().map(|plan| plan.capability.clone()).collect(),
        merge,
        warnings,
    };
    print_archive(&output, json)?;
    Ok(0)
}

/// Scan active changes without printing, for status and other services.
pub(crate) fn scan_changes(root: &Path) -> Result<Vec<ChangeSummary>, Error> {
    let changes_dir = changes_root(root);
    let mut entries = read_directories(&changes_dir)?;
    entries.retain(|path| path.file_name().is_some_and(|name| name != "archive"));

    let mut changes = Vec::new();
    for path in entries {
        let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let task_status = read_tasks(&path.join("tasks.md"))?;
        changes.push(ChangeSummary {
            id: id.to_string(),
            state: task_status.state(),
            completed: task_status.completed,
            total: task_status.total,
        });
    }
    changes.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(changes)
}

/// Scan canonical specifications and count recognized requirements.
pub(crate) fn scan_specifications(root: &Path) -> Result<Vec<SpecificationSummary>, Error> {
    let specs_dir = specs_root(root);
    let mut summaries = Vec::new();
    for capability_dir in read_directories(&specs_dir)? {
        let Some(capability) = capability_dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let spec_path = capability_dir.join("spec.md");
        if !spec_path.is_file() {
            continue;
        }
        let requirements = fs::read_to_string(&spec_path)
            .ok()
            .and_then(|content| parse_canonical(&content).ok())
            .map_or(0, |spec| spec.requirements.len());
        summaries.push(SpecificationSummary {
            capability,
            requirements,
        });
    }
    summaries.sort_by(|left, right| left.capability.cmp(&right.capability));
    Ok(summaries)
}

/// Validate canonical specs and active deltas, including archive mergeability.
pub(crate) fn validate_spec_tree(root: &Path) -> Result<Vec<SpecViolation>, Error> {
    let mut violations = Vec::new();
    let (spec_schema, _) = load_with_override(root, "schemas/spec.mdschema", SPEC_MDSCHEMA)?;
    let (delta_schema, _) =
        load_with_override(root, "schemas/delta-spec.mdschema", DELTA_SPEC_MDSCHEMA)?;

    for capability_dir in read_directories(&specs_root(root))? {
        let spec_path = capability_dir.join("spec.md");
        if !spec_path.is_file() {
            continue;
        }
        let content = read(&spec_path)?;
        violations.extend(schema_violations(root, &spec_path, &content, &spec_schema));
        if let Err(found) = parse_canonical(&content) {
            violations.extend(found.into_iter().map(|issue| SpecViolation {
                path: relative_display(root, &spec_path),
                line: issue.line,
                message: issue.message,
            }));
        }
    }

    for change_dir in read_directories(&changes_root(root))? {
        if change_dir.file_name().is_some_and(|name| name == "archive") {
            continue;
        }
        for capability_dir in read_directories(&change_dir.join("specs"))? {
            let delta_path = capability_dir.join("spec.md");
            if !delta_path.is_file() {
                continue;
            }
            let content = read(&delta_path)?;
            violations.extend(schema_violations(
                root,
                &delta_path,
                &content,
                &delta_schema,
            ));
            let operations = match parse_delta(&content) {
                Ok(operations) => operations,
                Err(found) => {
                    violations.extend(found.into_iter().map(|issue| SpecViolation {
                        path: relative_display(root, &delta_path),
                        line: issue.line,
                        message: issue.message,
                    }));
                    continue;
                }
            };
            let Some(capability) = capability_dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let canonical_path = specs_root(root).join(capability).join("spec.md");
            let mut canonical = if canonical_path.is_file() {
                let canonical_content = read(&canonical_path)?;
                match parse_canonical(&canonical_content) {
                    Ok(spec) => spec,
                    Err(_) => continue,
                }
            } else {
                CanonicalSpec::new(capability)
            };
            if let Err(error) = apply_delta(&mut canonical, &operations, capability) {
                violations.push(SpecViolation {
                    path: relative_display(root, &delta_path),
                    line: error.line,
                    message: error.message,
                });
            }
        }
    }

    Ok(violations)
}

fn schema_violations(root: &Path, path: &Path, content: &str, schema: &str) -> Vec<SpecViolation> {
    let relative = relative_display(root, path);
    commands::validate::mdschema::check(content, &relative, schema)
        .into_iter()
        .map(|diagnostic| SpecViolation {
            path: relative.clone(),
            line: diagnostic.line,
            message: diagnostic.message,
        })
        .collect()
}

#[derive(Debug, Default)]
struct TaskStatus {
    completed: usize,
    total: usize,
    unchecked: Vec<String>,
    tasks: Vec<ContextTask>,
}

impl TaskStatus {
    fn state(&self) -> ChangeState {
        if self.total > 0 && self.completed == self.total {
            ChangeState::Complete
        } else if self.completed > 0 {
            ChangeState::Active
        } else {
            ChangeState::Draft
        }
    }
}

fn read_tasks(path: &Path) -> Result<TaskStatus, Error> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TaskStatus::default());
        }
        Err(error) => return Err(io_error("read", path, error)),
    };
    Ok(parse_tasks(&content))
}

fn parse_tasks(content: &str) -> TaskStatus {
    let mut status = TaskStatus::default();
    let mut fence = None;
    for line in content.lines() {
        if update_fence(line, &mut fence) || fence.is_some() {
            continue;
        }
        let Some(captures) = TASK.captures(line) else {
            continue;
        };
        let text = captures[2].trim().to_string();
        let done = &captures[1] != " ";
        status.total += 1;
        if done {
            status.completed += 1;
        } else {
            status.unchecked.push(text.clone());
        }
        status.tasks.push(ContextTask { text, done });
    }
    status
}

fn update_fence(line: &str, fence: &mut Option<char>) -> bool {
    let trimmed = line.trim_start();
    let marker = if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    };
    let Some(marker) = marker else {
        return false;
    };
    if fence.is_some_and(|current| current == marker) {
        *fence = None;
    } else if fence.is_none() {
        *fence = Some(marker);
    }
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParseIssue {
    line: Option<usize>,
    message: String,
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
    let confined = commands::services::confine::confine_existing(root, &path)
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

fn archived_change_exists(root: &Path, id: &str) -> Result<bool, Error> {
    Ok(read_directories(&changes_root(root).join("archive"))?
        .iter()
        .any(|path| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            let Some(date) = name.get(..10) else {
                return false;
            };
            name.as_bytes().get(10) == Some(&b'-')
                && name.get(11..) == Some(id)
                && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
        }))
}

fn state_label(state: ChangeState) -> &'static str {
    match state {
        ChangeState::Draft => "draft",
        ChangeState::Active => "active",
        ChangeState::Complete => "complete",
    }
}

fn print_json(value: &impl Serialize) -> Result<(), Error> {
    let rendered = serde_json::to_string_pretty(value).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot serialize lifecycle result: {error}"),
        )
    })?;
    println!("{rendered}");
    Ok(())
}

fn print_propose(output: &ProposeOutput, json: bool) -> Result<(), Error> {
    if json {
        return print_json(output);
    }
    println!(
        "Created change '{}' for '{}':",
        output.change,
        output.capabilities.join(", ")
    );
    for path in &output.created {
        println!("  + {path}");
    }
    for relative in &output.template_overrides {
        println!("note: using template override {relative}");
    }
    println!("Next steps:");
    for step in &output.next_steps {
        println!("  - {step}");
    }
    Ok(())
}

fn print_context(output: &ContextOutput) {
    println!("# Work order: {}\n", output.id);
    println!("## Proposal\n\n{}\n", output.proposal.trim());
    if let Some(design) = &output.design {
        println!("## Design\n\n{design}\n");
    }
    for delta in &output.deltas {
        println!("## Delta: {}\n\n{}\n", delta.capability, delta.body.trim());
    }
    println!("## Tasks");
    if output.tasks.is_empty() {
        println!("\n_No checklist tasks found._");
        return;
    }
    for task in &output.tasks {
        if task.done {
            println!("- [x] {}", task.text);
        } else {
            println!("- [ ] **TODO: {}**", task.text);
        }
    }
}

fn print_archive(output: &ArchiveOutput, json: bool) -> Result<(), Error> {
    if json {
        return print_json(output);
    }
    for warning in &output.warnings {
        eprintln!("warning: {warning}");
    }
    println!(
        "Archived change '{}' as {} to {}.",
        output.change, output.status, output.archived_to
    );
    if output.status == "merged" {
        println!(
            "Merged {} added, {} modified, and {} removed requirement(s) across {} capability spec(s).",
            output.merge.added,
            output.merge.modified,
            output.merge.removed,
            output.capabilities.len()
        );
    }
    Ok(())
}

fn stamp_abandoned(path: &Path) -> Result<(), Error> {
    let content = read(path)?;
    let stamped = if let Some(rest) = content.strip_prefix("---\n") {
        let Some(end) = rest.find("\n---\n") else {
            return Err(Error::new(
                ErrorKind::Validate,
                format!("{} has an unterminated frontmatter block", path.display()),
            ));
        };
        let frontmatter = &rest[..end];
        let body = &rest[end + 5..];
        let mut found = false;
        let mut lines = frontmatter
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("status:") {
                    found = true;
                    "status: abandoned".to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>();
        if !found {
            lines.push("status: abandoned".to_string());
        }
        format!("---\n{}\n---\n{body}", lines.join("\n"))
    } else {
        format!("---\nstatus: abandoned\n---\n{content}")
    };
    fs::write(path, stamped).map_err(|error| io_error("write", path, error))
}

fn move_to_archive(source: &Path, destination: &Path) -> Result<(), Error> {
    let Some(parent) = destination.parent() else {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "archive destination has no parent: {}",
                destination.display()
            ),
        ));
    };
    fs::create_dir_all(parent).map_err(|error| io_error("create", parent, error))?;
    fs::rename(source, destination).map_err(|error| io_error("archive", source, error))
}

fn build_merge_plans(root: &Path, change_dir: &Path) -> Result<Vec<MergePlan>, Error> {
    let mut plans = Vec::new();
    for capability_dir in read_directories(&change_dir.join("specs"))? {
        let Some(capability) = capability_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        validate_slug(capability, "capability")?;
        let delta_path = capability_dir.join("spec.md");
        if !delta_path.is_file() {
            continue;
        }
        let operations = parse_delta(&read(&delta_path)?)
            .map_err(|issues| issues_error(&delta_path, "invalid delta specification", &issues))?;
        let destination = specs_root(root).join(capability).join("spec.md");
        let mut canonical = if destination.is_file() {
            parse_canonical(&read(&destination)?).map_err(|issues| {
                issues_error(&destination, "invalid canonical specification", &issues)
            })?
        } else {
            CanonicalSpec::new(capability)
        };
        let summary = apply_delta(&mut canonical, &operations, capability).map_err(|issue| {
            issues_error(&delta_path, "cannot merge delta specification", &[issue])
        })?;
        plans.push(MergePlan {
            capability: capability.to_string(),
            destination,
            content: canonical.render(),
            summary,
        });
    }
    if plans.is_empty() {
        return Err(Error::new(
            ErrorKind::Validate,
            format!(
                "change has no delta specifications under {}",
                change_dir.join("specs").display()
            ),
        ));
    }
    Ok(plans)
}

fn issues_error(path: &Path, context: &str, issues: &[ParseIssue]) -> Error {
    let details = issues
        .iter()
        .map(|issue| match issue.line {
            Some(line) => format!("{}:{line}: {}", path.display(), issue.message),
            None => format!("{}: {}", path.display(), issue.message),
        })
        .collect::<Vec<_>>()
        .join("; ");
    Error::new(ErrorKind::Validate, format!("{context}: {details}"))
}

fn parse_canonical(content: &str) -> Result<CanonicalSpec, Vec<ParseIssue>> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut issues = Vec::new();
    if !lines
        .first()
        .is_some_and(|line| line.starts_with("# ") && line.ends_with(" Specification"))
    {
        issues.push(ParseIssue {
            line: Some(1),
            message: "expected '# <Capability> Specification' heading".to_string(),
        });
    }
    let purpose = lines.iter().position(|line| line.trim() == "## Purpose");
    let requirements = lines
        .iter()
        .position(|line| line.trim() == "## Requirements");
    match (purpose, requirements) {
        (Some(purpose), Some(requirements)) if purpose < requirements => {
            if lines[purpose + 1..requirements]
                .iter()
                .all(|line| line.trim().is_empty())
            {
                issues.push(ParseIssue {
                    line: Some(purpose + 1),
                    message: "Purpose section must not be empty".to_string(),
                });
            }
        }
        (None, _) => issues.push(ParseIssue {
            line: None,
            message: "missing required '## Purpose' section".to_string(),
        }),
        (_, None) => issues.push(ParseIssue {
            line: None,
            message: "missing required '## Requirements' section".to_string(),
        }),
        _ => issues.push(ParseIssue {
            line: None,
            message: "Purpose must appear before Requirements".to_string(),
        }),
    }
    let Some(requirements_index) = requirements else {
        return Err(issues);
    };
    for (index, line) in lines.iter().enumerate().skip(requirements_index + 1) {
        if line.starts_with("## ") {
            issues.push(ParseIssue {
                line: Some(index + 1),
                message: "canonical specs cannot contain sections after '## Requirements'"
                    .to_string(),
            });
        }
    }
    let prefix = lines[..=requirements_index].join("\n");
    let parsed = parse_requirement_blocks(&lines, requirements_index + 1, lines.len(), true);
    let (requirements, mut block_issues) = parsed;
    issues.append(&mut block_issues);
    if issues.is_empty() {
        Ok(CanonicalSpec {
            prefix,
            requirements,
        })
    } else {
        Err(issues)
    }
}

fn parse_delta(content: &str) -> Result<Vec<DeltaOperation>, Vec<ParseIssue>> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut operations = Vec::new();
    let mut issues = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("## ") && delta_kind(line.trim()).is_none() {
            issues.push(ParseIssue {
                line: Some(index + 1),
                message: "delta sections must be ADDED, MODIFIED, or REMOVED Requirements"
                    .to_string(),
            });
        }
    }
    let mut section_indexes = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| delta_kind(line.trim()).map(|kind| (index, kind)))
        .collect::<Vec<_>>();
    if section_indexes.is_empty() {
        return Err(vec![ParseIssue {
            line: None,
            message: "delta must contain an ADDED, MODIFIED, or REMOVED Requirements section"
                .to_string(),
        }]);
    }
    section_indexes.push((lines.len(), DeltaKind::Added));
    let mut names = BTreeSet::new();
    for window in section_indexes.windows(2) {
        let (section, kind) = window[0];
        let end = window[1].0;
        let validate_body = kind != DeltaKind::Removed;
        let (requirements, mut found) =
            parse_requirement_blocks(&lines, section + 1, end, validate_body);
        issues.append(&mut found);
        if requirements.is_empty() {
            issues.push(ParseIssue {
                line: Some(section + 1),
                message: format!("{} Requirements must contain a requirement", kind.heading()),
            });
        }
        for requirement in requirements {
            if !names.insert(requirement.name.clone()) {
                issues.push(ParseIssue {
                    line: Some(requirement.line),
                    message: format!(
                        "requirement '{}' appears more than once in the delta",
                        requirement.name
                    ),
                });
            }
            operations.push(DeltaOperation { kind, requirement });
        }
    }
    if issues.is_empty() {
        Ok(operations)
    } else {
        Err(issues)
    }
}

fn delta_kind(line: &str) -> Option<DeltaKind> {
    match line {
        "## ADDED Requirements" => Some(DeltaKind::Added),
        "## MODIFIED Requirements" => Some(DeltaKind::Modified),
        "## REMOVED Requirements" => Some(DeltaKind::Removed),
        _ => None,
    }
}

fn parse_requirement_blocks(
    lines: &[&str],
    start: usize,
    end: usize,
    validate_body: bool,
) -> (Vec<Requirement>, Vec<ParseIssue>) {
    let headings = (start..end)
        .filter(|index| lines[*index].starts_with("### Requirement:"))
        .collect::<Vec<_>>();
    let mut requirements = Vec::new();
    let mut issues = Vec::new();
    let mut names = BTreeSet::new();
    for (position, heading) in headings.iter().enumerate() {
        let block_end = headings.get(position + 1).copied().unwrap_or(end);
        let name = lines[*heading]
            .trim_start_matches("### Requirement:")
            .trim()
            .to_string();
        if name.is_empty() {
            issues.push(ParseIssue {
                line: Some(*heading + 1),
                message: "requirement name must not be empty".to_string(),
            });
            continue;
        }
        if !names.insert(name.clone()) {
            issues.push(ParseIssue {
                line: Some(*heading + 1),
                message: format!("duplicate requirement '{name}'"),
            });
        }
        if validate_body {
            validate_requirement_body(lines, *heading, block_end, &name, &mut issues);
        }
        requirements.push(Requirement {
            name,
            content: lines[*heading..block_end].join("\n").trim().to_string(),
            line: *heading + 1,
        });
    }
    for (index, line) in lines.iter().enumerate().take(end).skip(start) {
        if line.starts_with("### ") && !line.starts_with("### Requirement:") {
            issues.push(ParseIssue {
                line: Some(index + 1),
                message: "level-three headings must use '### Requirement: <name>'".to_string(),
            });
        }
    }
    (requirements, issues)
}

fn validate_requirement_body(
    lines: &[&str],
    heading: usize,
    end: usize,
    name: &str,
    issues: &mut Vec<ParseIssue>,
) {
    let scenarios = (heading + 1..end)
        .filter(|index| lines[*index].starts_with("#### Scenario:"))
        .collect::<Vec<_>>();
    let body_end = scenarios.first().copied().unwrap_or(end);
    let body = lines[heading + 1..body_end].join("\n");
    if !body.split_whitespace().any(|word| {
        word.trim_matches(|character: char| !character.is_ascii_alphabetic()) == "SHALL"
    }) {
        issues.push(ParseIssue {
            line: Some(heading + 1),
            message: format!("requirement '{name}' must contain a SHALL statement in its body"),
        });
    }
    if scenarios.is_empty() {
        issues.push(ParseIssue {
            line: Some(heading + 1),
            message: format!("requirement '{name}' must contain at least one scenario"),
        });
        return;
    }
    for (position, scenario) in scenarios.iter().enumerate() {
        let scenario_end = scenarios.get(position + 1).copied().unwrap_or(end);
        let scenario_name = lines[*scenario].trim_start_matches("#### Scenario:").trim();
        if scenario_name.is_empty() {
            issues.push(ParseIssue {
                line: Some(*scenario + 1),
                message: "scenario name must not be empty".to_string(),
            });
        }
        let scenario_body = &lines[*scenario + 1..scenario_end];
        for keyword in ["WHEN", "THEN"] {
            let marker = format!("- **{keyword}**");
            if !scenario_body
                .iter()
                .any(|line| line.trim_start().starts_with(&marker))
            {
                issues.push(ParseIssue {
                    line: Some(*scenario + 1),
                    message: format!("scenario '{scenario_name}' is missing a {keyword} bullet"),
                });
            }
        }
    }
    for (index, line) in lines.iter().enumerate().take(end).skip(heading + 1) {
        if line.starts_with("#### ") && !line.starts_with("#### Scenario:") {
            issues.push(ParseIssue {
                line: Some(index + 1),
                message: "level-four headings must use '#### Scenario: <name>'".to_string(),
            });
        }
    }
}

fn apply_delta(
    canonical: &mut CanonicalSpec,
    operations: &[DeltaOperation],
    capability: &str,
) -> Result<MergeSummary, ParseIssue> {
    let mut summary = MergeSummary::default();
    for operation in operations {
        let existing = canonical
            .requirements
            .iter()
            .position(|requirement| requirement.name == operation.requirement.name);
        match (operation.kind, existing) {
            (DeltaKind::Added, None) => {
                canonical.requirements.push(operation.requirement.clone());
                summary.added += 1;
            }
            (DeltaKind::Added, Some(_)) => {
                return Err(ParseIssue {
                    line: Some(operation.requirement.line),
                    message: format!(
                        "cannot add existing requirement '{}' to capability '{capability}'",
                        operation.requirement.name
                    ),
                });
            }
            (DeltaKind::Modified, Some(index)) => {
                canonical.requirements[index] = operation.requirement.clone();
                summary.modified += 1;
            }
            (DeltaKind::Removed, Some(index)) => {
                canonical.requirements.remove(index);
                summary.removed += 1;
            }
            (DeltaKind::Modified | DeltaKind::Removed, None) => {
                return Err(ParseIssue {
                    line: Some(operation.requirement.line),
                    message: format!(
                        "cannot {} unknown requirement '{}' in capability '{capability}'",
                        operation.kind.heading().to_ascii_lowercase(),
                        operation.requirement.name
                    ),
                });
            }
        }
    }
    Ok(summary)
}

#[derive(Debug, Serialize)]
struct ShowChangeOutput {
    state: ChangeState,
    completed: usize,
    total: usize,
    #[serde(flatten)]
    context: ContextOutput,
}

#[derive(Debug, Serialize)]
struct ShowSpecOutput {
    capability: String,
    requirements: usize,
    content: String,
}

/// Render one active change or one canonical capability specification.
pub(crate) fn show(source: &str, name: &str, json: bool) -> Result<i32, Error> {
    validate_slug(name, "item name")?;
    let root = Path::new(source);
    let change_dir = changes_root(root).join(name);
    let spec_path = specs_root(root).join(name).join("spec.md");
    match (change_dir.is_dir(), spec_path.is_file()) {
        (true, true) => Err(Error::new(
            ErrorKind::Config,
            format!(
                "'{name}' is both an active change and a capability specification:\n  - change: {} (rune spec context {name})\n  - specification: {}\npick one of those forms",
                relative_display(root, &change_dir),
                relative_display(root, &spec_path)
            ),
        )),
        (true, false) => show_change(root, name, json),
        (false, true) => show_specification(root, name, &spec_path, json),
        (false, false) => {
            if archived_change_exists(root, name)? {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "change '{name}' is already archived under {}",
                        changes_root(root).join("archive").display()
                    ),
                ));
            }
            match prefix_match(root, name) {
                PrefixMatch::Change(id) => show_change(root, &id, json),
                PrefixMatch::Specification(capability) => {
                    let path = specs_root(root).join(&capability).join("spec.md");
                    show_specification(root, &capability, &path, json)
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
            }
        }
    }
}

enum PrefixMatch {
    Change(String),
    Specification(String),
    Ambiguous(Vec<String>),
    None,
}

/// An unambiguous prefix works everywhere a full id does, so `spec show
/// add` reaches `add-widget` without shell completion.
fn prefix_match(root: &Path, prefix: &str) -> PrefixMatch {
    let mut changes: Vec<String> = read_directories(&changes_root(root))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|directory| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter(|id| id != "archive" && id.starts_with(prefix))
        .collect();
    let mut specifications: Vec<String> = read_directories(&specs_root(root))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|directory| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter(|capability| capability.starts_with(prefix))
        .collect();
    changes.sort();
    specifications.sort();
    match (changes.len(), specifications.len()) {
        (1, 0) => PrefixMatch::Change(changes.remove(0)),
        (0, 1) => PrefixMatch::Specification(specifications.remove(0)),
        (0, 0) => PrefixMatch::None,
        _ => {
            changes.append(&mut specifications);
            PrefixMatch::Ambiguous(changes)
        }
    }
}

fn show_change(root: &Path, id: &str, json: bool) -> Result<i32, Error> {
    let task_status = read_tasks(&changes_root(root).join(id).join("tasks.md"))?;
    let output = ShowChangeOutput {
        state: task_status.state(),
        completed: task_status.completed,
        total: task_status.total,
        context: load_context_output(root, id)?,
    };
    if json {
        print_json(&output)?;
        return Ok(0);
    }
    println!(
        "{} · {} · {}/{} tasks\n",
        output.context.id,
        state_label(output.state),
        output.completed,
        output.total
    );
    print_context(&output.context);
    Ok(0)
}

fn show_specification(
    root: &Path,
    capability: &str,
    path: &Path,
    json: bool,
) -> Result<i32, Error> {
    let content = read(path)?;
    let requirements = parse_canonical(&content).map_or(0, |spec| spec.requirements.len());
    if json {
        print_json(&ShowSpecOutput {
            capability: capability.to_string(),
            requirements,
            content,
        })?;
        return Ok(0);
    }
    println!(
        "{} · {} requirement(s) · {}\n",
        capability,
        requirements,
        relative_display(root, path)
    );
    print!("{content}");
    Ok(0)
}

#[derive(Debug, Serialize)]
struct DoctorFinding {
    severity: &'static str,
    path: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct SpecDoctorOutput {
    changes: usize,
    specs: usize,
    findings: Vec<DoctorFinding>,
}

/// Report relationship health across the spec-driven change tree.
pub(crate) fn doctor(source: &str, json: bool) -> Result<i32, Error> {
    let root = Path::new(source);
    let mut findings = Vec::new();

    let change_dirs = read_directories(&changes_root(root))?
        .into_iter()
        .filter(|path| path.file_name().is_none_or(|name| name != "archive"))
        .collect::<Vec<_>>();
    for change_dir in &change_dirs {
        check_change_health(root, change_dir, &mut findings)?;
    }

    let specifications = scan_specifications(root)?;
    for specification in &specifications {
        if specification.requirements == 0 {
            findings.push(DoctorFinding {
                severity: "warning",
                path: format!("docs/specs/{}/spec.md", specification.capability),
                message: "canonical specification has no recognized requirements".to_string(),
            });
        }
    }

    for archived in read_directories(&changes_root(root).join("archive"))? {
        let Some(name) = archived.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let dated = name.len() > 11
            && name.as_bytes().get(10) == Some(&b'-')
            && name
                .get(..10)
                .is_some_and(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok());
        if !dated {
            findings.push(DoctorFinding {
                severity: "warning",
                path: relative_display(root, &archived),
                message: "archive entry is not named <YYYY-MM-DD>-<change-id>".to_string(),
            });
        }
    }

    let output = SpecDoctorOutput {
        changes: change_dirs.len(),
        specs: specifications.len(),
        findings,
    };
    let broken = output
        .findings
        .iter()
        .any(|finding| finding.severity == "error");
    if json {
        print_json(&output)?;
    } else if output.findings.is_empty() {
        println!(
            "spec tree healthy: {} active change(s), {} capability spec(s)",
            output.changes, output.specs
        );
    } else {
        for finding in &output.findings {
            println!(
                "{}: {}: {}",
                finding.severity, finding.path, finding.message
            );
        }
    }
    Ok(i32::from(broken))
}

fn check_change_health(
    root: &Path,
    change_dir: &Path,
    findings: &mut Vec<DoctorFinding>,
) -> Result<(), Error> {
    let display = relative_display(root, change_dir);
    let Some(id) = change_dir.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    if !change_dir.join("proposal.md").is_file() {
        findings.push(DoctorFinding {
            severity: "error",
            path: display.clone(),
            message: "change has no proposal.md".to_string(),
        });
    }
    let has_delta = read_directories(&change_dir.join("specs"))?
        .iter()
        .any(|capability_dir| capability_dir.join("spec.md").is_file());
    if !has_delta {
        findings.push(DoctorFinding {
            severity: "error",
            path: display.clone(),
            message: "change has no delta specification under specs/<capability>/spec.md"
                .to_string(),
        });
    }
    let task_status = read_tasks(&change_dir.join("tasks.md"))?;
    if task_status.total == 0 {
        findings.push(DoctorFinding {
            severity: "warning",
            path: display,
            message: "change has no checklist tasks in tasks.md".to_string(),
        });
    } else if task_status.state() == ChangeState::Complete {
        findings.push(DoctorFinding {
            severity: "warning",
            path: display,
            message: format!("all tasks are checked; archive with rune spec archive {id}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const ORIGINAL: &str = "# Search Specification\n\n## Purpose\n\nFind runes.\n\n## Requirements\n\n### Requirement: Existing\n\nThe system SHALL retain this behavior.\n\n#### Scenario: Existing behavior\n\n- **WHEN** search runs\n- **THEN** results appear\n\n### Requirement: Remove Me\n\nThe system SHALL remove this behavior later.\n\n#### Scenario: Old behavior\n\n- **WHEN** old search runs\n- **THEN** old results appear\n";

    const DELTA: &str = "## ADDED Requirements\n\n### Requirement: Added\n\nThe system SHALL add this behavior.\n\n#### Scenario: Added behavior\n\n- **WHEN** new search runs\n- **THEN** new results appear\n\n## MODIFIED Requirements\n\n### Requirement: Existing\n\nThe system SHALL replace this behavior.\n\n#### Scenario: Replacement behavior\n\n- **WHEN** search runs\n- **THEN** replacement results appear\n\n## REMOVED Requirements\n\n### Requirement: Remove Me\n";

    const MERGED: &str = "# Search Specification\n\n## Purpose\n\nFind runes.\n\n## Requirements\n\n### Requirement: Existing\n\nThe system SHALL replace this behavior.\n\n#### Scenario: Replacement behavior\n\n- **WHEN** search runs\n- **THEN** replacement results appear\n\n### Requirement: Added\n\nThe system SHALL add this behavior.\n\n#### Scenario: Added behavior\n\n- **WHEN** new search runs\n- **THEN** new results appear\n";

    fn write_change(root: &Path, id: &str, tasks: &str, delta: &str) {
        let change = changes_root(root).join(id);
        fs::create_dir_all(change.join("specs/search")).unwrap();
        fs::write(
            change.join("proposal.md"),
            "---\nadr: docs/decisions/ADR-0001.md\nstatus: proposed\n---\n# Change\n",
        )
        .unwrap();
        fs::write(change.join("tasks.md"), tasks).unwrap();
        fs::write(change.join("specs/search/spec.md"), delta).unwrap();
    }

    #[test]
    fn show_resolves_unambiguous_prefixes_and_rejects_ambiguity() {
        let root = TempDir::new().unwrap();
        write_change(root.path(), "add-widget", "- [ ] a\n", DELTA);
        write_change(root.path(), "add-gadget", "- [ ] a\n", DELTA);
        write_change(root.path(), "remove-legacy", "- [ ] a\n", DELTA);

        match prefix_match(root.path(), "rem") {
            PrefixMatch::Change(id) => assert_eq!(id, "remove-legacy"),
            _ => panic!("expected a unique change match"),
        }
        match prefix_match(root.path(), "add") {
            PrefixMatch::Ambiguous(candidates) => {
                assert_eq!(candidates, vec!["add-gadget", "add-widget"]);
            }
            _ => panic!("expected ambiguity"),
        }
        assert!(matches!(prefix_match(root.path(), "zz"), PrefixMatch::None));
    }

    #[test]
    fn resolver_prefers_native_and_detects_openspec_roots() {
        let native = TempDir::new().unwrap();
        fs::create_dir_all(native.path().join("docs/changes")).unwrap();
        assert!(changes_root(native.path()).ends_with("docs/changes"));

        let openspec = TempDir::new().unwrap();
        fs::create_dir_all(openspec.path().join("openspec/changes")).unwrap();
        assert!(changes_root(openspec.path()).ends_with("openspec/changes"));
        assert!(specs_root(openspec.path()).ends_with("openspec/specs"));

        let empty = TempDir::new().unwrap();
        assert!(changes_root(empty.path()).ends_with("docs/changes"));
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

        let violations = validate_spec_tree(root.path()).unwrap();

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
    fn validation_accepts_well_formed_canonical_and_delta_specs() {
        let root = TempDir::new().unwrap();
        let canonical = root.path().join("docs/specs/search/spec.md");
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(&canonical, ORIGINAL).unwrap();
        write_change(root.path(), "valid-delta", "- [ ] pending\n", DELTA);

        let violations = validate_spec_tree(root.path()).unwrap();

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
    fn show_renders_an_active_change() {
        let root = TempDir::new().unwrap();
        write_change(root.path(), "improve-search", "- [x] a\n- [ ] b\n", DELTA);

        let code = show(&root.path().to_string_lossy(), "improve-search", false).unwrap();

        assert_eq!(code, 0);
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

        let proposal =
            fs::read_to_string(root.path().join("docs/changes/dupes/proposal.md")).unwrap();
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
        let output = load_context_output(root.path(), "designed").unwrap();
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
}
