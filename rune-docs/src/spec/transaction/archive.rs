//! The archive state machine: merge/abandon execution through the phased
//! journal (`prepared`, `committing`, `canonical-complete`,
//! `archive-moved`, `cleanup`), staged canonical writes, the archive move,
//! and the recovery path that rolls back or finishes an interrupted run.

#![allow(clippy::wildcard_imports)]

use super::*;

impl<I: TransactionIo> Transaction<I> {
    pub(crate) fn execute(
        &mut self,
        operation: Operation,
        change_id: &str,
        active_path: &Path,
        archive_path: &Path,
        canonical_writes: &[FileWrite],
        active_update: Option<&FileWrite>,
    ) -> Result<(), Error> {
        if self.state_directory.exists() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "transaction state still exists at {}; retry recovery before archiving",
                    self.state_directory.display()
                ),
            ));
        }
        let journal = self.prepare(
            operation,
            change_id,
            active_path,
            archive_path,
            canonical_writes,
            active_update,
        )?;
        self.commit(journal)
    }

    fn prepare(
        &self,
        operation: Operation,
        change_id: &str,
        active_path: &Path,
        archive_path: &Path,
        canonical_writes: &[FileWrite],
        active_update: Option<&FileWrite>,
    ) -> Result<ArchiveJournal, Error> {
        let active_path = self.confine_absolute(active_path)?;
        let archive_path = self.confine_absolute(archive_path)?;
        let archive_staged_path = archive_staging_path(&archive_path)?;
        if !active_path.is_dir() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "active change is not a directory: {}",
                    active_path.display()
                ),
            ));
        }
        if archive_path.exists() {
            return Err(Error::new(
                ErrorKind::Config,
                format!("archive target already exists: {}", archive_path.display()),
            ));
        }
        if archive_staged_path.exists() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "staged archive path already exists: {}; inspect it before retrying",
                    archive_staged_path.display()
                ),
            ));
        }

        fs::create_dir(&self.state_directory)
            .map_err(|error| io_error("create", &self.state_directory, error))?;
        let preparation = self.prepare_after_directory(
            operation,
            change_id,
            &active_path,
            &archive_path,
            canonical_writes,
            active_update,
        );
        if preparation.is_err()
            && !self.journal_path.is_file()
            && let Err(cleanup_error) = fs::remove_dir_all(&self.state_directory)
        {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "transaction preparation failed and {} could not be cleaned: {cleanup_error}",
                    self.state_directory.display()
                ),
            ));
        }
        preparation
    }

    fn prepare_after_directory(
        &self,
        operation: Operation,
        change_id: &str,
        active_path: &Path,
        archive_path: &Path,
        canonical_writes: &[FileWrite],
        active_update: Option<&FileWrite>,
    ) -> Result<ArchiveJournal, Error> {
        let archive_staged_path = archive_staging_path(archive_path)?;
        let active_backup = self.state_directory.join("backups/active");
        copy_tree_exclusive(active_path, &active_backup)?;
        let active_original_sha256 = hash_tree(active_path)?;
        let active_backup_sha256 = hash_tree(&active_backup)?;
        if active_backup_sha256 != active_original_sha256 {
            return Err(Error::new(
                ErrorKind::Io,
                "active change backup verification failed",
            ));
        }

        let mut canonical = Vec::with_capacity(canonical_writes.len());
        for (index, write) in canonical_writes.iter().enumerate() {
            canonical.push(self.stage_file(write, "canonical", index)?);
        }
        let staged_active_update = active_update
            .map(|write| self.stage_file(write, "active-update", 0))
            .transpose()?;
        let active_intended_sha256 = match active_update {
            Some(write) => hash_tree_with_file_override(active_path, &write.path, &write.content)?,
            None => active_original_sha256.clone(),
        };

        let journal = ArchiveJournal {
            version: JOURNAL_VERSION,
            operation,
            change_id: change_id.to_string(),
            phase: Phase::Prepared,
            active_path: self.relative_string(active_path)?,
            archive_path: self.relative_string(archive_path)?,
            archive_staged_path: self.relative_string(&archive_staged_path)?,
            active_original_sha256,
            active_intended_sha256,
            active_backup_path: self.relative_string(&active_backup)?,
            canonical,
            active_update: staged_active_update,
            created_directories: Vec::new(),
        };
        self.persist_journal(&journal)?;
        self.io
            .phase_persisted(Phase::Prepared)
            .map_err(|error| io_error("continue after persisting", &self.journal_path, error))?;
        Ok(journal)
    }

    fn stage_file(
        &self,
        write: &FileWrite,
        category: &str,
        index: usize,
    ) -> Result<JournalFile, Error> {
        let destination = self.confine_absolute(&write.path)?;
        reject_symlink(&destination)?;
        let original_sha256 = if destination.exists() {
            if !destination.is_file() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "transaction destination is not a file: {}",
                        destination.display()
                    ),
                ));
            }
            Some(hash_file(&destination)?)
        } else {
            None
        };
        let backup_path = if original_sha256.is_some() {
            let backup = self
                .state_directory
                .join(format!("backups/{category}/{index}"));
            copy_file_exclusive(&destination, &backup)?;
            Some(self.relative_string(&backup)?)
        } else {
            None
        };
        let staged = self
            .state_directory
            .join(format!("staged/{category}/{index}"));
        write_file_exclusive(&staged, &write.content)?;
        let intended_sha256 = hash_bytes(&write.content);
        if hash_file(&staged)? != intended_sha256 {
            return Err(Error::new(
                ErrorKind::Io,
                format!("staged file verification failed: {}", staged.display()),
            ));
        }
        Ok(JournalFile {
            path: self.relative_string(&destination)?,
            original_sha256,
            intended_sha256,
            backup_path,
            staged_path: self.relative_string(&staged)?,
        })
    }

    fn commit(&self, mut journal: ArchiveJournal) -> Result<(), Error> {
        self.advance_phase(&mut journal, Phase::Committing)?;
        for index in 0..journal.canonical.len() {
            let file = journal.canonical[index].clone();
            self.install_staged_file(&mut journal, &file)?;
        }
        if let Some(file) = journal.active_update.clone() {
            self.install_staged_file(&mut journal, &file)?;
        }
        self.advance_phase(&mut journal, Phase::CanonicalComplete)?;
        self.move_archive(&mut journal)?;
        self.io.archive_move_completed().map_err(|error| {
            io_error("continue after moving archive", &self.journal_path, error)
        })?;
        self.advance_phase(&mut journal, Phase::ArchiveMoved)?;
        let active_path = self.absolute_from_relative(&journal.active_path)?;
        if active_path.exists() {
            verify_tree_hash(&active_path, &journal.active_intended_sha256)?;
            remove_verified_tree(&active_path)?;
        }
        self.advance_phase(&mut journal, Phase::Cleanup)?;
        self.remove_state_directory()
    }

    fn create_archive_destination_directories(
        &self,
        journal: &mut ArchiveJournal,
        destination_parent: &Path,
    ) -> Result<(), Error> {
        Self::create_destination_directories(
            &self.root,
            destination_parent,
            "archive destination",
            |directory| {
                journal
                    .created_directories
                    .push(self.relative_string(directory)?);
                if let Err(error) = self.persist_journal(journal) {
                    drop(journal.created_directories.pop());
                    return Err(error);
                }
                Ok(())
            },
        )
    }

    fn remove_empty_archive_directories(&self, directories: &[String]) -> Result<(), Error> {
        let directories = directories
            .iter()
            .map(|directory| self.absolute_from_relative(directory))
            .collect::<Result<Vec<_>, _>>()?;
        self.remove_empty_directories(&self.root, directories)
    }

    fn install_staged_file(
        &self,
        journal: &mut ArchiveJournal,
        file: &JournalFile,
    ) -> Result<(), Error> {
        let destination = self.absolute_from_relative(&file.path)?;
        let staged = self.absolute_from_relative(&file.staged_path)?;
        verify_file_hash(&staged, &file.intended_sha256)?;
        reject_symlink(&destination)?;
        let parent = destination.parent().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("file path has no parent: {}", destination.display()),
            )
        })?;
        self.create_archive_destination_directories(journal, parent)?;
        inspect_existing_ancestors(&self.root, &destination)?;
        reject_symlink(&destination)?;
        self.io
            .rename(&staged, &destination)
            .map_err(|error| io_error("replace", &destination, error))?;
        verify_file_hash(&destination, &file.intended_sha256)
    }

    fn move_archive(&self, journal: &mut ArchiveJournal) -> Result<(), Error> {
        let active_path = self.absolute_from_relative(&journal.active_path)?;
        let archive_path = self.absolute_from_relative(&journal.archive_path)?;
        let archive_parent = archive_path.parent().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("archive path has no parent: {}", archive_path.display()),
            )
        })?;
        self.create_archive_destination_directories(journal, archive_parent)?;
        inspect_existing_ancestors(&self.root, &archive_path)?;
        reject_symlink(&archive_path)?;
        match self.io.rename(&active_path, &archive_path) {
            Ok(()) => verify_tree_hash(&archive_path, &journal.active_intended_sha256),
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
                let staged_archive = self.absolute_from_relative(&journal.archive_staged_path)?;
                self.copy_archive_across_devices(
                    &active_path,
                    &archive_path,
                    &staged_archive,
                    &journal.active_intended_sha256,
                )
            }
            Err(error) => Err(io_error("archive", &active_path, error)),
        }
    }

    fn copy_archive_across_devices(
        &self,
        active_path: &Path,
        archive_path: &Path,
        staged_archive: &Path,
        intended_sha256: &str,
    ) -> Result<(), Error> {
        copy_tree_exclusive(active_path, staged_archive)?;
        verify_tree_hash(staged_archive, intended_sha256)?;
        self.io
            .rename(staged_archive, archive_path)
            .map_err(|error| io_error("publish staged archive", archive_path, error))?;
        verify_tree_hash(archive_path, intended_sha256)
    }

    pub(super) fn recover_archive(
        &self,
        journal: &ArchiveJournal,
    ) -> Result<RecoveryOutcome, Error> {
        match journal.phase {
            Phase::Prepared => {
                self.remove_state_directory()?;
                Ok(RecoveryOutcome::RolledBack(journal.operation))
            }
            Phase::Committing | Phase::CanonicalComplete => {
                self.rollback(journal)?;
                Ok(RecoveryOutcome::RolledBack(journal.operation))
            }
            Phase::ArchiveMoved | Phase::Cleanup => {
                self.finish_committed(journal)?;
                Ok(RecoveryOutcome::Completed {
                    operation: journal.operation,
                    converted: journal.canonical.len(),
                })
            }
        }
    }

    fn rollback(&self, journal: &ArchiveJournal) -> Result<(), Error> {
        for file in &journal.canonical {
            self.restore_file(file)?;
        }
        self.restore_active_change(journal)?;
        self.remove_staged_archive(journal)?;
        self.remove_empty_archive_directories(&journal.created_directories)?;
        self.remove_state_directory()
    }

    fn restore_file(&self, file: &JournalFile) -> Result<(), Error> {
        let destination = self.absolute_from_relative(&file.path)?;
        if destination.exists() {
            let current_sha256 = hash_file(&destination)?;
            let is_known_content = current_sha256 == file.intended_sha256
                || file
                    .original_sha256
                    .as_ref()
                    .is_some_and(|original| current_sha256 == *original);
            if !is_known_content {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "refusing rollback because {} has an unexpected SHA-256; transaction backups remain at {}",
                        destination.display(),
                        self.state_directory.display()
                    ),
                ));
            }
        }
        match (&file.original_sha256, &file.backup_path) {
            (Some(original_sha256), Some(backup_path)) => {
                let backup = self.absolute_from_relative(backup_path)?;
                verify_file_hash(&backup, original_sha256)?;
                replace_file_from_backup(&self.root, &backup, &destination)?;
                verify_file_hash(&destination, original_sha256)
            }
            (None, None) => {
                if destination.exists() {
                    fs::remove_file(&destination)
                        .map_err(|error| io_error("remove", &destination, error))?;
                }
                Ok(())
            }
            _ => Err(Error::new(
                ErrorKind::Io,
                format!(
                    "journal backup metadata is inconsistent for {}; transaction state is preserved",
                    destination.display()
                ),
            )),
        }
    }

    fn restore_active_change(&self, journal: &ArchiveJournal) -> Result<(), Error> {
        let active_path = self.absolute_from_relative(&journal.active_path)?;
        let archive_path = self.absolute_from_relative(&journal.archive_path)?;
        let backup_path = self.absolute_from_relative(&journal.active_backup_path)?;
        verify_tree_hash(&backup_path, &journal.active_original_sha256)?;

        if active_path.exists() {
            let active_sha256 = hash_tree(&active_path)?;
            if active_sha256 != journal.active_original_sha256
                && active_sha256 != journal.active_intended_sha256
            {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "refusing rollback because {} has an unexpected tree SHA-256; transaction state is preserved",
                        active_path.display()
                    ),
                ));
            }
            if active_sha256 != journal.active_original_sha256 {
                remove_verified_tree(&active_path)?;
                copy_tree_exclusive(&backup_path, &active_path)?;
            }
        } else {
            copy_tree_exclusive(&backup_path, &active_path)?;
        }
        verify_tree_hash(&active_path, &journal.active_original_sha256)?;

        if archive_path.exists() {
            verify_tree_hash(&archive_path, &journal.active_intended_sha256)?;
            remove_verified_tree(&archive_path)?;
        }
        Ok(())
    }

    fn finish_committed(&self, journal: &ArchiveJournal) -> Result<(), Error> {
        for file in &journal.canonical {
            let destination = self.absolute_from_relative(&file.path)?;
            verify_file_hash(&destination, &file.intended_sha256)?;
        }
        let archive_path = self.absolute_from_relative(&journal.archive_path)?;
        verify_tree_hash(&archive_path, &journal.active_intended_sha256)?;
        let active_path = self.absolute_from_relative(&journal.active_path)?;
        if active_path.exists() {
            verify_tree_hash(&active_path, &journal.active_intended_sha256)?;
            remove_verified_tree(&active_path)?;
        }
        self.remove_staged_archive(journal)?;
        self.remove_state_directory()
    }

    fn remove_staged_archive(&self, journal: &ArchiveJournal) -> Result<(), Error> {
        let staged_archive = self.absolute_from_relative(&journal.archive_staged_path)?;
        if staged_archive.exists() {
            remove_verified_tree(&staged_archive)?;
        }
        Ok(())
    }

    pub(super) fn validate_archive_journal_paths(
        &self,
        journal: &ArchiveJournal,
    ) -> Result<(), Error> {
        let changes_root = self.root.join("changes");
        let archive_root = changes_root.join("archive");
        let active_path = self.absolute_from_relative(&journal.active_path)?;
        if active_path.parent() != Some(changes_root.as_path())
            || active_path.file_name().and_then(|name| name.to_str())
                != Some(journal.change_id.as_str())
        {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "archive journal active path does not match change '{}': {}",
                    journal.change_id,
                    active_path.display()
                ),
            ));
        }

        let archive_path = self.absolute_from_relative(&journal.archive_path)?;
        let archive_name = archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Io,
                    format!(
                        "archive journal path has no UTF-8 name: {}",
                        archive_path.display()
                    ),
                )
            })?;
        if archive_path.parent() != Some(archive_root.as_path())
            || !archive_name.ends_with(&format!("-{}", journal.change_id))
        {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "archive journal destination is outside the expected archive subtree: {}",
                    archive_path.display()
                ),
            ));
        }

        let archive_staged_path = self.absolute_from_relative(&journal.archive_staged_path)?;
        let expected_archive_staged_path = archive_staging_path(&archive_path)?;
        if archive_staged_path != expected_archive_staged_path {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "archive journal staging path does not match its destination: {}",
                    archive_staged_path.display()
                ),
            ));
        }
        self.validate_archive_state_path(
            &journal.active_backup_path,
            &self.state_directory.join("backups/active"),
            "active backup",
        )?;

        let write_destinations =
            self.validate_archive_write_destinations(journal, &archive_path, &active_path)?;
        self.validate_archive_created_directories(journal, &write_destinations)
    }

    fn validate_archive_write_destinations(
        &self,
        journal: &ArchiveJournal,
        archive_path: &Path,
        active_path: &Path,
    ) -> Result<Vec<PathBuf>, Error> {
        let specifications_root = self.root.join("specs");
        let mut write_destinations = Vec::with_capacity(
            journal.canonical.len() + usize::from(journal.active_update.is_some()) + 1,
        );
        insert_non_overlapping_path(
            &mut write_destinations,
            archive_path,
            ErrorKind::Io,
            "archive journal write",
        )?;
        for (index, file) in journal.canonical.iter().enumerate() {
            let destination = self.absolute_from_relative(&file.path)?;
            if destination.parent().is_none()
                || destination == specifications_root
                || !destination.starts_with(&specifications_root)
            {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "archive journal canonical destination is outside specifications: {}",
                        destination.display()
                    ),
                ));
            }
            self.validate_archive_file_state(file, "canonical", index)?;
            insert_non_overlapping_path(
                &mut write_destinations,
                &destination,
                ErrorKind::Io,
                "archive journal write",
            )?;
        }
        if let Some(file) = &journal.active_update {
            let destination = self.absolute_from_relative(&file.path)?;
            if destination == active_path || !destination.starts_with(active_path) {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "archive journal active update is outside its change: {}",
                        destination.display()
                    ),
                ));
            }
            self.validate_archive_file_state(file, "active-update", 0)?;
            insert_non_overlapping_path(
                &mut write_destinations,
                &destination,
                ErrorKind::Io,
                "archive journal write",
            )?;
        }
        Ok(write_destinations)
    }

    fn validate_archive_created_directories(
        &self,
        journal: &ArchiveJournal,
        write_destinations: &[PathBuf],
    ) -> Result<(), Error> {
        let mut created_directories = HashSet::new();
        for directory in &journal.created_directories {
            let directory = self.absolute_from_relative(directory)?;
            if directory == self.root.join(LOCK_FILE)
                || directory.starts_with(&self.state_directory)
            {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "recorded archive directory uses reserved transaction state: {}",
                        directory.display()
                    ),
                ));
            }
            if !created_directories.insert(directory.clone()) {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "duplicate recorded archive directory: {}",
                        directory.display()
                    ),
                ));
            }
            if !write_destinations
                .iter()
                .any(|destination| destination != &directory && destination.starts_with(&directory))
            {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "recorded archive directory is not a destination ancestor: {}",
                        directory.display()
                    ),
                ));
            }
        }
        Ok(())
    }

    fn validate_archive_file_state(
        &self,
        file: &JournalFile,
        category: &str,
        index: usize,
    ) -> Result<(), Error> {
        self.validate_archive_state_path(
            &file.staged_path,
            &self
                .state_directory
                .join(format!("staged/{category}/{index}")),
            "staged file",
        )?;
        match (&file.original_sha256, &file.backup_path) {
            (Some(_), Some(backup_path)) => self.validate_archive_state_path(
                backup_path,
                &self
                    .state_directory
                    .join(format!("backups/{category}/{index}")),
                "file backup",
            ),
            (None, None) => Ok(()),
            _ => Err(Error::new(
                ErrorKind::Io,
                format!(
                    "archive journal backup metadata is inconsistent for {}",
                    file.path
                ),
            )),
        }
    }

    fn validate_archive_state_path(
        &self,
        relative: &str,
        expected: &Path,
        context: &str,
    ) -> Result<(), Error> {
        let actual = self.absolute_from_relative(relative)?;
        if actual == expected {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Io,
                format!(
                    "archive journal {context} path is not transaction-owned: {}",
                    actual.display()
                ),
            ))
        }
    }

    fn advance_phase(&self, journal: &mut ArchiveJournal, phase: Phase) -> Result<(), Error> {
        journal.phase = phase;
        self.persist_journal(journal)?;
        self.io
            .phase_persisted(phase)
            .map_err(|error| io_error("continue after persisting", &self.journal_path, error))
    }

    fn persist_journal(&self, journal: &ArchiveJournal) -> Result<(), Error> {
        persist_journal(
            &self.journal_path,
            &Journal::Archive(Box::new(journal.clone())),
        )
    }
}
