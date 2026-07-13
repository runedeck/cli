use commands::manifest;
use std::path::Path;

use super::ValidationReport;
use super::templates::InitTemplates;

pub fn check_template_drift(module_root: &Path, report: &mut ValidationReport) {
    let manifest_path = module_root.join(".manifest");
    if !manifest_path.is_file() {
        report.warn(
            ".manifest",
            ".manifest: missing — run rune init to establish baseline".to_string(),
        );
        return;
    }

    let manifest_content = match std::fs::read_to_string(&manifest_path) {
        Ok(content) => content,
        Err(error) => {
            report.warn(".manifest", format!(".manifest: cannot read: {error}"));
            return;
        }
    };

    let entries = match manifest::read(&manifest_content) {
        Ok(entries) => entries,
        Err(error) => {
            report.warn(".manifest", format!(".manifest: invalid format: {error}"));
            return;
        }
    };
    report.pass(".manifest");

    let module_name = resolve_module_name(module_root);

    let mut filenames: Vec<&String> = entries.keys().collect();
    filenames.sort();
    for filename in filenames {
        let target_path = module_root.join(filename);
        if !target_path.is_file() {
            report.warn(
                filename,
                format!("{filename}: tracked in manifest but missing"),
            );
            continue;
        }

        let Some(expected_hash) = template_hash(filename, &module_name) else {
            report.pass(filename);
            continue;
        };

        let Ok(content) = std::fs::read_to_string(&target_path) else {
            report.warn(filename, format!("{filename}: cannot read file"));
            continue;
        };

        let actual_hash = manifest::content_sha256(&content);
        if actual_hash == expected_hash {
            report.pass(filename);
        } else {
            report.warn(
                filename,
                format!("{filename}: drifted from current template"),
            );
        }
    }
}

fn template_hash(filename: &str, module_name: &str) -> Option<String> {
    let data = InitTemplates::get(filename)?;
    let template_content = std::str::from_utf8(data.data.as_ref()).ok()?;
    let content = super::templates::substitute(template_content, module_name);
    Some(manifest::content_sha256(&content))
}

fn resolve_module_name(module_root: &Path) -> String {
    module_root
        .canonicalize()
        .unwrap_or_else(|_| module_root.to_path_buf())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests;
