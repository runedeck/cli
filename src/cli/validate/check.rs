use commands::error::{Error, ErrorKind};
use commands::validate;
use std::fs;
use std::path::Path;

use super::ValidationReport;
use super::schema;
use crate::cli::config::read_file;

/// Validate all .md files in a flat content directory (agents/ or rules/).
///
/// For each .md file found:
///   1. If a `.schema.yaml` exists in the directory, validates frontmatter
///      fields and patterns against it
///   2. If a `.mdschema` exists, checks heading structure and section
///      requirements
///
/// Diagnostics are appended to `result.errors` as formatted strings.
pub fn flat_directory(
    dir: &Path,
    module_root: &Path,
    kind: &str,
    report: &mut ValidationReport,
) -> Result<(), Error> {
    let schema_content =
        schema::load_schema(dir).or_else(|| schema::embedded_schema(kind).map(String::from));
    let mdschema_content = schema::load_mdschema_or_fallback(dir, kind)
        .map_err(|error| Error::new(ErrorKind::Io, error))?;

    let mut files = markdown_files(dir)?;
    let user_dir = dir.join("user");
    if user_dir.is_dir() {
        files.extend(markdown_files(&user_dir)?);
        files.sort();
    }

    for path in files {
        let content = read_file(&path)?;
        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let checkpoint = report.checkpoint();
        collect_diagnostics(
            &content,
            &relative,
            schema_content.as_ref(),
            mdschema_content.as_ref(),
            None,
            report,
        );
        report.record_since(relative, checkpoint);
    }

    Ok(())
}

pub fn flat_directory_with_json_schema(
    dir: &Path,
    module_root: &Path,
    kind: &str,
    json_schema_content: Option<&String>,
    report: &mut ValidationReport,
) -> Result<(), Error> {
    let schema_content =
        schema::load_schema(dir).or_else(|| schema::embedded_schema(kind).map(String::from));
    let mdschema_content = schema::load_mdschema_or_fallback(dir, kind)
        .map_err(|error| Error::new(ErrorKind::Io, error))?;

    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;
    entries.sort_by_key(std::fs::DirEntry::path);

    for entry in entries {
        let path = entry.path();
        if path.is_dir() || path.extension().unwrap_or_default() != "md" {
            continue;
        }

        let content = read_file(&path)?;
        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let checkpoint = report.checkpoint();
        collect_diagnostics(
            &content,
            &relative,
            schema_content.as_ref(),
            mdschema_content.as_ref(),
            json_schema_content,
            report,
        );
        report.record_since(relative, checkpoint);
    }

    Ok(())
}

/// Validate .md files inside a skill subdirectory.
///
/// Skill directories contain `SKILL.md` (the main skill definition) plus
/// optional companion files (examples, references). Each file in the
/// directory is checked against the local `.mdschema` if one exists.
///
/// Unlike flat directories, skill dirs don't check `.schema.yaml` for
/// frontmatter — the skill's own frontmatter format is self-describing.
///
/// ```text
/// skills/
///   Explain/
///     SKILL.md          ← checked against skills/Explain/.mdschema
///     examples.md       ← also checked
///     .mdschema         ← structural constraints for this skill
/// ```
pub fn skill_directory(
    dir: &Path,
    module_root: &Path,
    report: &mut ValidationReport,
) -> Result<(), Error> {
    let mdschema_content = schema::load_mdschema_or_fallback(dir, "skills")
        .map_err(|error| Error::new(ErrorKind::Io, error))?;

    // Only validate base and user-override SKILL.md files against the schema —
    // companions are reference docs without skill frontmatter.
    for skill_file in [dir.join("SKILL.md"), dir.join("user/SKILL.md")]
        .into_iter()
        .filter(|path| path.is_file())
    {
        let content = read_file(&skill_file)?;
        let display_path = skill_file
            .strip_prefix(module_root)
            .unwrap_or(&skill_file)
            .to_string_lossy()
            .to_string();

        let skill_schema = schema::embedded_schema("skills").map(String::from);

        let checkpoint = report.checkpoint();
        collect_diagnostics(
            &content,
            &display_path,
            skill_schema.as_ref(),
            mdschema_content.as_ref(),
            None,
            report,
        );
        lint_skill(&content, dir, &display_path, report);
        report.record_since(display_path, checkpoint);
    }

    Ok(())
}

