//! Prose rules for specification artifacts, beyond structure and parsing:
//! one normative keyword, a length ceiling, and defined terms that must
//! exist in the ontology (or the glossary) and cite it. Validation and
//! doctor share these so acceptance cannot differ by command.

use super::terms::{TermSource, Terms};
use super::{DiagnosticSeverity, SpecViolation, relative_display};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
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
/// A single-asterisk italic run marks a defined term. The boundaries are
/// checked by [`italic_terms`], not consumed here, so two runs separated
/// by one space are both found.
static ITALIC_RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*([^*\n]+?)\*").expect("italic run regex is valid"));
/// The reference tag that may follow an italic run: `*term* [TAG]`.
/// Markdown labels are case-insensitive, so any case is a tag here.
static REFERENCE_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\[([A-Za-z0-9][A-Za-z0-9_-]*)\]").expect("reference tag regex is valid")
});
/// A reference definition, `[TAG]: <target>`, indented up to three spaces.
static REFERENCE_DEFINITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^ {0,3}\[([A-Za-z0-9][A-Za-z0-9_-]*)\]:\s*(\S+)")
        .expect("reference definition regex is valid")
});
static REQUIREMENT_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^###\s+Requirement:").expect("requirement heading regex is valid")
});
static STEP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*[-*]\s+\*\*(WHEN|THEN|AND|GIVEN)\*\*").expect("step regex is valid")
});
#[derive(Clone, Copy)]
pub(super) struct LintTarget<'target> {
    pub(super) repository: &'target Path,
    pub(super) path: &'target Path,
    /// `None` for a document outside the specification tree: a decision
    /// record or a rune, which has no capability and no name rule.
    pub(super) capability: Option<&'target str>,
    pub(super) change: Option<&'target str>,
}

/// Lint one canonical specification.
pub(super) fn lint_canonical(
    target: LintTarget<'_>,
    content: &str,
    terms: &Terms,
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
        terms,
        Some("spec-term-undefined"),
        diagnostics,
    );
    lint_prose(target, content, "spec", diagnostics);
    lint_names(target, "spec", diagnostics);
}

/// Lint one document outside the specification tree: a decision record or
/// a rune. Only the reference rules apply. An italic run that matches no
/// term is emphasis there, not a definition, so it is not reported.
pub(super) fn lint_document(
    target: LintTarget<'_>,
    content: &str,
    terms: &Terms,
    diagnostics: &mut Vec<SpecViolation>,
) {
    lint_terms(target, content, terms, None, diagnostics);
}

