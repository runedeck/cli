//! `rune spec fix`: the fixer. It edits what the validator only reports.
//! `--terms` adds a draft entry to the ontology for every term link that
//! names no term and writes the glossary pages. `--links` rewrites the
//! legacy `*term* [TAG]` form to the link form, emphasizes the first use
//! of each term in a file and no later use, points every definition at the
//! term's page, and adds a definition a known tag lacks. Both passes are
//! idempotent: a second run changes nothing.

use super::commands::print_json;
use super::glossary::write_glossary_output;
use super::prose::{prose_lines, without_bom};
use super::term_links::{
    Definition, Resolution, Target, TermLink, Via, classify, is_definition, legacy_links,
    markdown_files, reference_definitions, reference_links, relative_link, resolve,
    resolve_document,
};
use super::terms::{Term, TermSource, Terms, kebab, ontology_path, upper_camel};
use super::{Error, ErrorKind, read, relative_display, resolve_spec_root};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What `rune spec fix` did.
#[derive(Debug, Serialize)]
pub struct FixOutput {
    pub ontology: String,
    pub glossary: String,
    pub drafts: Vec<DraftTerm>,
    pub skipped: Vec<SkippedTerm>,
    /// Files `--links` rewrote, repository-relative.
    pub rewritten: Vec<String>,
    /// Whether the glossary pages were regenerated.
    pub pages_written: bool,
}

/// One draft entry `--terms` added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftTerm {
    pub label: String,
    pub name: String,
    pub page: String,
    /// `path:line` of the link that introduced the term.
    pub origin: String,
}

/// One unresolved link `--terms` left alone, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedTerm {
    pub text: String,
    pub tag: String,
    pub origin: String,
    pub reason: String,
}

/// Run the fixer without printing.
pub fn fix_output(source: &str, terms: bool, links: bool) -> Result<FixOutput, Error> {
    let spec_root = resolve_spec_root(Path::new(source))?;
    let repository = spec_root.repository();
    let mut vocabulary = Terms::load(repository, spec_root.specifications())?;
    if vocabulary.source() != TermSource::Ontology {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no ontology at {}; the fixer adds terms to the ontology",
                relative_display(repository, &ontology_path(repository)?)
            ),
        ));
    }
    let ontology = ontology_path(repository)?;
    let mut output = FixOutput {
        ontology: relative_display(repository, &ontology),
        glossary: relative_display(repository, vocabulary.glossary_dir()),
        drafts: Vec::new(),
        skipped: Vec::new(),
        rewritten: Vec::new(),
        pages_written: false,
    };
    let archive = spec_root.changes().join("archive");
    let files = markdown_files(repository, &[vocabulary.glossary_dir(), &archive])?;
    if links {
        for path in &files {
            let content = read(path)?;
            if let Some(rewritten) = migrate_links(&content, path, &vocabulary) {
                write(path, &rewritten)?;
                output.rewritten.push(relative_display(repository, path));
            }
        }
    }
    if terms {
        draft_terms(
            source,
            &spec_root,
            &ontology,
            &files,
            &mut vocabulary,
            &mut output,
        )?;
    }
    Ok(output)
}

