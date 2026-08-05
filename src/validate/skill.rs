//! Stable shell content rules.
//!
//! These are the rules Rune owns outright, in either checking path: standalone
//! `mdschema` cannot compare a heading against a directory name, and its
//! section-count vocabulary turns a fifth subsection into an error rather than
//! the advisory this convention wants.
//!
//! Everything here is a pure function over file content. Reporting belongs to
//! the caller.

use super::mdschema::{Heading, outline};
use super::{Diagnostic, Severity};

/// Direct H3 headings under `Instructions` past which the skill should route
/// detail into companion files instead.
const MAXIMUM_DIRECT_SUBSECTIONS: usize = 4;

/// Check that the H1, the frontmatter `name`, and the directory name agree.
///
/// Silent when any of the three is absent or when the file carries more than
/// one H1: the schema already reports that structure, and a second diagnostic
/// about the same defect helps nobody.
#[must_use]
pub fn check_identity(
    name: &str,
    directory_name: &str,
    headings: &[Heading],
    display_path: &str,
) -> Option<Diagnostic> {
    let top_level_headings = headings
        .iter()
        .filter(|heading| heading.level == 1)
        .collect::<Vec<_>>();
    if name.is_empty() || directory_name.is_empty() || top_level_headings.len() != 1 {
        return None;
    }

    let title = top_level_headings[0];
    if name == title.text.as_str() && name == directory_name {
        return None;
    }

    Some(Diagnostic {
        file: display_path.to_string(),
        line: Some(title.line),
        severity: Severity::Error,
        message: format!(
            "stable shell identity: frontmatter name '{name}', H1 '{}', and directory '{directory_name}' must be identical",
            title.text
        ),
    })
}

/// Warn when `Instructions` carries more direct H3 headings than a reader can
/// hold, which means the detail belongs in companion files or another skill.
#[must_use]
pub fn check_instruction_breadth(headings: &[Heading], display_path: &str) -> Option<Diagnostic> {
    let (instructions_index, instructions) = headings
        .iter()
        .enumerate()
        .find(|(_, heading)| heading.level == 2 && heading.text == "Instructions")?;

    let direct_subsections = headings[instructions_index + 1..]
        .iter()
        .take_while(|heading| heading.level > 2)
        .filter(|heading| heading.level == 3)
        .count();
    if direct_subsections <= MAXIMUM_DIRECT_SUBSECTIONS {
        return None;
    }

    Some(Diagnostic {
        file: display_path.to_string(),
        line: Some(instructions.line),
        severity: Severity::Warning,
        message: format!(
            "stable shell breadth: Instructions has more than {MAXIMUM_DIRECT_SUBSECTIONS} direct H3 headings; move detailed procedures into companion files or split the skill"
        ),
    })
}

/// Reject markdown tables in rune artifacts.
///
/// Runes are read by models, where a labeled list carries the same rows in
/// fewer tokens. Fenced blocks are exempt: a table inside an example is
/// content, not structure.
#[must_use]
pub fn check_no_tables(content: &str, display_path: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut inside_fence = false;
    let mut already_reported = false;

    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            inside_fence = !inside_fence;
            already_reported = false;
            continue;
        }
        if inside_fence || !trimmed.starts_with('|') {
            already_reported = false;
            continue;
        }
        if already_reported {
            continue;
        }
        diagnostics.push(Diagnostic {
            file: display_path.to_string(),
            line: Some(index + 1),
            severity: Severity::Error,
            message: "markdown table in a rune artifact; restate the rows as a labeled list (runes are AI-consumed and tables cost tokens without aiding comprehension)".to_string(),
        });
        already_reported = true;
    }

    diagnostics
}

/// Every Stable shell rule Rune owns, over one file.
#[must_use]
pub fn check(
    name: &str,
    directory_name: &str,
    content: &str,
    display_path: &str,
) -> Vec<Diagnostic> {
    let headings = outline(content);
    [
        check_identity(name, directory_name, &headings, display_path),
        check_instruction_breadth(&headings, display_path),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests;
