//! Term links in prose: `*[text][TAG]*` at first use, `[text][TAG]` after
//! that, resolved through the `[TAG]: <relative page path>` definition at
//! the end of the file to a glossary page whose `iri:` names the ontology
//! term. When the page is absent the tag resolves the term. The legacy
//! form `*text* [TAG]` with an IRI definition is still read, so the fixer
//! can rewrite it and the validator can warn about it.

use super::prose::prose_lines;
use super::terms::{Term, Terms, kebab};
use super::{Error, ErrorKind};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

/// Directories a document walk never enters by name: version control,
/// generated, dependency, and fixture trees. A deployed tree (a directory
/// below the root that holds a `.manifest`) and the directories a caller
/// excludes, such as the glossary and the change archive, are skipped
/// by path.
pub(super) const SKIPPED: [&str; 11] = [
    ".git",
    ".jj",
    ".provenance",
    ".specstory",
    ".trash",
    ".workspaces",
    ".worktrees",
    "build",
    "fixtures",
    "node_modules",
    "target",
];

/// The deployment record a provider tree carries; a directory that holds
/// one is deployed output, which the walk never edits.
const MANIFEST: &str = ".manifest";

/// A reference-style link `[text][TAG]`, with the `*` that may wrap it.
static REFERENCE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\*?)\[([^\[\]\n]+)\]\[([A-Za-z0-9][A-Za-z0-9_-]*)\](\*?)")
        .expect("reference link regex is valid")
});
/// The legacy form: a single-asterisk italic run followed by a bare tag.
static LEGACY_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\*([^*\[\]\n]+?)\*\s*\[([A-Za-z0-9][A-Za-z0-9_-]*)\]")
        .expect("legacy link regex is valid")
});
/// A reference definition, `[TAG]: <target>`, indented up to three spaces.
static NOT_PROSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[a-z][a-z0-9+.-]*://[^\s<>\])]+|<[^<>\n]*>")
        .expect("not-prose regex is valid")
});

static REFERENCE_DEFINITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^ {0,3}\[([A-Za-z0-9][A-Za-z0-9_-]*)\]:\s*(\S+)")
        .expect("reference definition regex is valid")
});

/// One link in a prose line that may name a term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TermLink {
    /// One-based line number.
    pub line: usize,
    pub text: String,
    /// The tag as written; Markdown compares it without case.
    pub tag: String,
    /// `*[text][TAG]*`: the form of a first use.
    pub emphasized: bool,
    /// `*text* [TAG]`: the form before the link model.
    pub legacy: bool,
    /// Byte span of the whole link within the line, emphasis included.
    pub start: usize,
    pub end: usize,
}

impl TermLink {
    /// The link in the link model, emphasized or plain.
    pub fn render(&self, emphasized: bool) -> String {
        if emphasized {
            format!("*[{}][{}]*", self.text, self.tag)
        } else {
            format!("[{}][{}]", self.text, self.tag)
        }
    }
}

/// A plain occurrence of a term's label on a prose line: the label or its
/// plural as whole words, outside any link, heading, or inline code (the
/// prose view blanks code), compared without case. Every occurrence of a
/// term in a file is a link, so a plain one is a finding and the fixer
/// links it with the tag the file already uses for the term.
pub(super) struct PlainOccurrence {
    pub start: usize,
    pub end: usize,
}