/// The `--terms` pass: a draft entry per term link that names no term,
/// then the glossary pages.
fn draft_terms(
    source: &str,
    spec_root: &super::SpecRoot,
    ontology: &Path,
    files: &[std::path::PathBuf],
    vocabulary: &mut Terms,
    output: &mut FixOutput,
) -> Result<(), Error> {
    let repository = spec_root.repository();
    let mut blocks = String::new();
    let mut drafted: BTreeSet<String> = BTreeSet::new();
    for path in files {
        let content = read(path)?;
        for unresolved in unresolved_links(&content, path, vocabulary) {
            let link = unresolved.link;
            let origin = format!("{}:{}", relative_display(repository, path), link.line);
            let label = draft_label(&link.text, unresolved.page.as_deref());
            let name = upper_camel(&label);
            let reason = if !unresolved.draftable {
                Some(unresolved.reason)
            } else if !name.chars().next().is_some_and(char::is_alphabetic) {
                Some("the link text gives no local name".to_string())
            } else if vocabulary.has_name(&name) {
                Some(format!(
                    "a term named {name} exists; point the definition at its page"
                ))
            } else {
                None
            };
            if let Some(reason) = reason {
                output.skipped.push(SkippedTerm {
                    text: link.text,
                    tag: link.tag,
                    origin,
                    reason,
                });
                continue;
            }
            if !drafted.insert(name.to_lowercase()) {
                continue;
            }
            let Some(block) = vocabulary.draft_block(&name, &label, &origin) else {
                continue;
            };
            blocks.push_str(&block);
            output.drafts.push(DraftTerm {
                label,
                name,
                page: String::new(),
                origin,
            });
        }
    }
    if blocks.is_empty() {
        return Ok(());
    }
    let mut text = read(ontology)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&blocks);
    write(ontology, &text)?;
    *vocabulary = Terms::load(repository, spec_root.specifications())?;
    let (_, namespace) = vocabulary.primary_prefix().unwrap_or_default();
    for draft in &mut output.drafts {
        draft.page = vocabulary
            .by_iri(&format!("{namespace}{}", draft.name))
            .map(|term: &Term| vocabulary.page_name(term).to_string())
            .unwrap_or_default();
    }
    write_glossary_output(source, None)?;
    output.pages_written = true;
    Ok(())
}

/// Run the fixer and print what it did.
pub fn fix(source: &str, terms: bool, links: bool, json: bool) -> Result<i32, Error> {
    let output = fix_output(source, terms, links)?;
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_fix(&output));
    }
    Ok(0)
}

pub fn render_fix(output: &FixOutput) -> String {
    use std::fmt::Write as _;
    let mut text = String::new();
    for path in &output.rewritten {
        let _ = writeln!(text, "rewrote {path}");
    }
    for draft in &output.drafts {
        let _ = writeln!(
            text,
            "added draft '{}' as {} ({}/{}) from {}",
            draft.label, draft.name, output.glossary, draft.page, draft.origin
        );
    }
    for skipped in &output.skipped {
        let _ = writeln!(
            text,
            "skipped '{}' [{}] at {}: {}",
            skipped.text, skipped.tag, skipped.origin, skipped.reason
        );
    }
    if output.pages_written {
        let _ = writeln!(text, "wrote {} and {}", output.ontology, output.glossary);
    }
    if text.is_empty() {
        text.push_str("nothing to fix\n");
    }
    text
}

fn write(path: &Path, content: &str) -> Result<(), Error> {
    crate::support::write_atomic(path, content)
        .map_err(|message| Error::new(ErrorKind::Io, message))
}

/// The prose view of a document as owned lines, keyed by line number.
fn prose(content: &str) -> BTreeMap<usize, String> {
    prose_lines(content).collect()
}

fn definitions_of(prose: &BTreeMap<usize, String>) -> BTreeMap<String, Definition> {
    reference_definitions(prose.iter().map(|(number, line)| (*number, line.as_str())))
}

/// Rewrite one document to the link model. `None` when nothing changes.
pub(super) fn migrate_links(content: &str, path: &Path, terms: &Terms) -> Option<String> {
    let bom = content.len() - without_bom(content).len();
    let body = without_bom(content);
    let trailing_newline = body.ends_with('\n');
    let file_dir = path.parent().unwrap_or(Path::new(""));
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();

    // Definitions: an IRI that names a term becomes the term's page.
    let view = prose(&join(&lines));
    for definition in definitions_of(&view).values() {
        if let Target::Iri(iri) = classify(Some(definition), file_dir, terms)
            && let Some(term) = terms.by_iri(&iri)
        {
            let relative = relative_link(file_dir, &terms.page_path(term));
            let line = &mut lines[definition.line - 1];
            line.replace_range(definition.target_start..definition.target_end, &relative);
        }
    }

    // The legacy form becomes a link when its tag reaches a term.
    let view = prose(&join(&lines));
    let definitions = definitions_of(&view);
    for (number, line) in &view {
        for link in legacy_links(*number, line).into_iter().rev() {
            if matches!(
                resolve(terms, &link, &definitions, file_dir),
                Resolution::Term { .. }
            ) {
                lines[number - 1].replace_range(link.start..link.end, &link.render(false));
            }
        }
    }

    // The first use of each term is emphasized and no later use is.
    let view = prose(&join(&lines));
    let definitions = definitions_of(&view);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut missing: Vec<(String, String)> = Vec::new();
    let mut known: BTreeSet<String> = BTreeSet::new();
    for (number, line) in &view {
        let mut edits = Vec::new();
        for link in reference_links(*number, line) {
            let Resolution::Term { term, via, .. } = resolve(terms, &link, &definitions, file_dir)
            else {
                continue;
            };
            let first = seen.insert(term.iri.clone());
            if link.emphasized != first {
                edits.push((link.start, link.end, link.render(first)));
            }
            if via == Via::NoDefinition && known.insert(link.tag.to_uppercase()) {
                missing.push((
                    link.tag.clone(),
                    relative_link(file_dir, &terms.page_path(term)),
                ));
            }
        }
        for (start, end, replacement) in edits.into_iter().rev() {
            lines[number - 1].replace_range(start..end, &replacement);
        }
    }

    // A known tag without a definition gets one at the end of the file.
    let appended = !missing.is_empty();
    if appended {
        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
        if !lines.last().is_some_and(|line| is_definition(line)) {
            lines.push(String::new());
        }
        for (tag, relative) in missing {
            lines.push(format!("[{tag}]: {relative}"));
        }
    }

    let mut result = String::with_capacity(content.len());
    result.push_str(&content[..bom]);
    result.push_str(&join(&lines));
    if trailing_newline || appended {
        result.push('\n');
    }
    (result != content).then_some(result)
}

