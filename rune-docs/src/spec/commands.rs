//! The five command families behind `rune spec`: propose, list, context,
//! archive, and show. Each has an output-returning form (`*_output`), a
//! pure renderer (`render_*`), and a printing wrapper kept for callers
//! that have not migrated to CLI-side rendering yet.

use super::tasks::{parse_tasks, read_tasks};
use super::templates::{DELTA_SPEC_TEMPLATE, DESIGN_TEMPLATE, PROPOSAL_TEMPLATE, TASKS_TEMPLATE};
use super::transaction::{self, FileWrite, Operation};
use super::{
    ChangeState, ChangeSummary, MergeSummary, SpecificationSummary, archived_change_exists,
    archived_change_path, archived_change_status, build_merge_plans, changes_root,
    discover_capabilities, io_error, load_with_override, parse_canonical, read, read_directories,
    relative_display, resolve_spec_root, specs_root, state_label, strip_frontmatter, substitute,
    validate_capability, validate_change_identifier,
};
use crate::error::{Error, ErrorKind};
use chrono::Utc;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Result of scaffolding a change; render with [`render_propose`] or
/// serialize for `--json`.
#[derive(Debug, Serialize)]
pub struct ProposeOutput {
    pub(super) change: String,
    pub(super) capabilities: Vec<String>,
    pub(super) created: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) template_overrides: Vec<String>,
    pub(super) next_steps: Vec<String>,
}

/// The `--json` wrapper for the change list.
#[derive(Debug, Serialize)]
pub struct ListOutput {
    pub changes: Vec<ChangeSummary>,
}

/// The `--json` wrapper for the specification list.
#[derive(Debug, Serialize)]
pub struct SpecsOutput {
    pub specs: Vec<SpecificationSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ContextDelta {
    pub(super) capability: String,
    pub(super) body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ContextTask {
    pub(super) text: String,
    pub(super) done: bool,
}

/// The agent-ready work order for one change; render with
/// [`render_context`] or serialize for `--json`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ContextOutput {
    pub(super) id: String,
    pub(super) proposal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) design: Option<String>,
    pub(super) deltas: Vec<ContextDelta>,
    pub(super) tasks: Vec<ContextTask>,
}

/// Result of archiving a change; render with [`render_archive`] (stdout)
/// plus [`ArchiveOutput::warnings`] (stderr), or serialize for `--json`.
#[derive(Debug, Serialize)]
pub struct ArchiveOutput {
    pub(super) change: String,
    pub(super) status: &'static str,
    pub(super) archived_to: String,
    pub(super) capabilities: Vec<String>,
    pub(super) merge: MergeSummary,
    pub(super) warnings: Vec<String>,
}

impl ArchiveOutput {
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// Scaffold a native change folder with one delta per capability.
pub fn propose(
    source: &str,
    id: &str,
    capabilities: &[String],
    design: bool,
    json: bool,
) -> Result<i32, Error> {
    let output = propose_output(source, id, capabilities, design)?;
    print_propose(&output, json)?;
    Ok(0)
}

/// Scaffold a change and return what was created, without printing.
pub fn propose_output(
    source: &str,
    id: &str,
    capabilities: &[String],
    design: bool,
) -> Result<ProposeOutput, Error> {
    validate_change_identifier(id)?;
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
        validate_capability(capability, "capability")?;
    }

    let root = Path::new(source);
    let change_dir = changes_root(root)?.join(id);
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
    Ok(ProposeOutput {
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
    })
}

/// Sort order for `spec list`, mirroring `openspec list`'s `--sort`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ListSort {
    /// Alphabetical by change id.
    #[default]
    Name,
    /// Least-complete changes first, so active work surfaces on top.
    Progress,
}

