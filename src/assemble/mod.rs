mod frontmatter;
#[allow(clippy::implicit_hasher)]
pub mod pipeline;
pub mod references;
pub mod variants;

pub use frontmatter::{map_field, set_field, strip_frontmatter};
pub use references::{extract, strip};
pub use variants::{BodyMergeMode, MergedVariant, merge_into_base, resolve};

/// Restore a trailing newline that `.lines()` silently drops.
///
/// Call with the flag saved *before* iterating and the reconstructed output.
fn restore_trailing_newline(output: &mut String, had_newline: bool) {
    if had_newline && !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
}

/// Full assembly pipeline: resolve variant, merge, strip frontmatter, strip refs.
///
/// Steps:
///   1. If variant content is present, merge its frontmatter and body with source
///   2. Strip frontmatter (keeping only `keep_fields`)
///   3. Strip reference-style links
pub fn assemble(
    source_content: &str,
    variant_content: Option<&str>,
    keep_fields: &[&str],
    strip_links: bool,
) -> Result<String, String> {
    let merged = match variant_content {
        Some(variant_content) => merge_into_base(source_content, variant_content)?.content,
        None => source_content.to_string(),
    };

    let stripped = strip_frontmatter(&merged, keep_fields);
    if strip_links {
        Ok(strip(&stripped))
    } else {
        Ok(stripped)
    }
}

#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod tests;
