//! The conversion state machine: import/export execution through the
//! phased journal (`prepared`, `committing`, `destinations-complete`,
//! `removing-sources`, `cleanup`), quarantine-based source removal, and
//! the recovery path that replays an interrupted conversion.

#![allow(clippy::wildcard_imports)]

use super::*;

impl<I: TransactionIo> Transaction<I> {
    pub(crate) fn execute_conversion(
        &mut self,
        operation: Operation,
        writes: &[FileWrite],
        removals: &[FileRemoval],
        removable_directories: &[PathBuf],
    ) -> Result<(), Error> {
        if !operation.is_conversion() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "operation {} is not an import or export conversion",
                    operation.label()
                ),
            ));
        }
        if self.state_directory.exists() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "transaction state still exists at {}; retry recovery before converting",
                    self.state_directory.display()
                ),
            ));
        }
        let plan = self.preflight_conversion(writes, removals, removable_directories)?;
        let journal = self.prepare_conversion(operation, &plan)?;
        self.commit_conversion(journal)
    }

    fn preflight_conversion(
        &self,
        writes: &[FileWrite],
        removals: &[FileRemoval],
        removable_directories: &[PathBuf],
    ) -> Result<ConversionPlan, Error> {
        let mut confined_writes = Vec::with_capacity(writes.len());
        let mut occupied_paths = Vec::with_capacity(writes.len() + removals.len());
        for write in writes {
            let destination = self.confine_repository_absolute(&write.path)?;
            self.reject_reserved_conversion_path(&destination, false)?;
            reject_symlink(&destination)?;
            if destination.exists() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "conversion destination already exists: {}",
                        destination.display()
                    ),
                ));
            }
            insert_non_overlapping_path(
                &mut occupied_paths,
                &destination,
                ErrorKind::Config,
                "conversion plan",
            )?;
            confined_writes.push(FileWrite {
                path: destination,
                content: write.content.clone(),
            });
        }

        let mut confined_removals = Vec::with_capacity(removals.len());
        for removal in removals {
            let source = self.confine_repository_absolute(&removal.path)?;
            self.reject_reserved_conversion_path(&source, false)?;
            reject_symlink(&source)?;
            if !source.is_file() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!("conversion source is not a file: {}", source.display()),
                ));
            }
            verify_file_hash(&source, &removal.sha256)?;
            insert_non_overlapping_path(
                &mut occupied_paths,
                &source,
                ErrorKind::Config,
                "conversion plan",
            )?;
            confined_removals.push(FileRemoval {
                path: source,
                sha256: removal.sha256.clone(),
            });
        }

        let mut confined_directories = Vec::with_capacity(removable_directories.len());
        let mut directory_paths = HashSet::new();
        for directory in removable_directories {
            let directory = self.confine_repository_absolute(directory)?;
            self.reject_reserved_conversion_path(&directory, true)?;
            reject_symlink(&directory)?;
            if !directory.is_dir() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "removable conversion path is not a directory: {}",
                        directory.display()
                    ),
                ));
            }
            if !directory_paths.insert(directory.clone()) {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "duplicate removable conversion directory: {}",
                        directory.display()
                    ),
                ));
            }
            confined_directories.push(directory);
        }

        Ok(ConversionPlan {
            writes: confined_writes,
            removals: confined_removals,
            removable_directories: confined_directories,
        })
    }

    fn prepare_conversion(
        &self,
        operation: Operation,
        plan: &ConversionPlan,
    ) -> Result<ConversionJournal, Error> {
        fs::create_dir(&self.state_directory)
            .map_err(|error| io_error("create", &self.state_directory, error))?;
        let preparation = self.prepare_conversion_after_directory(operation, plan);
        if preparation.is_err()
            && !self.journal_path.is_file()
            && let Err(cleanup_error) = fs::remove_dir_all(&self.state_directory)
        {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "conversion preparation failed and {} could not be cleaned: {cleanup_error}",
                    self.state_directory.display()
                ),
            ));
        }
        preparation
    }

    fn prepare_conversion_after_directory(
        &self,
        operation: Operation,
        plan: &ConversionPlan,
    ) -> Result<ConversionJournal, Error> {
        let mut writes = Vec::with_capacity(plan.writes.len());
        for (index, write) in plan.writes.iter().enumerate() {
            let staged_path = self
                .state_directory
                .join(format!("staged/conversion/{index}"));
            write_file_exclusive(&staged_path, &write.content)?;
            let intended_sha256 = hash_bytes(&write.content);
            verify_file_hash(&staged_path, &intended_sha256)?;
            let install_path = self
                .state_directory
                .join(format!("installed/conversion/{index}"));
            writes.push(ConversionJournalWrite {
                path: self.scoped_path(&write.path)?,
                intended_sha256,
                staged_path: self.scoped_path(&staged_path)?,
                install_path: self.scoped_path(&install_path)?,
            });
        }

        let mut removals = Vec::with_capacity(plan.removals.len());
        for (index, removal) in plan.removals.iter().enumerate() {
            verify_file_hash(&removal.path, &removal.sha256)?;
            let backup_path = self
                .state_directory
                .join(format!("backups/conversion/{index}"));
            copy_file_exclusive(&removal.path, &backup_path)?;
            verify_file_hash(&backup_path, &removal.sha256)?;
            let quarantine_path = self
                .state_directory
                .join(format!("quarantine/conversion/{index}"));
            removals.push(ConversionJournalRemoval {
                path: self.scoped_path(&removal.path)?,
                sha256: removal.sha256.clone(),
                backup_path: self.scoped_path(&backup_path)?,
                quarantine_path: self.scoped_path(&quarantine_path)?,
            });
        }

        let removable_directories = plan
            .removable_directories
            .iter()
            .map(|directory| self.scoped_path(directory))
            .collect::<Result<Vec<_>, _>>()?;
        let journal = ConversionJournal {
            version: JOURNAL_VERSION,
            operation,
            phase: ConversionPhase::Prepared,
            writes,
            removals,
            removable_directories,
            created_directories: Vec::new(),
        };
        self.persist_conversion_journal(&journal)?;
        self.io
            .conversion_phase_persisted(ConversionPhase::Prepared)
            .map_err(|error| io_error("continue after persisting", &self.journal_path, error))?;
        Ok(journal)
    }

    fn commit_conversion(&self, mut journal: ConversionJournal) -> Result<(), Error> {
        self.advance_conversion_phase(&mut journal, ConversionPhase::Committing)?;
        for index in 0..journal.writes.len() {
            let write = journal.writes[index].clone();
            self.install_conversion_write(&mut journal, &write)?;
        }
        self.advance_conversion_phase(&mut journal, ConversionPhase::DestinationsComplete)?;
        self.verify_conversion_destinations(&journal)?;
        self.advance_conversion_phase(&mut journal, ConversionPhase::RemovingSources)?;
        self.move_conversion_sources_to_quarantine(&journal)?;
        self.remove_empty_conversion_directories(&journal.removable_directories)?;
        self.advance_conversion_phase(&mut journal, ConversionPhase::Cleanup)?;
        self.cleanup_conversion_quarantine(&journal)?;
        self.remove_state_directory()
    }

    fn install_conversion_write(
        &self,
        journal: &mut ConversionJournal,
        write: &ConversionJournalWrite,
    ) -> Result<(), Error> {
        let destination = self.absolute_from_scoped(&write.path)?;
        let staged = self.absolute_from_scoped(&write.staged_path)?;
        let install = self.absolute_from_scoped(&write.install_path)?;
        verify_file_hash(&staged, &write.intended_sha256)?;
        copy_file_exclusive(&staged, &install)?;
        verify_file_hash(&install, &write.intended_sha256)?;
        reject_symlink(&destination)?;
        let parent = destination.parent().ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!("file path has no parent: {}", destination.display()),
            )
        })?;
        self.create_conversion_destination_directories(journal, parent)?;
        inspect_existing_ancestors(&self.repository, &destination)?;
        reject_symlink(&destination)?;
        fs::hard_link(&install, &destination)
            .map_err(|error| io_error("install create-only destination", &destination, error))?;
        verify_file_hash(&destination, &write.intended_sha256)?;
        let directory = File::open(parent).map_err(|error| io_error("open", parent, error))?;
        directory
            .sync_all()
            .map_err(|error| io_error("sync", parent, error))
    }

    fn create_conversion_destination_directories(
        &self,
        journal: &mut ConversionJournal,
        destination_parent: &Path,
    ) -> Result<(), Error> {
        Self::create_destination_directories(
            &self.repository,
            destination_parent,
            "conversion destination",
            |directory| {
                journal
                    .created_directories
                    .push(self.scoped_path(directory)?);
                if let Err(error) = self.persist_conversion_journal(journal) {
                    drop(journal.created_directories.pop());
                    return Err(error);
                }
                Ok(())
            },
        )
    }

    fn verify_conversion_destinations(&self, journal: &ConversionJournal) -> Result<(), Error> {
        for write in &journal.writes {
            let destination = self.absolute_from_scoped(&write.path)?;
            inspect_existing_ancestors(&self.repository, &destination)?;
            let (actual_sha256, _) = hash_file_with_identity(&destination)?;
            if actual_sha256 != write.intended_sha256 {
                return Err(unexpected_hash_error(
                    &destination,
                    &write.intended_sha256,
                    &actual_sha256,
                ));
            }
        }
        Ok(())
    }

    fn verify_conversion_sources_present(&self, journal: &ConversionJournal) -> Result<(), Error> {
        for removal in &journal.removals {
            let source = self.absolute_from_scoped(&removal.path)?;
            reject_symlink(&source)?;
            if !source.is_file() {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "conversion source disappeared before source removal: {}; transaction backups remain at {}",
                        source.display(),
                        self.state_directory.display()
                    ),
                ));
            }
            verify_file_hash(&source, &removal.sha256)?;
            let backup = self.absolute_from_scoped(&removal.backup_path)?;
            verify_file_hash(&backup, &removal.sha256)?;
            let quarantine = self.absolute_from_scoped(&removal.quarantine_path)?;
            if path_exists_without_following(&quarantine)? {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "conversion quarantine exists before source removal: {}; transaction state is preserved",
                        quarantine.display()
                    ),
                ));
            }
        }
        Ok(())
    }

    fn rollback_conversion(&self, journal: &ConversionJournal) -> Result<(), Error> {
        self.verify_conversion_sources_present(journal)?;
        for write in journal.writes.iter().rev() {
            let destination = self.absolute_from_scoped(&write.path)?;
            if !path_exists_without_following(&destination)? {
                continue;
            }
            reject_symlink(&destination)?;
            verify_file_hash(&destination, &write.intended_sha256)?;
            let staged = self.absolute_from_scoped(&write.staged_path)?;
            verify_file_hash(&staged, &write.intended_sha256)?;
            let install = self.absolute_from_scoped(&write.install_path)?;
            verify_file_hash(&install, &write.intended_sha256)?;
            if !is_same_file(&install, &destination)? {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "refusing conversion rollback because {} was not installed by this transaction; transaction state is preserved",
                        destination.display()
                    ),
                ));
            }
            inspect_existing_ancestors(&self.repository, &destination)?;
            reject_symlink(&destination)?;
            if !is_same_file(&install, &destination)? {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "conversion destination changed before rollback: {}; transaction state is preserved",
                        destination.display()
                    ),
                ));
            }
            self.io
                .remove_file(&destination)
                .map_err(|error| io_error("remove", &destination, error))?;
        }
        self.remove_empty_conversion_directories(&journal.created_directories)?;
        self.remove_state_directory()
    }

    fn move_conversion_sources_to_quarantine(
        &self,
        journal: &ConversionJournal,
    ) -> Result<(), Error> {
        self.verify_conversion_destinations(journal)?;
        for removal in &journal.removals {
            self.move_conversion_source_to_quarantine(removal)?;
        }
        Ok(())
    }

    fn move_conversion_source_to_quarantine(
        &self,
        removal: &ConversionJournalRemoval,
    ) -> Result<(), Error> {
        let source = self.absolute_from_scoped(&removal.path)?;
        let backup = self.absolute_from_scoped(&removal.backup_path)?;
        let quarantine = self.absolute_from_scoped(&removal.quarantine_path)?;
        verify_file_hash(&backup, &removal.sha256)?;
        let source_exists = path_exists_without_following(&source)?;
        let quarantine_exists = path_exists_without_following(&quarantine)?;
        match (source_exists, quarantine_exists) {
            (true, true) => Err(Error::new(
                ErrorKind::Io,
                format!(
                    "conversion source and quarantine both exist for {}; transaction state is preserved",
                    source.display()
                ),
            )),
            (false, false) => Err(Error::new(
                ErrorKind::Io,
                format!(
                    "conversion source and quarantine are both missing for {}; transaction state is preserved",
                    source.display()
                ),
            )),
            (false, true) => {
                inspect_existing_ancestors(&self.root, &quarantine)?;
                let (quarantine_sha256, _) = hash_file_with_identity(&quarantine)?;
                if quarantine_sha256 == removal.sha256 {
                    Ok(())
                } else {
                    Err(unexpected_hash_error(
                        &quarantine,
                        &removal.sha256,
                        &quarantine_sha256,
                    ))
                }
            }
            (true, false) => {
                let quarantine_parent = quarantine.parent().ok_or_else(|| {
                    Error::new(
                        ErrorKind::Config,
                        format!("quarantine path has no parent: {}", quarantine.display()),
                    )
                })?;
                create_confined_directories(&self.root, quarantine_parent)?;
                let (source_sha256, source_identity) = hash_file_with_identity(&source)?;
                if source_sha256 != removal.sha256 {
                    return Err(unexpected_hash_error(
                        &source,
                        &removal.sha256,
                        &source_sha256,
                    ));
                }
                inspect_existing_ancestors(&self.repository, &source)?;
                inspect_existing_ancestors(&self.root, &quarantine)?;
                reject_symlink(&source)?;
                reject_symlink(&quarantine)?;
                if file_identity(&source)? != source_identity {
                    return Err(Error::new(
                        ErrorKind::Io,
                        format!(
                            "conversion source changed before quarantine move: {}; transaction state is preserved",
                            source.display()
                        ),
                    ));
                }
                self.io
                    .rename(&source, &quarantine)
                    .map_err(|error| io_error("move source into quarantine", &source, error))?;
                let (quarantine_sha256, quarantine_identity) =
                    hash_file_with_identity(&quarantine)?;
                if quarantine_identity != source_identity {
                    return Err(Error::new(
                        ErrorKind::Io,
                        format!(
                            "quarantined source identity changed for {}; transaction state is preserved",
                            source.display()
                        ),
                    ));
                }
                if quarantine_sha256 != removal.sha256 {
                    return Err(unexpected_hash_error(
                        &quarantine,
                        &removal.sha256,
                        &quarantine_sha256,
                    ));
                }
                self.io.source_removed(&source).map_err(|error| {
                    io_error(
                        "continue after moving source into quarantine",
                        &source,
                        error,
                    )
                })
            }
        }
    }

    fn cleanup_conversion_quarantine(&self, journal: &ConversionJournal) -> Result<(), Error> {
        self.verify_conversion_destinations(journal)?;
        for removal in &journal.removals {
            let source = self.absolute_from_scoped(&removal.path)?;
            let quarantine = self.absolute_from_scoped(&removal.quarantine_path)?;
            if path_exists_without_following(&source)? {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "conversion source reappeared during cleanup: {}; transaction state is preserved",
                        source.display()
                    ),
                ));
            }
            if !path_exists_without_following(&quarantine)? {
                continue;
            }
            let (quarantine_sha256, quarantine_identity) = hash_file_with_identity(&quarantine)?;
            if quarantine_sha256 != removal.sha256 {
                return Err(unexpected_hash_error(
                    &quarantine,
                    &removal.sha256,
                    &quarantine_sha256,
                ));
            }
            inspect_existing_ancestors(&self.repository, &source)?;
            if path_exists_without_following(&source)? {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "conversion source reappeared during cleanup: {}; transaction state is preserved",
                        source.display()
                    ),
                ));
            }
            inspect_existing_ancestors(&self.root, &quarantine)?;
            reject_symlink(&quarantine)?;
            if file_identity(&quarantine)? != quarantine_identity {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "conversion quarantine changed before cleanup: {}; transaction state is preserved",
                        quarantine.display()
                    ),
                ));
            }
            self.io
                .remove_file(&quarantine)
                .map_err(|error| io_error("remove", &quarantine, error))?;
        }
        Ok(())
    }

    fn remove_empty_conversion_directories(&self, directories: &[ScopedPath]) -> Result<(), Error> {
        let directories = directories
            .iter()
            .map(|directory| self.absolute_from_scoped(directory))
            .collect::<Result<Vec<_>, _>>()?;
        self.remove_empty_directories(&self.repository, directories)
    }

    pub(super) fn recover_conversion(
        &self,
        mut journal: ConversionJournal,
    ) -> Result<RecoveryOutcome, Error> {
        match journal.phase {
            ConversionPhase::Prepared => {
                self.verify_conversion_sources_present(&journal)?;
                self.remove_state_directory()?;
                Ok(RecoveryOutcome::RolledBack(journal.operation))
            }
            ConversionPhase::Committing | ConversionPhase::DestinationsComplete => {
                self.rollback_conversion(&journal)?;
                Ok(RecoveryOutcome::RolledBack(journal.operation))
            }
            ConversionPhase::RemovingSources => {
                self.move_conversion_sources_to_quarantine(&journal)?;
                self.remove_empty_conversion_directories(&journal.removable_directories)?;
                self.advance_conversion_phase(&mut journal, ConversionPhase::Cleanup)?;
                self.cleanup_conversion_quarantine(&journal)?;
                self.remove_state_directory()?;
                Ok(RecoveryOutcome::Completed {
                    operation: journal.operation,
                    converted: journal.writes.len(),
                })
            }
            ConversionPhase::Cleanup => {
                self.remove_empty_conversion_directories(&journal.removable_directories)?;
                self.cleanup_conversion_quarantine(&journal)?;
                self.remove_state_directory()?;
                Ok(RecoveryOutcome::Completed {
                    operation: journal.operation,
                    converted: journal.writes.len(),
                })
            }
        }
    }

    pub(super) fn validate_conversion_journal_paths(
        &self,
        journal: &ConversionJournal,
    ) -> Result<(), Error> {
        let mut occupied_paths = Vec::with_capacity(journal.writes.len() + journal.removals.len());
        let mut write_destinations = Vec::with_capacity(journal.writes.len());
        for write in &journal.writes {
            let destination = self.absolute_from_scoped(&write.path)?;
            self.reject_reserved_conversion_path(&destination, false)?;
            insert_non_overlapping_path(
                &mut occupied_paths,
                &destination,
                ErrorKind::Io,
                "conversion journal",
            )?;
            self.validate_state_artifact(&write.staged_path, "staged")?;
            self.validate_state_artifact(&write.install_path, "installed")?;
            write_destinations.push(destination);
        }
        for removal in &journal.removals {
            let source = self.absolute_from_scoped(&removal.path)?;
            self.reject_reserved_conversion_path(&source, false)?;
            insert_non_overlapping_path(
                &mut occupied_paths,
                &source,
                ErrorKind::Io,
                "conversion journal",
            )?;
            self.validate_state_artifact(&removal.backup_path, "backups")?;
            self.validate_state_artifact(&removal.quarantine_path, "quarantine")?;
        }
        let mut directory_paths = HashSet::new();
        for directory in &journal.removable_directories {
            let directory = self.absolute_from_scoped(directory)?;
            self.reject_reserved_conversion_path(&directory, true)?;
            if !directory_paths.insert(directory.clone()) {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "duplicate removable conversion directory: {}",
                        directory.display()
                    ),
                ));
            }
        }
        for directory in &journal.created_directories {
            let directory = self.absolute_from_scoped(directory)?;
            self.reject_reserved_conversion_path(&directory, true)?;
            if !directory_paths.insert(directory.clone()) {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!(
                        "duplicate recorded conversion directory: {}",
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
                        "recorded conversion directory is not a destination ancestor: {}",
                        directory.display()
                    ),
                ));
            }
        }
        Ok(())
    }

    fn advance_conversion_phase(
        &self,
        journal: &mut ConversionJournal,
        phase: ConversionPhase,
    ) -> Result<(), Error> {
        journal.phase = phase;
        self.persist_conversion_journal(journal)?;
        self.io
            .conversion_phase_persisted(phase)
            .map_err(|error| io_error("continue after persisting", &self.journal_path, error))
    }

    fn persist_conversion_journal(&self, journal: &ConversionJournal) -> Result<(), Error> {
        persist_journal(&self.journal_path, &Journal::Conversion(journal.clone()))
    }

    fn reject_reserved_conversion_path(
        &self,
        path: &Path,
        _is_directory: bool,
    ) -> Result<(), Error> {
        let lock_path = self.root.join(LOCK_FILE);
        if path == lock_path || path.starts_with(&self.state_directory) {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "conversion path uses reserved transaction state: {}",
                    path.display()
                ),
            ));
        }
        Ok(())
    }
}