/// Scan active changes and sort them for listing, without printing.
pub fn sorted_changes(source: &str, sort: ListSort) -> Result<Vec<ChangeSummary>, Error> {
    let mut summaries = scan_changes(Path::new(source))?;
    if sort == ListSort::Progress {
        summaries.sort_by(|left, right| {
            left.completion_percent()
                .cmp(&right.completion_percent())
                .then_with(|| left.id.cmp(&right.id))
        });
    }
    Ok(summaries)
}

/// List active changes and their task completion fractions, or the
/// canonical capability specifications with `--specs`.
pub fn list(source: &str, specs: bool, sort: ListSort, json: bool) -> Result<i32, Error> {
    if specs {
        return list_specifications(Path::new(source), json);
    }
    let summaries = sorted_changes(source, sort)?;
    if json {
        print_json(&ListOutput { changes: summaries })?;
        return Ok(0);
    }

    let sheet = crate::sheet::Sheet::detect();
    if summaries.is_empty() {
        println!("{}", sheet.dim("No active changes."));
        return Ok(0);
    }
    print!("{}", render_change_list(&summaries, &sheet));
    Ok(0)
}

pub fn render_change_list(summaries: &[ChangeSummary], sheet: &crate::sheet::Sheet) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
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
        let _ = writeln!(
            rendered,
            "{state}{:<pad$} {} {progress}",
            "",
            sheet.bold(&format!("{:<32}", change.id)),
            pad = 10usize.saturating_sub(label.len()),
        );
    }
    rendered
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
    print!("{}", render_specification_list(&summaries));
    Ok(0)
}

pub fn render_specification_list(summaries: &[SpecificationSummary]) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    for specification in summaries {
        let _ = writeln!(
            rendered,
            "{:<32} {} requirement(s)",
            specification.capability, specification.requirements
        );
    }
    rendered
}

/// Emit an agent-ready work order for one active change.
pub fn context(source: &str, id: &str, json: bool) -> Result<i32, Error> {
    let output = context_output(source, id)?;
    if json {
        print_json(&output)?;
    } else {
        print_context(&output);
    }
    Ok(0)
}

/// Build the work order for one active change, without printing.
pub fn context_output(source: &str, id: &str) -> Result<ContextOutput, Error> {
    validate_change_identifier(id)?;
    load_context_output(Path::new(source), id)
}

