//! The readable copy of a Grok session: a port of the script's
//! `normalize_grok`. Each line of `chat_history.jsonl` becomes one
//! `## Type` section whose body is the message content.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Render one `chat_history.jsonl` to Markdown.
#[must_use]
pub fn render(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = entry
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("message");
        out.push_str("## ");
        out.push_str(&capitalize(kind));
        out.push_str("\n\n");
        out.push_str(&body(entry.get("content")));
        out.push_str("\n\n");
    }
    out
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn body(content: Option<&Value>) -> String {
    match content {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .map_or_else(|| item.to_string(), str::to_string)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
    }
}

/// Render every `chat_history.jsonl` under the native copies into the
/// transcripts folder, beside the same relative path, or one session's
/// history alone. A render is written beside the output and moved into
/// place, so a failure never destroys the last good copy. The result is
/// the number rendered; an error names the first failure.
pub fn render_all(
    sessions: &Path,
    transcripts: &Path,
    session: Option<&str>,
) -> Result<usize, String> {
    let mut histories = Vec::new();
    collect(sessions, &mut histories);
    let mut rendered = 0;
    let mut seen = 0;
    let mut failure = None;
    for history in histories {
        if let Some(id) = session {
            let in_session = history
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == id);
            if !in_session {
                continue;
            }
        }
        seen += 1;
        let relative = history
            .strip_prefix(sessions)
            .map_err(|_| format!("{} is outside {}", history.display(), sessions.display()))?;
        let output = transcripts
            .join(relative.with_extension(""))
            .with_extension("md");
        match render_file(&history, &output) {
            Ok(()) => rendered += 1,
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if session.is_some() && seen == 0 {
        return Err(format!(
            "grok session {} has no chat history under {}",
            session.unwrap_or_default(),
            sessions.display()
        ));
    }
    Ok(rendered)
}

/// The output path of one history: `<dir>/<session>.md`, since the file
/// itself is `chat_history.jsonl` inside the session's folder.
fn render_file(history: &Path, output: &Path) -> Result<(), String> {
    let output = output
        .parent()
        .and_then(|dir| dir.parent().map(|grand| (grand, dir)))
        .map_or_else(
            || output.to_path_buf(),
            |(grand, dir)| {
                grand.join(format!(
                    "{}.md",
                    dir.file_name().unwrap_or_default().to_string_lossy()
                ))
            },
        );
    let text = std::fs::read_to_string(history)
        .map_err(|error| format!("cannot read {}: {error}", history.display()))?;
    let markdown = render(&text);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let temporary = output.with_extension("md.tmp");
    std::fs::write(&temporary, markdown)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, &output).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!(
            "cannot move {} to {}: {error}",
            temporary.display(),
            output.display()
        )
    })
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path
            .file_name()
            .is_some_and(|name| name == "chat_history.jsonl")
        {
            out.push(path);
        }
    }
    out.sort();
}
