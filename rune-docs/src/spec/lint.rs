//! Prose rules for specification artifacts, beyond structure and parsing:
//! one normative keyword, a length ceiling, and defined terms that must
//! carry a glossary entry. Validation and doctor share these so acceptance
//! cannot differ by command.

use super::{DiagnosticSeverity, SpecViolation, relative_display};
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

/// A canonical specification longer than this describes more than one
/// capability. A delta this long is a warning: it is transient, and its
/// merged result is what the ceiling protects.
pub const MAX_SPEC_LINES: usize = 150;
/// The glossary sits beside the capability directories.
pub const GLOSSARY_FILE: &str = "glossary.md";

static SHALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bSHALL\b").expect("normative keyword regex is valid"));
/// A single-asterisk italic run marks a defined term. Bold (`**WHEN**`)
/// never matches because the captured run cannot start or end on `*`.
static DEFINED_TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^*\w])\*([^*\n]+?)\*(?:[^*\w]|$)").expect("defined term regex is valid")
});
static GLOSSARY_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*-\s+\*\*([^*\n]+?)\*\*\s*:").expect("glossary entry regex is valid")
});

/// The terms `glossary.md` defines, compared case-insensitively.
#[derive(Debug, Default)]
pub(super) struct Glossary {
    terms: BTreeSet<String>,
    present: bool,
    /// Where the glossary lives, repository-relative, for the diagnostic.
    location: String,
}

impl Glossary {
    /// Load `<specs root>/glossary.md`. An absent file is an empty glossary
    /// that becomes an error only once a specification defines a term.
    pub(super) fn load(repository: &Path, specifications: &Path) -> Self {
        let path = specifications.join(GLOSSARY_FILE);
        let location = relative_display(repository, &path);
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self {
                location,
                ..Self::default()
            };
        };
        Self {
            terms: content
                .lines()
                .filter_map(|line| GLOSSARY_ENTRY.captures(line))
                .map(|capture| normalize_term(&capture[1]))
                .collect(),
            present: true,
            location,
        }
    }

    /// A term matches its entry exactly or as a plain plural (`trailers`
    /// finds `trailer`), so prose can inflect without a second entry.
    fn defines(&self, term: &str) -> bool {
        let normalized = normalize_term(term);
        if self.terms.contains(&normalized) {
            return true;
        }
        normalized
            .strip_suffix('s')
            .is_some_and(|singular| self.terms.contains(singular))
    }
}

fn normalize_term(term: &str) -> String {
    term.trim().to_lowercase()
}

#[derive(Clone, Copy)]
pub(super) struct LintTarget<'target> {
    pub(super) repository: &'target Path,
    pub(super) path: &'target Path,
    pub(super) capability: &'target str,
    pub(super) change: Option<&'target str>,
}

/// Lint one canonical specification.
pub(super) fn lint_canonical(
    target: LintTarget<'_>,
    content: &str,
    glossary: &Glossary,
    diagnostics: &mut Vec<SpecViolation>,
) {
    lint_keywords(target, content, "spec-shall-keyword", diagnostics);
    lint_length(
        target,
        content,
        "spec-too-long",
        DiagnosticSeverity::Error,
        "canonical specification",
        diagnostics,
    );
    lint_terms(
        target,
        content,
        glossary,
        "spec-term-undefined",
        diagnostics,
    );
}

/// Lint one change delta.
pub(super) fn lint_delta(
    target: LintTarget<'_>,
    content: &str,
    glossary: &Glossary,
    diagnostics: &mut Vec<SpecViolation>,
) {
    lint_keywords(target, content, "delta-shall-keyword", diagnostics);
    lint_length(
        target,
        content,
        "delta-too-long",
        DiagnosticSeverity::Warning,
        "delta specification",
        diagnostics,
    );
    lint_terms(
        target,
        content,
        glossary,
        "delta-term-undefined",
        diagnostics,
    );
}

fn lint_keywords(
    target: LintTarget<'_>,
    content: &str,
    code: &str,
    diagnostics: &mut Vec<SpecViolation>,
) {
    for (index, line) in prose_lines(content) {
        if SHALL.is_match(&line) {
            diagnostics.push(violation(
                target,
                code,
                DiagnosticSeverity::Error,
                Some(index),
                "use MUST, not SHALL, for a normative statement".to_string(),
            ));
        }
    }
}

fn lint_length(
    target: LintTarget<'_>,
    content: &str,
    code: &str,
    severity: DiagnosticSeverity,
    label: &str,
    diagnostics: &mut Vec<SpecViolation>,
) {
    let lines = content.lines().count();
    if lines > MAX_SPEC_LINES {
        diagnostics.push(violation(
            target,
            code,
            severity,
            None,
            format!(
                "{label} has {lines} lines; the limit is {MAX_SPEC_LINES}, split the capability or move detail into design"
            ),
        ));
    }
}

fn lint_terms(
    target: LintTarget<'_>,
    content: &str,
    glossary: &Glossary,
    code: &str,
    diagnostics: &mut Vec<SpecViolation>,
) {
    let glossary_path = if glossary.location.is_empty() {
        Path::new("docs/specs")
            .join(GLOSSARY_FILE)
            .display()
            .to_string()
    } else {
        glossary.location.clone()
    };
    for (index, line) in prose_lines(content) {
        for capture in DEFINED_TERM.captures_iter(&line) {
            let term = capture[1].trim();
            // `*Rationale:*` and similar labels are emphasis, not definitions.
            if term.is_empty() || term.ends_with(':') {
                continue;
            }
            if glossary.defines(term) {
                continue;
            }
            let message = if glossary.present {
                format!(
                    "defined term '{term}' has no entry in {glossary_path}; add `- **{term}**: <definition>`"
                )
            } else {
                format!(
                    "defined term '{term}' needs a glossary; create {glossary_path} with `- **{term}**: <definition>`"
                )
            };
            diagnostics.push(violation(
                target,
                code,
                DiagnosticSeverity::Error,
                Some(index),
                message,
            ));
        }
    }
}

/// Non-fenced lines with their one-based numbers, inline code spans
/// blanked: a quoted literal such as `` `SHALL` `` is a mention, not a
/// normative statement or a defined term.
fn prose_lines(content: &str) -> impl Iterator<Item = (usize, String)> + '_ {
    let mut fence: Option<usize> = None;
    content
        .lines()
        .enumerate()
        .filter_map(move |(index, line)| {
            let trimmed = line.trim_start();
            let run = trimmed
                .chars()
                .take_while(|character| *character == '`')
                .count();
            match fence {
                Some(open) if run >= open && trimmed[run..].trim().is_empty() => {
                    fence = None;
                    None
                }
                Some(_) => None,
                None if run >= 3 => {
                    fence = Some(run);
                    None
                }
                None => Some((index + 1, without_inline_code(line))),
            }
        })
}

/// Replace every `` `...` `` span with spaces of the same length so column
/// positions survive and the span's text never matches a rule.
fn without_inline_code(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut inside = false;
    for character in line.chars() {
        if character == '`' {
            inside = !inside;
            result.push(' ');
        } else if inside {
            result.push(' ');
        } else {
            result.push(character);
        }
    }
    result
}

fn violation(
    target: LintTarget<'_>,
    code: &str,
    severity: DiagnosticSeverity,
    line: Option<usize>,
    message: String,
) -> SpecViolation {
    SpecViolation {
        code: code.to_string(),
        severity,
        path: relative_display(target.repository, target.path),
        line,
        column: None,
        message,
        operation: None,
        capability: Some(target.capability.to_string()),
        change: target.change.map(str::to_string),
    }
}

#[cfg(test)]
mod tests;