pub(super) fn load_context_output(root: &Path, id: &str) -> Result<ContextOutput, Error> {
    let change_dir = changes_root(root)?.join(id);
    if !change_dir.is_dir() {
        if archived_change_exists(root, id)? {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "change '{id}' is already archived under {}",
                    changes_root(root)?.join("archive").display()
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
    for (capability, delta_path) in discover_capabilities(&change_dir.join("specs"))? {
        deltas.push(ContextDelta {
            capability,
            body: read(&delta_path)?,
        });
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
pub fn archive(source: &str, id: &str, yes: bool, abandon: bool, json: bool) -> Result<i32, Error> {
    let output = archive_output(source, id, yes, abandon)?;
    print_archive(&output, json)?;
    Ok(0)
}

/// Merge or abandon a change into the archive and return the result,
/// without printing.
pub fn archive_output(
    source: &str,
    id: &str,
    yes: bool,
    abandon: bool,
) -> Result<ArchiveOutput, Error> {
    validate_change_identifier(id)?;
    let root = Path::new(source);
    let spec_root = resolve_spec_root(root)?;
    let mut transaction = transaction::acquire(&spec_root)?;
    let change_dir = spec_root.changes().join(id);
    if !change_dir.is_dir() {
        if let Some(existing_archive) = archived_change_path(spec_root.changes(), id)? {
            return Ok(ArchiveOutput {
                change: id.to_string(),
                status: archived_change_status(&existing_archive)?,
                archived_to: relative_display(root, &existing_archive),
                capabilities: Vec::new(),
                merge: MergeSummary::default(),
                warnings: vec!["change was already archived; no files were changed".to_string()],
            });
        }
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "active change '{id}' was not found at {}",
                change_dir.display()
            ),
        ));
    }

    let archive_name = format!("{}-{id}", Utc::now().format("%Y-%m-%d"));
    let archive_dir = spec_root.changes().join("archive").join(&archive_name);
    if abandon {
        archive_abandoned(root, id, &change_dir, &archive_dir, &mut transaction)
    } else {
        archive_merged(root, id, yes, &change_dir, &archive_dir, &mut transaction)
    }
}

fn archive_abandoned(
    root: &Path,
    id: &str,
    change_dir: &Path,
    archive_dir: &Path,
    transaction: &mut transaction::Transaction,
) -> Result<ArchiveOutput, Error> {
    let proposal_path = change_dir.join("proposal.md");
    let proposal_update = FileWrite {
        path: proposal_path.clone(),
        content: abandoned_content(&proposal_path)?.into_bytes(),
    };
    transaction.execute(
        Operation::Abandon,
        id,
        change_dir,
        archive_dir,
        &[],
        Some(&proposal_update),
    )?;
    Ok(ArchiveOutput {
        change: id.to_string(),
        status: "abandoned",
        archived_to: relative_display(root, archive_dir),
        capabilities: Vec::new(),
        merge: MergeSummary::default(),
        warnings: Vec::new(),
    })
}

fn archive_merged(
    root: &Path,
    id: &str,
    yes: bool,
    change_dir: &Path,
    archive_dir: &Path,
    transaction: &mut transaction::Transaction,
) -> Result<ArchiveOutput, Error> {
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

    let plans = build_merge_plans(root, change_dir)?;
    let writes = plans
        .iter()
        .filter(|plan| plan.changed)
        .map(|plan| FileWrite {
            path: plan.destination.clone(),
            content: plan.content.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    transaction.execute(Operation::Merge, id, change_dir, archive_dir, &writes, None)?;

    let mut merge = MergeSummary::default();
    for plan in &plans {
        merge.add(plan.summary);
        warnings.extend(plan.warnings.iter().cloned());
    }
    Ok(ArchiveOutput {
        change: id.to_string(),
        status: "merged",
        archived_to: relative_display(root, archive_dir),
        capabilities: plans.iter().map(|plan| plan.capability.clone()).collect(),
        merge,
        warnings,
    })
}

/// Scan active changes without printing, for status and other services.
pub fn scan_changes(root: &Path) -> Result<Vec<ChangeSummary>, Error> {
    let changes_dir = changes_root(root)?;
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
pub fn scan_specifications(root: &Path) -> Result<Vec<SpecificationSummary>, Error> {
    let specs_dir = specs_root(root)?;
    let mut summaries = Vec::new();
    for (capability, spec_path) in discover_capabilities(&specs_dir)? {
        let content = read(&spec_path)?;
        let requirements = parse_canonical(&content).map_or(0, |spec| spec.requirement_count());
        summaries.push(SpecificationSummary {
            capability,
            requirements,
        });
    }
    summaries.sort_by(|left, right| left.capability.cmp(&right.capability));
    Ok(summaries)
}

pub fn render_json(value: &impl Serialize) -> Result<String, Error> {
    serde_json::to_string_pretty(value).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot serialize lifecycle result: {error}"),
        )
    })
}

pub(super) fn print_json(value: &impl Serialize) -> Result<(), Error> {
    println!("{}", render_json(value)?);
    Ok(())
}

pub fn render_propose(output: &ProposeOutput) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "Created change '{}' for '{}':",
        output.change,
        output.capabilities.join(", ")
    );
    for path in &output.created {
        let _ = writeln!(rendered, "  + {path}");
    }
    for relative in &output.template_overrides {
        let _ = writeln!(rendered, "note: using template override {relative}");
    }
    let _ = writeln!(rendered, "Next steps:");
    for step in &output.next_steps {
        let _ = writeln!(rendered, "  - {step}");
    }
    rendered
}