pub(super) fn plain_occurrences(line: &str, label: &str) -> Vec<PlainOccurrence> {
    if line.trim_start().starts_with('#') || label.is_empty() || is_definition(line) {
        return Vec::new();
    }
    // Lower-case per character, keeping every byte offset: a character whose
    // lower-case form has another length (`İ`, the Kelvin sign) stays as it is.
    let lower: String = line
        .chars()
        .map(|c| {
            let mut lowered = c.to_lowercase();
            match (lowered.next(), lowered.next()) {
                (Some(l), None) if l.len_utf8() == c.len_utf8() => l,
                _ => c,
            }
        })
        .collect();
    let needle = label.to_lowercase();
    let mut masked = vec![false; line.len()];
    for link in reference_links(1, line) {
        masked[link.start..link.end].fill(true);
    }
    // A bare URL and anything in angle brackets (an autolink, an HTML tag
    // with its attributes) are not prose.
    for span in NOT_PROSE.find_iter(line) {
        masked[span.start()..span.end()].fill(true);
    }
    let mut depth = 0usize;
    // Inside the destination of an inline link `[text](path)`, which is not
    // prose; the count tracks balanced parentheses within it.
    let mut destination = 0usize;
    let bytes = line.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'[' if destination == 0 => depth += 1,
            b']' if destination == 0 => {
                depth = depth.saturating_sub(1);
                if depth == 0 && bytes.get(i + 1) == Some(&b'(') {
                    destination = 1;
                }
            }
            b'(' if destination > 0 && i > 0 && bytes[i - 1] != b']' => destination += 1,
            b')' if destination > 0 => {
                masked[i] = true;
                destination -= 1;
            }
            _ => {}
        }
        if depth > 0 || destination > 0 {
            masked[i] = true;
        }
    }
    // A label that ends in a consonant and y pluralizes to `ies`
    // (`capabilities`), so the search runs on the stem and accepts both
    // endings.
    let y_plural = needle.len() > 1
        && needle.ends_with('y')
        && !needle[..needle.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    let stem = if y_plural {
        &needle[..needle.len() - 1]
    } else {
        needle.as_str()
    };
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find(stem) {
        let start = from + at;
        let mut end = start + stem.len();
        if y_plural {
            if lower[end..].starts_with("ies") {
                end += 3;
            } else if lower[end..].starts_with('y') {
                end += 1;
            } else {
                from = end.max(start + 1);
                continue;
            }
        } else if lower[end..].starts_with("es") {
            // The plural: `changes`, `harnesses`, `statuses`. A label that
            // ends in s takes `es` only.
            end += 2;
        } else if lower[end..].starts_with('s') && !needle.ends_with('s') {
            end += 1;
        }
        let before = lower[..start].chars().next_back();
        let after = lower[end..].chars().next();
        let boundary =
            |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        if boundary(before)
            && boundary(after)
            && !masked[start..end].iter().any(|m| *m)
            && !lower[..start].ends_with('*')
        {
            found.push(PlainOccurrence { start, end });
        }
        from = end.max(start + 1);
    }
    found
}

/// The reference-style links of one prose line, in order. An image
/// `![alt][TAG]` is not a link to a page.
pub(super) fn reference_links(number: usize, line: &str) -> Vec<TermLink> {
    REFERENCE_LINK
        .captures_iter(line)
        .filter_map(|capture| {
            let whole = capture.get(0).expect("whole match");
            let opening = capture.get(1).expect("opening emphasis").as_str();
            let closing = capture.get(4).expect("closing emphasis").as_str();
            let emphasized = !opening.is_empty() && !closing.is_empty();
            // Without matching emphasis the asterisks belong to the text
            // around the link, so the span is the bare link.
            let start = if emphasized {
                whole.start()
            } else {
                whole.start() + opening.len()
            };
            let end = if emphasized {
                whole.end()
            } else {
                whole.end() - closing.len()
            };
            if line[..start].ends_with('!') {
                return None;
            }
            Some(TermLink {
                line: number,
                text: capture[2].to_string(),
                tag: capture[3].to_string(),
                emphasized,
                legacy: false,
                start,
                end,
            })
        })
        .collect()
}

