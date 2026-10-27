//! `rune spec glossary`: the term list, and the generated glossary pages
//! that term links point at: one page per term plus an index, under
//! `docs/specs/glossary/`. The ontology is the only source. People edit
//! it, never the pages, and `--check` regenerates every page in memory and
//! compares the bytes, so a hand edit is caught.

use super::commands::print_json;
use super::terms::{GLOSSARY_FILE, Term, TermKind, TermSource, Terms, ontology_path};
use super::{Error, ErrorKind, relative_display, resolve_spec_root};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The paragraph a draft term's page carries instead of a comment.
pub const DRAFT_COMMENT: &str = "Draft term. The fixer added it and nobody has defined it yet. Archive refuses a change that links to it.";
/// The index file of the glossary directory.
pub const INDEX_FILE: &str = "index.md";
/// The command the index and a drift report tell the reader to run.
const WRITE_COMMAND: &str = "rune spec glossary --write";

/// The one paragraph a page and the index show for a term.
pub(super) fn page_comment(term: &Term) -> &str {
    if term.draft {
        DRAFT_COMMENT
    } else {
        &term.comment
    }
}

/// The bytes of one term's page.
pub(super) fn render_page(terms: &Terms, term: &Term) -> String {
    format!(
        "---\ntitle: {label}\niri: {iri}\nkind: {kind}\nstatus: {status}\n---\n\n# {label}\n\n{comment}\n\nGenerated from `{location}`. Edit the ontology, not this page.\n",
        label = term.label,
        iri = term.iri,
        kind = term.kind.name(),
        status = if term.draft { "draft" } else { "defined" },
        comment = page_comment(term),
        location = terms.location(),
    )
}

/// The bytes of the index: one entry per term, sorted by label.
pub(super) fn render_index(terms: &Terms) -> String {
    use std::fmt::Write as _;
    let mut text = format!(
        "# Glossary\n\nGenerated from `{}`. Edit the ontology, not these pages.\n\n",
        terms.location()
    );
    for term in terms.terms() {
        let _ = writeln!(
            text,
            "- [{}]({}): {}",
            term.label,
            terms.page_name(term),
            page_comment(term)
        );
    }
    text
}

/// Every file of the glossary directory by file name: the pages and the index.
pub(super) fn render_pages(terms: &Terms) -> BTreeMap<String, String> {
    let mut files: BTreeMap<String, String> = terms
        .terms()
        .iter()
        .map(|term| (terms.page_name(term).to_string(), render_page(terms, term)))
        .collect();
    files.insert(INDEX_FILE.to_string(), render_index(terms));
    files
}

/// The term list `rune spec glossary` prints, and its `--json` form.
#[derive(Debug, Serialize)]
pub struct GlossaryOutput {
    pub source: String,
    /// Repository-relative path of the glossary directory.
    pub glossary: String,
    pub terms: Vec<GlossaryTerm>,
}

#[derive(Debug, Serialize)]
pub struct GlossaryTerm {
    pub label: String,
    pub comment: String,
    pub name: String,
    pub iri: String,
    pub kind: TermKind,
    pub tag: String,
    /// The page's file name under the glossary directory.
    pub page: String,
    pub status: &'static str,
}

impl GlossaryOutput {
    fn from_terms(repository: &Path, terms: &Terms) -> Self {
        Self {
            source: terms.location().to_string(),
            glossary: relative_display(repository, terms.glossary_dir()),
            terms: terms
                .terms()
                .iter()
                .map(|term| GlossaryTerm {
                    label: term.label.clone(),
                    comment: page_comment(term).to_string(),
                    name: term.name.clone(),
                    iri: term.iri.clone(),
                    kind: term.kind,
                    tag: terms.tag(term).to_string(),
                    page: terms.page_name(term).to_string(),
                    status: if term.draft { "draft" } else { "defined" },
                })
                .collect(),
        }
    }
}

pub fn glossary_output(repository: &Path, specifications: &Path) -> Result<GlossaryOutput, Error> {
    let terms = Terms::load(repository, specifications)?;
    Ok(GlossaryOutput::from_terms(repository, &terms))
}

/// Print the term list, or its JSON form.
pub fn glossary(source: &str, json: bool) -> Result<i32, Error> {
    let spec_root = resolve_spec_root(Path::new(source))?;
    let output = glossary_output(spec_root.repository(), spec_root.specifications())?;
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_glossary(&output));
    }
    Ok(0)
}

