//! Non-interactive access to TUI review comments.

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use commands::review::{self, ExportFormat};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Format {
    #[default]
    Markdown,
    Stdout,
}

impl From<Format> for ExportFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::Markdown => Self::Markdown,
            Format::Stdout => Self::Stdout,
        }
    }
}

pub fn list(target: Option<&str>) -> Result<i32, String> {
    let target = resolve_target(target)?;
    let mut stdout = std::io::stdout().lock();
    list_to(&target, &mut stdout)
}

pub fn export(target: Option<&str>, format: Format) -> Result<i32, String> {
    let target = resolve_target(target)?;
    let mut stdout = std::io::stdout().lock();
    export_to(&target, format, &mut stdout)
}

fn resolve_target(target: Option<&str>) -> Result<PathBuf, String> {
    if let Some(target) = target {
        return Ok(PathBuf::from(target));
    }
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("cannot read current directory: {error}"))?;
    if current_dir.join(".rune-comments.yaml").is_file() {
        return Ok(current_dir);
    }
    Ok(crate::cli::target::bound_target().unwrap_or(current_dir))
}

fn list_to(root: &Path, writer: &mut impl std::io::Write) -> Result<i32, String> {
    for comment in review::load(root)? {
        let location = review::sanitize_terminal_text(&comment.location());
        let module = review::sanitize_terminal_text(&comment.module);
        let text = review::sanitize_terminal_text(&comment.text);
        writeln!(
            writer,
            "{}\t{}\t{}\t{}",
            location,
            comment.kind.label(),
            module,
            text
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(0)
}

fn export_to(root: &Path, format: Format, writer: &mut impl std::io::Write) -> Result<i32, String> {
    let comments = review::load(root)?;
    if comments.is_empty() {
        return Err(format!(
            "no review comments in {}",
            root.join(".rune-comments.yaml").display()
        ));
    }
    writer
        .write_all(review::export(root, &comments, format.into()).as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use commands::review::{CommentKind, ReviewComment};

    #[test]
    fn list_prints_location_type_module_and_text() {
        let root = tempfile::tempdir().unwrap();
        review::persist(
            root.path(),
            &[ReviewComment {
                module: "rune".to_string(),
                path: "src/lib.rs".to_string(),
                line: 4,
                end_line: None,
                kind: CommentKind::Note,
                text: "explain this".to_string(),
            }],
        )
        .unwrap();
        let mut output = Vec::new();

        list_to(root.path(), &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "src/lib.rs:4\tNOTE\trune\texplain this\n"
        );
    }

    #[test]
    fn list_strips_terminal_control_sequences() {
        let root = tempfile::tempdir().unwrap();
        review::persist(
            root.path(),
            &[ReviewComment {
                module: "ru\x1b[2Jne".to_string(),
                path: "src/lib.rs".to_string(),
                line: 4,
                end_line: None,
                kind: CommentKind::Note,
                text: "copy \x1b]52;c;evil\x07 payload".to_string(),
            }],
        )
        .unwrap();
        let mut output = Vec::new();

        list_to(root.path(), &mut output).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains(['\x1b', '\x07']));
        assert!(output.contains("ru[2Jne"));
        assert!(output.contains("]52;c;evil payload"));
    }
}