/// The legacy `*text* [TAG]` runs of one prose line. A run counts when
/// the character before its opening `*` is neither `*`, a word character,
/// nor a backslash, and the tag is not the start of a link or a
/// definition.
pub(super) fn legacy_links(number: usize, line: &str) -> Vec<TermLink> {
    LEGACY_LINK
        .captures_iter(line)
        .filter_map(|capture| {
            let whole = capture.get(0).expect("whole match");
            let before = line[..whole.start()].chars().next_back();
            let bounded =
                before.is_none_or(|c| c != '*' && !c.is_alphanumeric() && c != '_' && c != '\\');
            let after = line[whole.end()..].chars().next();
            if !bounded || matches!(after, Some('[' | '(' | ':')) {
                return None;
            }
            Some(TermLink {
                line: number,
                text: capture[1].trim().to_string(),
                tag: capture[2].to_string(),
                emphasized: true,
                legacy: true,
                start: whole.start(),
                end: whole.end(),
            })
        })
        .collect()
}

/// One `[TAG]: <target>` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Definition {
    pub line: usize,
    /// The tag as written.
    pub tag: String,
    /// The destination, angle brackets removed.
    pub target: String,
    /// Byte span of the destination as written, angle brackets included.
    pub target_start: usize,
    pub target_end: usize,
}

/// Reference definitions by upper-cased tag (Markdown labels are
/// case-insensitive). The first definition of a tag wins, as in Markdown.
pub(super) fn reference_definitions<'a>(
    prose: impl Iterator<Item = (usize, &'a str)>,
) -> BTreeMap<String, Definition> {
    let mut definitions = BTreeMap::new();
    for (number, line) in prose {
        if let Some(capture) = REFERENCE_DEFINITION.captures(line) {
            let raw = capture.get(2).expect("destination");
            let target = raw
                .as_str()
                .strip_prefix('<')
                .and_then(|inner| inner.strip_suffix('>'))
                .unwrap_or(raw.as_str());
            definitions
                .entry(capture[1].to_uppercase())
                .or_insert(Definition {
                    line: number,
                    tag: capture[1].to_string(),
                    target: target.to_string(),
                    target_start: raw.start(),
                    target_end: raw.end(),
                });
        }
    }
    definitions
}

/// Whether a line is a reference definition.
pub(super) fn is_definition(line: &str) -> bool {
    REFERENCE_DEFINITION.is_match(line)
}

/// What a definition points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Target {
    /// A page under the glossary directory.
    Page(PathBuf),
    /// An IRI under a namespace the ontology declares terms in.
    Iri(String),
    /// Another file or address: an ordinary link.
    Other,
    /// The tag has no definition in the file.
    Undefined,
}

pub(super) fn classify(definition: Option<&Definition>, file_dir: &Path, terms: &Terms) -> Target {
    let Some(definition) = definition else {
        return Target::Undefined;
    };
    if definition.target.contains("://") {
        return if terms.in_namespace(&definition.target) {
            Target::Iri(definition.target.clone())
        } else {
            Target::Other
        };
    }
    let glossary = terms.glossary_dir();
    if glossary.as_os_str().is_empty() {
        return Target::Other;
    }
    let target = definition.target.split('#').next().unwrap_or_default();
    let path = normalize(&canonical(file_dir).join(target));
    if path.starts_with(canonical(glossary)) && path.extension().is_some_and(|e| e == "md") {
        Target::Page(path)
    } else {
        Target::Other
    }
}

/// The path with its deepest existing ancestor canonicalized, so two
/// spellings of one root (`/tmp` and `/private/tmp`, a symlinked
/// checkout) compare equal, and a file that does not exist yet still gets
/// a canonical parent.
pub(super) fn canonical(path: &Path) -> PathBuf {
    let path = normalize(path);
    let mut rest = Vec::new();
    let mut ancestor = path.as_path();
    loop {
        if let Ok(real) = ancestor.canonicalize() {
            let mut result = real;
            for component in rest.iter().rev() {
                result.push(component);
            }
            return result;
        }
        let Some(name) = ancestor.file_name() else {
            return path;
        };
        rest.push(name.to_os_string());
        let Some(parent) = ancestor.parent() else {
            return path;
        };
        ancestor = parent;
    }
}