/// The term list: one `- **label**: comment [TAG]` line per term, then the
/// `[TAG]: <page>` definitions with repository-relative page paths. A
/// document that copies them makes the paths relative to itself.
pub fn render_glossary(output: &GlossaryOutput) -> String {
    let mut lines: Vec<String> = output.terms.iter().map(entry_line).collect();
    if !output.terms.is_empty() {
        lines.push(String::new());
        lines.extend(
            output
                .terms
                .iter()
                .map(|term| format!("[{}]: {}/{}", term.tag, output.glossary, term.page)),
        );
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// One `- **label**: comment [TAG]` line. A term without a comment keeps
/// one space before its tag.
fn entry_line(term: &GlossaryTerm) -> String {
    let mut line = format!("- **{}**:", term.label);
    if !term.comment.is_empty() {
        line.push(' ');
        line.push_str(&term.comment);
    }
    line.push_str(" [");
    line.push_str(&term.tag);
    line.push(']');
    line
}

/// Where the glossary directory sits and what the rendering looks like,
/// for `--write` and `--check`. The ontology is the only source a page can
/// be rendered from: a repository without one has nothing to render.
struct GlossaryDir {
    path: PathBuf,
    display: String,
    custom: bool,
    /// The single file an older render wrote, when the default directory
    /// is in use.
    stale_file: Option<PathBuf>,
    files: BTreeMap<String, String>,
    terms: usize,
}

impl GlossaryDir {
    fn resolve(source: &str, path: Option<&str>) -> Result<Self, Error> {
        let spec_root = resolve_spec_root(Path::new(source))?;
        let repository = spec_root.repository();
        let terms = Terms::load(repository, spec_root.specifications())?;
        if terms.source() != TermSource::Ontology {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "no ontology at {}; the glossary pages are rendered from the ontology",
                    relative_display(repository, &ontology_path(repository)?)
                ),
            ));
        }
        let directory = match path {
            Some(path) => repository.join(path),
            None => terms.glossary_dir().to_path_buf(),
        };
        Ok(Self {
            display: relative_display(repository, &directory),
            path: directory,
            custom: path.is_some(),
            stale_file: path
                .is_none()
                .then(|| spec_root.specifications().join(GLOSSARY_FILE)),
            files: render_pages(&terms),
            terms: terms.terms().len(),
        })
    }

    fn write_command(&self) -> String {
        if self.custom {
            format!("{WRITE_COMMAND} {}", self.display)
        } else {
            WRITE_COMMAND.to_string()
        }
    }

    /// The `.md` files in the directory, by name.
    fn existing(&self) -> Result<BTreeMap<String, PathBuf>, Error> {
        let entries = match std::fs::read_dir(&self.path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BTreeMap::new());
            }
            Err(error) => return Err(self.io_error(&error)),
        };
        let mut files = BTreeMap::new();
        for entry in entries {
            let path = entry.map_err(|error| self.io_error(&error))?.path();
            if path.is_file()
                && path.extension().is_some_and(|extension| extension == "md")
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                files.insert(name.to_string(), path);
            }
        }
        Ok(files)
    }

    /// Whether the older single file still exists and is a render.
    fn stale_file_present(&self) -> bool {
        self.stale_file.as_ref().is_some_and(|file| {
            std::fs::read_to_string(file).is_ok_and(|text| text.starts_with("# Glossary\n"))
        })
    }

    fn io_error(&self, error: &std::io::Error) -> Error {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", self.display),
        )
    }
}

/// What `--write` reports.
#[derive(Debug, Serialize)]
pub struct GlossaryWriteOutput {
    /// Repository-relative path of the glossary directory.
    pub path: String,
    pub terms: usize,
    /// Files created or changed, by name.
    pub written: Vec<String>,
    /// Stale pages and the older single file removed, repository-relative.
    pub removed: Vec<String>,
}

