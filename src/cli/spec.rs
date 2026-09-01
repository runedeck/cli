//! `rune spec` adapter over the rune-docs crate: installs the config hooks the
//! crate cannot own, bridges the `mdschema` validator, converts errors at the
//! boundary, and keeps the one interactive piece (the `OpenSpec` root choice)
//! on the CLI side where config writes belong.

use rune::error::{Error, ErrorKind};

use crate::cli::docs_boundary::convert;
use std::path::Path;

use rune_docs::spec::MdschemaDiagnostic;
pub(crate) use rune_docs::spec::{DiagnosticSeverity, ListSort, SpecViolation};

/// Install the crate hooks once per process: `spec.root` comes from the
/// repo's merged config, exactly as the CLI resolves every other key.
pub(crate) fn install_hooks() {
    let _ = rune_docs::spec::set_root_config_lookup(|root| {
        crate::cli::config::load_merged_config(root)
            .map(|merged| crate::cli::config::source_spec_root(&merged))
            .map_err(|error| format!("cannot read config for spec.root: {error}"))
    });
    rune_docs::sheet::set_no_color(crate::cli::style::global_no_color());
}

pub(crate) fn propose(
    source: &str,
    change_id: &str,
    capabilities: &[String],
    design: bool,
    json: bool,
) -> Result<i32, Error> {
    rune_docs::spec::propose(source, change_id, capabilities, design, json)
        .map_err(|error| convert(&error))
}

pub(crate) fn list(source: &str, specs: bool, sort: ListSort, json: bool) -> Result<i32, Error> {
    rune_docs::spec::list(source, specs, sort, json).map_err(|error| convert(&error))
}

pub(crate) fn show(source: &str, name: &str, json: bool) -> Result<i32, Error> {
    rune_docs::spec::show(source, name, json).map_err(|error| convert(&error))
}

pub(crate) fn context(source: &str, id: &str, json: bool) -> Result<i32, Error> {
    rune_docs::spec::context(source, id, json).map_err(|error| convert(&error))
}

pub(crate) fn doctor(source: &str, json: bool) -> Result<i32, Error> {
    rune_docs::spec::doctor(source, json).map_err(|error| convert(&error))
}

pub(crate) fn validate(source: &str, name: Option<&str>, json: bool) -> Result<i32, Error> {
    rune_docs::spec::validate(source, name, json, mdschema_bridge).map_err(|error| convert(&error))
}

pub(crate) fn archive(
    source: &str,
    id: &str,
    yes: bool,
    abandon: bool,
    json: bool,
) -> Result<i32, Error> {
    rune_docs::spec::archive(source, id, yes, abandon, json).map_err(|error| convert(&error))
}

pub(crate) fn scan_changes(root: &Path) -> Result<Vec<rune_docs::spec::ChangeSummary>, Error> {
    rune_docs::spec::scan_changes(root).map_err(|error| convert(&error))
}

pub(crate) fn scan_specifications(
    root: &Path,
) -> Result<Vec<rune_docs::spec::SpecificationSummary>, Error> {
    rune_docs::spec::scan_specifications(root).map_err(|error| convert(&error))
}

/// Validate canonical specs and active deltas with the CLI's mdschema engine.
pub(crate) fn validate_spec_tree(root: &Path) -> Result<Vec<SpecViolation>, Error> {
    rune_docs::spec::validate_spec_tree(root, mdschema_bridge).map_err(|error| convert(&error))
}

fn mdschema_bridge(content: &str, file_path: &str, schema: &str) -> Vec<MdschemaDiagnostic> {
    rune::validate::mdschema::check(content, file_path, schema)
        .into_iter()
        .map(|diagnostic| MdschemaDiagnostic {
            file: diagnostic.file,
            line: diagnostic.line,
            severity: match diagnostic.severity {
                rune::validate::Severity::Error => DiagnosticSeverity::Error,
                rune::validate::Severity::Warning => DiagnosticSeverity::Warning,
            },
            message: diagnostic.message,
        })
        .collect()
}

/// One-time root choice for a repo that already uses `OpenSpec`: an
/// `openspec/` tree, no native `docs/` tree, and no configured `spec.root`
/// gets a single-choice offer: keep the openspec layout (persisted to the
/// repo's `config.yaml`, rune operates on it natively) or migrate to `docs/`
/// via the existing converter. Non-interactive runs keep the silent
/// autodetect and print one note naming the config key, so scripts never
/// block.
pub(crate) fn offer_root_choice(source: &str, json: bool) -> Result<(), Error> {
    use std::io::IsTerminal as _;
    use std::io::Write as _;

    let root = Path::new(source);
    // An unreadable config is not an unset one: skip the offer and let the
    // command itself surface the load error.
    let Ok(merged) = crate::cli::config::load_merged_config(root) else {
        return Ok(());
    };
    if crate::cli::config::source_spec_root(&merged).is_some() {
        return Ok(());
    }
    let native =
        root.join("docs").join("changes").is_dir() || root.join("docs").join("specs").is_dir();
    let openspec = root.join("openspec").join("changes").is_dir()
        || root.join("openspec").join("specs").is_dir();
    if native || !openspec {
        return Ok(());
    }

    if json || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        eprintln!(
            "note: operating on the openspec/ tree; persist with `spec.root: openspec` in config.yaml, or migrate with `rune spec import --openspec`"
        );
        return Ok(());
    }

    println!("This repository uses OpenSpec (openspec/). How should rune spec operate?");
    println!("  1) Keep the openspec layout: rune works on openspec/ natively [default]");
    println!("  2) Migrate to docs/: moves openspec/changes and openspec/specs under docs/");
    print!("choice [1/2]: ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    match std::io::stdin().read_line(&mut answer) {
        // EOF is not an answer: record nothing.
        Ok(0) | Err(_) => return Ok(()),
        Ok(_) => {}
    }
    let choice = answer.trim();
    if !matches!(choice, "" | "1" | "2") {
        println!("unrecognized choice '{choice}'; nothing recorded");
        return Ok(());
    }

    let config_path = root.join("config.yaml");
    let config_is_symlink = config_path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_symlink());
    if config_is_symlink {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{} is a symlink; refusing to write through it",
                config_path.display()
            ),
        ));
    }
    apply_root_choice(root, source, choice, json)
}

fn apply_root_choice(root: &Path, source: &str, choice: &str, json: bool) -> Result<(), Error> {
    let config_path = root.join("config.yaml");
    if choice == "2" {
        crate::cli::ontology::set_nested_in_file(&config_path, "spec", "root", "docs")
            .map_err(|error| Error::new(ErrorKind::Config, error))?;
        crate::cli::spec_interop::import_openspec(source, json)?;
        println!("spec.root: docs recorded in {}", config_path.display());
    } else {
        crate::cli::ontology::set_nested_in_file(&config_path, "spec", "root", "openspec")
            .map_err(|error| Error::new(ErrorKind::Config, error))?;
        println!(
            "spec.root: openspec recorded in {}; rune spec operates on openspec/ natively",
            config_path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
