//! Prose rules for specification artifacts, beyond structure and parsing:
//! one normative keyword, a length ceiling, and term links that must reach
//! a term the ontology defines. Validation and doctor share these so
//! acceptance cannot differ by command.

use super::prose::prose_lines;
use super::term_links::{
    Definition, Resolution, TermLink, Via, canonical, legacy_links, reference_definitions,
    reference_links, relative_link, resolve,
};
use super::terms::{Term, TermSource, Terms};
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
    lint_terms(target, content, terms, "spec", diagnostics);
    lint_prose(target, content, "spec", diagnostics);
    lint_names(target, "spec", diagnostics);
}

/// Lint one document outside the specification tree: a decision record or
/// a rune. Only the term link rules apply, under the `spec-` codes.
pub(super) fn lint_document(
    target: LintTarget<'_>,
    content: &str,
    terms: &Terms,
    diagnostics: &mut Vec<SpecViolation>,
) {
    lint_terms(target, content, terms, "spec", diagnostics);
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
    lint_terms(target, content, terms, "delta", diagnostics);
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

/// Every term link in the file reaches a term the ontology defines. The
/// rules, under `<prefix>-term-*`: `undefined` (error: the link names no
/// term), `draft` (warning: the term is a draft), `first-use` (error: the
/// first use in the file is not emphasized, or a later use is), `page`
/// (error: the page is missing, differs from the render, or the tag has
/// no definition), and `legacy` (warning: the old `*term* [TAG]` form, or
/// a definition that names an IRI instead of the page). Plain italics are
/// emphasis and are not checked. A repository without an ontology has no
/// terms, so nothing is checked.
fn lint_terms(
    target: LintTarget<'_>,
    content: &str,
    terms: &Terms,
    code_prefix: &str,
    diagnostics: &mut Vec<SpecViolation>,
) {
    if terms.source() != TermSource::Ontology {
        return;
    }
    let view: Vec<(usize, String)> = prose_lines(content).collect();
    let definitions =
        reference_definitions(view.iter().map(|(number, line)| (*number, line.as_str())));
    let mut lint = TermLint {
        target,
        terms,
        code_prefix,
        file_dir: target.path.parent().unwrap_or(Path::new("")),
        seen: BTreeSet::new(),
        reported: BTreeSet::new(),
        tags: BTreeMap::new(),
        first_link: BTreeMap::new(),
        pages: BTreeMap::new(),
        diagnostics,
    };
    for definition in definitions.values() {
        if terms.in_namespace(&definition.target) && terms.by_iri(&definition.target).is_none() {
            lint.report(
                "undefined",
                DiagnosticSeverity::Error,
                definition.line,
                format!(
                    "[{}] names {}, which is not a term in {}",
                    definition.tag,
                    definition.target,
                    terms.location()
                ),
            );
        }
    }
    for (number, line) in &view {
        for link in legacy_links(*number, line) {
            lint.legacy(&link, &definitions);
        }
        for link in reference_links(*number, line) {
            lint.link(&link, &definitions);
        }
    }
    lint.plain_occurrences(&view);
}

/// The state of the term rules over one file.
struct TermLint<'a> {
    target: LintTarget<'a>,
    terms: &'a Terms,
    code_prefix: &'a str,
    file_dir: &'a Path,
    /// Terms linked so far, by IRI: the first link is the first use.
    seen: BTreeSet<String>,
    /// Findings reported once per term and rule.
    reported: BTreeSet<(String, &'static str)>,
    /// The tag this file writes for each linked term, first link wins.
    tags: BTreeMap<String, String>,
    /// Where the first link of each term sits, as (line, column), so a plain
    /// use before it gets the emphasized form as its hint.
    first_link: BTreeMap<String, (usize, usize)>,
    /// Whether each page read so far matches its render.
    pages: BTreeMap<std::path::PathBuf, bool>,
    diagnostics: &'a mut Vec<SpecViolation>,
}

impl TermLint<'_> {
    fn report(&mut self, rule: &str, severity: DiagnosticSeverity, line: usize, message: String) {
        self.diagnostics.push(violation(
            self.target,
            &format!("{}-term-{rule}", self.code_prefix),
            severity,
            Some(line),
            message,
        ));
    }

    fn once(&mut self, key: &str, rule: &'static str) -> bool {
        self.reported.insert((key.to_string(), rule))
    }

    /// The definition a file at this path writes for a term.
    fn page_link(&self, term: &Term) -> String {
        relative_link(self.file_dir, &self.terms.page_path(term))
    }

    /// The legacy `*text* [TAG]` form warns when its tag reaches a term or
    /// its definition sits under a term namespace.
    fn legacy(&mut self, link: &TermLink, definitions: &BTreeMap<String, Definition>) {
        let resolution = resolve(self.terms, link, definitions, self.file_dir);
        let under_namespace = definitions
            .get(&link.tag.to_uppercase())
            .is_some_and(|definition| self.terms.in_namespace(&definition.target));
        let relative = match resolution {
            Resolution::Term { term, .. } => self.page_link(term),
            _ if under_namespace => format!("<path to the page of {}>", link.tag),
            _ => return,
        };
        self.report(
            "legacy",
            DiagnosticSeverity::Warning,
            link.line,
            format!(
                "`*{text}* [{tag}]` is the legacy form; write `*[{text}][{tag}]*` at first use, `[{text}][{tag}]` later, and define `[{tag}]: {relative}`",
                text = link.text,
                tag = link.tag
            ),
        );
    }

    fn link(&mut self, link: &TermLink, definitions: &BTreeMap<String, Definition>) {
        match resolve(self.terms, link, definitions, self.file_dir) {
            Resolution::Ordinary => {}
            Resolution::Undefined { reason, hint, .. } => self.report(
                "undefined",
                DiagnosticSeverity::Error,
                link.line,
                format!(
                    "term link [{}][{}] names no term: {reason}; {hint}",
                    link.text, link.tag
                ),
            ),
            Resolution::Term { term, via, page } => {
                let first = self.seen.insert(term.iri.clone());
                self.tags
                    .entry(term.iri.clone())
                    .or_insert_with(|| link.tag.clone());
                self.first_link
                    .entry(term.iri.clone())
                    .or_insert((link.line, link.start));
                self.first_use(link, first);
                if term.draft && self.once(&term.iri, "draft") {
                    self.report(
                        "draft",
                        DiagnosticSeverity::Warning,
                        link.line,
                        format!(
                            "term '{}' is a draft; define it in {} before archive",
                            term.label,
                            self.terms.location()
                        ),
                    );
                }
                self.reached(link, term, via, page);
            }
        }
    }

    /// Every occurrence of a term in a file is a link. Once a file links a
    /// term, a plain later occurrence of its label is a finding.
    fn plain_occurrences(&mut self, view: &[(usize, String)]) {
        let linked: Vec<(String, String, String)> = self
            .seen
            .iter()
            .filter_map(|iri| self.terms.by_iri(iri))
            .map(|term| {
                let tag = self
                    .tags
                    .get(&term.iri)
                    .cloned()
                    .unwrap_or_else(|| term.name.to_uppercase());
                (term.iri.clone(), term.label.clone(), tag)
            })
            .collect();
        for (number, line) in view {
            // The same selection as the fixer: where two terms overlap, the
            // longer wins, so one span gets one diagnostic.
            let mut hits: Vec<(usize, usize, &str, &str, &str)> = linked
                .iter()
                .flat_map(|(iri, label, tag)| {
                    super::term_links::plain_occurrences(line, label)
                        .into_iter()
                        .map(move |hit| {
                            (
                                hit.start,
                                hit.end,
                                iri.as_str(),
                                label.as_str(),
                                tag.as_str(),
                            )
                        })
                })
                .collect();
            hits.sort_by_key(|(start, end, ..)| (*start, std::cmp::Reverse(*end)));
            let mut last_end = 0;
            for (start, end, iri, label, tag) in hits {
                if start < last_end {
                    continue;
                }
                last_end = end;
                let text = &line[start..end];
                let before_first =
                    self.first_link
                        .get(iri)
                        .is_some_and(|(first_line, first_start)| {
                            (*number, start) < (*first_line, *first_start)
                        });
                let form = if before_first {
                    format!("*[{text}][{tag}]*, which becomes the first use")
                } else {
                    format!("[{text}][{tag}]")
                };
                if self.once(&format!("{iri}:{number}:{start}"), "unlinked") {
                    self.report(
                        "unlinked",
                        DiagnosticSeverity::Error,
                        *number,
                        format!(
                            "'{text}' is the term '{label}' and this file links it elsewhere; every occurrence links, write `{form}`"
                        ),
                    );
                }
            }
        }
    }

    /// The first use of a term in a file is `*[text][TAG]*`; every later
    /// use is `[text][TAG]`.
    fn first_use(&mut self, link: &TermLink, first: bool) {
        if link.emphasized == first {
            return;
        }
        let message = if first {
            format!(
                "first use of [{text}][{tag}] in this file is not emphasized; write `*[{text}][{tag}]*`",
                text = link.text,
                tag = link.tag
            )
        } else {
            format!(
                "later use of [{text}][{tag}] is emphasized; only the first use in a file is, write `[{text}][{tag}]`",
                text = link.text,
                tag = link.tag
            )
        };
        self.report("first-use", DiagnosticSeverity::Error, link.line, message);
    }

    /// The page behind a resolved link exists and matches the render, the
    /// tag is defined, and the definition points at the page.
    fn reached(
        &mut self,
        link: &TermLink,
        term: &Term,
        via: Via,
        page: Option<std::path::PathBuf>,
    ) {
        let expected = self.page_link(term);
        match via {
            Via::Page => {
                let page = page.expect("a page resolution carries its path");
                let terms = self.terms;
                let current = *self.pages.entry(page.clone()).or_insert_with(|| {
                    std::fs::read_to_string(&page)
                        .is_ok_and(|bytes| bytes == super::glossary::render_page(terms, term))
                });
                if !current && self.once(&term.iri, "page") {
                    self.report(
                        "page",
                        DiagnosticSeverity::Error,
                        link.line,
                        format!(
                            "glossary page {} differs from the ontology render; run `rune spec glossary --write`",
                            relative_display(&canonical(self.target.repository), &page)
                        ),
                    );
                }
            }
            Via::PageMissing => {
                if self.once(&term.iri, "page") {
                    let page = page.expect("a missing page resolution carries its path");
                    self.report(
                        "page",
                        DiagnosticSeverity::Error,
                        link.line,
                        format!(
                            "glossary page {} is missing; run `rune spec glossary --write`",
                            relative_display(&canonical(self.target.repository), &page)
                        ),
                    );
                }
            }
            Via::NoDefinition => {
                if self.once(&link.tag.to_uppercase(), "page") {
                    self.report(
                        "page",
                        DiagnosticSeverity::Error,
                        link.line,
                        format!(
                            "[{}] has no definition; add `[{}]: {expected}` at the end of the file",
                            link.tag, link.tag
                        ),
                    );
                }
            }
            Via::Iri => {
                if self.once(&term.iri, "legacy") {
                    self.report(
                        "legacy",
                        DiagnosticSeverity::Warning,
                        link.line,
                        format!(
                            "[{}] is defined as an IRI, the legacy form; point it at the page: `[{}]: {expected}`",
                            link.tag, link.tag
                        ),
                    );
                }
            }
        }
    }
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