/// How a link reached its term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Via {
    /// The page exists and its `iri:` names the term.
    Page,
    /// The page is absent; the tag names the term.
    PageMissing,
    /// The tag has no definition; the tag names the term.
    NoDefinition,
    /// The definition is the term's IRI, the form before pages existed.
    Iri,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolution<'t> {
    /// Not a term link: its definition points at another file or address.
    Ordinary,
    Term {
        term: &'t Term,
        via: Via,
        /// The page the definition points at, when it points at one.
        page: Option<PathBuf>,
    },
    /// A link that names no term: its definition reaches the glossary or
    /// a term namespace without naming one, or it has no definition at
    /// all, which no link resolves through.
    Undefined {
        page: Option<PathBuf>,
        /// The page is absent and the tag names nothing: the fixer may
        /// add a draft.
        draftable: bool,
        reason: String,
        /// What settles the link, for the validator's message.
        hint: String,
    },
}

pub(super) fn resolve<'t>(
    terms: &'t Terms,
    link: &TermLink,
    definitions: &BTreeMap<String, Definition>,
    file_dir: &Path,
) -> Resolution<'t> {
    let definition = definitions.get(&link.tag.to_uppercase());
    let define = format!("define it in {}", terms.location());
    let fixer = "run `rune spec fix --terms` to add a draft".to_string();
    match classify(definition, file_dir, terms) {
        Target::Page(page) => match std::fs::read_to_string(&page) {
            Ok(text) => match page_iri(&text) {
                Some(iri) => terms.by_iri(iri).map_or_else(
                    || Resolution::Undefined {
                        page: Some(page.clone()),
                        draftable: false,
                        reason: format!(
                            "its page names {iri}, which is not a term in {}",
                            terms.location()
                        ),
                        hint: define.clone(),
                    },
                    |term| Resolution::Term {
                        term,
                        via: Via::Page,
                        page: Some(page.clone()),
                    },
                ),
                None => Resolution::Undefined {
                    page: Some(page),
                    draftable: false,
                    reason: "its page has no iri field".to_string(),
                    hint: "run `rune spec glossary --write`".to_string(),
                },
            },
            Err(_) => terms.by_tag(&link.tag).map_or_else(
                || Resolution::Undefined {
                    page: Some(page.clone()),
                    draftable: true,
                    reason: format!(
                        "the page is missing and no term in {} has the tag",
                        terms.location()
                    ),
                    hint: fixer.clone(),
                },
                |term| Resolution::Term {
                    term,
                    via: Via::PageMissing,
                    page: Some(page.clone()),
                },
            ),
        },
        Target::Iri(iri) => terms.by_iri(&iri).map_or_else(
            || Resolution::Undefined {
                page: None,
                draftable: false,
                reason: format!("{iri} is not a term in {}", terms.location()),
                hint: define.clone(),
            },
            |term| Resolution::Term {
                term,
                via: Via::Iri,
                page: None,
            },
        ),
        Target::Other => Resolution::Ordinary,
        // No definition at all: nothing resolves such a link, so it is a
        // dangling link whether or not a term was meant.
        Target::Undefined => terms.by_tag(&link.tag).map_or_else(
            || {
                let glossary = terms.glossary_dir();
                let page = if glossary.as_os_str().is_empty() {
                    "<page path>".to_string()
                } else {
                    relative_link(
                        file_dir,
                        &glossary.join(format!("{}.md", kebab(&link.text))),
                    )
                };
                Resolution::Undefined {
                    page: None,
                    draftable: false,
                    reason: "the tag has no definition in this file and no term has the tag"
                        .to_string(),
                    hint: format!(
                        "add `[{}]: {page}` at the end of the file for a term, or the target of an ordinary link",
                        link.tag
                    ),
                }
            },
            |term| Resolution::Term {
                term,
                via: Via::NoDefinition,
                page: None,
            },
        ),
    }
}

