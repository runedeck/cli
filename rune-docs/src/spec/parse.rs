use super::model::{
    BodyElement, CanonicalSpec, DeltaKind, DeltaOperation, ParseIssue, Requirement,
};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

static NORMATIVE_KEYWORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(SHALL|MUST)\b").expect("normative keyword regex is valid"));

#[derive(Clone, Copy)]
struct ScannedLine<'content> {
    start: usize,
    full_end: usize,
    number: usize,
    text: &'content str,
    fenced: bool,
}

#[derive(Clone, Copy)]
struct DeltaSection {
    kind: DeltaKind,
    start: usize,
    end: usize,
    line: usize,
}

pub(super) fn parse_canonical(content: &str) -> Result<CanonicalSpec, Vec<ParseIssue>> {
    let scanned_lines = scan_lines(content);
    let requirements_sections = scanned_lines
        .iter()
        .filter(|scanned_line| {
            !scanned_line.fenced
                && heading_text(scanned_line.text, 2)
                    .is_some_and(|heading| heading.eq_ignore_ascii_case("Requirements"))
        })
        .copied()
        .collect::<Vec<_>>();
    let Some(requirements_heading) = requirements_sections.first().copied() else {
        return Err(vec![ParseIssue {
            line: None,
            message: "missing required '## Requirements' section".to_string(),
        }]);
    };
    let mut issues = Vec::new();
    if requirements_sections.len() > 1 {
        issues.push(ParseIssue {
            line: Some(requirements_sections[1].number),
            message: "canonical specification contains more than one Requirements section"
                .to_string(),
        });
    }
    let section_end = scanned_lines
        .iter()
        .find(|scanned_line| {
            scanned_line.start >= requirements_heading.full_end
                && !scanned_line.fenced
                && heading_text(scanned_line.text, 2).is_some()
        })
        .map_or(content.len(), |scanned_line| scanned_line.start);
    let (body, mut requirement_issues) = parse_canonical_body(
        content,
        &scanned_lines,
        requirements_heading.full_end,
        section_end,
    );
    issues.append(&mut requirement_issues);
    if issues.is_empty() {
        Ok(CanonicalSpec::parsed(
            content[..requirements_heading.full_end].to_string(),
            body,
            content[section_end..].to_string(),
            first_line_ending(content).to_string(),
        ))
    } else {
        Err(issues)
    }
}

pub(super) fn parse_delta(content: &str) -> Result<Vec<DeltaOperation>, Vec<ParseIssue>> {
    let scanned_lines = scan_lines(content);
    let sections = delta_sections(content, &scanned_lines)?;
    if sections.is_empty() {
        return Err(vec![ParseIssue {
            line: None,
            message:
                "delta must contain an ADDED, MODIFIED, REMOVED, or RENAMED Requirements section"
                    .to_string(),
        }]);
    }

    let mut operations = Vec::new();
    let mut issues = Vec::new();
    for section in sections {
        match section.kind {
            DeltaKind::Added | DeltaKind::Modified => {
                let (requirements, mut found_issues) = parse_delta_requirements(
                    content,
                    &scanned_lines,
                    section.start,
                    section.end,
                    section.kind,
                );
                issues.append(&mut found_issues);
                if requirements.is_empty() {
                    issues.push(ParseIssue {
                        line: Some(section.line),
                        message: format!(
                            "{} Requirements must contain a requirement",
                            section.kind.heading()
                        ),
                    });
                }
                operations.extend(
                    requirements
                        .into_iter()
                        .map(|requirement| match section.kind {
                            DeltaKind::Added => DeltaOperation::Added(requirement),
                            DeltaKind::Modified => DeltaOperation::Modified(requirement),
                            DeltaKind::Removed | DeltaKind::Renamed => unreachable!(),
                        }),
                );
            }
            DeltaKind::Removed => {
                let removed = parse_removed(&scanned_lines, section.start, section.end);
                if removed.is_empty() {
                    issues.push(ParseIssue {
                        line: Some(section.line),
                        message: "REMOVED Requirements must contain a requirement".to_string(),
                    });
                }
                operations.extend(removed);
            }
            DeltaKind::Renamed => {
                let (renamed, mut found_issues) =
                    parse_renamed(&scanned_lines, section.start, section.end, section.line);
                issues.append(&mut found_issues);
                if renamed.is_empty() {
                    issues.push(ParseIssue {
                        line: Some(section.line),
                        message: "RENAMED Requirements must contain an ordered FROM and TO pair"
                            .to_string(),
                    });
                }
                operations.extend(renamed);
            }
        }
    }
    validate_delta_conflicts(&operations, &mut issues);
    if issues.is_empty() {
        Ok(operations)
    } else {
        Err(issues)
    }
}