/// Warning-severity conformance lint for a SKILL.md: agentskills.io limits
/// plus trigger-phrase and reserved-name hygiene. Warnings inform; only
/// schema errors block.
fn lint_skill(content: &str, dir: &Path, display_path: &str, report: &mut ValidationReport) {
    let mut warn = |message: String| {
        report
            .result
            .warnings
            .push(format!("{display_path}: {message}"));
    };

    let name = commands::parse::frontmatter_value(content, "name").unwrap_or_default();
    let description =
        commands::parse::frontmatter_value(content, "description").unwrap_or_default();
    let directory_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if !name.is_empty() && name != directory_name {
        warn(format!(
            "skill name '{name}' does not match its directory '{directory_name}' (agentskills.io requires them equal)"
        ));
    }
    if name.len() > 64 {
        warn(format!(
            "skill name is {} characters; agentskills.io allows at most 64",
            name.len()
        ));
    }
    if description.len() > 1024 {
        warn(format!(
            "description is {} characters; agentskills.io allows at most 1024",
            description.len()
        ));
    }
    for reserved in ["claude", "anthropic"] {
        if name.to_lowercase().contains(reserved) {
            warn(format!(
                "skill name contains the reserved word '{reserved}'; harnesses reject or shadow such names"
            ));
        }
    }
    // A lone comparison (`count > 0`) is fine; a matched pair reads as an
    // XML-like tag, which breaks harness prompt assembly.
    let looks_tagged =
        |text: &str| text.contains('<') && text[text.find('<').unwrap_or(0)..].contains('>');
    if looks_tagged(&name) || looks_tagged(&description) {
        warn("frontmatter name or description contains an angle-bracket pair; XML-like text breaks harness prompt assembly".to_string());
    }
    let description_lower = description.to_lowercase();
    if !description.is_empty()
        && !["use when", " when ", "invoke", "use for", "use this"]
            .iter()
            .any(|phrase| description_lower.contains(phrase))
    {
        warn(
            "description has no trigger phrasing (e.g. 'USE WHEN …'); the model cannot tell when to invoke this skill"
                .to_string(),
        );
    }
    let body = commands::parse::frontmatter_body(content).trim();
    if body.len() < 50 {
        warn(format!(
            "skill body is {} characters; too short to instruct anything",
            body.len()
        ));
    }
}

fn markdown_files(directory: &Path) -> Result<Vec<std::path::PathBuf>, Error> {
    let entries = fs::read_dir(directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", directory.display()),
        )
    })?;
    let mut files = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::new(ErrorKind::Io, format!("directory entry error: {error}")))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|extension| extension == "md")
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

/// Run schema and mdschema validation, appending any diagnostics to the result.
fn collect_diagnostics(
    content: &str,
    file_path: &str,
    schema_content: Option<&String>,
    mdschema_content: Option<&String>,
    json_schema_content: Option<&String>,
    report: &mut ValidationReport,
) {
    if let Some(schema) = schema_content {
        let diagnostics = validate::validate_frontmatter(content, schema, file_path);
        for diag in diagnostics {
            report.diagnostic(&diag);
        }
    }

    if let Some(mdschema) = mdschema_content {
        let diagnostics = validate::mdschema::check(content, file_path, mdschema);
        for diag in diagnostics {
            report.diagnostic(&diag);
        }
    }

    if let Some(json_schema) = json_schema_content {
        let diagnostics =
            validate::validate_frontmatter_against_json_schema(content, json_schema, file_path);
        for diag in diagnostics {
            report.diagnostic(&diag);
        }
    }
}
