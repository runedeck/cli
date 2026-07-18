//! `rune todo`: TODO.txt at the repo root, strict todo.txt syntax, is the
//! canonical store. The Obsidian Tasks transform maps items to checkbox
//! markdown and back through the normalized `TodoItem` model; unknown
//! `key:value` extensions survive round-trips untouched.

mod obsidian;
mod parse;

pub use parse::{TodoItem, parse_line, render_line};

use commands::error::{Error, ErrorKind};
use std::path::Path;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum TodoAction {
    /// Append a task in todo.txt syntax.
    Add {
        /// The task text, e.g. "(A) fix the thing +rune @cli due:2026-08-01".
        text: String,
    },
    /// Complete a task by its 1-based list position.
    Do { position: usize },
    /// List tasks, optionally filtered by +project, @context, or (P).
    Ls {
        /// Filter: +project, @context, or a priority letter in parentheses.
        filter: Option<String>,
    },
    /// Emit the tasks as Obsidian Tasks markdown.
    Obsidian,
    /// Append tasks from an Obsidian Tasks markdown file.
    Import {
        /// Markdown file whose `- [ ]` task lines convert to todo.txt items.
        file: String,
    },
}

pub fn execute(action: Option<TodoAction>, all: bool, json: bool) -> Result<i32, Error> {
    if all {
        match action {
            None | Some(TodoAction::Ls { filter: None }) => {
                return aggregate(Path::new("."), json);
            }
            Some(_) => {
                return Err(Error::new(
                    ErrorKind::Config,
                    "--all only lists across the workspace; run add/do/import in the member's directory".to_string(),
                ));
            }
        }
    }
    execute_at(Path::new("."), action, json)
}

/// One listing per workspace member: the consumer root itself, then every
/// `dirs:` entry of its `.rune` that carries a TODO.txt. Missing required
/// members error; optional members note and skip.
fn aggregate(root: &Path, json: bool) -> Result<i32, Error> {
    let manifest = crate::cli::dotrune::load(root)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "no .rune here; --all aggregates over its dirs: members".to_string(),
        )
    })?;
    let sheet = crate::cli::style::Sheet::detect(false);
    let mut members: Vec<(String, std::path::PathBuf)> =
        vec![(".".to_string(), root.to_path_buf())];
    for member in &manifest.dirs {
        let member_root = root.join(&member.path);
        if !member_root.is_dir() {
            if member.required {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("required workspace member '{}' is missing", member.path),
                ));
            }
            if !json {
                println!(
                    "{}",
                    sheet.dim(&format!("{} — absent, skipped", member.path))
                );
            }
            continue;
        }
        // Use-time guard, independent of parse-time validation: the resolved
        // member stays inside the workspace container (the root's parent).
        let container = root
            .canonicalize()
            .ok()
            .and_then(|canonical| canonical.parent().map(std::path::Path::to_path_buf));
        if let (Some(container), Ok(canonical_member)) = (&container, member_root.canonicalize())
            && !canonical_member.starts_with(container)
        {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "workspace member '{}' resolves outside the workspace container",
                    member.path
                ),
            ));
        }
        members.push((member.path.clone(), member_root));
    }
    if json {
        // One workspace-level document: a member array, each with its tasks.
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (label, member_root) in members {
            let items = load_items(&member_root)?;
            let tasks: Vec<serde_json::Value> = items
                .iter()
                .enumerate()
                .map(|(index, item)| task_json(index, item))
                .collect();
            rows.push(serde_json::json!({ "member": label, "tasks": tasks }));
        }
        println!("{}", serde_json::json!({ "workspace": rows }));
        return Ok(0);
    }
    for (label, member_root) in members {
        println!("{}", sheet.heading(&label));
        list(&member_root, None, false)?;
    }
    Ok(0)
}