fn join(lines: &[String]) -> String {
    lines.join("\n")
}

/// One link of a document that names no term.
pub(super) struct Unresolved {
    pub link: TermLink,
    /// The glossary page its definition names, when it names one.
    pub page: Option<std::path::PathBuf>,
    /// Whether the fixer may draft the term.
    pub draftable: bool,
    pub reason: String,
}

/// The links of one document that name no term.
pub(super) fn unresolved_links(content: &str, path: &Path, terms: &Terms) -> Vec<Unresolved> {
    resolve_document(content, path, terms)
        .into_iter()
        .filter_map(|(link, resolution)| match resolution {
            Resolution::Undefined {
                page,
                draftable,
                reason,
                ..
            } => Some(Unresolved {
                link,
                page,
                draftable,
                reason,
            }),
            _ => None,
        })
        .collect()
}

/// The label of a draft: the link text as written, unless the definition
/// names a page whose stem is not the text's (a plural link text such as
/// `[gizmos][GIZMO]` over `gizmo.md`), in which case the page stem names
/// the term so the link resolves once the page exists.
fn draft_label(text: &str, page: Option<&Path>) -> String {
    let text = text.trim();
    let stem = page
        .and_then(Path::file_stem)
        .map(|stem| stem.to_string_lossy().into_owned());
    match stem {
        Some(stem) if !stem.is_empty() && kebab(text) != stem => stem.replace('-', " "),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::glossary::DRAFT_COMMENT;
    use super::super::terms::tests::TURTLE;
    use super::*;

    fn repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("ontology")).unwrap();
        std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
        std::fs::create_dir_all(root.path().join("docs/decisions")).unwrap();
        std::fs::write(root.path().join("ontology/rune.ttl"), TURTLE).unwrap();
        write_glossary_output(root.path().to_str().unwrap(), None).unwrap();
        root
    }

    fn source(root: &tempfile::TempDir) -> &str {
        root.path().to_str().unwrap()
    }

    #[test]
    fn links_migrates_the_legacy_form_emphasis_and_definitions_once() {
        let root = repository();
        let record = root.path().join("docs/decisions/CLI-0001 Example.md");
        std::fs::write(
            &record,
            "\u{FEFF}# Example\n\nEvery *change* [CHANGE] has a *lifecycle stage* [STAGE]. A *[change][CHANGE]* again.\nThe [tight][TIGHT] one, then *[tight][TIGHT]*. Plain *emphasis* stays.\n\n[CHANGE]: https://runedeck.ai/ns#Change\n[STAGE]: <https://runedeck.ai/ns#Stage>\n",
        )
        .unwrap();

        let output = fix_output(source(&root), false, true).unwrap();
        assert_eq!(output.rewritten, ["docs/decisions/CLI-0001 Example.md"]);
        assert!(output.drafts.is_empty());
        let migrated = std::fs::read_to_string(&record).unwrap();
        assert_eq!(
            migrated,
            "\u{FEFF}# Example\n\nEvery *[change][CHANGE]* has a *[lifecycle stage][STAGE]*. A [change][CHANGE] again.\nThe *[tight][TIGHT]* one, then [tight][TIGHT]. Plain *emphasis* stays.\n\n[CHANGE]: ../specs/glossary/change.md\n[STAGE]: ../specs/glossary/lifecycle-stage.md\n[TIGHT]: ../specs/glossary/tight.md\n"
        );

        let again = fix_output(source(&root), false, true).unwrap();
        assert!(again.rewritten.is_empty(), "{again:?}");
        assert_eq!(std::fs::read_to_string(&record).unwrap(), migrated);
    }

    #[test]
    fn links_leaves_ordinary_links_unknown_tags_and_fenced_code_alone() {
        let root = repository();
        let record = root.path().join("docs/decisions/X.md");
        let content = "# X\n\nSee [the readme][README] and *ghost* [GHOST].\n\n```md\nEvery *change* [CHANGE] here.\n```\n\n[README]: ../../README.md\n[GHOST]: https://runedeck.ai/ns#Ghost\n";
        std::fs::write(&record, content).unwrap();
        let output = fix_output(source(&root), false, true).unwrap();
        assert!(output.rewritten.is_empty(), "{output:?}");
        assert_eq!(std::fs::read_to_string(&record).unwrap(), content);
    }

    #[test]
    fn terms_adds_a_draft_writes_its_page_and_is_idempotent() {
        let root = repository();
        let record = root.path().join("docs/decisions/X.md");
        std::fs::write(
            &record,
            "# X\n\nA *[gadget][GADGET]* and a *[Change][CHANGE]* and [gadget][GADGET] twice; the *[sprocket wheel][SPROCKETWHEEL]* too.\n\n[GADGET]: ../specs/glossary/gadget.md\n[CHANGE]: ../specs/glossary/change.md\n[SPROCKETWHEEL]: ../specs/glossary/sprocket-wheel.md\n",
        )
        .unwrap();

        let output = fix_output(source(&root), true, false).unwrap();
        assert_eq!(
            output.drafts,
            [
                DraftTerm {
                    label: "gadget".into(),
                    name: "Gadget".into(),
                    page: "gadget.md".into(),
                    origin: "docs/decisions/X.md:3".into(),
                },
                DraftTerm {
                    label: "sprocket wheel".into(),
                    name: "SprocketWheel".into(),
                    page: "sprocket-wheel.md".into(),
                    origin: "docs/decisions/X.md:3".into(),
                },
            ]
        );
        assert!(output.skipped.is_empty(), "{output:?}");
        assert!(output.pages_written);
        let ontology = std::fs::read_to_string(root.path().join("ontology/rune.ttl")).unwrap();
        assert!(ontology.ends_with(
            "# Draft term. The fixer added it from docs/decisions/X.md:3; define it or remove the link.\nrune:SprocketWheel a rdfs:Class ;\n    rdfs:label \"sprocket wheel\" ;\n    rune:status \"draft\" .\n"
        ));
        let page =
            std::fs::read_to_string(root.path().join("docs/specs/glossary/gadget.md")).unwrap();
        assert!(page.contains("status: draft\n"));
        assert!(page.contains(DRAFT_COMMENT));
        assert_eq!(
            render_fix(&output),
            "added draft 'gadget' as Gadget (docs/specs/glossary/gadget.md) from docs/decisions/X.md:3\nadded draft 'sprocket wheel' as SprocketWheel (docs/specs/glossary/sprocket-wheel.md) from docs/decisions/X.md:3\nwrote ontology/rune.ttl and docs/specs/glossary\n"
        );

        let again = fix_output(source(&root), true, true).unwrap();
        assert!(again.drafts.is_empty(), "{again:?}");
        assert!(!again.pages_written);
        assert_eq!(render_fix(&again), "nothing to fix\n");
    }

    #[test]
    fn terms_skips_a_link_whose_name_exists_and_a_page_with_a_foreign_iri() {
        let root = repository();
        let record = root.path().join("docs/decisions/X.md");
        std::fs::write(
            root.path().join("docs/specs/glossary/foreign.md"),
            "---\ntitle: foreign\niri: https://example.org/Foreign\n---\n\n# foreign\n",
        )
        .unwrap();
        std::fs::write(
            &record,
            "# X\n\nA *[stage][STAGEX]* and a *[foreign][FOREIGN]*.\n\n[STAGEX]: ../specs/glossary/stage.md\n[FOREIGN]: ../specs/glossary/foreign.md\n",
        )
        .unwrap();
        let output = fix_output(source(&root), true, false).unwrap();
        assert!(output.drafts.is_empty(), "{output:?}");
        let reasons: Vec<_> = output
            .skipped
            .iter()
            .map(|s| (s.text.as_str(), s.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                (
                    "stage",
                    "a term named Stage exists; point the definition at its page"
                ),
                (
                    "foreign",
                    "its page names https://example.org/Foreign, which is not a term in ontology/rune.ttl"
                ),
            ]
        );
        assert!(!output.pages_written);
    }

    #[test]
    fn terms_names_a_plural_draft_after_the_page_its_definition_names() {
        let root = repository();
        let record = root.path().join("docs/decisions/Plural.md");
        std::fs::write(
            &record,
            "# Plural\n\nTwo *[gizmos][GIZMO]* and *[Murphy's law][MURPHYSLAW]*.\n\n[GIZMO]: ../specs/glossary/gizmo.md\n[MURPHYSLAW]: ../specs/glossary/murphys-law.md\n",
        )
        .unwrap();

        let output = fix_output(source(&root), true, false).unwrap();
        let drafts: Vec<_> = output
            .drafts
            .iter()
            .map(|draft| {
                (
                    draft.label.as_str(),
                    draft.name.as_str(),
                    draft.page.as_str(),
                )
            })
            .collect();
        assert_eq!(
            drafts,
            [
                ("gizmo", "Gizmo", "gizmo.md"),
                ("Murphy's law", "MurphysLaw", "murphys-law.md"),
            ],
            "{output:?}"
        );
        assert!(output.skipped.is_empty(), "{output:?}");
        let vocabulary = Terms::load(root.path(), &root.path().join("docs/specs")).unwrap();
        assert!(
            unresolved_links(
                &std::fs::read_to_string(&record).unwrap(),
                &record,
                &vocabulary
            )
            .is_empty(),
            "the plural link resolves through the page the draft created"
        );

        let again = fix_output(source(&root), true, false).unwrap();
        assert_eq!(render_fix(&again), "nothing to fix\n");
    }

    #[test]
    fn terms_skips_a_tag_without_a_definition_and_names_the_line_to_add() {
        let root = repository();
        let record = root.path().join("docs/decisions/Dangling.md");
        std::fs::write(&record, "# Dangling\n\nA [phantom][PHANTOM] here.\n").unwrap();

        let output = fix_output(source(&root), true, true).unwrap();
        assert!(output.drafts.is_empty(), "{output:?}");
        assert!(output.rewritten.is_empty(), "{output:?}");
        assert_eq!(
            render_fix(&output),
            "skipped 'phantom' [PHANTOM] at docs/decisions/Dangling.md:3: the tag has no definition in this file and no term has the tag\n"
        );
    }

    #[test]
    fn the_walk_skips_generated_deployed_and_archived_trees() {
        let root = repository();
        const LEGACY: &str = "*change* [CHANGE]\n\n[CHANGE]: https://runedeck.ai/ns#Change\n";
        for directory in [
            "docs/changes/archive/x",
            "build",
            ".trash",
            "docs/specs/glossary",
            ".claude/rules",
            "docs/decisions/archive",
        ] {
            std::fs::create_dir_all(root.path().join(directory)).unwrap();
            std::fs::write(root.path().join(directory).join("a.md"), LEGACY).unwrap();
        }
        // A deployed tree carries a manifest; only the change archive is an
        // archive.
        std::fs::write(root.path().join(".claude/.manifest"), "{}\n").unwrap();
        let output = fix_output(source(&root), false, true).unwrap();
        assert_eq!(
            output.rewritten,
            ["docs/decisions/archive/a.md"],
            "{output:?}"
        );
    }

    #[test]
    fn a_repository_without_an_ontology_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
        let error = fix_output(source(&root), true, true).unwrap_err();
        assert!(error.message().contains("no ontology at ontology/rune.ttl"));
    }
}