fn print_propose(output: &ProposeOutput, json: bool) -> Result<(), Error> {
    if json {
        return print_json(output);
    }
    print!("{}", render_propose(output));
    Ok(())
}

pub fn render_context(output: &ContextOutput) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    let _ = writeln!(rendered, "# Work order: {}\n", output.id);
    let _ = writeln!(rendered, "## Proposal\n\n{}\n", output.proposal.trim());
    if let Some(design) = &output.design {
        let _ = writeln!(rendered, "## Design\n\n{design}\n");
    }
    for delta in &output.deltas {
        let _ = writeln!(
            rendered,
            "## Delta: {}\n\n{}\n",
            delta.capability,
            delta.body.trim()
        );
    }
    let _ = writeln!(rendered, "## Tasks");
    if output.tasks.is_empty() {
        let _ = writeln!(rendered, "\n_No checklist tasks found._");
        return rendered;
    }
    for task in &output.tasks {
        if task.done {
            let _ = writeln!(rendered, "- [x] {}", task.text);
        } else {
            let _ = writeln!(rendered, "- [ ] **TODO: {}**", task.text);
        }
    }
    rendered
}

fn print_context(output: &ContextOutput) {
    print!("{}", render_context(output));
}

pub fn render_archive(output: &ArchiveOutput) -> String {
    use std::fmt::Write as _;
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "Archived change '{}' as {} to {}.",
        output.change, output.status, output.archived_to
    );
    if output.status == "merged" {
        let _ = writeln!(
            rendered,
            "Merged {} added, {} modified, and {} removed requirement(s) across {} capability spec(s).",
            output.merge.added,
            output.merge.modified,
            output.merge.removed,
            output.capabilities.len()
        );
    }
    rendered
}

fn print_archive(output: &ArchiveOutput, json: bool) -> Result<(), Error> {
    if json {
        return print_json(output);
    }
    for warning in &output.warnings {
        eprintln!("warning: {warning}");
    }
    print!("{}", render_archive(output));
    Ok(())
}

fn abandoned_content(path: &Path) -> Result<String, Error> {
    let content = read(path)?;
    if let Some(rest) = content.strip_prefix("---\n") {
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
        Ok(format!("---\n{}\n---\n{body}", lines.join("\n")))
    } else {
        Ok(format!("---\nstatus: abandoned\n---\n{content}"))
    }
}

/// One active change with its task state; render with
/// [`render_show_change`] or serialize for `--json`.
#[derive(Debug, Serialize)]
pub struct ShowChangeOutput {
    pub(super) state: ChangeState,
    pub(super) completed: usize,
    pub(super) total: usize,
    #[serde(flatten)]
    pub(super) context: ContextOutput,
}

/// One canonical specification; render with [`render_show_specification`]
/// or serialize for `--json`.
#[derive(Debug, Serialize)]
pub struct ShowSpecOutput {
    pub(super) capability: String,
    pub(super) requirements: usize,
    pub(super) content: String,
}

/// Render one active change or one canonical capability specification.
pub fn show(source: &str, name: &str, json: bool) -> Result<i32, Error> {
    match show_output(source, name)? {
        ShowOutput::Change(output) => {
            if json {
                print_json(&output)?;
            } else {
                print!("{}", render_show_change(&output));
            }
        }
        ShowOutput::Specification { output, relative } => {
            if json {
                print_json(&output)?;
            } else {
                print!("{}", render_show_specification(&output, &relative));
            }
        }
    }
    Ok(0)
}

/// What `spec show` resolved: one active change, or one canonical
/// capability specification together with its repository-relative path for
/// the human header.
pub enum ShowOutput {
    Change(ShowChangeOutput),
    Specification {
        output: ShowSpecOutput,
        relative: String,
    },
}

