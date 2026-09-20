//! Prose rules for specification artifacts, beyond structure and parsing:
//! one normative keyword, a length ceiling, and defined terms that must
//! carry a glossary entry. Validation and doctor share these so acceptance
//! cannot differ by command.

use super::{DiagnosticSeverity, SpecViolation, relative_display};
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{LazyLock, OnceLock};

/// A canonical specification longer than this describes more than one
/// capability. A delta this long is a warning: it is transient, and its
/// merged result is what the ceiling protects.
pub const MAX_SPEC_LINES: usize = 150;
/// A requirement statement longer than this is several requirements in
/// one paragraph. Split it: one MUST cluster per heading, each with its
/// own scenarios. Words are whitespace-separated runs outside code fences.
pub const MAX_REQUIREMENT_WORDS: usize = 100;
/// One `WHEN`, `THEN`, or `AND` step states one fact. Longer steps hide
/// several facts in one bullet.
pub const MAX_STEP_WORDS: usize = 30;
/// A change id and a capability name carry at least this many
/// hyphen-separated words, so a name reads as a change and not as a
/// category. The repository config key `spec.min_name_words` overrides it,
/// and `0` turns the rule off.
pub const DEFAULT_MIN_NAME_WORDS: usize = 3;
/// The glossary sits beside the capability directories.
pub const GLOSSARY_FILE: &str = "glossary.md";

/// How the CLI hands `spec.min_name_words` from the repository's merged
/// config to this crate, which knows nothing about config files.
pub type NameRuleLookup = fn(&Path) -> Result<Option<usize>, String>;
static NAME_RULE_LOOKUP: OnceLock<NameRuleLookup> = OnceLock::new();

pub fn set_name_rule_lookup(lookup: NameRuleLookup) -> bool {
    NAME_RULE_LOOKUP.set(lookup).is_ok()
}

/// The minimum word count for names in this repository.
pub fn min_name_words(repository: &Path) -> usize {
    NAME_RULE_LOOKUP
        .get()
        .and_then(|lookup| lookup(repository).ok().flatten())
        .unwrap_or(DEFAULT_MIN_NAME_WORDS)
}

/// Words in a kebab-case name: non-empty runs between hyphens.
pub fn name_words(name: &str) -> usize {
    name.split('-').filter(|part| !part.is_empty()).count()
}

static SHALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bSHALL\b").expect("normative keyword regex is valid"));
/// A single-asterisk italic run marks a defined term. Bold (`**WHEN**`)
/// never matches because the captured run cannot start or end on `*`.
static DEFINED_TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^*\w])\*([^*\n]+?)\*(?:[^*\w]|$)").expect("defined term regex is valid")
});
static REQUIREMENT_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^###\s+Requirement:").expect("requirement heading regex is valid")
});
static STEP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*[-*]\s+\*\*(WHEN|THEN|AND|GIVEN)\*\*").expect("step regex is valid")
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
    lint_prose(target, content, "spec", diagnostics);
    lint_names(target, "spec", diagnostics);
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
    lint_prose(target, content, "delta", diagnostics);
    lint_names(target, "delta", diagnostics);
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

/// The capability name and the change id have at least `spec.min_name_words`
/// hyphen-separated words. A nested capability is judged by its last segment.
fn lint_names(target: LintTarget<'_>, code_prefix: &str, diagnostics: &mut Vec<SpecViolation>) {
    let minimum = min_name_words(target.repository);
    if minimum == 0 {
        return;
    }
    let capability = target
        .capability
        .rsplit('/')
        .next()
        .unwrap_or(target.capability);
    if name_words(capability) < minimum {
        diagnostics.push(violation(
            target,
            &format!("{code_prefix}-capability-name-short"),
            DiagnosticSeverity::Error,
            None,
            format!(
                "capability '{capability}' has {} words; a name needs at least {minimum} hyphen-separated words",
                name_words(capability)
            ),
        ));
    }
    if let Some(change) = target.change
        && name_words(change) < minimum
    {
        diagnostics.push(violation(
            target,
            "change-name-short",
            DiagnosticSeverity::Error,
            None,
            format!(
                "change '{change}' has {} words; a change id needs at least {minimum} hyphen-separated words",
                name_words(change)
            ),
        ));
    }
}

/// A requirement statement is the prose between `### Requirement:` and
/// the next heading. A step is one `**WHEN**`, `**THEN**`, `**AND**`, or
/// `**GIVEN**` bullet. Both have a word cap, and both are errors: the
/// cap is what keeps a specification readable in one pass.
fn lint_prose(
    target: LintTarget<'_>,
    content: &str,
    code_prefix: &str,
    diagnostics: &mut Vec<SpecViolation>,
) {
    let mut requirement: Option<(usize, usize)> = None;
    let close = |requirement: &mut Option<(usize, usize)>, diagnostics: &mut Vec<SpecViolation>| {
        if let Some((line, words)) = requirement.take()
            && words > MAX_REQUIREMENT_WORDS
        {
            diagnostics.push(violation(
                target,
                &format!("{code_prefix}-requirement-too-long"),
                DiagnosticSeverity::Error,
                Some(line),
                format!(
                    "requirement statement has {words} words; the limit is {MAX_REQUIREMENT_WORDS}, split it into one requirement per MUST cluster"
                ),
            ));
        }
    };
    for (index, line) in prose_lines(content) {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            close(&mut requirement, diagnostics);
            if REQUIREMENT_HEADING.is_match(trimmed) {
                requirement = Some((index, 0));
            }
            continue;
        }
        if STEP.is_match(&line) {
            let words = line.split_whitespace().count();
            if words > MAX_STEP_WORDS {
                diagnostics.push(violation(
                    target,
                    &format!("{code_prefix}-step-too-long"),
                    DiagnosticSeverity::Error,
                    Some(index),
                    format!(
                        "scenario step has {words} words; the limit is {MAX_STEP_WORDS}, state one fact per step"
                    ),
                ));
            }
            continue;
        }
        if let Some((_, words)) = requirement.as_mut() {
            *words += line.split_whitespace().count();
        }
    }
    close(&mut requirement, diagnostics);
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