fn parse_canonical_body(
    content: &str,
    scanned_lines: &[ScannedLine<'_>],
    section_start: usize,
    section_end: usize,
) -> (Vec<BodyElement>, Vec<ParseIssue>) {
    let headings = requirement_headings(scanned_lines, section_start, section_end);
    let mut elements = Vec::new();
    let mut issues = Vec::new();
    let mut names = BTreeSet::new();
    let mut cursor = section_start;
    for (position, (heading_index, name)) in headings.iter().enumerate() {
        let heading = scanned_lines[*heading_index];
        if cursor < heading.start {
            elements.push(BodyElement::Text(
                content[cursor..heading.start].to_string(),
            ));
        }
        let boundary = headings
            .get(position + 1)
            .map_or(section_end, |(next_index, _)| {
                scanned_lines[*next_index].start
            });
        let raw_block = &content[heading.start..boundary];
        let trimmed_length = raw_block.trim_end().len();
        let content_end = heading.start + trimmed_length;
        let requirement_content = content[heading.start..content_end].to_string();
        if !names.insert(name.clone()) {
            issues.push(ParseIssue {
                line: Some(heading.number),
                message: format!("duplicate requirement '{name}'"),
            });
        }
        let requirement = build_requirement(name, requirement_content, heading.number);
        validate_normative_body(&requirement, false, &mut issues);
        elements.push(BodyElement::Requirement(requirement));
        cursor = content_end;
    }
    if cursor < section_end {
        elements.push(BodyElement::Text(content[cursor..section_end].to_string()));
    }
    (elements, issues)
}

fn parse_delta_requirements(
    content: &str,
    scanned_lines: &[ScannedLine<'_>],
    section_start: usize,
    section_end: usize,
    kind: DeltaKind,
) -> (Vec<Requirement>, Vec<ParseIssue>) {
    let headings = requirement_headings(scanned_lines, section_start, section_end);
    let mut requirements = Vec::new();
    let mut issues = Vec::new();
    for (position, (heading_index, name)) in headings.iter().enumerate() {
        let heading = scanned_lines[*heading_index];
        let boundary = headings
            .get(position + 1)
            .map_or(section_end, |(next_index, _)| {
                scanned_lines[*next_index].start
            });
        let raw_block = &content[heading.start..boundary];
        let requirement_content = raw_block.trim_end().to_string();
        let requirement = build_requirement(name, requirement_content, heading.number);
        validate_normative_body(&requirement, true, &mut issues);
        requirements.push(requirement);
    }
    let mut names = BTreeSet::new();
    for requirement in &requirements {
        if !names.insert(requirement.name.as_str()) {
            issues.push(ParseIssue {
                line: Some(requirement.line),
                message: format!(
                    "requirement '{}' appears more than once in {} Requirements",
                    requirement.name,
                    kind.heading()
                ),
            });
        }
    }
    (requirements, issues)
}

fn parse_removed(
    scanned_lines: &[ScannedLine<'_>],
    section_start: usize,
    section_end: usize,
) -> Vec<DeltaOperation> {
    scanned_lines
        .iter()
        .filter(|scanned_line| {
            scanned_line.start >= section_start
                && scanned_line.start < section_end
                && !scanned_line.fenced
        })
        .filter_map(|scanned_line| {
            parse_removed_name(scanned_line.text).map(|name| DeltaOperation::Removed {
                name,
                line: scanned_line.number,
            })
        })
        .collect()
}

fn parse_renamed(
    scanned_lines: &[ScannedLine<'_>],
    section_start: usize,
    section_end: usize,
    section_line: usize,
) -> (Vec<DeltaOperation>, Vec<ParseIssue>) {
    let mut operations = Vec::new();
    let mut issues = Vec::new();
    let mut pending_from: Option<(String, usize)> = None;
    for scanned_line in scanned_lines.iter().filter(|scanned_line| {
        scanned_line.start >= section_start
            && scanned_line.start < section_end
            && !scanned_line.fenced
    }) {
        if let Some(name) = parse_rename_reference(scanned_line.text, "FROM:") {
            if pending_from.is_some() {
                issues.push(ParseIssue {
                    line: Some(scanned_line.number),
                    message: "RENAMED Requirements contains a FROM without a preceding TO"
                        .to_string(),
                });
            }
            pending_from = Some((name, scanned_line.number));
            continue;
        }
        if let Some(name) = parse_rename_reference(scanned_line.text, "TO:") {
            let Some((from, line)) = pending_from.take() else {
                issues.push(ParseIssue {
                    line: Some(scanned_line.number),
                    message: "RENAMED Requirements contains a TO before FROM".to_string(),
                });
                continue;
            };
            operations.push(DeltaOperation::Renamed {
                from,
                to: name,
                line,
            });
        }
    }
    if let Some((_, line)) = pending_from {
        issues.push(ParseIssue {
            line: Some(line),
            message: "RENAMED Requirements contains a FROM without a following TO".to_string(),
        });
    }
    if operations.is_empty() && issues.is_empty() {
        issues.push(ParseIssue {
            line: Some(section_line),
            message: "RENAMED Requirements contains no recognized FROM and TO pair".to_string(),
        });
    }
    (operations, issues)
}

fn validate_delta_conflicts(operations: &[DeltaOperation], issues: &mut Vec<ParseIssue>) {
    let mut names_by_kind: BTreeMap<DeltaKindKey, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut renamed_from = BTreeMap::new();
    let mut renamed_to = BTreeMap::new();
    for operation in operations {
        match operation {
            DeltaOperation::Added(requirement) => {
                insert_delta_name(
                    &mut names_by_kind,
                    DeltaKindKey::Added,
                    &requirement.name,
                    requirement.line,
                    issues,
                );
            }
            DeltaOperation::Modified(requirement) => {
                insert_delta_name(
                    &mut names_by_kind,
                    DeltaKindKey::Modified,
                    &requirement.name,
                    requirement.line,
                    issues,
                );
            }
            DeltaOperation::Removed { name, line } => {
                insert_delta_name(
                    &mut names_by_kind,
                    DeltaKindKey::Removed,
                    name,
                    *line,
                    issues,
                );
            }
            DeltaOperation::Renamed { from, to, line } => {
                insert_rename_name(&mut renamed_from, from, *line, "FROM", issues);
                insert_rename_name(&mut renamed_to, to, *line, "TO", issues);
            }
        }
    }

    let added = names_by_kind.get(&DeltaKindKey::Added);
    let modified = names_by_kind.get(&DeltaKindKey::Modified);
    let removed = names_by_kind.get(&DeltaKindKey::Removed);
    report_cross_conflicts(added, modified, "ADDED", "MODIFIED", issues);
    report_cross_conflicts(added, removed, "ADDED", "REMOVED", issues);
    report_cross_conflicts(modified, removed, "MODIFIED", "REMOVED", issues);
    report_cross_conflicts(Some(&renamed_to), added, "RENAMED TO", "ADDED", issues);
    report_cross_conflicts(
        Some(&renamed_from),
        modified,
        "RENAMED FROM",
        "MODIFIED",
        issues,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DeltaKindKey {
    Added,
    Modified,
    Removed,
}

fn insert_delta_name<'name>(
    names_by_kind: &mut BTreeMap<DeltaKindKey, BTreeMap<&'name str, usize>>,
    kind: DeltaKindKey,
    name: &'name str,
    line: usize,
    issues: &mut Vec<ParseIssue>,
) {
    let names = names_by_kind.entry(kind).or_default();
    if names.insert(name, line).is_some() {
        issues.push(ParseIssue {
            line: Some(line),
            message: format!("duplicate {} requirement '{name}'", delta_kind_label(kind)),
        });
    }
}

fn insert_rename_name<'name>(
    names: &mut BTreeMap<&'name str, usize>,
    name: &'name str,
    line: usize,
    marker: &str,
    issues: &mut Vec<ParseIssue>,
) {
    if names.insert(name, line).is_some() {
        issues.push(ParseIssue {
            line: Some(line),
            message: format!("duplicate RENAMED {marker} requirement '{name}'"),
        });
    }
}

fn report_cross_conflicts(
    first: Option<&BTreeMap<&str, usize>>,
    second: Option<&BTreeMap<&str, usize>>,
    first_label: &str,
    second_label: &str,
    issues: &mut Vec<ParseIssue>,
) {
    let (Some(first), Some(second)) = (first, second) else {
        return;
    };
    for (name, line) in first {
        if second.contains_key(name) {
            issues.push(ParseIssue {
                line: Some(*line),
                message: format!(
                    "requirement '{name}' appears in both {first_label} and {second_label} Requirements"
                ),
            });
        }
    }
}

fn delta_kind_label(kind: DeltaKindKey) -> &'static str {
    match kind {
        DeltaKindKey::Added => "ADDED",
        DeltaKindKey::Modified => "MODIFIED",
        DeltaKindKey::Removed => "REMOVED",
    }
}

fn delta_sections(
    content: &str,
    scanned_lines: &[ScannedLine<'_>],
) -> Result<Vec<DeltaSection>, Vec<ParseIssue>> {
    let section_headings = scanned_lines
        .iter()
        .filter_map(|scanned_line| {
            if scanned_line.fenced {
                return None;
            }
            let title = heading_text(scanned_line.text, 2)?;
            delta_kind(title).map(|kind| (*scanned_line, kind))
        })
        .collect::<Vec<_>>();
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut sections = Vec::new();
    for (position, (heading, kind)) in section_headings.iter().enumerate() {
        if !seen.insert(kind.heading()) {
            issues.push(ParseIssue {
                line: Some(heading.number),
                message: format!("duplicate {} Requirements section", kind.heading()),
            });
        }
        let section_end = scanned_lines
            .iter()
            .find(|scanned_line| {
                scanned_line.start >= heading.full_end
                    && !scanned_line.fenced
                    && heading_text(scanned_line.text, 2).is_some()
            })
            .map_or(content.len(), |scanned_line| scanned_line.start);
        let next_known_start = section_headings
            .get(position + 1)
            .map_or(content.len(), |(next_heading, _)| next_heading.start);
        sections.push(DeltaSection {
            kind: *kind,
            start: heading.full_end,
            end: section_end.min(next_known_start),
            line: heading.number,
        });
    }
    if issues.is_empty() {
        Ok(sections)
    } else {
        Err(issues)
    }
}

fn delta_kind(title: &str) -> Option<DeltaKind> {
    if title.eq_ignore_ascii_case("ADDED Requirements") {
        Some(DeltaKind::Added)
    } else if title.eq_ignore_ascii_case("MODIFIED Requirements") {
        Some(DeltaKind::Modified)
    } else if title.eq_ignore_ascii_case("REMOVED Requirements") {
        Some(DeltaKind::Removed)
    } else if title.eq_ignore_ascii_case("RENAMED Requirements") {
        Some(DeltaKind::Renamed)
    } else {
        None
    }
}

fn requirement_headings(
    scanned_lines: &[ScannedLine<'_>],
    section_start: usize,
    section_end: usize,
) -> Vec<(usize, String)> {
    scanned_lines
        .iter()
        .enumerate()
        .filter(|(_, scanned_line)| {
            scanned_line.start >= section_start
                && scanned_line.start < section_end
                && !scanned_line.fenced
        })
        .filter_map(|(index, scanned_line)| {
            requirement_name(scanned_line.text).map(|name| (index, name))
        })
        .collect()
}

fn build_requirement(name: &str, content: String, line: usize) -> Requirement {
    let scenarios = scan_lines(&content)
        .into_iter()
        .filter(|scanned_line| !scanned_line.fenced)
        .filter_map(|scanned_line| heading_text(scanned_line.text, 4).map(str::to_string))
        .collect();
    Requirement {
        name: name.to_string(),
        content,
        line,
        scenarios,
    }
}

fn validate_normative_body(
    requirement: &Requirement,
    scenario_required: bool,
    issues: &mut Vec<ParseIssue>,
) {
    let scanned_lines = scan_lines(&requirement.content);
    let body = scanned_lines
        .iter()
        .skip(1)
        .take_while(|scanned_line| {
            scanned_line.fenced || heading_level(scanned_line.text).is_none_or(|level| level < 3)
        })
        .filter(|scanned_line| !scanned_line.fenced)
        .map(|scanned_line| scanned_line.text)
        .collect::<Vec<_>>()
        .join("\n");
    if !NORMATIVE_KEYWORD.is_match(&body) {
        issues.push(ParseIssue {
            line: Some(requirement.line),
            message: format!(
                "requirement '{}' must contain SHALL or MUST in its body",
                requirement.name
            ),
        });
    }
    if scenario_required && requirement.scenarios.is_empty() {
        issues.push(ParseIssue {
            line: Some(requirement.line),
            message: format!(
                "requirement '{}' must contain at least one level-four scenario",
                requirement.name
            ),
        });
    }
}

fn parse_removed_name(line: &str) -> Option<String> {
    if let Some(name) = requirement_name(line) {
        return Some(name);
    }
    let bullet = line.trim_start().strip_prefix('-')?.trim();
    let unquoted = strip_optional_backticks(bullet);
    requirement_name(unquoted)
}

fn parse_rename_reference(line: &str, marker: &str) -> Option<String> {
    let unbulleted = line
        .trim_start()
        .strip_prefix('-')
        .map_or_else(|| line.trim_start(), str::trim_start);
    let reference = unbulleted.strip_prefix(marker)?.trim();
    requirement_name(strip_optional_backticks(reference))
}

fn strip_optional_backticks(value: &str) -> &str {
    value
        .strip_prefix('`')
        .and_then(|unquoted| unquoted.strip_suffix('`'))
        .unwrap_or(value)
        .trim()
}

fn requirement_name(line: &str) -> Option<String> {
    let remainder = exact_heading_remainder(line, 3)?;
    let prefix = "Requirement:";
    if remainder.len() < prefix.len() || !remainder[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let name = remainder[prefix.len()..].trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn heading_text(line: &str, level: usize) -> Option<&str> {
    let remainder = exact_heading_remainder(line, level)?;
    let text = remainder.trim();
    (!text.is_empty()).then_some(text)
}

fn exact_heading_remainder(line: &str, level: usize) -> Option<&str> {
    let markers = "#".repeat(level);
    let remainder = line.strip_prefix(&markers)?;
    if remainder.starts_with('#') {
        return None;
    }
    if level == 3 {
        return Some(remainder.trim_start());
    }
    remainder
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
        .then(|| remainder.trim_start())
}

fn heading_level(line: &str) -> Option<usize> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (level > 0).then_some(level)
}

fn first_line_ending(content: &str) -> &'static str {
    let bytes = content.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => return "\r\n",
            b'\r' => return "\r",
            b'\n' => return "\n",
            _ => {}
        }
    }
    "\n"
}

fn scan_lines(content: &str) -> Vec<ScannedLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut number = 1;
    let mut fence: Option<(char, usize)> = None;
    while start < content.len() {
        let next_newline = content[start..].find('\n').map(|offset| start + offset);
        let full_end = next_newline.map_or(content.len(), |index| index + 1);
        let mut content_end = next_newline.unwrap_or(content.len());
        if content_end > start && content.as_bytes()[content_end - 1] == b'\r' {
            content_end -= 1;
        }
        let text = &content[start..content_end];
        let fenced = update_fence(text, &mut fence);
        lines.push(ScannedLine {
            start,
            full_end,
            number,
            text,
            fenced,
        });
        start = full_end;
        number += 1;
    }
    if content.is_empty() {
        lines.push(ScannedLine {
            start: 0,
            full_end: 0,
            number: 1,
            text: "",
            fenced: false,
        });
    }
    lines
}

fn update_fence(line: &str, fence: &mut Option<(char, usize)>) -> bool {
    if let Some((marker, minimum_length)) = *fence {
        if closing_fence(line, marker, minimum_length) {
            *fence = None;
        }
        return true;
    }
    if let Some((marker, length)) = opening_fence(line) {
        *fence = Some((marker, length));
        return true;
    }
    false
}

fn opening_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }
    let length = trimmed
        .chars()
        .take_while(|character| *character == marker)
        .count();
    (length >= 3).then_some((marker, length))
}

fn closing_fence(line: &str, marker: char, minimum_length: usize) -> bool {
    let trimmed = line.trim();
    let marker_length = trimmed
        .chars()
        .take_while(|character| *character == marker)
        .count();
    marker_length >= minimum_length && marker_length == trimmed.chars().count()
}

#[cfg(test)]
mod tests;
