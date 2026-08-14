use rune::error::Error;
use rune::validate;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use tempfile::TempDir;

use super::ValidationReport;
use super::mdschema_tool;
use super::schema;
use crate::cli::{
    assemble::sources,
    config::{self, read_file},
};

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
    let schema_content = schema::load_schema(dir)
        .map_err(Error::io)?
        .or_else(|| schema::embedded_schema(kind).map(String::from));
    let mdschema_source = schema::load_mdschema_or_fallback(&[dir], kind).map_err(Error::io)?;

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
        collect_diagnostics(&content, &relative, schema_content.as_ref(), None, report);
        check_mdschema(&content, &path, &relative, mdschema_source.as_ref(), report);
        check_no_tables(&content, &relative, report);
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
    let schema_content = schema::load_schema(dir)
        .map_err(Error::io)?
        .or_else(|| schema::embedded_schema(kind).map(String::from));
    let mdschema_source = schema::load_mdschema_or_fallback(&[dir], kind).map_err(Error::io)?;

    let entries = fs::read_dir(dir)
        .map_err(|error| Error::io(format!("cannot read {}: {error}", dir.display())))?;
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::io(format!("directory entry error: {error}")))?;
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
            json_schema_content,
            report,
        );
        check_mdschema(&content, &path, &relative, mdschema_source.as_ref(), report);
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
/// Unlike flat directories, skill dirs validate canonical frontmatter against
/// the embedded Agent Skills schema.
///
/// ```text
/// skills/
///   explain/
///     SKILL.md          ← checked against skills/explain/.mdschema
///     examples.md       ← also checked
///     .mdschema         ← structural constraints for this skill
/// ```
pub fn skill_directory(
    dir: &Path,
    module_root: &Path,
    report: &mut ValidationReport,
) -> Result<(), Error> {
    // Per-skill `.mdschema` wins; the kind-level `skills/.mdschema` covers
    // every skill without one; the embedded template is the last resort.
    let mut search_directories = vec![dir];
    if let Some(kind_directory) = dir.parent() {
        search_directories.push(kind_directory);
    }
    let mdschema_source =
        schema::load_mdschema_or_fallback(&search_directories, "skills").map_err(Error::io)?;

    let base_skill_file = dir.join("SKILL.md");
    if base_skill_file.is_file() {
        let base_content = read_file(&base_skill_file)?;
        let display_path = relative_display_path(&base_skill_file, module_root);
        let skill_schema = schema::embedded_schema("skills").map(String::from);
        let checkpoint = report.checkpoint();
        collect_diagnostics(
            &base_content,
            &display_path,
            None,
            skill_schema.as_ref(),
            report,
        );
        check_mdschema(
            &base_content,
            &base_skill_file,
            &display_path,
            mdschema_source.as_ref(),
            report,
        );
        lint_skill(&base_content, dir, &display_path, report);
        report.record_since(display_path, checkpoint);

        for variant_file in skill_variant_files(dir, module_root)? {
            let variant_content = read_file(&variant_file)?;
            let display_path = relative_display_path(&variant_file, module_root);
            let checkpoint = report.checkpoint();
            match rune::assemble::variants::merge_into_base(&base_content, &variant_content) {
                Ok(merged) => {
                    let merged_file = tempfile::NamedTempFile::new().map_err(|error| {
                        Error::io(format!("cannot create merged skill file: {error}"))
                    })?;
                    std::fs::write(merged_file.path(), &merged.content).map_err(|error| {
                        Error::io(format!("cannot write merged skill file: {error}"))
                    })?;
                    check_mdschema(
                        &merged.content,
                        merged_file.path(),
                        &display_path,
                        mdschema_source.as_ref(),
                        report,
                    );
                    lint_skill(&merged.content, dir, &display_path, report);
                    report.record_since(display_path, checkpoint);
                }
                Err(error) => report.fail(
                    display_path.clone(),
                    format!("{display_path}: cannot merge skill variant: {error}"),
                ),
            }
        }
    }

    // Companions are prose the deck ships too: their tables obey the same
    // alignment contract as the entrypoint's.
    for companion in markdown_files(dir)? {
        let content = read_file(&companion)?;
        let display_path = companion
            .strip_prefix(module_root)
            .unwrap_or(&companion)
            .to_string_lossy()
            .to_string();
        let checkpoint = report.checkpoint();
        check_no_tables(&content, &display_path, report);
        report.record_since(display_path, checkpoint);
    }

    Ok(())
}

