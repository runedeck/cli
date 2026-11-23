use super::ManifestEntry;
use std::collections::HashMap;
use std::path::Path;

/// Deserialize a nested YAML manifest tree into a flat `HashMap`.
///
/// The nested structure:
/// ```yaml
/// rules:
///     cz:
///         PersonalTaxIncome.md:
///             fingerprint: abc123...
/// ```
///
/// Becomes flat key `rules/cz/PersonalTaxIncome.md` → `ManifestEntry`.
pub fn read(manifest_content: &str) -> Result<HashMap<String, ManifestEntry>, String> {
    let parsed: serde_yaml::Value = serde_yaml::from_str(manifest_content)
        .map_err(|error| format!("failed to parse manifest YAML: {error}"))?;

    let root = parsed
        .as_mapping()
        .ok_or_else(|| "manifest root must be a mapping".to_string())?;

    let mut entries = HashMap::new();
    flatten_tree(root, String::new(), &mut entries)?;
    Ok(entries)
}

/// Recursively walk the YAML tree, building flat keys from the path.
#[allow(clippy::needless_pass_by_value)]
fn flatten_tree(
    mapping: &serde_yaml::Mapping,
    prefix: String,
    entries: &mut HashMap<String, ManifestEntry>,
) -> Result<(), String> {
    for (key, value) in mapping {
        let Some(key_string) = key.as_str() else {
            continue;
        };
        // A manifest written on Windows with `\` between the parts of a key
        // names the same file as the `/` form. Elsewhere `\` is a character
        // of a name and stays.
        let key_string = super::slash_lossy(Path::new(key_string));

        let full_path = if prefix.is_empty() {
            key_string
        } else {
            format!("{prefix}/{key_string}")
        };

        let Some(child_map) = value.as_mapping() else {
            continue;
        };

        if child_map.contains_key("fingerprint") {
            let fingerprint = super::extract::string_field(value, "fingerprint", &full_path)?;
            let provenance = super::extract::string_field(value, "provenance", &full_path)
                .ok()
                .map(|path| super::slash_lossy(Path::new(&path)));

            entries.insert(
                full_path,
                ManifestEntry {
                    fingerprint,
                    provenance,
                },
            );
        } else {
            flatten_tree(child_map, full_path, entries)?;
        }
    }

    Ok(())
}
