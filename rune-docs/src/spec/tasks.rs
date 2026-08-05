//! Task-checkbox parsing for `tasks.md` checklists.
//!
//! The fence tracker here is looser than the parser's
//! (`parse.rs::update_fence`): it matches any leading backtick or tilde
//! fence run and tracks only the marker character, while the parser also
//! enforces the `CommonMark` minimum closing-fence length. The tests below
//! capture the divergence; unifying the two is safe only if they stay
//! green through the swap.

use super::ContextTask;
use super::io_error;
use crate::error::Error;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

static TASK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*[-*]\s+\[([ xX])\]\s*(.*)$").expect("static task regex is valid")
});

#[derive(Debug, Default)]
pub(super) struct TaskStatus {
    pub(super) completed: usize,
    pub(super) total: usize,
    pub(super) unchecked: Vec<String>,
    pub(super) tasks: Vec<ContextTask>,
}

impl TaskStatus {
    pub(super) fn state(&self) -> super::ChangeState {
        if self.total > 0 && self.completed == self.total {
            super::ChangeState::Complete
        } else if self.completed > 0 {
            super::ChangeState::Active
        } else {
            super::ChangeState::Draft
        }
    }
}

pub(super) fn read_tasks(path: &Path) -> Result<TaskStatus, Error> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TaskStatus::default());
        }
        Err(error) => return Err(io_error("read", path, error)),
    };
    Ok(parse_tasks(&content))
}

pub(super) fn parse_tasks(content: &str) -> TaskStatus {
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

#[cfg(test)]
mod tests;
