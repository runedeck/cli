//! Persisted source-review comments and agent-ready exports.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};

const STORE_VERSION: u32 = 1;
const SIDECAR_NAME: &str = ".rune-comments.yaml";

#[derive(
    Debug,
    Clone,
    Copy,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize
)]
#[serde(rename_all = "lowercase")]
pub enum CommentKind {
    Issue,
    Note,
    Suggestion,
    Praise,
}

impl CommentKind {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Issue => Self::Note,
            Self::Note => Self::Suggestion,
            Self::Suggestion => Self::Praise,
            Self::Praise => Self::Issue,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Issue => "ISSUE",
            Self::Note => "NOTE",
            Self::Suggestion => "SUGGESTION",
            Self::Praise => "PRAISE",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
pub struct ReviewComment {
    pub module: String,
    pub path: String,
    pub line: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    pub kind: CommentKind,
    pub text: String,
}

impl ReviewComment {
    #[must_use]
    pub fn last_line(&self) -> usize {
        self.end_line.unwrap_or(self.line).max(self.line)
    }

    #[must_use]
    pub fn location(&self) -> String {
        if self.last_line() == self.line {
            format!("{}:{}", self.path, self.line)
        } else {
            format!("{}:{}-{}", self.path, self.line, self.last_line())
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct CommentStore {
    version: u32,
    comments: Vec<ReviewComment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Stdout,
}

/// Load the persisted review sidecar from a module or repository root.
///
/// A missing sidecar is an empty review. Version 1 records without
/// `end_line` remain valid and deserialize as single-line comments.
pub fn load(root: &Path) -> Result<Vec<ReviewComment>, String> {
    let path = root.join(SIDECAR_NAME);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    let store: CommentStore = serde_yaml::from_str(&content)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    if store.version != STORE_VERSION {
        return Err(format!(
            "unsupported comment sidecar version {} in {}",
            store.version,
            path.display()
        ));
    }
    Ok(store.comments)
}

/// Atomically persist review comments to `.rune-comments.yaml`.
pub fn persist(root: &Path, comments: &[ReviewComment]) -> Result<(), String> {
    let path = root.join(SIDECAR_NAME);
    let store = CommentStore {
        version: STORE_VERSION,
        comments: comments.to_vec(),
    };
    let content = serde_yaml::to_string(&store).map_err(|error| error.to_string())?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = root.join(format!("{SIDECAR_NAME}.tmp-{}-{nonce}", std::process::id()));
    std::fs::write(&temporary, content).map_err(|error| error.to_string())?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

/// Render persisted comments grouped by file with their selected source lines.
#[must_use]
pub fn export(root: &Path, comments: &[ReviewComment], format: ExportFormat) -> String {
    match format {
        ExportFormat::Markdown => export_markdown(root, comments),
        ExportFormat::Stdout => export_stdout(root, comments),
    }
}

/// Copy text to the system clipboard, preferring `pbcopy` on macOS and using
/// terminal clipboard integration when no native pasteboard is available.
pub fn copy_to_clipboard(text: &str) -> Result<bool, String> {
    if cfg!(target_os = "macos") && try_clipboard_command("/usr/bin/pbcopy", &[], text) {
        return Ok(false);
    }
    if std::env::var_os("TMUX").is_some() {
        return copy_with_tmux(text).map(|()| true);
    }
    let stdout = std::io::stdout();
    write_osc52(stdout.lock(), text).map(|()| true)
}

fn try_clipboard_command(program: &str, args: &[&str], text: &str) -> bool {
    use std::process::{Command, Stdio};

    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(text.as_bytes()).is_err()
    {
        return false;
    }
    child.wait().is_ok_and(|status| status.success())
}

fn copy_with_tmux(text: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("tmux")
        .args(["load-buffer", "-w", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to run tmux: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed to write to tmux: {error}"))?;
    }
    let status = child
        .wait()
        .map_err(|error| format!("tmux load-buffer failed: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "tmux load-buffer exited with an error".to_string())
}

fn write_osc52(mut writer: impl IoWrite, text: &str) -> Result<(), String> {
    let encoded = BASE64.encode(text);
    write!(writer, "\x1b]52;c;{encoded}\x07").map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn grouped(comments: &[ReviewComment]) -> BTreeMap<&str, Vec<&ReviewComment>> {
    let mut groups: BTreeMap<&str, Vec<&ReviewComment>> = BTreeMap::new();
    for comment in comments {
        groups.entry(&comment.path).or_default().push(comment);
    }
    for values in groups.values_mut() {
        values.sort_by_key(|comment| (comment.line, comment.last_line(), comment.kind));
    }
    groups
}

fn export_markdown(root: &Path, comments: &[ReviewComment]) -> String {
    let mut output = String::from(
        "I reviewed your code and have the following comments. Please address them.\n",
    );
    let mut number = 1;
    for (path, group) in grouped(comments) {
        let _ = write!(output, "\n## `{path}`\n");
        for comment in group {
            let _ = write!(
                output,
                "\n{number}. **[{}]** `{}` - {}\n",
                comment.kind.label(),
                comment.location(),
                comment.text
            );
            write_markdown_context(&mut output, root, comment);
            number += 1;
        }
    }
    output
}

fn export_stdout(root: &Path, comments: &[ReviewComment]) -> String {
    let mut output = String::new();
    for (path, group) in grouped(comments) {
        let _ = writeln!(output, "{path}");
        for comment in group {
            let _ = writeln!(
                output,
                "  [{}] {} - {}",
                comment.kind.label(),
                comment.location(),
                comment.text
            );
            for (line_number, line) in source_context(root, comment) {
                let _ = writeln!(output, "    {line_number:>4} | {line}");
            }
        }
    }
    output
}

fn write_markdown_context(output: &mut String, root: &Path, comment: &ReviewComment) {
    let context = source_context(root, comment);
    if context.is_empty() {
        return;
    }
    output.push_str("\n   ```text\n");
    for (line_number, line) in context {
        let _ = writeln!(output, "   {line_number:>4} | {line}");
    }
    output.push_str("   ```\n");
}

fn source_context(root: &Path, comment: &ReviewComment) -> Vec<(usize, String)> {
    let candidate = safe_source_path(root, &comment.path);
    let Ok(source) = std::fs::read_to_string(candidate) else {
        return Vec::new();
    };
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let number = index + 1;
            (number >= comment.line && number <= comment.last_line())
                .then(|| (number, line.to_string()))
        })
        .collect()
}

fn safe_source_path(root: &Path, relative: &str) -> PathBuf {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return PathBuf::new();
    }
    root.join(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_comment_without_end_line_loads() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(SIDECAR_NAME),
            "version: 1\ncomments:\n- module: rune\n  path: src/lib.rs\n  line: 2\n  kind: issue\n  text: fix it\n",
        )
        .unwrap();

        let comments = load(root.path()).unwrap();

        assert_eq!(comments[0].end_line, None);
        assert_eq!(comments[0].location(), "src/lib.rs:2");
    }

    #[test]
    fn markdown_export_groups_files_and_includes_range_context() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/lib.rs"), "one\ntwo\nthree\n").unwrap();
        let comments = vec![ReviewComment {
            module: "rune".to_string(),
            path: "src/lib.rs".to_string(),
            line: 2,
            end_line: Some(3),
            kind: CommentKind::Issue,
            text: "tighten this".to_string(),
        }];

        let output = export(root.path(), &comments, ExportFormat::Markdown);

        assert!(output.contains("## `src/lib.rs`"));
        assert!(output.contains("`src/lib.rs:2-3`"));
        assert!(output.contains("   2 | two"));
        assert!(output.contains("   3 | three"));
    }
}
