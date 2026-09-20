//! `CHANGELOG.md` shape: Keep a Changelog 1.1.0 structure with Common
//! Changelog line discipline. A release is a heading, at most one notice
//! paragraph, and grouped one-line entries. Nothing else, because a
//! changelog is not a blog. Pure analysis: the caller renders the report.
//!
//! Sources: <https://keepachangelog.com/en/1.1.0/> (sections, Unreleased,
//! dates, order) and <https://common-changelog.org/> (one line per change,
//! present-tense verb first, no encoded prefixes).

use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

/// One change is one line. A line longer than this is a paragraph that
/// belongs in the commit, the change, or a linked reference.
pub const MAX_ENTRY_CHARS: usize = 200;
/// The change groups, in the order a release lists them.
pub const GROUPS: [&str; 6] = [
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
];

static RELEASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^## \[(\d+)\.(\d+)\.(\d+)\] - (\d{4}-\d{2}-\d{2})( \[YANKED\])?$")
        .expect("release heading regex is valid")
});
/// Conventional-commit style prefixes: `feat:`, `fix(scope)!:`.
static ENCODED_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]+(\([^)]*\))?!?:").expect("prefix regex is valid"));

#[derive(Debug, Default)]
pub struct ChangelogReport {
    pub present: bool,
    pub releases: usize,
    pub entries: usize,
    pub errors: Vec<String>,
}

/// Check `<root>/CHANGELOG.md`. An absent file is not an error: not every
/// repository keeps one.
pub fn check(root: &Path) -> Result<ChangelogReport, String> {
    let path = root.join("CHANGELOG.md");
    if !path.is_file() {
        return Ok(ChangelogReport::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut report = lint(&content);
    report.present = true;
    Ok(report)
}

#[derive(Default)]
struct Release {
    line: usize,
    version: Option<(u64, u64, u64)>,
    groups_seen: Vec<usize>,
    notice_taken: bool,
    saw_group: bool,
}

/// Lint the text. Errors carry `line: rule: detail`.
pub fn lint(content: &str) -> ChangelogReport {
    let mut report = ChangelogReport::default();
    let mut release: Option<Release> = None;
    let mut previous_version: Option<(u64, u64, u64)> = None;
    let mut in_group = false;
    let mut fence = false;
    let mut title_seen = false;
    let push = |report: &mut ChangelogReport, line: usize, rule: &str, detail: String| {
        report.errors.push(format!("{line}: {rule}: {detail}"));
    };

    for (index, raw) in content.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim_end();
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence || line.trim().is_empty() {
            continue;
        }
        if !title_seen {
            if line != "# Changelog" {
                push(&mut report, number, "title", format!("the first heading must be `# Changelog`, found `{line}`"));
            }
            title_seen = true;
            continue;
        }
        if let Some(heading) = line.strip_prefix("## ") {
            report.releases += 1;
            in_group = false;
            let mut next = Release { line: number, ..Release::default() };
            if heading == "[Unreleased]" {
                if report.releases != 1 {
                    push(&mut report, number, "unreleased-first", "`[Unreleased]` must be the first release heading".to_string());
                }
            } else if let Some(capture) = RELEASE.captures(line) {
                let version = (
                    capture[1].parse().unwrap_or(0),
                    capture[2].parse().unwrap_or(0),
                    capture[3].parse().unwrap_or(0),
                );
                if let Some(previous) = previous_version
                    && version >= previous
                {
                    push(&mut report, number, "release-order", format!("releases must be newest first, {} follows a lower version", &capture[0][3..]));
                }
                previous_version = Some(version);
                next.version = Some(version);
            } else {
                push(&mut report, number, "release-heading", format!("a release heading is `## [X.Y.Z] - YYYY-MM-DD` or `## [Unreleased]`, found `{heading}`"));
            }
            release = Some(next);
            continue;
        }
        if let Some(group) = line.strip_prefix("### ") {
            in_group = true;
            let Some(current) = release.as_mut() else {
                push(&mut report, number, "group-outside-release", format!("`### {group}` before any release heading"));
                continue;
            };
            current.saw_group = true;
            match GROUPS.iter().position(|known| *known == group) {
                None => push(&mut report, number, "group-name", format!("`{group}` is not one of {}", GROUPS.join(", "))),
                Some(position) => {
                    if current.groups_seen.contains(&position) {
                        push(&mut report, number, "group-repeated", format!("`{group}` appears twice in one release"));
                    } else if current.groups_seen.iter().any(|seen| *seen > position) {
                        push(&mut report, number, "group-order", format!("`{group}` must come before the groups already listed; the order is {}", GROUPS.join(", ")));
                    }
                    current.groups_seen.push(position);
                }
            }
            continue;
        }
        if line.starts_with('#') {
            push(&mut report, number, "heading-depth", "only `##` releases and `###` groups are headings".to_string());
            continue;
        }
        let Some(current) = release.as_mut() else {
            // Preamble between the title and the first release is free text.
            continue;
        };
        if let Some(entry) = line.strip_prefix("- ") {
            if !in_group {
                push(&mut report, number, "entry-outside-group", "a change line needs a `###` group above it".to_string());
            }
            report.entries += 1;
            lint_entry(&mut report, number, entry);
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            push(&mut report, number, "entry-wrapped", "a change is one line; an indented continuation line is a paragraph".to_string());
            continue;
        }
        if in_group {
            push(&mut report, number, "prose-in-group", "a group holds only `- ` change lines".to_string());
        } else if current.notice_taken || current.saw_group {
            push(&mut report, number, "release-prose", "a release holds one notice paragraph at most, then groups".to_string());
        } else {
            current.notice_taken = true;
        }
    }
    let _ = release.map(|current| current.line);
    report
}

fn lint_entry(report: &mut ChangelogReport, number: usize, entry: &str) {
    let chars = entry.chars().count();
    if chars > MAX_ENTRY_CHARS {
        report.errors.push(format!(
            "{number}: entry-length: {chars} characters; the limit is {MAX_ENTRY_CHARS}, one line per change, detail goes in the change or the commit"
        ));
    }
    if entry.contains('\u{2014}') {
        report.errors.push(format!("{number}: entry-dash: no em-dash in a change line"));
    }
    if ENCODED_PREFIX.is_match(entry) {
        report.errors.push(format!(
            "{number}: entry-prefix: no `type:` prefix, start with the verb (Add, Fix, Change, Remove)"
        ));
    }
    let text = entry.strip_prefix("**Breaking:**").map_or(entry, str::trim_start);
    let first = text.split_whitespace().next().unwrap_or_default();
    let starts_upper = first.chars().next().is_some_and(char::is_uppercase);
    let article = matches!(first, "The" | "A" | "An" | "This" | "It");
    if !starts_upper || article {
        report.errors.push(format!(
            "{number}: entry-verb: start with a capitalized present-tense verb (Add, Fix, Change, Remove), found `{first}`"
        ));
    }
}

#[cfg(test)]
mod tests;
