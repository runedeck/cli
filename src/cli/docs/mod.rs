//! `rune docs`: native checks over the repo's `docs/` tree, plus a local
//! `mint dev` shell-out when a `docs.json` exists. Broken internal links
//! are errors; orphan pages are warnings; hosting stays out of scope.

use commands::error::{Error, ErrorKind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, clap::Subcommand)]
pub enum DocsAction {
    /// Broken internal links and orphan pages across docs/.
    Check,
    /// Local preview: shells out to `mint dev` when docs.json exists.
    Dev,
}

pub fn execute(action: &DocsAction, json: bool) -> Result<i32, Error> {
    match action {
        DocsAction::Check => check_at(Path::new("."), json),
        DocsAction::Dev => dev(Path::new(".")),
    }
}

fn dev(root: &Path) -> Result<i32, Error> {
    if !root.join("docs.json").is_file() {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!(
            "{}",
            sheet.dim("no docs.json here — the mint local preview needs one (https://mintlify.com/docs); rune docs check works without it")
        );
        return Ok(0);
    }
    let status = std::process::Command::new("mint")
        .arg("dev")
        .current_dir(root)
        .status()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot run mint (install: npm i -g mint): {error}"),
            )
        })?;
    Ok(status.code().unwrap_or(1))
}

#[derive(Debug, Default)]
struct DocsReport {
    broken: Vec<String>,
    orphans: Vec<String>,
}

fn check_at(root: &Path, json: bool) -> Result<i32, Error> {
    let docs_root = root.join("docs");
    if !docs_root.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} has no docs/ directory", root.display()),
        ));
    }
    let pages = collect_markdown(&docs_root);
    let report = link_report(root, &docs_root, &pages);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "pages": pages.len(),
                "broken_links": report.broken,
                "orphans": report.orphans,
            })
        );
        return Ok(i32::from(!report.broken.is_empty()));
    }

    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading("docs"));
    for broken in &report.broken {
        println!("{}", sheet.fail(broken));
    }
    for orphan in &report.orphans {
        println!("{}", sheet.warn(&format!("orphan: {orphan}")));
    }
    if report.broken.is_empty() && report.orphans.is_empty() {
        println!(
            "{}",
            sheet.ok(&format!("{} pages, links resolve", pages.len()))
        );
    } else {
        println!(
            "   {}",
            sheet.dim(&format!(
                "{} pages · {} broken · {} orphan",
                pages.len(),
                report.broken.len(),
                report.orphans.len()
            ))
        );
    }
    Ok(i32::from(!report.broken.is_empty()))
}

fn collect_markdown(docs_root: &Path) -> Vec<PathBuf> {
    let mut pages = Vec::new();
    let mut stack = vec![docs_root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
                pages.push(path);
            }
        }
    }
    pages.sort();
    pages
}

fn link_report(root: &Path, docs_root: &Path, pages: &[PathBuf]) -> DocsReport {
    let mut report = DocsReport::default();
    let basenames: BTreeMap<String, PathBuf> = pages
        .iter()
        .filter_map(|page| {
            page.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| (stem.to_string(), page.clone()))
        })
        .collect();
    let mut linked: BTreeSet<PathBuf> = BTreeSet::new();

    for page in pages {
        let Ok(content) = std::fs::read_to_string(page) else {
            continue;
        };
        let page_dir = page.parent().unwrap_or(docs_root);
        for target in link_targets(&content) {
            match &target {
                LinkTarget::Relative(relative) => {
                    let resolved = page_dir.join(decode_spaces(relative));
                    if resolved.exists() {
                        if let Ok(canonical) = resolved.canonicalize() {
                            linked.insert(canonical);
                        }
                    } else {
                        report.broken.push(format!(
                            "{}: broken link {relative}",
                            display_relative(page, root)
                        ));
                    }
                }
                LinkTarget::Wikilink(name) => match basenames.get(name.as_str()) {
                    Some(found) => {
                        if let Ok(canonical) = found.canonicalize() {
                            linked.insert(canonical);
                        }
                    }
                    None => report.broken.push(format!(
                        "{}: unresolved wikilink [[{name}]]",
                        display_relative(page, root)
                    )),
                },
            }
        }
    }

    for page in pages {
        let Ok(canonical) = page.canonicalize() else {
            continue;
        };
        let is_entry = page
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "README.md" || name == "SKILL.md");
        if !linked.contains(&canonical) && !is_entry && !under_generated_tree(page, docs_root) {
            report.orphans.push(display_relative(page, root));
        }
    }
    report
}

/// changes/ and specs/ belong to `rune spec`; decisions/ to `rune adr`.
/// Their members are reached through tooling, not page links.
fn under_generated_tree(page: &Path, docs_root: &Path) -> bool {
    let Ok(relative) = page.strip_prefix(docs_root) else {
        return false;
    };
    matches!(
        relative
            .components()
            .next()
            .and_then(|component| { component.as_os_str().to_str() }),
        Some("changes" | "specs" | "decisions" | "todos" | "plans")
    )
}

#[derive(Debug, PartialEq, Eq)]
enum LinkTarget {
    Relative(String),
    Wikilink(String),
}

fn link_targets(content: &str) -> Vec<LinkTarget> {
    let mut targets = Vec::new();
    let mut in_code_fence = false;
    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        let line = strip_inline_code(line);
        collect_inline_links(&line, &mut targets);
        collect_reference_definition(&line, &mut targets);
        collect_wikilinks(&line, &mut targets);
    }
    targets
}

/// Inline code spans carry syntax examples, not links; only text between
/// backtick spans participates in link collection.
fn strip_inline_code(line: &str) -> String {
    line.split('`')
        .enumerate()
        .filter_map(|(index, segment)| (index % 2 == 0).then_some(segment))
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_inline_links(line: &str, targets: &mut Vec<LinkTarget>) {
    let mut rest = line;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else { break };
        push_if_internal(&after[..close], targets);
        rest = &after[close + 1..];
    }
}

fn collect_reference_definition(line: &str, targets: &mut Vec<LinkTarget>) {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return;
    }
    if let Some(close) = trimmed.find("]:") {
        let destination = trimmed[close + 2..].trim();
        let destination = destination
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches('<')
            .trim_matches('>');
        push_if_internal(destination, targets);
    }
}

fn collect_wikilinks(line: &str, targets: &mut Vec<LinkTarget>) {
    let mut rest = line;
    while let Some(open) = rest.find("[[") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("]]") else { break };
        let inner = &after[..close];
        let name = inner.split('|').next().unwrap_or_default().trim();
        if !name.is_empty() {
            targets.push(LinkTarget::Wikilink(name.to_string()));
        }
        rest = &after[close + 2..];
    }
}

fn push_if_internal(destination: &str, targets: &mut Vec<LinkTarget>) {
    let destination = destination.split('#').next().unwrap_or_default().trim();
    if destination.is_empty()
        || destination.contains("://")
        || destination.starts_with("mailto:")
        || destination.starts_with("obsidian:")
    {
        return;
    }
    targets.push(LinkTarget::Relative(destination.to_string()));
}

fn decode_spaces(target: &str) -> String {
    target.replace("%20", " ")
}

fn display_relative(page: &Path, root: &Path) -> String {
    page.strip_prefix(root)
        .unwrap_or(page)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests;
