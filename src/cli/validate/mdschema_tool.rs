use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use rune::validate::{Diagnostic, Severity};

use super::ValidationReport;

/// True when the standalone `mdschema` binary is on PATH.
///
/// The standalone checker (jackchuka/mdschema) owns the parts of the
/// `.mdschema` vocabulary that `validate::mdschema` does not implement:
/// optional sections and their permitted children, section order, sections
/// outside the declared vocabulary, `unique_per_level`, `count`,
/// `allow_additional`, field types and formats, word counts, and required or
/// forbidden text.
///
/// For the Stable shell that means the standalone checker is the only thing
/// enforcing the H2 order, rejecting unexpected H2 sections, keeping
/// `Prerequisites` and `References` flat, and holding H3 to its permitted
/// parents. Without it, every optional section in the convention is unchecked.
///
/// When present it supersedes the built-in checks; the built-in subset remains
/// the fallback.
pub fn available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("mdschema")
            .arg("version")
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

/// Run `mdschema check --schema <schema_path> <file>` and merge findings.
///
/// Violation lines carry a severity glyph: `✗` error, `⚠` warning,
/// `ℹ` info (reported as a warning). The exit code is 1 even for
/// warning-only runs, so findings are read from the output lines.
pub fn check_file(
    schema_path: &Path,
    file_path: &Path,
    display_path: &str,
    report: &mut ValidationReport,
) {
    let output = Command::new("mdschema")
        .arg("check")
        .arg("--schema")
        .arg(schema_path)
        .arg(file_path)
        .output();

    let Ok(output) = output else {
        report.fail(
            display_path,
            format!("{display_path}: cannot execute the mdschema binary"),
        );
        return;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let diagnostics = parse_violations(&stdout, display_path);

    // The checker exits 0 on a clean file and 1 when it found violations. A
    // non-zero exit with nothing parseable means it crashed, spoke a dialect
    // this parser does not recognise, or wrote its findings to stderr. Passing
    // that silence through would report a clean, fully checked file while
    // nothing was checked, with no fallback and no notice: worse than having
    // no binary at all. Fail the file loudly instead.
    if !output.status.success() && diagnostics.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .trim()
            .lines()
            .next()
            .unwrap_or("no output")
            .to_string();
        report.fail(
            display_path,
            format!(
                "{display_path}: mdschema check failed without reporting findings ({detail}); its output format may have changed"
            ),
        );
        return;
    }

    for diagnostic in diagnostics {
        report.diagnostic(&diagnostic);
    }
}

/// Parse `mdschema check` text output into diagnostics.
///
/// ```text
/// /abs/path/file.md
///   ✗ 1:1 [frontmatter] Frontmatter field 'x' should be a boolean
///   ⚠ 6:3 [structure] Required element "## Y" not found within "Z"
///
/// ✗ Found 2 violation(s) in 1 file(s)
/// ```
fn parse_violations(stdout: &str, display_path: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        let severity = if let Some(rest) = trimmed.strip_prefix('✗') {
            if rest.trim_start().starts_with("Found ") {
                continue;
            }
            Severity::Error
        } else if trimmed.starts_with('⚠') || trimmed.starts_with('ℹ') {
            Severity::Warning
        } else {
            continue;
        };

        let body = trimmed
            .trim_start_matches(['✗', '⚠', 'ℹ'])
            .trim_start()
            .to_string();
        let (line_number, message) = split_position(&body);

        diagnostics.push(Diagnostic {
            file: display_path.to_string(),
            line: line_number,
            severity,
            message,
        });
    }
    diagnostics
}

/// Split a leading `line:column ` position off a violation body.
fn split_position(body: &str) -> (Option<usize>, String) {
    let Some((position, rest)) = body.split_once(' ') else {
        return (None, body.to_string());
    };
    let Some((line_text, column_text)) = position.split_once(':') else {
        return (None, body.to_string());
    };
    if line_text.parse::<usize>().is_ok() && column_text.parse::<usize>().is_ok() {
        (line_text.parse().ok(), rest.to_string())
    } else {
        (None, body.to_string())
    }
}

#[cfg(test)]
mod tests;