/// Lint one change delta.
pub(super) fn lint_delta(
    target: LintTarget<'_>,
    content: &str,
    terms: &Terms,
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
        terms,
        Some("delta-term-undefined"),
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
    let Some(full_name) = target.capability else {
        return;
    };
    let capability = full_name.rsplit('/').next().unwrap_or(full_name);
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

/// Every italic term exists in the term source, reported under
/// `undefined_code` when the caller passes one. With an ontology, the first
/// italic use of each term in the file also cites it: `*term* [TAG]` with
/// `[TAG]: <iri>` defined in the file, and a definition under an ontology
/// namespace names a real term.
fn lint_terms(
    target: LintTarget<'_>,
    content: &str,
    terms: &Terms,
    undefined_code: Option<&str>,
    diagnostics: &mut Vec<SpecViolation>,
) {
    let location = terms.location();
    let tags = terms.tags();
    let definitions = reference_definitions(content);
    if terms.source() == TermSource::Ontology {
        for (tag, (line, iri)) in &definitions {
            if terms.in_namespace(iri) && terms.by_iri(iri).is_none() {
                diagnostics.push(violation(
                    target,
                    "term-reference-unresolved",
                    DiagnosticSeverity::Error,
                    Some(*line),
                    format!("[{tag}] names {iri}, which is not a term in {location}"),
                ));
            }
        }
    }
    let mut cited = BTreeSet::new();
    for (index, line) in prose_lines(content) {
        for (term, tag) in italic_terms(&line) {
            // `*Rationale:*` and similar labels are emphasis, not definitions.
            if term.is_empty() || term.ends_with(':') {
                continue;
            }
            let Some(entry) = terms.resolve(term) else {
                if let Some(code) = undefined_code {
                    diagnostics.push(violation(
                        target,
                        code,
                        DiagnosticSeverity::Error,
                        Some(index),
                        undefined_message(terms, term),
                    ));
                }
                continue;
            };
            if terms.source() != TermSource::Ontology || !cited.insert(entry.iri.clone()) {
                continue;
            }
            let expected = tags.get(&entry.iri).cloned().unwrap_or_default();
            match tag {
                None => diagnostics.push(violation(
                    target,
                    "term-reference-missing",
                    DiagnosticSeverity::Error,
                    Some(index),
                    format!(
                        "first use of *{term}* has no reference tag; write `*{term}* [{expected}]` and define `[{expected}]: {}`",
                        entry.iri
                    ),
                )),
                Some(tag) => {
                    let defined = definitions
                        .get(&tag.to_uppercase())
                        .map(|(_, iri)| iri.as_str());
                    if defined != Some(entry.iri.as_str()) {
                        diagnostics.push(violation(
                            target,
                            "term-reference-unresolved",
                            DiagnosticSeverity::Error,
                            Some(index),
                            format!("[{tag}] must be defined as `[{tag}]: {}`", entry.iri),
                        ));
                    }
                }
            }
        }
    }
}

/// The single-asterisk italic runs of one prose line with the tag that
/// follows each, in order. A run counts when the character before its
/// opening `*` and after its closing `*` is neither `*` nor a word
/// character, and the opening `*` is not escaped.
fn italic_terms(line: &str) -> Vec<(&str, Option<&str>)> {
    let mut found = Vec::new();
    for capture in ITALIC_RUN.captures_iter(line) {
        let whole = capture.get(0).expect("whole match");
        let before = line[..whole.start()].chars().next_back();
        let after = line[whole.end()..].chars().next();
        let bounded =
            |c: Option<char>| c.is_none_or(|c| c != '*' && !c.is_alphanumeric() && c != '_');
        if !bounded(before) || before == Some('\\') {
            continue;
        }
        let rest = &line[whole.end()..];
        let tag = REFERENCE_TAG
            .captures(rest)
            .map(|tag| tag.get(1).expect("tag").as_str());
        let after_tag = tag.map_or(after, |_| {
            let end = REFERENCE_TAG.find(rest).map_or(0, |m| m.end());
            rest[end..].chars().next()
        });
        if !bounded(after) && tag.is_none() || !bounded(after_tag) {
            continue;
        }
        found.push((capture.get(1).expect("term").as_str().trim(), tag));
    }
    found
}

fn undefined_message(terms: &Terms, term: &str) -> String {
    let location = terms.location();
    match terms.source() {
        TermSource::Ontology => format!(
            "defined term '{term}' is not in {location}; add a term with `rdfs:label \"{term}\"` or drop the emphasis"
        ),
        TermSource::Glossary => format!(
            "defined term '{term}' has no entry in {location}; add `- **{term}**: <definition>`"
        ),
        TermSource::Absent => format!(
            "defined term '{term}' needs a glossary; create {location} with `- **{term}**: <definition>`"
        ),
    }
}

/// Reference definitions by upper-cased tag (Markdown labels are
/// case-insensitive), with the line they sit on. The first definition of a
/// tag wins, as in Markdown, and an angle-bracketed destination is unwrapped.
fn reference_definitions(content: &str) -> BTreeMap<String, (usize, String)> {
    let mut definitions = BTreeMap::new();
    for (index, line) in prose_lines(content) {
        if let Some(capture) = REFERENCE_DEFINITION.captures(&line) {
            let destination = capture[2]
                .strip_prefix('<')
                .and_then(|inner| inner.strip_suffix('>'))
                .unwrap_or(&capture[2]);
            definitions
                .entry(capture[1].to_uppercase())
                .or_insert((index, destination.to_string()));
        }
    }
    definitions
}

/// Non-fenced lines with their one-based numbers, inline code spans
/// blanked: a quoted literal such as `` `SHALL` `` is a mention, not a
/// normative statement or a defined term. Backtick and tilde fences both
/// count. A leading YAML front matter block, closed by `---` or `...`, is
/// data, not prose, and is skipped whole; an unclosed block is prose.
fn prose_lines(content: &str) -> impl Iterator<Item = (usize, String)> + '_ {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let mut fence: Option<(char, usize)> = None;
    let mut front_matter = front_matter_end(content);
    content
        .lines()
        .enumerate()
        .filter_map(move |(index, line)| {
            if front_matter.is_some_and(|end| index <= end) {
                if front_matter == Some(index) {
                    front_matter = None;
                }
                return None;
            }
            let trimmed = line.trim_start();
            let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let run = marker.map_or(0, |c| trimmed.chars().take_while(|d| *d == c).count());
            match fence {
                Some((open, width))
                    if marker == Some(open) && run >= width && trimmed[run..].trim().is_empty() =>
                {
                    fence = None;
                    None
                }
                Some(_) => None,
                None if run >= 3 => {
                    fence = marker.map(|c| (c, run));
                    None
                }
                None => Some((index + 1, without_inline_code(line))),
            }
        })
}

/// The zero-based line index of the `---` or `...` that closes a leading
/// YAML front matter block, when the content opens with `---` and a
/// closing line exists.
fn front_matter_end(content: &str) -> Option<usize> {
    let mut lines = content.lines().enumerate();
    let (_, first) = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    lines
        .find(|(_, line)| matches!(line.trim_end(), "---" | "..."))
        .map(|(index, _)| index)
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
        capability: target.capability.map(str::to_string),
        change: target.change.map(str::to_string),
    }
}

#[cfg(test)]
mod tests;
