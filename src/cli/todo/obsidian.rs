//! Obsidian Tasks transform: `- [ ] text 📅 due ➕ created ✅ done ⏫`.
//! The mapping follows the Tasks plugin emoji fields; todo.txt extensions
//! without an Obsidian equivalent stay as literal `key:value` text so the
//! round trip back to todo.txt loses nothing.

use super::parse::{TodoItem, parse_line};
use std::fmt::Write as _;

const DUE: char = '📅';
const CREATED: char = '➕';
const COMPLETED: char = '✅';
const HIGH: char = '⏫';
const MEDIUM: char = '🔼';
const LOW: char = '🔽';

pub fn to_obsidian(item: &TodoItem) -> String {
    let checkbox = if item.done { "- [x]" } else { "- [ ]" };
    let mut line = checkbox.to_string();

    let mut body_tokens: Vec<String> = Vec::new();
    for token in item.text.split_whitespace() {
        if let Some((key, value)) = token.split_once(':')
            && key == "due"
            && !value.is_empty()
        {
            continue; // re-emitted as the 📅 field below
        }
        body_tokens.push(match token.strip_prefix('+') {
            Some(project) if !project.is_empty() => format!("#{project}"),
            _ => token.to_string(),
        });
    }
    if !body_tokens.is_empty() {
        line.push(' ');
        line.push_str(&body_tokens.join(" "));
    }

    match item.priority {
        Some('A') => {
            line.push(' ');
            line.push(HIGH);
        }
        Some('B') => {
            line.push(' ');
            line.push(MEDIUM);
        }
        Some(_) => {
            line.push(' ');
            line.push(LOW);
        }
        None => {}
    }
    if let Some(due) = item
        .extensions
        .iter()
        .find(|(key, _)| key == "due")
        .map(|(_, value)| value)
    {
        let _ = write!(line, " {DUE} {due}");
    }
    if let Some(created) = &item.creation_date {
        let _ = write!(line, " {CREATED} {created}");
    }
    if let Some(completed) = &item.completion_date {
        let _ = write!(line, " {COMPLETED} {completed}");
    }
    line
}

pub fn from_obsidian(line: &str) -> Option<TodoItem> {
    let trimmed = line.trim_start();
    let (done, rest) = if let Some(rest) = trimmed
        .strip_prefix("- [x]")
        .or_else(|| trimmed.strip_prefix("- [X]"))
    {
        (true, rest)
    } else {
        (false, trimmed.strip_prefix("- [ ]")?)
    };

    let mut priority: Option<char> = None;
    let mut due: Option<String> = None;
    let mut created: Option<String> = None;
    let mut completed: Option<String> = None;
    let mut body: Vec<String> = Vec::new();

    let mut tokens = rest.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        let field_date =
            |slot: &mut Option<String>,
             tokens: &mut std::iter::Peekable<std::str::SplitWhitespace>| {
                if let Some(date) = tokens.next() {
                    *slot = Some(date.to_string());
                }
            };
        match token.chars().next() {
            Some(HIGH) => priority = Some('A'),
            Some(MEDIUM) => priority = Some('B'),
            Some(LOW) => priority = Some('C'),
            Some(DUE) => field_date(&mut due, &mut tokens),
            Some(CREATED) => field_date(&mut created, &mut tokens),
            Some(COMPLETED) => field_date(&mut completed, &mut tokens),
            _ => body.push(match token.strip_prefix('#') {
                Some(tag) if !tag.is_empty() => format!("+{tag}"),
                _ => token.to_string(),
            }),
        }
    }

    let mut source = String::new();
    if done {
        source.push('x');
        if let Some(completion) = &completed {
            source.push(' ');
            source.push_str(completion);
        }
        if let Some(creation) = &created {
            source.push(' ');
            source.push_str(creation);
        }
    } else {
        if let Some(priority) = priority {
            let _ = write!(source, "({priority})");
        }
        if let Some(creation) = &created {
            if !source.is_empty() {
                source.push(' ');
            }
            source.push_str(creation);
        }
    }
    if !body.is_empty() {
        if !source.is_empty() {
            source.push(' ');
        }
        source.push_str(&body.join(" "));
    }
    if let Some(due) = due {
        let _ = write!(source, " due:{due}");
    }
    Some(parse_line(&source))
}