fn relative_display_path(path: &Path, module_root: &Path) -> String {
    path.strip_prefix(module_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn skill_variant_files(
    skill_directory: &Path,
    module_root: &Path,
) -> Result<Vec<std::path::PathBuf>, Error> {
    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let models = config::load_models(module_root);
    let provider_names = providers.keys().cloned().collect::<Vec<_>>();
    let mut qualifier_names = sources::build_valid_qualifiers(&provider_names, &models)
        .into_iter()
        .collect::<Vec<_>>();
    qualifier_names.sort();

    let mut variant_files = Vec::new();
    let user_variant = skill_directory.join("user/SKILL.md");
    if user_variant.is_file() {
        variant_files.push(user_variant);
    }

    for provider_name in &qualifier_names {
        let provider_directory = skill_directory.join(provider_name);
        let provider_variant = provider_directory.join("SKILL.md");
        if provider_variant.is_file() {
            variant_files.push(provider_variant);
        }
        if !provider_directory.is_dir() {
            continue;
        }
        for model_name in &qualifier_names {
            let model_variant = provider_directory.join(model_name).join("SKILL.md");
            if model_variant.is_file() {
                variant_files.push(model_variant);
            }
        }
    }

    variant_files.sort();
    variant_files.dedup();
    Ok(variant_files)
}

/// Validate Stable shell identity, then report advisory Agent Skills and authoring findings.
///
/// Stable shell is the heading convention. `RuneShell` is the deck rule that
/// distributes it. Diagnostics name the convention, never the rule.
fn lint_skill(content: &str, dir: &Path, display_path: &str, report: &mut ValidationReport) {
    let name = rune::parse::frontmatter_value(content, "name").unwrap_or_default();
    let description = rune::parse::frontmatter_value(content, "description").unwrap_or_default();
    let directory_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    for diagnostic in validate::skill::check(&name, directory_name, content, display_path) {
        report.diagnostic(&diagnostic);
    }

    let mut warn = |message: String| {
        report.diagnostic(&validate::Diagnostic {
            file: display_path.to_string(),
            line: None,
            severity: validate::Severity::Warning,
            message,
        });
    };

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
    // A skill the model never loads on its own has no trigger to phrase: the
    // harness keeps its description out of the skill listing, so the text only
    // names the skill for whoever types its slash command.
    let model_invocable = rune::parse::frontmatter_value(content, "disable-model-invocation")
        .is_none_or(|value| !value.trim().eq_ignore_ascii_case("true"));
    let description_lower = description.to_lowercase();
    if model_invocable
        && !description.is_empty()
        && !["use when", " when ", "invoke", "use for", "use this"]
            .iter()
            .any(|phrase| description_lower.contains(phrase))
    {
        warn(
            "description has no trigger phrasing (e.g. 'USE WHEN …'); the model cannot tell when to invoke this skill"
                .to_string(),
        );
    }
    let body = rune::parse::frontmatter_body(content).trim();
    if body.len() < 50 {
        warn(format!(
            "skill body is {} characters; too short to instruct anything",
            body.len()
        ));
    }
    let body_lines = body.lines().count();
    if body_lines > 100 {
        warn(format!(
            "skill body is {body_lines} lines; the deck standard advises 100 (150 is the hard cut) — extract companions"
        ));
    }
}

pub(crate) fn check_no_tables(content: &str, display_path: &str, report: &mut ValidationReport) {
    for diagnostic in validate::skill::check_no_tables(content, display_path) {
        report.diagnostic(&diagnostic);
    }
}

fn markdown_files(directory: &Path) -> Result<Vec<std::path::PathBuf>, Error> {
    let entries = fs::read_dir(directory)
        .map_err(|error| Error::io(format!("cannot read {}: {error}", directory.display())))?;
    let mut files = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::io(format!("directory entry error: {error}")))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|extension| extension == "md")
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

/// Run frontmatter schema validation, appending any diagnostics to the result.
fn collect_diagnostics(
    content: &str,
    file_path: &str,
    schema_content: Option<&String>,
    json_schema_content: Option<&String>,
    report: &mut ValidationReport,
) {
    if let Some(schema) = schema_content {
        let diagnostics = validate::validate_frontmatter(content, schema, file_path);
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

/// Write an embedded schema to a file so the standalone checker, which only
/// reads files, can use it.
///
/// The file goes in a private directory this process creates, not at a
/// predictable path in the shared temporary directory. Shipped schemas have
/// public digests, so a predictable name would let another user on the machine
/// pre-create the file, or a symlink standing in for it, and choose what this
/// process validates against. `tempfile` creates the directory with owner-only
/// access and a name nobody can guess.
///
/// The directory outlives every path handed to a child process because it is
/// held in a `static`, which also means its destructor never runs: the
/// directory survives until the operating system clears its temporary
/// directory. That is the trade for handing stable paths to child processes
/// without tracking their lifetimes.
///
/// The file name carries the content digest, so repeat calls within a run share
/// one file and an edited schema lands on a new one. Returns `None` when the
/// directory or the write fails, which routes the caller to the built-in subset
/// rather than failing validation over a temporary file.
fn materialize_schema(content: &str) -> Option<std::path::PathBuf> {
    static SCHEMA_DIRECTORY: OnceLock<Option<TempDir>> = OnceLock::new();

    let directory = SCHEMA_DIRECTORY
        .get_or_init(
            || match tempfile::Builder::new().prefix("rune-schema-").tempdir() {
                Ok(directory) => Some(directory),
                Err(error) => {
                    eprintln!("warning: cannot create a private schema directory ({error})");
                    None
                }
            },
        )
        .as_ref()?;

    let digest = rune::manifest::content_sha256(content);
    let path = directory.path().join(format!("{digest}.mdschema"));
    if path.is_file() {
        return Some(path);
    }

    // Write beside the target and rename into place. A direct write that fails
    // partway leaves a truncated file that the next call would find with
    // `is_file` and hand to the checker, which would then report a clean run
    // against a broken schema. Rename is atomic within one directory, so the
    // target either does not exist or is complete.
    let staging = path.with_extension("mdschema.partial");
    if let Err(error) = std::fs::write(&staging, content) {
        eprintln!("warning: could not write {} ({error})", staging.display());
        let _ = std::fs::remove_file(&staging);
        return None;
    }
    match std::fs::rename(&staging, &path) {
        Ok(()) => Some(path),
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            // Parallel callers share the staging name, so the loser's rename
            // can fail after the winner consumed it. The target the winner
            // placed is complete and identical, so losing the race is not
            // losing the file.
            if path.is_file() {
                return Some(path);
            }
            eprintln!("warning: could not place {} ({error})", path.display());
            None
        }
    }
}

/// Check a file against its resolved `.mdschema`.
///
/// The standalone `mdschema` binary enforces the schema's full vocabulary and
/// supersedes the built-in subset whenever it is on PATH. A schema already on
/// disk is passed straight through; an embedded-template schema is written out
/// first, so shipping a schema inside the binary no longer costs strict
/// checking. The built-in subset runs only when the binary is absent or the
/// write fails, and it covers field presence, heading discipline, and required
/// sections, and nothing else.
fn check_mdschema(
    content: &str,
    file_path: &Path,
    display_path: &str,
    mdschema_source: Option<&schema::MdschemaSource>,
    report: &mut ValidationReport,
) {
    check_mdschema_with_availability(
        content,
        file_path,
        display_path,
        mdschema_source,
        mdschema_tool::available(),
        report,
    );
}

pub(super) fn check_mdschema_with_availability(
    content: &str,
    file_path: &Path,
    display_path: &str,
    mdschema_source: Option<&schema::MdschemaSource>,
    standalone_available: bool,
    report: &mut ValidationReport,
) {
    let Some(source) = mdschema_source else {
        return;
    };

    if standalone_available {
        if let Some(schema_path) = source.path.as_ref() {
            mdschema_tool::check_file(schema_path, file_path, display_path, report);
            return;
        }
        // An embedded schema has no path only because it ships inside the
        // binary. Writing it out once earns strict checking for modules that
        // never authored their own `.mdschema`, which is most of them.
        if let Some(materialized) = materialize_schema(&source.content) {
            mdschema_tool::check_file(&materialized, file_path, display_path, report);
            return;
        }
        // The binary is installed; the temporary file is what failed. Saying
        // the checker is unavailable would send the reader to install it again.
        report.report_unusable_standalone_checker();
    } else {
        report.report_missing_standalone_checker();
    }

    let diagnostics = validate::mdschema::check(content, display_path, &source.content);
    for diagnostic in diagnostics {
        report.diagnostic(&diagnostic);
    }
}
