//! Per-provider deploy-set overlays: resolve `.rune` provider toggles into
//! one map the assemble loop consults, and answer whether one source file is
//! toggled off for one provider. Overlay entries are kind-qualified names
//! (`skills/Deslop`), so one name never collides across kinds.

use std::collections::{BTreeMap, BTreeSet};

use crate::cli::assemble::sources::SourceFile;
use crate::cli::dotrune::parse::DotRune;

/// Kind-qualified rune keys per provider, merged across every source.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ToggleMap {
    excluded: BTreeMap<String, BTreeSet<String>>,
    included: BTreeMap<String, BTreeSet<String>>,
}

/// Merge the per-provider overlays of every source into one map.
#[must_use]
pub fn toggle_map(manifest: &DotRune) -> ToggleMap {
    let mut map = ToggleMap::default();
    for list in manifest.runes.values() {
        for (provider, toggles) in &list.providers {
            let excluded = map.excluded.entry(provider.clone()).or_default();
            excluded.extend(toggles.exclude.iter().cloned());
            let included = map.included.entry(provider.clone()).or_default();
            included.extend(toggles.include.iter().cloned());
        }
    }
    map
}

/// True when this source file is toggled off for this provider.
/// An `include` entry restores a name on top of the exclusion.
#[must_use]
pub fn toggled_off(map: &ToggleMap, provider: &str, file: &SourceFile) -> bool {
    if let Some(canonical) = file.rune_id.as_deref()
        && toggled_off_key(map, provider, canonical)
    {
        return true;
    }
    let Some(key) = rune_key(file) else {
        return false;
    };
    toggled_off_key(map, provider, &key)
}

/// True when this kind-qualified key is toggled off for this provider.
#[must_use]
pub fn toggled_off_key(map: &ToggleMap, provider: &str, key: &str) -> bool {
    let excluded = map
        .excluded
        .get(provider)
        .is_some_and(|names| names.contains(key));
    if !excluded {
        return false;
    }
    !map.included
        .get(provider)
        .is_some_and(|names| names.contains(key))
}

/// The kind-qualified toggle key of one source file: the skill directory
/// name, or the file stem for agents, rules, and hooks.
#[must_use]
pub fn rune_key(file: &SourceFile) -> Option<String> {
    let mut segments = file.relative_path.split('/');
    let kind_segment = segments.next()?;
    if kind_segment != file.kind.as_str() {
        return None;
    }
    let name_segment = segments.next()?;
    let name = if kind_segment == "skills" {
        name_segment
    } else {
        name_segment.strip_suffix(".md").unwrap_or(name_segment)
    };
    if name.is_empty() {
        return None;
    }
    Some(format!("{kind_segment}/{name}"))
}

/// Apply one resolved toggle to `.rune` at `repo_root`. The caller resolves
/// the rune to one source label and one overlay key (the deck-canonical id,
/// or `kind/Name` for a module source). The write is surgical: only the
/// owned `providers:` block of that source and the `version:` line may
/// change, every other byte survives.
pub fn apply(
    repo_root: &std::path::Path,
    label: &str,
    key: &str,
    providers: &[String],
    off: bool,
) -> Result<Vec<String>, rune::error::Error> {
    use rune::error::{Error, ErrorKind};

    let manifest = super::load(repo_root)?.ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            format!("no .rune manifest at {}", repo_root.display()),
        )
        .with_code("toggle.no_manifest")
        .with_fix_command("rune add <id> --source <deck-path>")
    })?;
    let mut lists = manifest
        .runes
        .get(label)
        .cloned()
        .unwrap_or_default()
        .providers;
    for provider in providers {
        let entry = lists.entry(provider.clone()).or_default();
        entry.exclude.retain(|existing| existing != key);
        entry.include.retain(|existing| existing != key);
        if off {
            entry.exclude.push(key.to_string());
            entry.exclude.sort();
        }
    }
    lists.retain(|_, toggles| !toggles.is_empty());

    let path = repo_root.join(".rune");
    let content = std::fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    let mut updated = splice_providers(&content, label, &lists).map_err(|detail| {
        Error::new(ErrorKind::Parse, format!(".rune: {detail}"))
            .with_code("toggle.splice_failed")
            .with_fix_command("rune config check")
    })?;
    let any_toggles = !lists.is_empty()
        || manifest
            .runes
            .iter()
            .any(|(other, list)| other != label && !list.providers.is_empty());
    if any_toggles {
        updated = bump_version(&updated);
    }
    super::parse::parse(&updated)?;
    super::write_atomic_content(repo_root, &updated)?;

    let state = if off { "off" } else { "on" };
    Ok(providers
        .iter()
        .map(|provider| format!("{key} {state} for {provider}"))
        .collect())
}

