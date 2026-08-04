//! Crash-safe execution for archive and conversion: an OS-backed exclusive
//! lock, a journaled plan ([`journal`]), hash verification ([`verify`]),
//! staged writes ([`staging`]), and path confinement ([`confine`]). This
//! file keeps acquisition, the public operation surface, and the archive
//! and conversion state machines that recovery replays.

mod archive;
mod confine;
mod conversion;
mod journal;
mod staging;
mod verify;

pub(crate) use verify::sha256_file;

use super::root::SpecRoot;
use crate::error::{Error, ErrorKind};
use confine::{
    create_confined_directories, file_identity, inspect_existing_ancestors, is_same_file,
    path_exists_without_following, reject_symlink,
};
use journal::{
    ArchiveJournal, ConversionJournal, ConversionJournalRemoval, ConversionJournalWrite, Journal,
    JournalFile, PathScope, ScopedPath, insert_non_overlapping_path, persist_journal,
    relative_string,
};
use serde::{Deserialize, Serialize};
use staging::{
    archive_staging_path, copy_file_exclusive, copy_tree_exclusive, remove_verified_tree,
    replace_file_from_backup, write_file_exclusive,
};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use verify::{
    hash_bytes, hash_file, hash_file_with_identity, hash_tree, hash_tree_with_file_override,
    unexpected_hash_error, verify_file_hash, verify_tree_hash,
};

/// On-disk transaction state names, owned here and shared with the
/// `OpenSpec` interop layer. Archive and import/export deliberately guard
/// each other through the one `LOCK_FILE` under the spec root: whichever
/// operation acquires it first excludes the other, and the journal inside
/// `TRANSACTION_DIRECTORY` (not the lock) decides whether recovery runs.
pub(crate) const JOURNAL_VERSION: u32 = 1;
pub(crate) const TRANSACTION_DIRECTORY: &str = ".rune-transaction";
pub(crate) const JOURNAL_FILE: &str = "journal.yaml";
pub(crate) const LOCK_FILE: &str = ".rune-archive.lock";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Operation {
    Merge,
    Abandon,
    ImportOpenSpec,
    ExportOpenSpec,
}