/// One task serializer for both the plain listing and the workspace
/// aggregation, so machine consumers see one schema.
fn task_json(index: usize, item: &TodoItem) -> serde_json::Value {
    serde_json::json!({
        "position": index + 1,
        "done": item.done,
        "priority": item.priority,
        "text": item.text,
        "projects": item.projects,
        "contexts": item.contexts,
    })
}

fn todo_path(root: &Path) -> std::path::PathBuf {
    root.join("TODO.txt")
}

fn load_items(root: &Path) -> Result<Vec<TodoItem>, Error> {
    let path = todo_path(root);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    Ok(content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_line)
        .collect())
}

fn save_items(root: &Path, items: &[TodoItem]) -> Result<(), Error> {
    let path = todo_path(root);
    let mut rendered = items.iter().map(render_line).collect::<Vec<_>>().join("\n");
    rendered.push('\n');
    crate::cli::config::write_atomic(&path, &rendered)
}

pub fn execute_at(root: &Path, action: Option<TodoAction>, json: bool) -> Result<i32, Error> {
    match action {
        None | Some(TodoAction::Ls { filter: None }) => list(root, None, json),
        Some(TodoAction::Ls { filter }) => list(root, filter.as_deref(), json),
        Some(TodoAction::Add { text }) => {
            let mut items = load_items(root)?;
            items.push(parse_line(&text));
            save_items(root, &items)?;
            if json {
                println!("{}", serde_json::json!({ "added": items.len() }));
            } else {
                println!("added {}", items.len());
            }
            Ok(0)
        }
        Some(TodoAction::Do { position }) => {
            let mut items = load_items(root)?;
            if position == 0 || position > items.len() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("no task {position}; {} task(s) listed", items.len()),
                ));
            }
            items[position - 1].complete_today();
            save_items(root, &items)?;
            if json {
                println!("{}", serde_json::json!({ "done": position }));
            } else {
                println!("done {position}");
            }
            Ok(0)
        }
        Some(TodoAction::Obsidian) => {
            for item in load_items(root)? {
                println!("{}", obsidian::to_obsidian(&item));
            }
            Ok(0)
        }
        Some(TodoAction::Import { file }) => {
            let content = std::fs::read_to_string(&file).map_err(|error| {
                Error::new(ErrorKind::Io, format!("cannot read {file}: {error}"))
            })?;
            let mut items = load_items(root)?;
            let mut imported = 0;
            for line in content.lines() {
                if let Some(item) = obsidian::from_obsidian(line) {
                    items.push(item);
                    imported += 1;
                }
            }
            save_items(root, &items)?;
            if json {
                println!("{}", serde_json::json!({ "imported": imported }));
            } else {
                println!("imported {imported}");
            }
            Ok(0)
        }
    }
}

fn list(root: &Path, filter: Option<&str>, json: bool) -> Result<i32, Error> {
    let items = load_items(root)?;
    let matches = |item: &TodoItem| match filter {
        None => true,
        Some(term) => item.matches_filter(term),
    };
    if json {
        let rows: Vec<serde_json::Value> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| matches(item))
            .map(|(index, item)| task_json(index, item))
            .collect();
        println!("{}", serde_json::json!({ "tasks": rows }));
        return Ok(0);
    }
    let sheet = crate::cli::style::Sheet::detect(false);
    if items.is_empty() {
        println!("{}", sheet.dim("no tasks — add one: rune todo add \"...\""));
        return Ok(0);
    }
    for (index, item) in items.iter().enumerate() {
        if !matches(item) {
            continue;
        }
        let position = format!("{:>3}", index + 1);
        let line = render_line(item);
        if item.done {
            println!(" {} {}", sheet.dim(&position), sheet.dim(&line));
        } else if item.priority == Some('A') {
            println!(" {} {}", sheet.dim(&position), sheet.red(&line));
        } else if item.priority.is_some() {
            println!(" {} {}", sheet.dim(&position), sheet.yellow(&line));
        } else {
            println!(" {} {line}", sheet.dim(&position));
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests;
