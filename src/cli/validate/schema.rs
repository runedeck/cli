use std::fs;
use std::path::{Path, PathBuf};

use super::templates;

const AGENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/agent.schema.yaml"
));
const SKILL_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/skill.schema.yaml"
));
const RULE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/rule.schema.yaml"
));
const MODULE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/module.schema.yaml"
));
const ADR_JSON_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/rune-adr.schema.json"
));

pub fn embedded_schema(kind: &str) -> Option<&'static str> {
    match kind {
        "agents" => Some(AGENT_SCHEMA),
        "skills" => Some(SKILL_SCHEMA),
        "rules" => Some(RULE_SCHEMA),
        "module" => Some(MODULE_SCHEMA),
        _ => None,
    }
}

/// Load `.schema.yaml` from a directory if present.
///
/// Provider-specific schema files define required frontmatter fields
/// and pattern constraints. For example, `agents/.schema.yaml` might
/// require `name` matching `PascalCase`:
///
/// ```yaml
/// required: [name, description]
/// properties:
///     name:
///         type: string
///         pattern: "^[A-Z][a-zA-Z0-9]{2,50}$"
/// ```
///
/// Returns `Ok(None)` when no `.schema.yaml` exists in the directory; an
/// unreadable schema is an error, because validating against the default
/// contract instead would silently pass the wrong checks.
pub fn load_schema(dir: &Path) -> Result<Option<String>, String> {
    let schema_path = dir.join(".schema.yaml");
    match fs::read_to_string(&schema_path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", schema_path.display())),
    }
}

/// Load `.mdschema` from a directory if present.
///
/// The `.mdschema` file defines structural constraints for markdown
/// files in the directory — required frontmatter fields, heading rules,
/// and section structure:
///
/// ```yaml
/// frontmatter:
///     fields:
///         - name: status
///           type: string
/// heading_rules:
///     no_skip_levels: true
///     max_depth: 3
/// ```
///
/// Returns `Ok(None)` when no `.mdschema` exists, `Err` on I/O errors.
pub fn load_mdschema(dir: &Path) -> Result<Option<String>, String> {
    let mdschema_path = dir.join(".mdschema");
    match fs::read_to_string(&mdschema_path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", mdschema_path.display())),
    }
}

/// An `.mdschema` resolved for a directory: its content plus the on-disk
/// path when it came from a file. Embedded-template fallbacks have no
/// path; dispatch writes their content to a private temporary file so the
/// standalone `mdschema` binary (which reads schemas from disk) still
/// checks them, and the built-in subset applies only when that write or
/// the binary itself is unavailable.
pub struct MdschemaSource {
    pub content: String,
    pub path: Option<PathBuf>,
}

/// Resolve an `.mdschema` for a file, searching `directories` in order
/// and falling back to the embedded template for the content kind.
///
/// Skill directories pass `[skill_dir, skills_kind_dir]` so a per-skill
/// schema wins over the kind-level one; flat directories pass just
/// themselves.
pub fn load_mdschema_or_fallback(
    directories: &[&Path],
    kind: &str,
) -> Result<Option<MdschemaSource>, String> {
    for directory in directories {
        if let Some(content) = load_mdschema(directory)? {
            return Ok(Some(MdschemaSource {
                content,
                path: Some(directory.join(".mdschema")),
            }));
        }
    }
    Ok(
        templates::embedded_mdschema(kind).map(|content| MdschemaSource {
            content,
            path: None,
        }),
    )
}

pub fn load_json_schema(directory: &Path) -> Result<String, String> {
    let schema_path = directory.join(".schema.json");
    match fs::read_to_string(&schema_path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ADR_JSON_SCHEMA.to_string())
        }
        Err(error) => Err(format!("cannot read {}: {error}", schema_path.display())),
    }
}