/// Write the glossary pages and the index from the ontology, creating the
/// directory, removing pages of terms the ontology no longer defines and
/// the single file an older render wrote.
pub fn write_glossary_output(
    source: &str,
    path: Option<&str>,
) -> Result<GlossaryWriteOutput, Error> {
    let directory = GlossaryDir::resolve(source, path)?;
    let repository = resolve_spec_root(Path::new(source))?
        .repository()
        .to_path_buf();
    std::fs::create_dir_all(&directory.path).map_err(|error| directory.io_error(&error))?;
    let existing = directory.existing()?;
    let mut written = Vec::new();
    for (name, bytes) in &directory.files {
        let file = directory.path.join(name);
        if std::fs::read_to_string(&file).is_ok_and(|current| &current == bytes) {
            continue;
        }
        std::fs::write(&file, bytes).map_err(|error| directory.io_error(&error))?;
        written.push(name.clone());
    }
    let mut removed = Vec::new();
    for (name, file) in existing {
        if !directory.files.contains_key(&name) {
            std::fs::remove_file(&file).map_err(|error| directory.io_error(&error))?;
            removed.push(relative_display(&repository, &file));
        }
    }
    if directory.stale_file_present()
        && let Some(file) = &directory.stale_file
    {
        std::fs::remove_file(file).map_err(|error| directory.io_error(&error))?;
        removed.push(relative_display(&repository, file));
    }
    Ok(GlossaryWriteOutput {
        path: directory.display,
        terms: directory.terms,
        written,
        removed,
    })
}

pub fn write_glossary(source: &str, path: Option<&str>, json: bool) -> Result<i32, Error> {
    let output = write_glossary_output(source, path)?;
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_glossary_write(&output));
    }
    Ok(0)
}

pub fn render_glossary_write(output: &GlossaryWriteOutput) -> String {
    use std::fmt::Write as _;
    let mut text = format!(
        "wrote {}: {} page(s) and the index, {} file(s) changed\n",
        output.path,
        output.terms,
        output.written.len()
    );
    for removed in &output.removed {
        let _ = writeln!(text, "  removed  {removed}");
    }
    text
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GlossaryStatus {
    Current,
    Stale,
    Missing,
}

/// One file a drift report names: a page by its term's label and file
/// name, the index, or an extra file by its path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlossaryEntry {
    pub label: String,
    pub page: String,
}

impl GlossaryEntry {
    fn render(&self) -> String {
        if self.label.is_empty() {
            self.page.clone()
        } else {
            format!("{} ({})", self.label, self.page)
        }
    }
}

/// What `--check` reports: the pages the directory lacks, the files it
/// holds that the render does not produce, and the pages whose bytes
/// differ.
#[derive(Debug, Serialize)]
pub struct GlossaryCheckOutput {
    pub path: String,
    pub status: GlossaryStatus,
    pub write_command: String,
    pub missing: Vec<GlossaryEntry>,
    pub extra: Vec<GlossaryEntry>,
    pub changed: Vec<GlossaryEntry>,
}

impl GlossaryCheckOutput {
    pub fn is_current(&self) -> bool {
        self.status == GlossaryStatus::Current
    }
}

/// The label a page's bytes carry, for a drift report.
fn page_label(bytes: &str) -> String {
    bytes
        .lines()
        .find_map(|line| line.strip_prefix("title: "))
        .unwrap_or_default()
        .to_string()
}

/// Render every page in memory and compare the directory with it, byte
/// for byte.
pub fn check_glossary_output(
    source: &str,
    path: Option<&str>,
) -> Result<GlossaryCheckOutput, Error> {
    let directory = GlossaryDir::resolve(source, path)?;
    let repository = resolve_spec_root(Path::new(source))?
        .repository()
        .to_path_buf();
    let mut output = GlossaryCheckOutput {
        path: directory.display.clone(),
        status: GlossaryStatus::Current,
        write_command: directory.write_command(),
        missing: Vec::new(),
        extra: Vec::new(),
        changed: Vec::new(),
    };
    if !directory.path.is_dir() {
        output.status = GlossaryStatus::Missing;
        return Ok(output);
    }
    let existing = directory.existing()?;
    for (name, bytes) in &directory.files {
        let entry = GlossaryEntry {
            label: if name == INDEX_FILE {
                "index".to_string()
            } else {
                page_label(bytes)
            },
            page: name.clone(),
        };
        match existing.get(name) {
            None => output.missing.push(entry),
            Some(file) => {
                let current = std::fs::read_to_string(file).map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!(
                            "cannot read {}: {error}",
                            relative_display(&repository, file)
                        ),
                    )
                })?;
                if &current != bytes {
                    output.changed.push(entry);
                }
            }
        }
    }
    output.extra = existing
        .iter()
        .filter(|(name, _)| !directory.files.contains_key(*name))
        .map(|(name, _)| GlossaryEntry {
            label: String::new(),
            page: name.clone(),
        })
        .collect();
    if directory.stale_file_present()
        && let Some(file) = &directory.stale_file
    {
        output.extra.push(GlossaryEntry {
            label: String::new(),
            page: relative_display(&repository, file),
        });
    }
    if !(output.missing.is_empty() && output.extra.is_empty() && output.changed.is_empty()) {
        output.status = GlossaryStatus::Stale;
    }
    Ok(output)
}

