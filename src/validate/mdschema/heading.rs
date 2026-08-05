use crate::parse;

use super::{Diagnostic, Severity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub line: usize,
    pub level: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fence {
    marker: char,
    length: usize,
}

fn fence(line: &str) -> Option<Fence> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }

    let length = trimmed
        .chars()
        .take_while(|candidate| *candidate == marker)
        .count();
    (length >= 3).then_some(Fence { marker, length })
}

fn heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let level = trimmed
        .chars()
        .take_while(|candidate| *candidate == '#')
        .count();
    if !(1..=6).contains(&level) {
        return None;
    }

    let text = trimmed.get(level..)?;
    if !text.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }

    Some((level, text.trim().to_string()))
}

pub(super) fn extract_headings(body: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut open_fence: Option<Fence> = None;

    for (line_index, line) in body.lines().enumerate() {
        if let Some(candidate) = fence(line) {
            match open_fence {
                Some(open)
                    if candidate.marker == open.marker && candidate.length >= open.length =>
                {
                    open_fence = None;
                }
                None => open_fence = Some(candidate),
                Some(_) => {}
            }
            continue;
        }

        if open_fence.is_some() {
            continue;
        }

        if let Some((level, text)) = heading(line) {
            headings.push(Heading {
                line: line_index + 1,
                level,
                text,
            });
        }
    }

    headings
}

fn body_line_offset(file_content: &str) -> usize {
    match parse::split_frontmatter(file_content) {
        Some((yaml_text, _)) => {
            let prefix_length = 4 + yaml_text.len() + 4;
            let bounded = prefix_length.min(file_content.len());
            file_content[..bounded].lines().count()
        }
        None => 0,
    }
}

#[must_use]
pub fn outline(file_content: &str) -> Vec<Heading> {
    let body = parse::frontmatter_body(file_content);
    let offset = body_line_offset(file_content);

    extract_headings(body)
        .into_iter()
        .map(|mut heading| {
            heading.line += offset;
            heading
        })
        .collect()
}

pub(super) fn check(
    file_content: &str,
    file_path: &str,
    schema: &serde_yaml::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(heading_rules) = schema.get("heading_rules") else {
        return;
    };

    let no_skip_levels = heading_rules
        .get("no_skip_levels")
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(false);

    let max_depth = heading_rules
        .get("max_depth")
        .and_then(serde_yaml::Value::as_u64)
        .map(|depth| usize::try_from(depth).unwrap_or(usize::MAX));

    let headings = outline(file_content);
    let mut previous_level: Option<usize> = None;

    for heading in &headings {
        if let Some(maximum) = max_depth
            && heading.level > maximum
        {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: Some(heading.line),
                severity: Severity::Error,
                message: format!(
                    "heading '{}' at depth {} exceeds max_depth {}",
                    heading.text, heading.level, maximum
                ),
            });
        }

        if no_skip_levels
            && let Some(previous) = previous_level
            && heading.level > previous + 1
        {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: Some(heading.line),
                severity: Severity::Error,
                message: format!(
                    "heading '{}' skips from h{} to h{}",
                    heading.text, previous, heading.level
                ),
            });
        }

        previous_level = Some(heading.level);
    }
}