/// Resolve a name or unambiguous prefix to a change or specification and
/// return its content, without printing.
pub fn show_output(source: &str, name: &str) -> Result<ShowOutput, Error> {
    validate_capability(name, "item name")?;
    let root = Path::new(source);
    let change_dir = changes_root(root)?.join(name);
    let spec_path = specs_root(root)?.join(name).join("spec.md");
    let change_exists = !name.contains('/') && change_dir.is_dir();
    match (change_exists, spec_path.is_file()) {
        (true, true) => Err(Error::new(
            ErrorKind::Config,
            format!(
                "'{name}' is both an active change and a capability specification:\n  - change: {} (rune spec context {name})\n  - specification: {}\npick one of those forms",
                relative_display(root, &change_dir),
                relative_display(root, &spec_path)
            ),
        )),
        (true, false) => show_change_output(root, name),
        (false, true) => show_specification_output(root, name, &spec_path),
        (false, false) => {
            if !name.contains('/') && archived_change_exists(root, name)? {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "change '{name}' is already archived under {}",
                        changes_root(root)?.join("archive").display()
                    ),
                ));
            }
            match prefix_match(root, name)? {
                PrefixMatch::Change(id) => show_change_output(root, &id),
                PrefixMatch::Specification(capability) => {
                    let path = specs_root(root)?.join(&capability).join("spec.md");
                    show_specification_output(root, &capability, &path)
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

pub(super) enum PrefixMatch {
    Change(String),
    Specification(String),
    Ambiguous(Vec<String>),
    None,
}

/// An unambiguous prefix works everywhere a full id does, so `spec show
/// add` reaches `add-widget` without shell completion.
pub(super) fn prefix_match(root: &Path, prefix: &str) -> Result<PrefixMatch, Error> {
    if prefix.len() < 2 {
        return Ok(PrefixMatch::None);
    }
    let mut changes: Vec<String> = read_directories(&changes_root(root)?)?
        .into_iter()
        .filter_map(|directory| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter(|id| id != "archive" && id.starts_with(prefix))
        .collect();
    let mut specifications: Vec<String> = discover_capabilities(&specs_root(root)?)?
        .into_iter()
        .map(|(capability, _)| capability)
        .filter(|capability| capability.starts_with(prefix))
        .collect();
    changes.sort();
    specifications.sort();
    Ok(match (changes.len(), specifications.len()) {
        (1, 0) => PrefixMatch::Change(changes.remove(0)),
        (0, 1) => PrefixMatch::Specification(specifications.remove(0)),
        (0, 0) => PrefixMatch::None,
        _ => {
            changes.append(&mut specifications);
            PrefixMatch::Ambiguous(changes)
        }
    })
}

fn show_change_output(root: &Path, id: &str) -> Result<ShowOutput, Error> {
    let task_status = read_tasks(&changes_root(root)?.join(id).join("tasks.md"))?;
    Ok(ShowOutput::Change(ShowChangeOutput {
        state: task_status.state(),
        completed: task_status.completed,
        total: task_status.total,
        context: load_context_output(root, id)?,
    }))
}

pub fn render_show_change(output: &ShowChangeOutput) -> String {
    format!(
        "{} · {} · {}/{} tasks\n\n{}",
        output.context.id,
        state_label(output.state),
        output.completed,
        output.total,
        render_context(&output.context)
    )
}

fn show_specification_output(
    root: &Path,
    capability: &str,
    path: &Path,
) -> Result<ShowOutput, Error> {
    let content = read(path)?;
    let requirements = parse_canonical(&content).map_or(0, |spec| spec.requirement_count());
    Ok(ShowOutput::Specification {
        output: ShowSpecOutput {
            capability: capability.to_string(),
            requirements,
            content,
        },
        relative: relative_display(root, path),
    })
}

pub fn render_show_specification(output: &ShowSpecOutput, relative: &str) -> String {
    format!(
        "{} · {} requirement(s) · {relative}\n\n{}",
        output.capability, output.requirements, output.content
    )
}
