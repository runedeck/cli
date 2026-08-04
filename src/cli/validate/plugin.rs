use rune::validate::{Diagnostic, validate_hooks_manifest, validate_json_manifest};
use std::fs;
use std::path::Path;

use super::ValidationReport;

/// Validate Claude Code plugin scaffolding, if present.
///
/// Runs only when `.claude-plugin/plugin.json` exists, so non-plugin modules
/// are unaffected. Checks the plugin manifest, the optional marketplace
/// manifest, and `hooks/hooks.json` (including that every referenced hook
/// script exists and is executable).
pub fn check_plugin_scaffolding(module_root: &Path, report: &mut ValidationReport) {
    let plugin_manifest = module_root.join(".claude-plugin/plugin.json");
    if !plugin_manifest.is_file() {
        return;
    }

    check_manifest(
        module_root,
        &plugin_manifest,
        report,
        validate_json_manifest,
    );

    let marketplace_manifest = module_root.join(".claude-plugin/marketplace.json");
    if marketplace_manifest.is_file() {
        check_manifest(
            module_root,
            &marketplace_manifest,
            report,
            validate_json_manifest,
        );
    }

    let hooks_manifest = module_root.join("hooks/hooks.json");
    if hooks_manifest.is_file() {
        check_hooks(module_root, &hooks_manifest, report);
    }
}

fn check_manifest(
    module_root: &Path,
    path: &Path,
    report: &mut ValidationReport,
    validator: impl Fn(&str, &str) -> Vec<Diagnostic>,
) {
    let display_path = relative_path(module_root, path);
    let checkpoint = report.checkpoint();
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(read_error) => {
            report
                .result
                .errors
                .push(format!("{display_path}: cannot read: {read_error}"));
            report.record_since(display_path, checkpoint);
            return;
        }
    };
    push_diagnostics(validator(&content, &display_path), &mut report.result);
    report.record_since(display_path, checkpoint);
}

fn check_hooks(module_root: &Path, hooks_manifest: &Path, report: &mut ValidationReport) {
    let display_path = relative_path(module_root, hooks_manifest);
    let checkpoint = report.checkpoint();
    let content = match fs::read_to_string(hooks_manifest) {
        Ok(content) => content,
        Err(read_error) => {
            report
                .result
                .errors
                .push(format!("{display_path}: cannot read: {read_error}"));
            report.record_since(display_path, checkpoint);
            return;
        }
    };

    let (diagnostics, scripts) = validate_hooks_manifest(&content, &display_path);
    push_diagnostics(diagnostics, &mut report.result);
    report.record_since(&display_path, checkpoint);

    for script in scripts {
        check_hook_script(module_root, &display_path, &script, report);
    }
}

fn check_hook_script(
    module_root: &Path,
    manifest_path: &str,
    script: &str,
    report: &mut ValidationReport,
) {
    let script_path = module_root.join(script);
    if !script_path.is_file() {
        report.fail(
            script,
            format!("{manifest_path}: hook script not found: {script}"),
        );
        return;
    }
    if is_executable(&script_path) {
        report.pass(script);
    } else {
        report.fail(
            script,
            format!("{manifest_path}: hook script is not executable (chmod +x): {script}"),
        );
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    // The executable bit is not meaningful on non-unix targets; treat the
    // script as runnable so validation does not flag a phantom problem.
    true
}

fn push_diagnostics(diagnostics: Vec<Diagnostic>, result: &mut rune::result::ActionResult) {
    for diagnostic in diagnostics {
        result.errors.push(format!(
            "{}: {} ({:?})",
            diagnostic.file, diagnostic.message, diagnostic.severity
        ));
    }
}

fn relative_path(module_root: &Path, path: &Path) -> String {
    path.strip_prefix(module_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