/// Replace or insert the `providers:` block of one source entry, touching
/// no other line.
fn splice_providers(
    content: &str,
    label: &str,
    providers: &std::collections::BTreeMap<String, super::parse::ProviderToggles>,
) -> Result<String, String> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let indent_of = |line: &str| line.len() - line.trim_start().len();
    let is_blank = |line: &str| line.trim().is_empty();

    let runes_line = lines
        .iter()
        .position(|line| line.trim_end() == "runes:")
        .ok_or("no `runes:` section")?;
    let runes_indent = indent_of(lines[runes_line]);

    let is_comment = |line: &str| line.trim_start().starts_with('#');
    let mut entry_indent_level = None;
    let mut entry_line = None;
    for (index, line) in lines.iter().enumerate().skip(runes_line + 1) {
        if is_blank(line) || is_comment(line) {
            continue;
        }
        let indent = indent_of(line);
        if indent <= runes_indent {
            break;
        }
        let level = *entry_indent_level.get_or_insert(indent);
        if indent != level {
            continue;
        }
        if line.trim_end() == format!("{}{}:", " ".repeat(indent), label).trim_end()
            && line.trim_start().starts_with(&format!("{label}:"))
        {
            entry_line = Some((index, indent));
            break;
        }
    }
    let (entry_line, entry_indent) =
        entry_line.ok_or_else(|| format!("no `runes` entry for source '{label}'"))?;
    let unit = entry_indent - runes_indent;

    let mut entry_end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(entry_line + 1) {
        if !is_blank(line) && !is_comment(line) && indent_of(line) <= entry_indent {
            entry_end = index;
            break;
        }
    }

    let providers_indent = entry_indent + unit;
    let mut providers_span = None;
    for (index, line) in lines[entry_line + 1..entry_end].iter().enumerate() {
        let index = index + entry_line + 1;
        if !is_blank(line)
            && indent_of(line) == providers_indent
            && line.trim_end().trim_start() == "providers:"
        {
            let mut end = entry_end;
            for (inner, inner_line) in lines.iter().enumerate().skip(index + 1) {
                if inner >= entry_end {
                    break;
                }
                if !is_blank(inner_line)
                    && !is_comment(inner_line)
                    && indent_of(inner_line) <= providers_indent
                {
                    end = inner;
                    break;
                }
            }
            providers_span = Some((index, end));
            break;
        }
    }

    let rendered = render_providers(providers, providers_indent, unit);
    let mut output = String::new();
    let (head_end, tail_start) = if let Some((start, end)) = providers_span {
        (start, end)
    } else {
        let insert_at = last_content_line(&lines, entry_line + 1, entry_end);
        (insert_at, insert_at)
    };
    for line in &lines[..head_end] {
        output.push_str(line);
    }
    output.push_str(&rendered);
    for line in &lines[tail_start..] {
        output.push_str(line);
    }
    Ok(output)
}

fn last_content_line(lines: &[&str], start: usize, end: usize) -> usize {
    let mut insert_at = start;
    for (index, line) in lines.iter().enumerate().take(end).skip(start) {
        if !line.trim().is_empty() {
            insert_at = index + 1;
        }
    }
    insert_at
}

fn render_providers(
    providers: &std::collections::BTreeMap<String, super::parse::ProviderToggles>,
    indent: usize,
    unit: usize,
) -> String {
    use std::fmt::Write as _;
    if providers.is_empty() {
        return String::new();
    }
    let pad = " ".repeat(indent);
    let pad_provider = " ".repeat(indent + unit);
    let pad_list = " ".repeat(indent + 2 * unit);
    let mut rendered = format!("{pad}providers:\n");
    for (provider, toggles) in providers {
        let _ = writeln!(rendered, "{pad_provider}{provider}:");
        if !toggles.exclude.is_empty() {
            let _ = writeln!(
                rendered,
                "{pad_list}exclude: [{}]",
                toggles.exclude.join(", ")
            );
        }
        if !toggles.include.is_empty() {
            let _ = writeln!(
                rendered,
                "{pad_list}include: [{}]",
                toggles.include.join(", ")
            );
        }
    }
    rendered
}

/// Raise the `version:` line to 3 and keep any trailing comment.
fn bump_version(content: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::new();
    let mut bumped = false;
    for line in content.split_inclusive('\n') {
        if !bumped && line.starts_with("version:") {
            let comment = line.find('#').map(|at| line[at..].to_string());
            let newline = if line.ends_with('\n') { "\n" } else { "" };
            match comment {
                Some(comment) => {
                    let _ = write!(output, "version: 3 {comment}");
                }
                None => {
                    let _ = write!(output, "version: 3{newline}");
                }
            }
            bumped = true;
        } else {
            output.push_str(line);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::dotrune::parse;

    fn file(relative_path: &str, kind: rune::provider::ContentKind) -> SourceFile {
        SourceFile {
            relative_path: relative_path.to_string(),
            full_path: String::new(),
            content: String::new(),
            content_bytes: None,
            kind,
            passthrough: false,
            qualifier: None,
            targets: None,
            rune_id: None,
            providers: None,
            source_uri: None,
        }
    }

    fn manifest(yaml: &str) -> DotRune {
        parse::parse(yaml).expect("manifest parses")
    }

    const TOGGLED: &str = include_str!("../../../tests/fixtures/toggle/toggled.rune");

    #[test]
    fn excluded_file_is_off_for_the_named_provider_only() {
        let map = toggle_map(&manifest(TOGGLED));
        let skill = file(
            "skills/Deslop/SKILL.md",
            rune::provider::ContentKind::Skills,
        );
        assert!(toggled_off(&map, "claude", &skill));
        assert!(!toggled_off(&map, "codex", &skill));
    }

    #[test]
    fn include_restores_a_name_on_top_of_the_exclusion() {
        let map = toggle_map(&manifest(TOGGLED));
        let rule = file("rules/Style.md", rune::provider::ContentKind::Rules);
        assert!(!toggled_off(&map, "claude", &rule));
    }

    #[test]
    fn passthrough_files_inside_a_skill_share_its_key() {
        let map = toggle_map(&manifest(TOGGLED));
        let asset = file(
            "skills/Deslop/scripts/run.py",
            rune::provider::ContentKind::Skills,
        );
        assert!(toggled_off(&map, "claude", &asset));
    }

    #[test]
    fn toggles_require_schema_version_three() {
        let error = parse::parse(include_str!(
            "../../../tests/fixtures/toggle/version2-toggles.rune"
        ))
        .expect_err("version 2 must reject toggles");
        assert!(error.message().contains("version 3"));
    }
}