/// Every reference-style link of a document with its resolution, in
/// document order.
pub(super) fn resolve_document<'t>(
    content: &str,
    path: &Path,
    terms: &'t Terms,
) -> Vec<(TermLink, Resolution<'t>)> {
    let file_dir = path.parent().unwrap_or(Path::new(""));
    let view: Vec<(usize, String)> = prose_lines(content).collect();
    let definitions =
        reference_definitions(view.iter().map(|(number, line)| (*number, line.as_str())));
    view.iter()
        .flat_map(|(number, line)| reference_links(*number, line))
        .map(|link| {
            let resolution = resolve(terms, &link, &definitions, file_dir);
            (link, resolution)
        })
        .collect()
}

/// Every Markdown file below `root`, sorted, outside the skipped
/// directories, outside every directory in `excludes`, and outside any
/// deployed tree below the root. Symlinks are not followed.
pub(super) fn markdown_files(root: &Path, excludes: &[&Path]) -> Result<Vec<PathBuf>, Error> {
    let io_error = |path: &Path, error: std::io::Error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    };
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_error(&directory, error)),
        };
        for entry in entries {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io_error(&path, error))?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let name = entry.file_name();
                if !excludes.contains(&path.as_path())
                    && !SKIPPED.contains(&name.to_string_lossy().as_ref())
                    && !path.join(MANIFEST).is_file()
                {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// The `iri:` value of a page's front matter.
pub(super) fn page_iri(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    rest[..end]
        .lines()
        .find_map(|line| line.strip_prefix("iri:"))
        .map(str::trim)
        .filter(|iri| !iri.is_empty())
}

/// The path without `.` and `..` components, resolved lexically.
pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push("..");
                }
            }
            other => result.push(other),
        }
    }
    result
}

