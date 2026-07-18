//! `rune adr`: lifecycle for architecture decision records under
//! `docs/decisions/`, in the forge ADR schema. Ids are `<PREFIX>-<NNNN>`
//! with per-prefix numbering; the valid prefix set comes from `adr.prefixes`
//! in the merged config, falling back to the prefixes already present.

use commands::error::{Error, ErrorKind};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const TEMPLATE: &str = include_str!("template.md");

#[derive(Debug, Clone, clap::Subcommand)]
pub enum AdrAction {
    /// Scaffold the next record for a prefix.
    New {
        /// Decision title, e.g. "Launch Profile Composition".
        title: String,
        /// Id prefix (CLI, ARCH, ...). Must be in the configured set.
        #[arg(long)]
        prefix: String,
    },
    /// List records with status.
    List,
    /// Flip a superseded record and cross-link its replacement.
    Supersede {
        /// The record being superseded, by id (e.g. CLI-0018).
        old: String,
        /// The superseding record, by id.
        new: String,
    },
    /// Regenerate the decisions index table.
    Index,
}

#[derive(Debug)]
struct AdrRecord {
    id: String,
    title: String,
    status: String,
    path: PathBuf,
}

pub fn execute(action: AdrAction, json: bool) -> Result<i32, Error> {
    execute_at(Path::new("."), action, json)
}

fn decisions_dir(root: &Path) -> PathBuf {
    root.join("docs/decisions")
}

fn scan(root: &Path) -> Result<Vec<AdrRecord>, Error> {
    let dir = decisions_dir(root);
    let mut records = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(records);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some((id, title)) = name.split_once(' ') else {
            continue;
        };
        if !is_adr_id(id) {
            continue;
        }
        let content = std::fs::read_to_string(&path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", path.display()),
            )
        })?;
        let status = frontmatter_value(&content, "status").unwrap_or_else(|| "unknown".to_string());
        records.push(AdrRecord {
            id: id.to_string(),
            title: title.to_string(),
            status,
            path,
        });
    }
    records.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(records)
}

fn is_adr_id(candidate: &str) -> bool {
    match candidate.split_once('-') {
        Some((prefix, number)) => {
            !prefix.is_empty()
                && prefix.chars().all(|letter| letter.is_ascii_uppercase())
                && number.len() == 4
                && number.chars().all(|digit| digit.is_ascii_digit())
        }
        None => false,
    }
}

fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let (frontmatter, _) = commands::parse::split_frontmatter(content)?;
    for line in frontmatter.lines() {
        if let Some((candidate, value)) = line.split_once(':')
            && candidate.trim() == key
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn allowed_prefixes(root: &Path, records: &[AdrRecord]) -> Result<Vec<String>, Error> {
    let merged = crate::cli::config::load_merged_config(root)?;
    if let Some(configured) = commands::yaml::yaml_list(&merged, "adr.prefixes") {
        return Ok(configured
            .split(',')
            .map(|prefix| prefix.trim().to_string())
            .filter(|prefix| !prefix.is_empty())
            .collect());
    }
    let mut derived: Vec<String> = records
        .iter()
        .filter_map(|record| {
            record
                .id
                .split_once('-')
                .map(|(prefix, _)| prefix.to_string())
        })
        .collect();
    derived.sort();
    derived.dedup();
    Ok(derived)
}

pub fn execute_at(root: &Path, action: AdrAction, json: bool) -> Result<i32, Error> {
    match action {
        AdrAction::New { title, prefix } => new_record(root, &title, &prefix, json),
        AdrAction::List => list(root, json),
        AdrAction::Supersede { old, new } => supersede(root, &old, &new, json),
        AdrAction::Index => index(root, json),
    }
}

fn new_record(root: &Path, title: &str, prefix: &str, json: bool) -> Result<i32, Error> {
    if prefix.is_empty() || !prefix.chars().all(|letter| letter.is_ascii_uppercase()) {
        return Err(Error::new(
            ErrorKind::Config,
            format!("prefix must be uppercase letters only, got '{prefix}'"),
        ));
    }
    if title.is_empty()
        || title
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '"' | '|' | '\n' | '\r'))
        || title.starts_with('.')
    {
        return Err(Error::new(
            ErrorKind::Config,
            "title must be plain text (no slashes, quotes, pipes, or leading dots)".to_string(),
        ));
    }
    let records = scan(root)?;
    let allowed = allowed_prefixes(root, &records)?;
    if !allowed.is_empty() && !allowed.iter().any(|candidate| candidate == prefix) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "unknown prefix '{prefix}'; configured: {} (set adr.prefixes in config.yaml)",
                allowed.join(", ")
            ),
        ));
    }
    let next = records
        .iter()
        .filter_map(|record| {
            record
                .id
                .strip_prefix(&format!("{prefix}-"))
                .and_then(|number| number.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        + 1;
    let id = format!("{prefix}-{next:04}");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let content = TEMPLATE
        .replace("${ID}", &id)
        .replace("${TITLE}", title)
        .replace("${DATE}", &date);
    let dir = decisions_dir(root);
    std::fs::create_dir_all(&dir).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", dir.display()),
        )
    })?;
    let path = dir.join(format!("{id} {title}.md"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", path.display()),
            )
        })?;
    std::io::Write::write_all(&mut file, content.as_bytes()).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", path.display()),
        )
    })?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "id": id, "path": path.to_string_lossy() })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!("{}", sheet.ok(&format!("{id} → {}", path.display())));
    }
    Ok(0)
}

