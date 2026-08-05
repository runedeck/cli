//! The transaction journal: the typed, versioned on-disk record that
//! decides recovery. Every path is scoped (spec root or repository root)
//! and every content change carries its original and intended hashes.

use super::staging::write_file_atomic;
use super::{ConversionPhase, Operation, Phase};
use crate::error::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PathScope {
    SpecRoot,
    RepositoryRoot,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ScopedPath {
    pub(super) scope: PathScope,
    pub(super) path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct JournalFile {
    pub(super) path: String,
    pub(super) original_sha256: Option<String>,
    pub(super) intended_sha256: String,
    pub(super) backup_path: Option<String>,
    pub(super) staged_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ArchiveJournal {
    pub(super) version: u32,
    pub(super) operation: Operation,
    pub(super) change_id: String,
    pub(super) phase: Phase,
    pub(super) active_path: String,
    pub(super) archive_path: String,
    pub(super) archive_staged_path: String,
    pub(super) active_original_sha256: String,
    pub(super) active_intended_sha256: String,
    pub(super) active_backup_path: String,
    pub(super) canonical: Vec<JournalFile>,
    pub(super) active_update: Option<JournalFile>,
    #[serde(default)]
    pub(super) created_directories: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ConversionJournalWrite {
    pub(super) path: ScopedPath,
    pub(super) intended_sha256: String,
    pub(super) staged_path: ScopedPath,
    pub(super) install_path: ScopedPath,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ConversionJournalRemoval {
    pub(super) path: ScopedPath,
    pub(super) sha256: String,
    pub(super) backup_path: ScopedPath,
    pub(super) quarantine_path: ScopedPath,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ConversionJournal {
    pub(super) version: u32,
    pub(super) operation: Operation,
    pub(super) phase: ConversionPhase,
    pub(super) writes: Vec<ConversionJournalWrite>,
    pub(super) removals: Vec<ConversionJournalRemoval>,
    pub(super) removable_directories: Vec<ScopedPath>,
    #[serde(default)]
    pub(super) created_directories: Vec<ScopedPath>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum Journal {
    Archive(Box<ArchiveJournal>),
    Conversion(ConversionJournal),
}

pub(super) fn persist_journal(path: &Path, journal: &Journal) -> Result<(), Error> {
    let content = serde_yaml::to_string(journal).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot serialize transaction journal: {error}"),
        )
    })?;
    write_file_atomic(path, content.as_bytes())
}

pub(super) fn relative_string(path: &Path, scope: &str) -> Result<String, Error> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("invalid transaction path in {scope}: {}", path.display()),
            ));
        };
        let segment = segment.to_str().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "transaction path in {scope} is not UTF-8: {}",
                    path.display()
                ),
            )
        })?;
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(Error::new(ErrorKind::Config, "transaction path is empty"));
    }
    Ok(segments.join("/"))
}

pub(super) fn insert_non_overlapping_path(
    occupied_paths: &mut Vec<PathBuf>,
    candidate: &Path,
    error_kind: ErrorKind,
    context: &str,
) -> Result<(), Error> {
    if let Some(conflict) = occupied_paths.iter().find(|occupied| {
        candidate == occupied.as_path()
            || candidate.starts_with(occupied)
            || occupied.starts_with(candidate)
    }) {
        return Err(Error::new(
            error_kind,
            format!(
                "{context} paths overlap: {} and {}",
                conflict.display(),
                candidate.display()
            ),
        ));
    }
    occupied_paths.push(candidate.to_path_buf());
    Ok(())
}