impl Operation {
    fn label(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Abandon => "abandon",
            Self::ImportOpenSpec => "import-openspec",
            Self::ExportOpenSpec => "export-openspec",
        }
    }

    fn is_conversion(self) -> bool {
        matches!(self, Self::ImportOpenSpec | Self::ExportOpenSpec)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Phase {
    Prepared,
    Committing,
    CanonicalComplete,
    ArchiveMoved,
    Cleanup,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Committing => "committing",
            Self::CanonicalComplete => "canonical-complete",
            Self::ArchiveMoved => "archive-moved",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ConversionPhase {
    Prepared,
    Committing,
    DestinationsComplete,
    RemovingSources,
    Cleanup,
}

impl ConversionPhase {
    fn label(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Committing => "committing",
            Self::DestinationsComplete => "destinations-complete",
            Self::RemovingSources => "removing-sources",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileWrite {
    pub(crate) path: PathBuf,
    pub(crate) content: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct FileRemoval {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
}

struct ConversionPlan {
    writes: Vec<FileWrite>,
    removals: Vec<FileRemoval>,
    removable_directories: Vec<PathBuf>,
}

/// A transaction-layer health finding (lock contention, incomplete or
/// invalid journal), converted into the doctor report by `doctor_output`.
pub(super) struct TransactionHealthFinding {
    pub(super) severity: super::DiagnosticSeverity,
    pub(super) path: String,
    pub(super) message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryOutcome {
    RolledBack(Operation),
    Completed {
        operation: Operation,
        converted: usize,
    },
}

pub(crate) struct Transaction<I: TransactionIo = SystemIo> {
    repository: PathBuf,
    root: PathBuf,
    state_directory: PathBuf,
    journal_path: PathBuf,
    _lock: File,
    io: I,
    recovery: Option<RecoveryOutcome>,
}

/// The failure-injection seam, and the deliberate exception to RUST-0008
/// (no traits for internal types): recovery is testable only if every
/// phase can fail on demand. Production holds [`SystemIo`] alone; the
/// crate-private `acquire_with_io` is the only injection point, and the
/// public `acquire` pins it to [`SystemIo`]. The other implementations
/// live in the interop and transaction test modules
/// (`FailAfterSourceRemoval`, `FailAtCleanupPhase`, and their siblings).
pub(crate) trait TransactionIo {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()>;

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }

    fn phase_persisted(&self, _phase: Phase) -> io::Result<()> {
        Ok(())
    }

    fn conversion_phase_persisted(&self, _phase: ConversionPhase) -> io::Result<()> {
        Ok(())
    }

    fn archive_move_completed(&self) -> io::Result<()> {
        Ok(())
    }

    fn source_removed(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemIo;

impl TransactionIo for SystemIo {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }
}

pub(crate) fn acquire(spec_root: &SpecRoot) -> Result<Transaction, Error> {
    Transaction::acquire_with_io(spec_root, SystemIo)
}

impl<I: TransactionIo> Transaction<I> {
    pub(crate) fn acquire_with_io(spec_root: &SpecRoot, io: I) -> Result<Self, Error> {
        fs::create_dir_all(spec_root.base())
            .map_err(|error| io_error("create", spec_root.base(), error))?;
        let repository = spec_root
            .repository()
            .canonicalize()
            .map_err(|error| io_error("resolve", spec_root.repository(), error))?;
        let root = spec_root
            .base()
            .canonicalize()
            .map_err(|error| io_error("resolve", spec_root.base(), error))?;
        let lock_path = root.join(LOCK_FILE);
        reject_symlink(&lock_path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| io_error("open", &lock_path, error))?;
        lock.try_lock().map_err(|error| {
            let message = match error {
                fs::TryLockError::WouldBlock => format!(
                    "another spec archive transaction holds {}; retry after it finishes",
                    lock_path.display()
                ),
                fs::TryLockError::Error(error) => {
                    format!("cannot lock {}: {error}", lock_path.display())
                }
            };
            Error::new(ErrorKind::Io, message)
        })?;
        let state_directory = root.join(TRANSACTION_DIRECTORY);
        reject_symlink(&state_directory)?;
        let journal_path = state_directory.join(JOURNAL_FILE);
        reject_symlink(&journal_path)?;
        let mut transaction = Self {
            repository,
            root,
            state_directory,
            journal_path,
            _lock: lock,
            io,
            recovery: None,
        };
        transaction.recovery = transaction.recover()?;
        Ok(transaction)
    }

    pub(crate) fn recovery(&self) -> Option<RecoveryOutcome> {
        self.recovery
    }

    fn create_destination_directories(
        confinement_root: &Path,
        destination_parent: &Path,
        context: &str,
        mut record_created_directory: impl FnMut(&Path) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut missing_directories = Vec::new();
        let mut current = destination_parent;
        while current != confinement_root {
            match fs::symlink_metadata(current) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(Error::new(
                        ErrorKind::Config,
                        format!(
                            "{context} ancestor is not a regular directory: {}",
                            current.display()
                        ),
                    ));
                }
                Ok(_) => break,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    missing_directories.push(current.to_path_buf());
                }
                Err(error) => return Err(io_error("inspect", current, error)),
            }
            current = current.parent().ok_or_else(|| {
                Error::new(
                    ErrorKind::Config,
                    format!(
                        "{context} ancestor is outside confinement root: {}",
                        destination_parent.display()
                    ),
                )
            })?;
        }
        missing_directories.reverse();

        for directory in missing_directories {
            inspect_existing_ancestors(confinement_root, &directory)?;
            match fs::create_dir(&directory) {
                Ok(()) => {
                    inspect_existing_ancestors(confinement_root, &directory)?;
                    if let Err(error) = record_created_directory(&directory) {
                        return match fs::remove_dir(&directory) {
                            Ok(()) => Err(error),
                            Err(cleanup_error) => Err(Error::new(
                                ErrorKind::Io,
                                format!(
                                    "{}; cannot clean created directory {}: {cleanup_error}",
                                    error.message(),
                                    directory.display()
                                ),
                            )),
                        };
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    inspect_existing_ancestors(confinement_root, &directory)?;
                    let metadata = fs::symlink_metadata(&directory)
                        .map_err(|error| io_error("inspect", &directory, error))?;
                    if metadata.file_type().is_symlink() || !metadata.is_dir() {
                        return Err(Error::new(
                            ErrorKind::Config,
                            format!(
                                "{context} ancestor is not a regular directory: {}",
                                directory.display()
                            ),
                        ));
                    }
                }
                Err(error) => return Err(io_error("create", &directory, error)),
            }
        }
        Ok(())
    }

    fn remove_empty_directories(
        &self,
        confinement_root: &Path,
        mut directories: Vec<PathBuf>,
    ) -> Result<(), Error> {
        directories.sort_by_key(|directory| std::cmp::Reverse(directory.components().count()));
        for directory in directories {
            reject_symlink(&directory)?;
            let mut entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(io_error("read", &directory, error)),
            };
            if entries
                .next()
                .transpose()
                .map_err(|error| io_error("read", &directory, error))?
                .is_some()
            {
                continue;
            }
            inspect_existing_ancestors(confinement_root, &directory)?;
            reject_symlink(&directory)?;
            match self.io.remove_dir(&directory) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => return Err(io_error("remove", &directory, error)),
            }
        }
        Ok(())
    }

    fn recover(&mut self) -> Result<Option<RecoveryOutcome>, Error> {
        if !self.state_directory.exists() {
            return Ok(None);
        }
        reject_symlink(&self.state_directory)?;
        reject_symlink(&self.journal_path)?;
        if !self.journal_path.is_file() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "incomplete transaction has no journal at {}; preserve the directory and inspect its backups before retrying",
                    self.journal_path.display()
                ),
            ));
        }
        let outcome = match self.load_journal()? {
            Journal::Archive(journal) => self.recover_archive(&journal)?,
            Journal::Conversion(journal) => self.recover_conversion(journal)?,
        };
        Ok(Some(outcome))
    }

    fn load_journal(&self) -> Result<Journal, Error> {
        let content = fs::read_to_string(&self.journal_path)
            .map_err(|error| io_error("read", &self.journal_path, error))?;
        let journal: Journal = serde_yaml::from_str(&content).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot parse transaction journal {}: {error}; preserve it and its backups",
                    self.journal_path.display()
                ),
            )
        })?;
        match &journal {
            Journal::Archive(archive) => {
                self.validate_journal_version(archive.version)?;
                if archive.operation.is_conversion() {
                    return Err(Error::new(
                        ErrorKind::Io,
                        "archive transaction journal contains a conversion operation",
                    ));
                }
                self.validate_archive_journal_paths(archive)?;
            }
            Journal::Conversion(conversion) => {
                self.validate_journal_version(conversion.version)?;
                if !conversion.operation.is_conversion() {
                    return Err(Error::new(
                        ErrorKind::Io,
                        "conversion transaction journal contains an archive operation",
                    ));
                }
                self.validate_conversion_journal_paths(conversion)?;
            }
        }
        Ok(journal)
    }

    fn validate_journal_version(&self, version: u32) -> Result<(), Error> {
        if version == JOURNAL_VERSION {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Io,
                format!(
                    "unsupported transaction journal version {version} at {}; preserve it and its backups",
                    self.journal_path.display()
                ),
            ))
        }
    }

    fn validate_state_artifact(
        &self,
        scoped: &ScopedPath,
        category: &str,
    ) -> Result<PathBuf, Error> {
        if scoped.scope != PathScope::SpecRoot {
            return Err(Error::new(
                ErrorKind::Io,
                format!("transaction {category} path must use spec-root scope"),
            ));
        }
        let path = self.absolute_from_scoped(scoped)?;
        let category_root = self.state_directory.join(category);
        if !path.starts_with(&category_root) {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "transaction {category} path is outside {}: {}",
                    category_root.display(),
                    path.display()
                ),
            ));
        }
        Ok(path)
    }

    fn remove_state_directory(&self) -> Result<(), Error> {
        fs::remove_dir_all(&self.state_directory)
            .map_err(|error| io_error("remove", &self.state_directory, error))
    }

    fn confine_absolute(&self, candidate: &Path) -> Result<PathBuf, Error> {
        let relative = candidate.strip_prefix(&self.root).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "transaction path {} is outside spec root {}: {error}",
                    candidate.display(),
                    self.root.display()
                ),
            )
        })?;
        let confined = self.absolute_from_path(relative)?;
        inspect_existing_ancestors(&self.root, &confined)?;
        Ok(confined)
    }

    fn confine_repository_absolute(&self, candidate: &Path) -> Result<PathBuf, Error> {
        let relative = candidate.strip_prefix(&self.repository).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "conversion path {} is outside repository {}: {error}",
                    candidate.display(),
                    self.repository.display()
                ),
            )
        })?;
        Self::absolute_from_path_at(&self.repository, relative, "repository")
    }

    fn scoped_path(&self, path: &Path) -> Result<ScopedPath, Error> {
        if let Ok(relative) = path.strip_prefix(&self.root) {
            return Ok(ScopedPath {
                scope: PathScope::SpecRoot,
                path: relative_string(relative, "spec root")?,
            });
        }
        let relative = path.strip_prefix(&self.repository).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "cannot store conversion path {} relative to repository {}: {error}",
                    path.display(),
                    self.repository.display()
                ),
            )
        })?;
        Ok(ScopedPath {
            scope: PathScope::RepositoryRoot,
            path: relative_string(relative, "repository")?,
        })
    }

    fn absolute_from_scoped(&self, scoped: &ScopedPath) -> Result<PathBuf, Error> {
        match scoped.scope {
            PathScope::SpecRoot => {
                Self::absolute_from_path_at(&self.root, Path::new(&scoped.path), "spec root")
            }
            PathScope::RepositoryRoot => {
                Self::absolute_from_path_at(&self.repository, Path::new(&scoped.path), "repository")
            }
        }
    }

    fn absolute_from_relative(&self, relative: &str) -> Result<PathBuf, Error> {
        self.absolute_from_path(Path::new(relative))
    }

    fn absolute_from_path(&self, relative: &Path) -> Result<PathBuf, Error> {
        Self::absolute_from_path_at(&self.root, relative, "spec root")
    }

    fn absolute_from_path_at(root: &Path, relative: &Path, scope: &str) -> Result<PathBuf, Error> {
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "transaction journal path must be relative to the {scope}: {}",
                    relative.display()
                ),
            ));
        }
        let candidate = root.join(relative);
        if !candidate.starts_with(root) {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "transaction journal path escapes {scope}: {}",
                    relative.display()
                ),
            ));
        }
        inspect_existing_ancestors(root, &candidate)?;
        Ok(candidate)
    }

    fn relative_string(&self, path: &Path) -> Result<String, Error> {
        let relative = path.strip_prefix(&self.root).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "cannot store transaction path {} relative to {}: {error}",
                    path.display(),
                    self.root.display()
                ),
            )
        })?;
        relative_string(relative, "spec root")
    }
}