pub fn render_glossary_check(output: &GlossaryCheckOutput) -> String {
    use std::fmt::Write as _;
    let mut text = match output.status {
        GlossaryStatus::Current => return format!("{} is current\n", output.path),
        GlossaryStatus::Missing => {
            return format!(
                "{} is missing: run `{}`\n",
                output.path, output.write_command
            );
        }
        GlossaryStatus::Stale => {
            format!("{} is stale: run `{}`\n", output.path, output.write_command)
        }
    };
    let rows = [
        ("missing", &output.missing),
        ("extra", &output.extra),
        ("changed", &output.changed),
    ];
    for (kind, entries) in rows {
        for entry in entries {
            let _ = writeln!(text, "  {kind:<8} {}", entry.render());
        }
    }
    text
}

pub fn check_glossary(source: &str, path: Option<&str>, json: bool) -> Result<i32, Error> {
    let output = check_glossary_output(source, path)?;
    if json {
        print_json(&output)?;
    } else {
        print!("{}", render_glossary_check(&output));
    }
    Ok(i32::from(!output.is_current()))
}

#[cfg(test)]
mod tests {
    use super::super::terms::tests::TURTLE;
    use super::*;

    /// A repository root with the ontology and an empty `docs/specs`.
    pub(in crate::spec) fn ontology_repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("ontology")).unwrap();
        std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
        std::fs::write(root.path().join("ontology/rune.ttl"), TURTLE).unwrap();
        root
    }

    fn source(root: &tempfile::TempDir) -> &str {
        root.path().to_str().unwrap()
    }

    #[test]
    fn a_page_and_the_index_follow_the_exact_shape() {
        let root = ontology_repository();
        let terms = Terms::load(root.path(), &root.path().join("docs/specs")).unwrap();
        let change = terms.by_iri("https://runedeck.ai/ns#Change").unwrap();
        assert_eq!(
            render_page(&terms, change),
            "---\ntitle: change\niri: https://runedeck.ai/ns#Change\nkind: class\nstatus: defined\n---\n\n# change\n\nOne spec-driven unit of work.\n\nGenerated from `ontology/rune.ttl`. Edit the ontology, not this page.\n"
        );
        let widget = terms.by_iri("https://runedeck.ai/ns#Widget").unwrap();
        assert_eq!(
            render_page(&terms, widget),
            format!(
                "---\ntitle: widget\niri: https://runedeck.ai/ns#Widget\nkind: class\nstatus: draft\n---\n\n# widget\n\n{DRAFT_COMMENT}\n\nGenerated from `ontology/rune.ttl`. Edit the ontology, not this page.\n"
            )
        );
        let index = render_index(&terms);
        assert!(index.starts_with(
            "# Glossary\n\nGenerated from `ontology/rune.ttl`. Edit the ontology, not these pages.\n\n- [Occam's razor](occams-razor.md): Of two instructions that pass the same check, the shorter wins.\n- [change](change.md): One spec-driven unit of work.\n- [change](change-key.md): The change a record belongs to.\n"
        ));
        assert!(index.ends_with(&format!("- [widget](widget.md): {DRAFT_COMMENT}\n")));
        let files = render_pages(&terms);
        assert_eq!(files.len(), 8);
        assert!(files.contains_key("index.md"));
        assert!(files.contains_key("lifecycle-stage.md"));
    }

    #[test]
    fn the_term_list_names_tags_and_repository_relative_pages() {
        let root = ontology_repository();
        let output = glossary_output(root.path(), &root.path().join("docs/specs")).unwrap();
        assert_eq!(output.glossary, "docs/specs/glossary");
        let text = render_glossary(&output);
        assert!(text.contains("- **change**: One spec-driven unit of work. [CHANGE]\n"));
        assert!(text.contains("- **change**: The change a record belongs to. [CHANGE-KEY]\n"));
        assert!(text.contains(&format!("- **widget**: {DRAFT_COMMENT} [WIDGET]\n")));
        assert!(text.contains("\n[CHANGE-KEY]: docs/specs/glossary/change-key.md\n"));
        assert!(text.ends_with("[WIDGET]: docs/specs/glossary/widget.md\n"));
        assert_eq!(output.terms[0].status, "defined");
        assert_eq!(output.terms[0].page, "occams-razor.md");
    }

    #[test]
    fn two_writes_give_the_same_bytes_and_remove_the_single_file() {
        let root = ontology_repository();
        let single = root.path().join("docs/specs/glossary.md");
        std::fs::write(&single, "# Glossary\n\nold render\n").unwrap();

        let first = write_glossary_output(source(&root), None).unwrap();
        assert_eq!(first.path, "docs/specs/glossary");
        assert_eq!(first.terms, 7);
        assert_eq!(first.written.len(), 8);
        assert_eq!(first.removed, ["docs/specs/glossary.md"]);
        assert!(!single.exists());
        let page =
            std::fs::read_to_string(root.path().join("docs/specs/glossary/change.md")).unwrap();
        assert!(page.starts_with("---\ntitle: change\n"));

        let second = write_glossary_output(source(&root), None).unwrap();
        assert_eq!(second.written, [] as [String; 0]);
        assert_eq!(second.removed, [] as [String; 0]);
        let check = check_glossary_output(source(&root), None).unwrap();
        assert!(check.is_current());
        assert_eq!(
            render_glossary_check(&check),
            "docs/specs/glossary is current\n"
        );
    }

    #[test]
    fn a_hand_edit_a_missing_page_an_extra_file_and_the_single_file_are_drift() {
        let root = ontology_repository();
        write_glossary_output(source(&root), None).unwrap();
        let directory = root.path().join("docs/specs/glossary");
        let page = directory.join("change-key.md");
        std::fs::write(
            &page,
            std::fs::read_to_string(&page)
                .unwrap()
                .replace("record", "file"),
        )
        .unwrap();
        std::fs::remove_file(directory.join("tight.md")).unwrap();
        std::fs::write(directory.join("stray.md"), "# stray\n").unwrap();
        std::fs::write(root.path().join("docs/specs/glossary.md"), "# Glossary\n").unwrap();

        let check = check_glossary_output(source(&root), None).unwrap();
        assert_eq!(check.status, GlossaryStatus::Stale);
        assert_eq!(
            render_glossary_check(&check),
            "docs/specs/glossary is stale: run `rune spec glossary --write`\n  missing  tight (tight.md)\n  extra    stray.md\n  extra    docs/specs/glossary.md\n  changed  change (change-key.md)\n"
        );

        let rewrite = write_glossary_output(source(&root), None).unwrap();
        assert_eq!(rewrite.written, ["change-key.md", "tight.md"]);
        assert_eq!(
            rewrite.removed,
            ["docs/specs/glossary/stray.md", "docs/specs/glossary.md"]
        );
        assert!(
            check_glossary_output(source(&root), None)
                .unwrap()
                .is_current()
        );
    }

    #[test]
    fn a_missing_directory_and_a_custom_path_are_reported_with_their_command() {
        let root = ontology_repository();
        let check = check_glossary_output(source(&root), None).unwrap();
        assert_eq!(check.status, GlossaryStatus::Missing);
        assert_eq!(
            render_glossary_check(&check),
            "docs/specs/glossary is missing: run `rune spec glossary --write`\n"
        );

        write_glossary_output(source(&root), Some("docs/terms")).unwrap();
        let custom = check_glossary_output(source(&root), Some("docs/terms")).unwrap();
        assert_eq!(custom.path, "docs/terms");
        assert!(custom.is_current());
        assert_eq!(
            custom.write_command,
            "rune spec glossary --write docs/terms"
        );
        assert!(root.path().join("docs/terms/index.md").is_file());
    }

    #[test]
    fn a_repository_without_an_ontology_cannot_render_pages() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("docs/specs")).unwrap();
        let error = write_glossary_output(source(&root), None).unwrap_err();
        assert!(error.message().contains("no ontology at ontology/rune.ttl"));
        assert!(check_glossary_output(source(&root), None).is_err());
    }
}
