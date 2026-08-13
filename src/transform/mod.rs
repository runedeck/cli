mod kebab_case;
mod links;
mod remap_tools;
mod to_toml;

pub use kebab_case::{to_kebab_case, to_kebab_path};
pub use links::rewrite_markdown_links;
pub use remap_tools::remap_tools;
pub use to_toml::markdown_to_toml;

use std::collections::HashMap;

use crate::provider::AssemblyRule;

/// Apply assembly rules to content and filename in order.
///
/// Returns the transformed `(content, filename)` pair.
///
/// Rules are applied sequentially:
///   - `KebabCase` — transforms filename from `PascalCase` to kebab-case
///   - `KebabCaseSkills` — the same transform, skills only, so a provider can
///     normalize skill trees while agents and rules keep their authored casing
///   - `RemapTools` — replaces tool names in backtick spans
///   - `AgentsToToml` — converts an agent's markdown to TOML and `.md` to `.toml`;
///     agents only, so skills (Codex reads `SKILL.md`) and rules stay markdown
pub fn apply_rules(
    content: &str,
    filename: &str,
    rules: &[AssemblyRule],
    tool_mappings: &HashMap<String, String>,
    kind: &str,
) -> Result<(String, String), String> {
    let mut current_content = content.to_string();
    let mut current_filename = filename.to_string();

    for rule in rules {
        match rule {
            AssemblyRule::KebabCase => {
                (current_content, current_filename) =
                    kebab_case_tree(current_content, &current_filename);
            }
            AssemblyRule::KebabCaseSkills => {
                if kind == "skills" {
                    (current_content, current_filename) =
                        kebab_case_tree(current_content, &current_filename);
                }
            }
            AssemblyRule::KebabCaseAgents => {
                if kind == "agents" {
                    let (stem, extension) = split_extension(&current_filename);
                    let kebab = to_kebab_case(&stem);
                    current_filename = format!("{kebab}{extension}");
                    current_content =
                        crate::assemble::map_field(&current_content, "name", to_kebab_case);
                }
            }
            AssemblyRule::RemapTools => {
                current_content = remap_tools(&current_content, tool_mappings);
            }
            AssemblyRule::AgentsToToml => {
                if kind == "agents" {
                    current_content = markdown_to_toml(&current_filename, &current_content)?;
                    let (stem, _) = split_extension(&current_filename);
                    current_filename = format!("{stem}.toml");
                }
            }
            AssemblyRule::StripLinks => {}
        }
    }

    Ok((current_content, current_filename))
}

/// Kebab-case a file's path and, for Markdown documents only, its frontmatter
/// name and link targets. Passthrough assets (scripts, templates, images)
/// follow their renamed directories but keep their bytes untouched: link
/// rewriting on non-Markdown content would mangle code like
/// `callbacks[0](arg_one)`.
fn kebab_case_tree(content: String, filename: &str) -> (String, String) {
    let renamed = to_kebab_path(filename);
    if !std::path::Path::new(&renamed)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        return (content, renamed);
    }
    let content = crate::assemble::map_field(&content, "name", to_kebab_case);
    (rewrite_markdown_links(&content, to_kebab_path), renamed)
}

/// Split a filename into stem and extension (including the dot).
///
/// `SecurityArchitect.md` → (`SecurityArchitect`, `.md`)
/// `no-extension`         → (`no-extension`, empty string)
fn split_extension(filename: &str) -> (String, String) {
    if let Some(dot_pos) = filename.rfind('.') {
        let stem = filename[..dot_pos].to_string();
        let extension = filename[dot_pos..].to_string();
        (stem, extension)
    } else {
        (filename.to_string(), String::new())
    }
}

#[cfg(test)]
mod links_tests;
#[cfg(test)]
mod tests;