pub(super) fn health_findings(
    spec_root: &SpecRoot,
) -> Result<Vec<TransactionHealthFinding>, Error> {
    if !spec_root.base().exists() {
        return Ok(Vec::new());
    }
    let root = spec_root
        .base()
        .canonicalize()
        .map_err(|error| io_error("resolve", spec_root.base(), error))?;
    // Diagnosis never writes: the lock is probed only if it already exists
    // (acquisition creates it), and a created lock is never unlinked here
    // because removal races a concurrent acquirer.
    let lock_path = root.join(LOCK_FILE);
    reject_symlink(&lock_path)?;
    let lock = match OpenOptions::new().read(true).write(true).open(&lock_path) {
        Ok(lock) => Some(lock),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_error("open", &lock_path, error)),
    };
    if let Some(lock) = &lock
        && let Err(error) = lock.try_lock()
    {
        return match error {
            fs::TryLockError::WouldBlock => Ok(vec![TransactionHealthFinding {
                severity: super::DiagnosticSeverity::Warning,
                path: LOCK_FILE.to_string(),
                message: "another process holds the spec archive lock".to_string(),
            }]),
            fs::TryLockError::Error(error) => Err(io_error("lock", &lock_path, error)),
        };
    }

    let state_directory = root.join(TRANSACTION_DIRECTORY);
    if !state_directory.exists() {
        return Ok(Vec::new());
    }
    let journal_path = state_directory.join(JOURNAL_FILE);
    if !journal_path.is_file() {
        return Ok(vec![TransactionHealthFinding {
            severity: super::DiagnosticSeverity::Error,
            path: format!("{TRANSACTION_DIRECTORY}/{JOURNAL_FILE}"),
            message: "incomplete transaction has no readable journal".to_string(),
        }]);
    }
    let Some(lock) = lock else {
        return Ok(vec![TransactionHealthFinding {
            severity: super::DiagnosticSeverity::Error,
            path: format!("{TRANSACTION_DIRECTORY}/{JOURNAL_FILE}"),
            message: "transaction state exists without its lock file; rerun the owning operation to recover"
                .to_string(),
        }]);
    };
    incomplete_transaction_findings(spec_root, root, state_directory, journal_path, lock)
}