/// The relative link from a directory to a file, with `/` separators, as
/// `os.path.relpath` computes it.
pub(super) fn relative_link(from_dir: &Path, to: &Path) -> String {
    let from_dir = canonical(from_dir);
    let to = canonical(to);
    let from: Vec<_> = from_dir.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from
        .iter()
        .zip(to.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts: Vec<String> = vec!["..".to_string(); from.len() - shared];
    parts.extend(
        to[shared..]
            .iter()
            .map(|component| component.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_links_carry_emphasis_and_skip_images() {
        let links = reference_links(
            3,
            "A *[harness][HARNESS]* loads a [canon][CANON]; ![alt][IMG] *[x][X] and [y][Y]*.",
        );
        let summary: Vec<_> = links
            .iter()
            .map(|link| (link.text.as_str(), link.tag.as_str(), link.emphasized, &"A *[harness][HARNESS]* loads a [canon][CANON]; ![alt][IMG] *[x][X] and [y][Y]*."[link.start..link.end]))
            .collect();
        assert_eq!(
            summary,
            [
                ("harness", "HARNESS", true, "*[harness][HARNESS]*"),
                ("canon", "CANON", false, "[canon][CANON]"),
                ("x", "X", false, "[x][X]"),
                ("y", "Y", false, "[y][Y]"),
            ]
        );
        assert_eq!(links[0].render(false), "[harness][HARNESS]");
        assert_eq!(links[1].render(true), "*[canon][CANON]*");
    }

    #[test]
    fn legacy_links_need_a_bare_tag_after_an_italic_run() {
        let line =
            "Every *harness* [HARNESS] and *canon* [CANON](x) or x*not* [N] and *[new][NEW]*.";
        let links = legacy_links(1, line);
        let found: Vec<_> = links
            .iter()
            .map(|link| &line[link.start..link.end])
            .collect();
        assert_eq!(found, ["*harness* [HARNESS]"]);
        assert!(links[0].legacy);
        assert_eq!(links[0].text, "harness");
    }

    #[test]
    fn definitions_keep_the_first_tag_and_unwrap_angle_brackets() {
        let content = "[A]: <https://x/A>\n[a]: other\n   [B]: ../glossary/b.md \"title\"\n";
        let definitions =
            reference_definitions(content.lines().enumerate().map(|(i, l)| (i + 1, l)));
        assert_eq!(definitions["A"].target, "https://x/A");
        assert_eq!(definitions["A"].line, 1);
        assert_eq!(definitions["B"].target, "../glossary/b.md");
        assert_eq!(
            &content.lines().nth(2).unwrap()
                [definitions["B"].target_start..definitions["B"].target_end],
            "../glossary/b.md"
        );
        assert!(is_definition("[X]: y"));
        assert!(!is_definition("[x][y]"));
    }

    #[test]
    fn paths_normalize_and_relative_links_climb() {
        assert_eq!(
            normalize(Path::new("/r/docs/changes/x/./specs/../../glossary/a.md")),
            PathBuf::from("/r/docs/changes/glossary/a.md")
        );
        assert_eq!(
            relative_link(
                Path::new("/r/docs/changes/x/specs/y"),
                Path::new("/r/docs/specs/glossary/a.md")
            ),
            "../../../../specs/glossary/a.md"
        );
        assert_eq!(
            relative_link(
                Path::new("/r/docs/specs"),
                Path::new("/r/docs/specs/glossary/a.md")
            ),
            "glossary/a.md"
        );
    }

    #[test]
    fn plain_occurrences_skip_links_headings_code_and_partial_words() {
        let line = "A [change][CHANGE] and a change, two changes, no changed,          masked.";
        let hits: Vec<&str> = plain_occurrences(line, "change")
            .into_iter()
            .map(|hit| &line[hit.start..hit.end])
            .collect();
        assert_eq!(hits, vec!["change", "changes"]);
        assert!(plain_occurrences("## The change", "change").is_empty());
        assert!(plain_occurrences("*[change][CHANGE]* only", "change").is_empty());
        let inline = "See [the design](../changes/x/design.md) for the change.";
        let hits: Vec<&str> = plain_occurrences(inline, "change")
            .into_iter()
            .map(|hit| &inline[hit.start..hit.end])
            .collect();
        assert_eq!(hits, vec!["change"]);
        let nested = "See [x](dir(foo)/change.md) then a change.";
        assert_eq!(plain_occurrences(nested, "change").len(), 1);
        let plural = "The harnesses and the harness.";
        let hits: Vec<&str> = plain_occurrences(plural, "harness")
            .into_iter()
            .map(|hit| &plural[hit.start..hit.end])
            .collect();
        assert_eq!(hits, vec!["harnesses", "harness"]);
        let plural_y = "Two capabilities, one capability, no capabilitys.";
        let hits: Vec<&str> = plain_occurrences(plural_y, "capability")
            .into_iter()
            .map(|hit| &plural_y[hit.start..hit.end])
            .collect();
        assert_eq!(hits, vec!["capabilities", "capability"]);
        let not_prose =
            "See https://example.com/harness and <a href=\"harness\">harness</a>, then a harness.";
        assert_eq!(plain_occurrences(not_prose, "harness").len(), 2);
        let unicode = "İ change. K change.";
        let hits: Vec<&str> = plain_occurrences(unicode, "change")
            .into_iter()
            .map(|hit| &unicode[hit.start..hit.end])
            .collect();
        assert_eq!(hits, vec!["change", "change"]);
    }

    #[test]
    fn a_page_iri_comes_from_the_front_matter() {
        assert_eq!(
            page_iri("---\ntitle: x\niri: https://runedeck.ai/ns#X\n---\n\n# x\n"),
            Some("https://runedeck.ai/ns#X")
        );
        assert_eq!(page_iri("# no front matter\n"), None);
        assert_eq!(page_iri("---\ntitle: x\n---\n"), None);
    }
}