fn list(root: &Path, json: bool) -> Result<i32, Error> {
    let records = scan(root)?;
    if json {
        let rows: Vec<serde_json::Value> = records
            .iter()
            .map(|record| {
                serde_json::json!({
                    "id": record.id,
                    "title": record.title,
                    "status": record.status,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "decisions": rows }));
        return Ok(0);
    }
    let sheet = crate::cli::style::Sheet::detect(false);
    if records.is_empty() {
        println!("{}", sheet.dim("no decisions under docs/decisions/"));
        return Ok(0);
    }
    println!("{}", sheet.heading("decisions"));
    for record in records {
        let status = match record.status.as_str() {
            "accepted" => sheet.green(&record.status),
            "proposed" => sheet.yellow(&record.status),
            "superseded" | "deprecated" => sheet.dim(&record.status),
            other => sheet.dim(other),
        };
        println!(
            "   {} {status:<12} {}",
            sheet.bold(&format!("{:<10}", record.id)),
            record.title
        );
    }
    Ok(0)
}

fn supersede(root: &Path, old: &str, new: &str, json: bool) -> Result<i32, Error> {
    let records = scan(root)?;
    let find = |id: &str| {
        records
            .iter()
            .find(|record| record.id == id)
            .ok_or_else(|| Error::new(ErrorKind::Config, format!("no decision with id {id}")))
    };
    let old_record = find(old)?;
    let new_record = find(new)?;

    let old_stem = format!("{} {}", old_record.id, old_record.title);
    let new_stem = format!("{} {}", new_record.id, new_record.title);

    // Precompute both rewrites so a parse failure on either record aborts
    // before any file changes.
    let old_content = rewritten_frontmatter(&old_record.path, |frontmatter| {
        let updated = set_frontmatter_status(frontmatter, "superseded");
        append_related(&updated, &new_stem)
    })?;
    let new_content = rewritten_frontmatter(&new_record.path, |frontmatter| {
        append_related(frontmatter, &old_stem)
    })?;
    crate::cli::config::write_atomic_all(&[
        (&old_record.path, &old_content),
        (&new_record.path, &new_content),
    ])?;

    if json {
        println!("{}", serde_json::json!({ "superseded": old, "by": new }));
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!("{}", sheet.ok(&format!("{old} superseded by {new}")));
    }
    Ok(0)
}

fn rewritten_frontmatter(path: &Path, transform: impl Fn(&str) -> String) -> Result<String, Error> {
    let content = std::fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    let Some((frontmatter, body)) = commands::parse::split_frontmatter(&content) else {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} has no frontmatter", path.display()),
        ));
    };
    Ok(format!("---\n{}---\n{body}", transform(frontmatter)))
}

fn set_frontmatter_status(frontmatter: &str, status: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in frontmatter.lines() {
        // Only the top-level key: nested status fields keep their indentation.
        if line.starts_with("status:") {
            lines.push(format!("status: {status}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        lines.push(format!("status: {status}"));
    }
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

fn append_related(frontmatter: &str, stem: &str) -> String {
    if frontmatter.contains(stem) {
        let mut unchanged = frontmatter.to_string();
        if !unchanged.ends_with('\n') {
            unchanged.push('\n');
        }
        return unchanged;
    }
    let entry = format!("    - \"{stem}\"");
    let mut lines: Vec<String> = frontmatter.lines().map(str::to_string).collect();
    if let Some(position) = lines.iter().position(|line| line.starts_with("related:")) {
        let existing = lines[position].trim();
        if existing == "related: []" {
            lines[position] = "related:".to_string();
            lines.insert(position + 1, entry);
        } else if let Some(inline) = existing
            .strip_prefix("related: [")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            // Inline flow lists stay inline.
            lines[position] = format!("related: [{inline}, \"{stem}\"]");
        } else {
            lines.insert(position + 1, entry);
        }
    } else {
        lines.push("related:".to_string());
        lines.push(entry);
    }
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

fn index(root: &Path, json: bool) -> Result<i32, Error> {
    let records = scan(root)?;
    let mut table = String::from(
        "# Decisions\n\nGenerated by `rune adr index`.\n\n| Id | Title | Status |\n|----|-------|--------|\n",
    );
    for record in &records {
        let _ = writeln!(
            table,
            "| {} | [{}]({}) | {} |",
            record.id,
            record.title,
            urlencoded_basename(&record.path),
            record.status
        );
    }
    let path = decisions_dir(root).join("README.md");
    if path.is_file() {
        let existing = std::fs::read_to_string(&path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", path.display()),
            )
        })?;
        if !existing.contains("Generated by `rune adr index`") {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} exists and was not generated by rune adr index; move it aside first",
                    path.display()
                ),
            ));
        }
    }
    crate::cli::config::write_atomic(&path, &table)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "indexed": records.len(), "path": path.to_string_lossy() })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!("{}", sheet.ok(&format!("indexed → {}", path.display())));
    }
    Ok(0)
}

fn urlencoded_basename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().replace(' ', "%20"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