fn incomplete_transaction_findings(
    spec_root: &SpecRoot,
    root: PathBuf,
    state_directory: PathBuf,
    journal_path: PathBuf,
    lock: File,
) -> Result<Vec<TransactionHealthFinding>, Error> {
    let content = fs::read_to_string(&journal_path)
        .map_err(|error| io_error("read", &journal_path, error))?;
    let journal: Journal = match serde_yaml::from_str(&content) {
        Ok(journal) => journal,
        Err(error) => {
            return Ok(vec![TransactionHealthFinding {
                severity: super::DiagnosticSeverity::Error,
                path: format!("{TRANSACTION_DIRECTORY}/{JOURNAL_FILE}"),
                message: format!("transaction journal cannot be parsed: {error}"),
            }]);
        }
    };
    let repository = spec_root
        .repository()
        .canonicalize()
        .map_err(|error| io_error("resolve", spec_root.repository(), error))?;
    let transaction = Transaction {
        repository,
        root,
        state_directory,
        journal_path,
        _lock: lock,
        io: SystemIo,
        recovery: None,
    };
    let validation = match &journal {
        Journal::Archive(archive) => transaction
            .validate_journal_version(archive.version)
            .and_then(|()| transaction.validate_archive_journal_paths(archive)),
        Journal::Conversion(conversion) => transaction
            .validate_journal_version(conversion.version)
            .and_then(|()| transaction.validate_conversion_journal_paths(conversion)),
    };
    if let Err(error) = validation {
        return Ok(vec![TransactionHealthFinding {
            severity: super::DiagnosticSeverity::Error,
            path: format!("{TRANSACTION_DIRECTORY}/{JOURNAL_FILE}"),
            message: error.message().to_string(),
        }]);
    }
    let message = match journal {
        Journal::Archive(archive) => format!(
            "incomplete {} archive transaction for '{}' is at phase {}; rerun archive to recover",
            archive.operation.label(),
            archive.change_id,
            archive.phase.label()
        ),
        Journal::Conversion(conversion) => format!(
            "incomplete {} conversion transaction is at phase {}; rerun the conversion to recover",
            conversion.operation.label(),
            conversion.phase.label()
        ),
    };
    Ok(vec![TransactionHealthFinding {
        severity: super::DiagnosticSeverity::Error,
        path: format!("{TRANSACTION_DIRECTORY}/{JOURNAL_FILE}"),
        message,
    }])
}

fn io_error(action: &str, path: &Path, error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Io,
        format!("cannot {action} {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests;
