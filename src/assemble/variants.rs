use serde_yaml::{Mapping, Value};
use std::path::{Path, PathBuf};

/// How a variant's body joins the base body.
///
/// Frontmatter does not use these. Variant keys always replace base keys; this
/// governs only the prose beneath the frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyMergeMode {
    Append,
    Prepend,
    Replace,
}

impl BodyMergeMode {
    fn from_frontmatter(value: Option<Value>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Replace);
        };
        let Value::String(value) = value else {
            return Err("variant frontmatter field 'mode' must be a string".to_string());
        };
        match value.as_str() {
            "append" => Ok(Self::Append),
            "prepend" => Ok(Self::Prepend),
            "replace" => Ok(Self::Replace),
            _ => Err(format!(
                "unknown variant mode '{value}'; expected append, prepend, or replace"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Prepend => "prepend",
            Self::Replace => "replace",
        }
    }
}

/// A base document with its variant merged in.
#[derive(Debug)]
pub struct MergedVariant {
    pub content: String,
    pub mode: BodyMergeMode,
}

/// Find the best variant file in qualifier directories.
///
/// Checks directories in precedence order:
///   `user/` > `provider/model/` > `provider/` > (none)
///
/// The `qualifiers` slice encodes the search path. Typical values:
///   `["user", "anthropic", "sonnet"]` checks `user/`, `anthropic/sonnet/`, `anthropic/`.
///
/// Returns the path to the first matching file, or `None`.
pub fn resolve(source_directory: &Path, filename: &str, qualifiers: &[String]) -> Option<PathBuf> {
    if qualifiers.iter().any(|qualifier| qualifier == "user") {
        let user_path = source_directory.join("user").join(filename);
        if user_path.is_file() {
            return Some(user_path);
        }
    }

    let non_user: Vec<&String> = qualifiers
        .iter()
        .filter(|qualifier| qualifier.as_str() != "user")
        .collect();

    if non_user.len() >= 2 {
        let provider = non_user[0];
        let model = non_user[1];
        let model_path = source_directory.join(provider).join(model).join(filename);
        if model_path.is_file() {
            return Some(model_path);
        }
    }

    if let Some(provider) = non_user.first() {
        let provider_path = source_directory.join(provider).join(filename);
        if provider_path.is_file() {
            return Some(provider_path);
        }
    }

    None
}

/// Merge a variant into its base document.
///
/// Frontmatter and body follow different rules. A variant frontmatter key
/// replaces the base key outright, nested values included, so this is not the
/// deep merge in `yaml::merge`. The body follows the variant's `mode`, which is
/// consumed here and never reaches the assembled output.
pub fn merge_into_base(base_content: &str, variant_content: &str) -> Result<MergedVariant, String> {
    let (mut base_frontmatter, base_body) = parse_document(base_content, "base")?;
    let (mut variant_frontmatter, variant_body) = parse_document(variant_content, "variant")?;
    let mode = BodyMergeMode::from_frontmatter(
        variant_frontmatter.remove(Value::String("mode".to_string())),
    )?;

    for (key, value) in variant_frontmatter {
        base_frontmatter.insert(key, value);
    }

    let body = merge_bodies(base_body, variant_body, mode);
    let content = serialize_document(&base_frontmatter, &body)?;

    Ok(MergedVariant { content, mode })
}

fn parse_document<'a>(content: &'a str, document_name: &str) -> Result<(Mapping, &'a str), String> {
    if !content.starts_with("---") {
        return Ok((Mapping::new(), content));
    }

    let (frontmatter, body) = crate::parse::split_frontmatter(content)
        .ok_or_else(|| format!("{document_name} frontmatter is not closed"))?;
    let mapping = if frontmatter.trim().is_empty() {
        Mapping::new()
    } else {
        serde_yaml::from_str(frontmatter)
            .map_err(|error| format!("cannot parse {document_name} frontmatter: {error}"))?
    };

    Ok((mapping, body))
}

fn merge_bodies(base_body: &str, variant_body: &str, mode: BodyMergeMode) -> String {
    match mode {
        BodyMergeMode::Append | BodyMergeMode::Prepend if variant_body.trim().is_empty() => {
            base_body.to_string()
        }
        BodyMergeMode::Append | BodyMergeMode::Prepend if base_body.trim().is_empty() => {
            variant_body.to_string()
        }
        BodyMergeMode::Append => format!("{base_body}\n{variant_body}"),
        BodyMergeMode::Prepend => format!("{variant_body}\n{base_body}"),
        BodyMergeMode::Replace => variant_body.to_string(),
    }
}

fn serialize_document(frontmatter: &Mapping, body: &str) -> Result<String, String> {
    if frontmatter.is_empty() {
        return Ok(body.to_string());
    }

    let serialized = serde_yaml::to_string(frontmatter)
        .map_err(|error| format!("cannot serialize merged frontmatter: {error}"))?;
    Ok(format!("---\n{serialized}---\n{body}"))
}
