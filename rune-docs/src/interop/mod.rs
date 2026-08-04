//! `OpenSpec` interop preserves the complete `openspec/` file set while the
//! selected spec root owns direct changes and specifications.
//!
//! Module map, by conversion stage: [`manifest`] owns the typed ownership
//! record and its hashes, [`preflight`] plans every write and removal
//! before anything moves, [`walk`] enumerates and classifies files, and
//! [`verify`] proves owned bytes match the manifest. This file keeps the
//! public conversion surface, the artifact classification, and the report
//! the CLI renders.

mod manifest;
mod preflight;
mod verify;
mod walk;

use crate::error::{Error, ErrorKind};
use crate::spec::transaction::{self, Operation, SystemIo, Transaction, TransactionIo};
use crate::spec::{SpecRoot, resolve_spec_root};
#[cfg(test)]
use manifest::{MANIFEST_VERSION, Manifest, ManifestEntry, serialize_manifest, validate_manifest};
use manifest::{load_manifest, manifest_path};
use preflight::{preflight_export, preflight_import};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::io;
use std::path::Path;
use verify::{verify_exported_manifest, verify_imported_manifest};
use walk::{classify_path, is_reserved_state_path, openspec_root, walk_regular_files};

use transaction::{LOCK_FILE, TRANSACTION_DIRECTORY};

/// Interop-owned on-disk names; the transaction state names above come from
/// the transaction layer, which owns them for both modules.
const INTEROP_DIRECTORY: &str = ".interop/openspec";
const MIRROR_DIRECTORY: &str = ".interop/openspec/files";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Classification {
    Change,
    Specification,
    File,
}

impl Classification {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Change => "change",
            Self::Specification => "specification",
            Self::File => "file",
        }
    }
}

pub(crate) struct OwnedArtifact {
    pub(crate) path: String,
    pub(crate) classification: Classification,
}

pub fn export_openspec(source: &str, json: bool) -> Result<i32, Error> {
    let report = export_openspec_with_io(source, SystemIo)?;
    print_report(&report, json)?;
    Ok(0)
}

pub fn import_openspec(source: &str, json: bool) -> Result<i32, Error> {
    let report = import_openspec_with_io(source, None, SystemIo)?;
    print_report(&report, json)?;
    Ok(0)
}

/// Import into an explicitly named `spec.root`, bypassing the installed
/// config lookup. For the migration flow that records the root only after
/// the import succeeds.
pub fn import_openspec_into(source: &str, spec_root: &str, json: bool) -> Result<i32, Error> {
    let report = import_openspec_with_io(source, Some(spec_root), SystemIo)?;
    print_report(&report, json)?;
    Ok(0)
}

/// Export and return the conversion report, without printing.
pub fn export_openspec_output(source: &str) -> Result<ConversionReport, Error> {
    export_openspec_with_io(source, SystemIo)
}

/// Import and return the conversion report, without printing.
pub fn import_openspec_output(source: &str) -> Result<ConversionReport, Error> {
    import_openspec_with_io(source, None, SystemIo)
}

/// Import into an explicitly named `spec.root` and return the conversion
/// report, without printing.
pub fn import_openspec_into_output(
    source: &str,
    spec_root: &str,
) -> Result<ConversionReport, Error> {
    import_openspec_with_io(source, Some(spec_root), SystemIo)
}

pub(crate) fn opaque_artifacts(spec_root: &SpecRoot) -> Result<Vec<OwnedArtifact>, Error> {
    let manifest = manifest_path(spec_root);
    if manifest.is_file() {
        return Ok(load_manifest(spec_root)?
            .entries
            .into_iter()
            .filter(|entry| entry.classification == Classification::File)
            .map(|entry| OwnedArtifact {
                path: entry.path,
                classification: entry.classification,
            })
            .collect());
    }
    if spec_root.layout() != crate::spec::SpecLayout::OpenSpec || !spec_root.base().exists() {
        return Ok(Vec::new());
    }
    Ok(walk_regular_files(spec_root.base())?
        .files
        .into_iter()
        .filter(|(path, _)| path != LOCK_FILE && !is_reserved_state_path(path))
        .filter(|(path, _)| classify_path(path) == Classification::File)
        .map(|(path, _)| OwnedArtifact {
            path,
            classification: Classification::File,
        })
        .collect())
}

fn export_openspec_with_io<I: TransactionIo>(
    source: &str,
    io: I,
) -> Result<ConversionReport, Error> {
    let spec_root = resolve_spec_root(Path::new(source))?;
    let mut transaction = Transaction::acquire_with_io(&spec_root, io)?;
    if let Some(transaction::RecoveryOutcome::Completed {
        operation: Operation::ExportOpenSpec,
        converted,
    }) = transaction.recovery()
    {
        return Ok(ConversionReport {
            converted,
            destination: openspec_root(&spec_root).display().to_string(),
            recovered: true,
        });
    }
    let plan = preflight_export(&spec_root)?;
    if !plan.writes.is_empty() || !plan.removals.is_empty() {
        transaction.execute_conversion(
            Operation::ExportOpenSpec,
            &plan.writes,
            &plan.removals,
            &plan.removable_directories,
        )?;
    }
    verify_exported_manifest(&spec_root, &plan.manifest)?;
    Ok(ConversionReport {
        converted: plan.manifest.entries.len(),
        destination: plan.destination.display().to_string(),
        recovered: false,
    })
}

fn import_openspec_with_io<I: TransactionIo>(
    source: &str,
    configured: Option<&str>,
    io: I,
) -> Result<ConversionReport, Error> {
    let spec_root = match configured {
        Some(configured) => crate::spec::resolve_spec_root_with(Path::new(source), configured)?,
        None => resolve_spec_root(Path::new(source))?,
    };
    let mut transaction = Transaction::acquire_with_io(&spec_root, io)?;
    let plan = preflight_import(&spec_root)?;
    if !plan.writes.is_empty() || !plan.removals.is_empty() {
        transaction.execute_conversion(
            Operation::ImportOpenSpec,
            &plan.writes,
            &plan.removals,
            &plan.removable_directories,
        )?;
    }
    verify_imported_manifest(&spec_root, &plan.manifest)?;
    Ok(ConversionReport {
        converted: plan.manifest.entries.len(),
        destination: plan.destination.display().to_string(),
        recovered: false,
    })
}

fn io_error(action: &str, path: &Path, error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("cannot {action} {}: {error}", path.display()),
    )
}

/// Result of one import or export; render with [`render_report`] or
/// [`render_report_json`].
#[derive(Debug, Serialize)]
pub struct ConversionReport {
    converted: usize,
    destination: String,
    recovered: bool,
}

pub fn render_report_json(report: &ConversionReport) -> Result<String, Error> {
    serde_json::to_string(report).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot serialize conversion report: {error}"),
        )
    })
}

pub fn render_report(report: &ConversionReport, sheet: &crate::sheet::Sheet) -> String {
    let suffix = if report.recovered {
        " (completed by crash recovery)"
    } else {
        ""
    };
    format!(
        "{}\n",
        sheet.ok(&format!(
            "{} file(s) converted → {}{suffix}",
            report.converted, report.destination
        ))
    )
}

fn print_report(report: &ConversionReport, json: bool) -> Result<(), Error> {
    if json {
        println!("{}", render_report_json(report)?);
    } else {
        let sheet = crate::sheet::Sheet::detect();
        print!("{}", render_report(report, &sheet));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
