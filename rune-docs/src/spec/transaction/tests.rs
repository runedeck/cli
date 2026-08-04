use super::*;
use std::cell::{Cell, RefCell};
use tempfile::TempDir;

#[test]
fn health_findings_never_create_the_lock_file() {
    let repository = TempDir::new().unwrap();
    fs::create_dir_all(repository.path().join("docs/changes")).unwrap();
    let spec_root = crate::spec::resolve_spec_root(repository.path()).unwrap();

    let findings = health_findings(&spec_root).unwrap();

    assert!(findings.is_empty());
    assert!(!repository.path().join("docs").join(LOCK_FILE).exists());
}

#[test]
fn health_findings_report_a_journal_without_its_lock_file() {
    let repository = TempDir::new().unwrap();
    let state = repository.path().join("docs").join(TRANSACTION_DIRECTORY);
    fs::create_dir_all(repository.path().join("docs/changes")).unwrap();
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join(JOURNAL_FILE), "version: 1\n").unwrap();
    let spec_root = crate::spec::resolve_spec_root(repository.path()).unwrap();

    let findings = health_findings(&spec_root).unwrap();

    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("without its lock file"));
    assert!(!repository.path().join("docs").join(LOCK_FILE).exists());
}

const PROPOSAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/spec/transaction-proposal.md"
));
const ORIGINAL_CANONICAL: &[u8] = b"original canonical\n";
const INTENDED_CANONICAL: &[u8] = b"intended canonical\n";

#[derive(Default)]
struct InjectedIo {
    fail_phase: Option<Phase>,
    fail_conversion_phase: Option<ConversionPhase>,
    fail_archive_move_completed: bool,
    fail_after_source_removal: bool,
    fail_rename_call: Option<usize>,
    rename_calls: Cell<usize>,
    source_removal_calls: Cell<usize>,
    cross_device_once: Cell<bool>,
    collision_destination: Option<(PathBuf, Vec<u8>)>,
    replacement_before_quarantine: Option<(PathBuf, Vec<u8>)>,
    missing_before_quarantine: Option<PathBuf>,
    failed_phases: RefCell<Vec<Phase>>,
    failed_conversion_phases: RefCell<Vec<ConversionPhase>>,
}

impl InjectedIo {
    fn failing_phase(phase: Phase) -> Self {
        Self {
            fail_phase: Some(phase),
            ..Self::default()
        }
    }

    fn failing_conversion_phase(phase: ConversionPhase) -> Self {
        Self {
            fail_conversion_phase: Some(phase),
            ..Self::default()
        }
    }

    fn cross_device() -> Self {
        Self {
            cross_device_once: Cell::new(true),
            ..Self::default()
        }
    }
}

impl TransactionIo for InjectedIo {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        let call = self.rename_calls.get() + 1;
        self.rename_calls.set(call);
        if self.fail_rename_call == Some(call) {
            return Err(io::Error::other("injected rename failure"));
        }
        if self.missing_before_quarantine.as_deref() == Some(source) {
            fs::remove_file(source)?;
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        if let Some((replacement_path, replacement_content)) = &self.replacement_before_quarantine
            && replacement_path == source
        {
            fs::remove_file(source)?;
            fs::write(source, replacement_content)?;
        }
        let is_archive_move = source.is_dir()
            && destination
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "archive");
        if is_archive_move && self.cross_device_once.replace(false) {
            return Err(io::Error::from(io::ErrorKind::CrossesDevices));
        }
        fs::rename(source, destination)
    }

    fn phase_persisted(&self, phase: Phase) -> io::Result<()> {
        if self.fail_phase == Some(phase) && !self.failed_phases.borrow().contains(&phase) {
            self.failed_phases.borrow_mut().push(phase);
            return Err(io::Error::other("injected phase failure"));
        }
        Ok(())
    }

    fn conversion_phase_persisted(&self, phase: ConversionPhase) -> io::Result<()> {
        if phase == ConversionPhase::Committing
            && let Some((path, content)) = &self.collision_destination
            && !path.exists()
        {
            fs::write(path, content)?;
        }
        if self.fail_conversion_phase == Some(phase)
            && !self.failed_conversion_phases.borrow().contains(&phase)
        {
            self.failed_conversion_phases.borrow_mut().push(phase);
            return Err(io::Error::other("injected conversion phase failure"));
        }
        Ok(())
    }

    fn archive_move_completed(&self) -> io::Result<()> {
        if self.fail_archive_move_completed {
            Err(io::Error::other("injected archive move failure"))
        } else {
            Ok(())
        }
    }

    fn source_removed(&self, _path: &Path) -> io::Result<()> {
        let calls = self.source_removal_calls.get() + 1;
        self.source_removal_calls.set(calls);
        if self.fail_after_source_removal && calls == 1 {
            Err(io::Error::other("injected source removal failure"))
        } else {
            Ok(())
        }
    }
}

struct TransactionFixture {
    _temporary: TempDir,
    spec_root: SpecRoot,
    active_path: PathBuf,
    archive_path: PathBuf,
    canonical_path: PathBuf,
}

fn transaction_fixture() -> TransactionFixture {
    let temporary = TempDir::new().unwrap();
    fs::create_dir_all(temporary.path().join("docs/changes")).unwrap();
    let spec_root = super::super::resolve_spec_root(temporary.path()).unwrap();
    let active_path = spec_root.changes().join("change");
    fs::create_dir_all(&active_path).unwrap();
    fs::write(active_path.join("proposal.md"), PROPOSAL).unwrap();
    fs::write(active_path.join("tasks.md"), "- [x] complete\n").unwrap();
    let canonical_path = spec_root.specifications().join("search/spec.md");
    fs::create_dir_all(canonical_path.parent().unwrap()).unwrap();
    fs::write(&canonical_path, ORIGINAL_CANONICAL).unwrap();
    let archive_path = spec_root.changes().join("archive/2026-07-24-change");
    TransactionFixture {
        _temporary: temporary,
        spec_root,
        active_path,
        archive_path,
        canonical_path,
    }
}

fn canonical_write(fixture: &TransactionFixture) -> FileWrite {
    FileWrite {
        path: fixture.canonical_path.clone(),
        content: INTENDED_CANONICAL.to_vec(),
    }
}

fn conversion_source(fixture: &TransactionFixture, name: &str) -> FileRemoval {
    let path = fixture.spec_root.repository().join("openspec").join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, format!("source {name}\n")).unwrap();
    FileRemoval {
        sha256: sha256_file(&path).unwrap(),
        path,
    }
}

fn fail_at_phase(phase: Phase) -> TransactionFixture {
    let fixture = transaction_fixture();
    let mut transaction =
        Transaction::acquire_with_io(&fixture.spec_root, InjectedIo::failing_phase(phase)).unwrap();
    let error = transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[canonical_write(&fixture)],
            None,
        )
        .unwrap_err();
    assert!(error.message().contains("injected phase failure"));
    drop(transaction);
    fixture
}

fn recover(fixture: &TransactionFixture) {
    drop(acquire(&fixture.spec_root).unwrap());
}

fn archive_journal_path(fixture: &TransactionFixture) -> PathBuf {
    fixture
        .spec_root
        .base()
        .join(TRANSACTION_DIRECTORY)
        .join(JOURNAL_FILE)
}

fn read_archive_journal(fixture: &TransactionFixture) -> ArchiveJournal {
    let content = fs::read_to_string(archive_journal_path(fixture)).unwrap();
    match serde_yaml::from_str::<Journal>(&content).unwrap() {
        Journal::Archive(journal) => *journal,
        Journal::Conversion(_) => panic!("expected archive journal"),
    }
}

fn write_archive_journal(fixture: &TransactionFixture, journal: &ArchiveJournal) {
    persist_journal(
        &archive_journal_path(fixture),
        &Journal::Archive(Box::new(journal.clone())),
    )
    .unwrap();
}

fn acquire_error(fixture: &TransactionFixture) -> Error {
    match acquire(&fixture.spec_root) {
        Ok(_) => panic!("expected transaction acquisition to fail"),
        Err(error) => error,
    }
}

fn assert_rolled_back(fixture: &TransactionFixture) {
    assert!(fixture.active_path.is_dir());
    assert!(!fixture.archive_path.exists());
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        ORIGINAL_CANONICAL
    );
    assert!(
        !fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .exists()
    );
}

fn assert_committed(fixture: &TransactionFixture) {
    assert!(!fixture.active_path.exists());
    assert!(fixture.archive_path.is_dir());
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        INTENDED_CANONICAL
    );
    assert!(
        !fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .exists()
    );
}

#[test]
fn lock_contention_preserves_the_permanent_lock_file() {
    let fixture = transaction_fixture();
    let transaction = acquire(&fixture.spec_root).unwrap();

    let error = acquire_error(&fixture);

    assert!(error.message().contains("another spec archive transaction"));
    drop(transaction);
    assert!(fixture.spec_root.base().join(LOCK_FILE).is_file());
}

#[test]
fn prepared_recovery_discards_staged_state() {
    let fixture = fail_at_phase(Phase::Prepared);

    recover(&fixture);

    assert_rolled_back(&fixture);
}

#[test]
fn committing_recovery_restores_originals() {
    let fixture = fail_at_phase(Phase::Committing);

    recover(&fixture);

    assert_rolled_back(&fixture);
}

#[test]
fn canonical_complete_recovery_restores_originals() {
    let fixture = fail_at_phase(Phase::CanonicalComplete);
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        INTENDED_CANONICAL
    );

    recover(&fixture);

    assert_rolled_back(&fixture);
}

#[test]
fn archive_rollback_removes_only_created_canonical_ancestors() {
    let fixture = transaction_fixture();
    let specifications = fixture.spec_root.specifications();
    let destination = specifications.join("new-capability/nested/spec.md");
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo::failing_phase(Phase::CanonicalComplete),
    )
    .unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[FileWrite {
                path: destination.clone(),
                content: INTENDED_CANONICAL.to_vec(),
            }],
            None,
        )
        .unwrap_err();
    assert!(destination.is_file());
    drop(transaction);

    recover(&fixture);

    assert!(specifications.is_dir());
    assert!(!specifications.join("new-capability").exists());
    assert_rolled_back(&fixture);
}

#[test]
fn archive_rollback_preserves_preexisting_canonical_ancestors() {
    let fixture = transaction_fixture();
    let preexisting_directory = fixture.spec_root.specifications().join("new-capability");
    fs::create_dir(&preexisting_directory).unwrap();
    let destination = preexisting_directory.join("nested/spec.md");
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo::failing_phase(Phase::CanonicalComplete),
    )
    .unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[FileWrite {
                path: destination.clone(),
                content: INTENDED_CANONICAL.to_vec(),
            }],
            None,
        )
        .unwrap_err();
    drop(transaction);

    recover(&fixture);

    assert!(preexisting_directory.is_dir());
    assert!(!destination.exists());
    assert!(
        fs::read_dir(&preexisting_directory)
            .unwrap()
            .next()
            .is_none()
    );
    assert_rolled_back(&fixture);
}

#[test]
fn archive_moved_recovery_finishes_the_commit() {
    let fixture = fail_at_phase(Phase::ArchiveMoved);

    recover(&fixture);

    assert_committed(&fixture);
}

#[test]
fn archive_moved_recovery_removes_cross_device_source() {
    let fixture = transaction_fixture();
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            fail_phase: Some(Phase::ArchiveMoved),
            cross_device_once: Cell::new(true),
            ..InjectedIo::default()
        },
    )
    .unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[canonical_write(&fixture)],
            None,
        )
        .unwrap_err();
    assert!(fixture.active_path.is_dir());
    assert!(fixture.archive_path.is_dir());
    drop(transaction);
    recover(&fixture);

    assert_committed(&fixture);
}

#[test]
fn cleanup_recovery_finishes_the_commit() {
    let fixture = fail_at_phase(Phase::Cleanup);

    recover(&fixture);

    assert_committed(&fixture);
}

#[test]
fn partial_canonical_commit_recovers_every_original() {
    let fixture = transaction_fixture();
    let second_path = fixture.spec_root.specifications().join("other/spec.md");
    fs::create_dir_all(second_path.parent().unwrap()).unwrap();
    fs::write(&second_path, ORIGINAL_CANONICAL).unwrap();
    let writes = vec![
        canonical_write(&fixture),
        FileWrite {
            path: second_path.clone(),
            content: INTENDED_CANONICAL.to_vec(),
        },
    ];
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            fail_rename_call: Some(2),
            ..InjectedIo::default()
        },
    )
    .unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &writes,
            None,
        )
        .unwrap_err();
    drop(transaction);
    recover(&fixture);

    assert_rolled_back(&fixture);
    assert_eq!(fs::read(second_path).unwrap(), ORIGINAL_CANONICAL);
}

#[test]
fn archive_move_before_phase_update_rolls_back_the_archive() {
    let fixture = transaction_fixture();
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            fail_archive_move_completed: true,
            ..InjectedIo::default()
        },
    )
    .unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[canonical_write(&fixture)],
            None,
        )
        .unwrap_err();
    assert!(!fixture.active_path.exists());
    assert!(fixture.archive_path.exists());
    drop(transaction);
    recover(&fixture);

    assert_rolled_back(&fixture);
    assert!(!fixture.spec_root.changes().join("archive").exists());
}

#[test]
fn archive_move_rollback_preserves_preexisting_archive_parent() {
    let fixture = transaction_fixture();
    let archive_parent = fixture.spec_root.changes().join("archive");
    fs::create_dir(&archive_parent).unwrap();
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            fail_archive_move_completed: true,
            ..InjectedIo::default()
        },
    )
    .unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[canonical_write(&fixture)],
            None,
        )
        .unwrap_err();
    drop(transaction);

    recover(&fixture);

    assert_rolled_back(&fixture);
    assert!(archive_parent.is_dir());
    assert!(fs::read_dir(archive_parent).unwrap().next().is_none());
}

#[test]
fn abandon_archive_move_before_phase_restores_the_complete_active_tree() {
    let fixture = transaction_fixture();
    let proposal_path = fixture.active_path.join("proposal.md");
    let update = FileWrite {
        path: proposal_path.clone(),
        content: PROPOSAL
            .replace("status: proposed", "status: abandoned")
            .into_bytes(),
    };
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            fail_archive_move_completed: true,
            ..InjectedIo::default()
        },
    )
    .unwrap();

    transaction
        .execute(
            Operation::Abandon,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[],
            Some(&update),
        )
        .unwrap_err();
    drop(transaction);
    recover(&fixture);

    assert_rolled_back(&fixture);
    assert_eq!(fs::read_to_string(proposal_path).unwrap(), PROPOSAL);
}

#[test]
fn cross_device_move_copies_verifies_publishes_and_removes_source() {
    let fixture = transaction_fixture();
    let mut transaction =
        Transaction::acquire_with_io(&fixture.spec_root, InjectedIo::cross_device()).unwrap();

    transaction
        .execute(
            Operation::Merge,
            "change",
            &fixture.active_path,
            &fixture.archive_path,
            &[canonical_write(&fixture)],
            None,
        )
        .unwrap();

    assert_committed(&fixture);
    assert!(
        !archive_staging_path(&fixture.archive_path)
            .unwrap()
            .exists()
    );
}

#[test]
fn recovery_refuses_unknown_live_content_and_preserves_backups() {
    let fixture = fail_at_phase(Phase::CanonicalComplete);
    fs::write(&fixture.canonical_path, b"tampered canonical\n").unwrap();

    let error = acquire_error(&fixture);

    assert!(error.message().contains("unexpected SHA-256"));
    assert!(
        fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .is_dir()
    );
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        b"tampered canonical\n"
    );
}

#[test]
fn conversion_recovery_rolls_back_before_source_removal() {
    let fixture = transaction_fixture();
    let removal = conversion_source(&fixture, "source.md");
    let destination = fixture.spec_root.base().join("converted.md");
    let write = FileWrite {
        path: destination.clone(),
        content: b"converted\n".to_vec(),
    };
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo::failing_conversion_phase(ConversionPhase::DestinationsComplete),
    )
    .unwrap();

    transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[write],
            std::slice::from_ref(&removal),
            &[],
        )
        .unwrap_err();
    assert_eq!(fs::read(&destination).unwrap(), b"converted\n");
    assert!(removal.path.is_file());
    drop(transaction);

    recover(&fixture);

    assert!(!destination.exists());
    assert_eq!(sha256_file(&removal.path).unwrap(), removal.sha256);
    assert!(
        !fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .exists()
    );
}

#[test]
fn conversion_rollback_removes_only_created_destination_ancestors() {
    let repository = TempDir::new().unwrap();
    fs::create_dir_all(repository.path().join("openspec/changes")).unwrap();
    let spec_root = super::super::resolve_spec_root(repository.path()).unwrap();
    assert_eq!(spec_root.layout(), super::super::root::SpecLayout::OpenSpec);
    let repository_path = spec_root.repository().to_path_buf();
    fs::create_dir(repository_path.join("docs")).unwrap();
    let source = repository_path.join("openspec/project.md");
    fs::write(&source, b"source\n").unwrap();
    let removal = FileRemoval {
        sha256: sha256_file(&source).unwrap(),
        path: source.clone(),
    };
    let destination = repository_path.join("docs/changes/add-widget/proposal.md");
    let mut transaction = Transaction::acquire_with_io(
        &spec_root,
        InjectedIo::failing_conversion_phase(ConversionPhase::DestinationsComplete),
    )
    .unwrap();

    let error = transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: destination.clone(),
                content: b"converted\n".to_vec(),
            }],
            std::slice::from_ref(&removal),
            &[],
        )
        .unwrap_err();
    assert!(
        error
            .message()
            .contains("injected conversion phase failure"),
        "{}",
        error.message()
    );
    assert!(destination.is_file());
    assert!(repository_path.join("docs/changes").is_dir());
    drop(transaction);

    drop(acquire(&spec_root).unwrap());

    assert!(repository_path.join("docs").is_dir());
    assert!(!repository_path.join("docs/changes").exists());
    assert_eq!(sha256_file(&source).unwrap(), removal.sha256);
    let resolved = super::super::resolve_spec_root(repository.path()).unwrap();
    assert_eq!(resolved.layout(), super::super::root::SpecLayout::OpenSpec);
}

#[test]
fn conversion_rollback_preserves_preexisting_destination_ancestors() {
    let repository = TempDir::new().unwrap();
    fs::create_dir_all(repository.path().join("openspec/changes")).unwrap();
    let spec_root = super::super::resolve_spec_root(repository.path()).unwrap();
    let repository_path = spec_root.repository().to_path_buf();
    let preexisting_directory = repository_path.join("docs/changes");
    fs::create_dir_all(&preexisting_directory).unwrap();
    let source = repository_path.join("openspec/project.md");
    fs::write(&source, b"source\n").unwrap();
    let removal = FileRemoval {
        sha256: sha256_file(&source).unwrap(),
        path: source,
    };
    let destination = preexisting_directory.join("add-widget/proposal.md");
    let mut transaction = Transaction::acquire_with_io(
        &spec_root,
        InjectedIo::failing_conversion_phase(ConversionPhase::DestinationsComplete),
    )
    .unwrap();

    transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: destination.clone(),
                content: b"converted\n".to_vec(),
            }],
            std::slice::from_ref(&removal),
            &[],
        )
        .unwrap_err();
    drop(transaction);

    drop(acquire(&spec_root).unwrap());

    assert!(preexisting_directory.is_dir());
    assert!(!destination.exists());
    assert!(
        fs::read_dir(&preexisting_directory)
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn conversion_recovery_finishes_after_a_source_removal() {
    let fixture = transaction_fixture();
    let first_removal = conversion_source(&fixture, "first.md");
    let second_removal = conversion_source(&fixture, "second.md");
    let source_directory = first_removal.path.parent().unwrap().to_path_buf();
    let destination = fixture.spec_root.base().join("converted.md");
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            fail_after_source_removal: true,
            ..InjectedIo::default()
        },
    )
    .unwrap();

    transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: destination.clone(),
                content: b"converted\n".to_vec(),
            }],
            &[first_removal.clone(), second_removal.clone()],
            std::slice::from_ref(&source_directory),
        )
        .unwrap_err();
    assert!(!first_removal.path.exists());
    assert!(second_removal.path.is_file());
    assert_eq!(
        sha256_file(
            &fixture
                .spec_root
                .base()
                .join(TRANSACTION_DIRECTORY)
                .join("quarantine/conversion/0")
        )
        .unwrap(),
        first_removal.sha256
    );
    drop(transaction);

    recover(&fixture);

    assert_eq!(fs::read(destination).unwrap(), b"converted\n");
    assert!(!first_removal.path.exists());
    assert!(!second_removal.path.exists());
    assert!(!source_directory.exists());
}

#[test]
fn conversion_destination_collision_never_overwrites_the_raced_file() {
    let fixture = transaction_fixture();
    let removal = conversion_source(&fixture, "source.md");
    let destination = fixture.spec_root.base().join("converted.md");
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            collision_destination: Some((destination.clone(), b"raced\n".to_vec())),
            ..InjectedIo::default()
        },
    )
    .unwrap();

    let error = transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: destination.clone(),
                content: b"converted\n".to_vec(),
            }],
            std::slice::from_ref(&removal),
            &[],
        )
        .unwrap_err();

    assert!(error.message().contains("create-only destination"));
    assert_eq!(fs::read(&destination).unwrap(), b"raced\n");
    assert_eq!(sha256_file(&removal.path).unwrap(), removal.sha256);
    assert!(
        fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .is_dir()
    );
}

#[test]
fn conversion_preflight_rejects_component_prefix_conflicts() {
    let fixture = transaction_fixture();
    let destination = fixture.spec_root.base().join("converted");
    let child_destination = destination.join("child.md");
    let mut transaction = acquire(&fixture.spec_root).unwrap();

    let error = transaction
        .execute_conversion(
            Operation::ExportOpenSpec,
            &[
                FileWrite {
                    path: destination.clone(),
                    content: b"parent\n".to_vec(),
                },
                FileWrite {
                    path: child_destination.clone(),
                    content: b"child\n".to_vec(),
                },
            ],
            &[],
            &[],
        )
        .unwrap_err();

    assert!(error.message().contains("conversion plan paths overlap"));
    assert!(!destination.exists());
    assert!(!child_destination.exists());
    assert!(
        !fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .exists()
    );
}

#[test]
fn conversion_source_replacement_before_quarantine_preserves_state() {
    let fixture = transaction_fixture();
    let removal = conversion_source(&fixture, "source.md");
    let destination = fixture.spec_root.base().join("converted.md");
    let replacement = b"replacement\n".to_vec();
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            replacement_before_quarantine: Some((removal.path.clone(), replacement.clone())),
            ..InjectedIo::default()
        },
    )
    .unwrap();

    let error = transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: destination.clone(),
                content: b"converted\n".to_vec(),
            }],
            std::slice::from_ref(&removal),
            &[],
        )
        .unwrap_err();

    assert!(
        error
            .message()
            .contains("quarantined source identity changed")
    );
    assert!(!removal.path.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"converted\n");
    let state_directory = fixture.spec_root.base().join(TRANSACTION_DIRECTORY);
    assert_eq!(
        fs::read(state_directory.join("quarantine/conversion/0")).unwrap(),
        replacement
    );
    assert_eq!(
        sha256_file(&state_directory.join("backups/conversion/0")).unwrap(),
        removal.sha256
    );
    drop(transaction);

    let recovery_error = acquire_error(&fixture);

    assert!(
        recovery_error
            .message()
            .contains("SHA-256 verification failed")
    );
    assert!(state_directory.is_dir());
}

#[test]
fn conversion_source_disappearance_before_quarantine_preserves_state() {
    let fixture = transaction_fixture();
    let removal = conversion_source(&fixture, "source.md");
    let destination = fixture.spec_root.base().join("converted.md");
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo {
            missing_before_quarantine: Some(removal.path.clone()),
            ..InjectedIo::default()
        },
    )
    .unwrap();

    let error = transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: destination.clone(),
                content: b"converted\n".to_vec(),
            }],
            std::slice::from_ref(&removal),
            &[],
        )
        .unwrap_err();

    assert!(error.message().contains("move source into quarantine"));
    assert!(!removal.path.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"converted\n");
    let state_directory = fixture.spec_root.base().join(TRANSACTION_DIRECTORY);
    assert!(!state_directory.join("quarantine/conversion/0").exists());
    assert_eq!(
        sha256_file(&state_directory.join("backups/conversion/0")).unwrap(),
        removal.sha256
    );
    drop(transaction);

    let recovery_error = acquire_error(&fixture);

    assert!(
        recovery_error
            .message()
            .contains("source and quarantine are both missing")
    );
    assert!(state_directory.is_dir());
}

#[test]
fn conversion_journal_scopes_repository_paths_and_rejects_reserved_or_external_paths() {
    let fixture = transaction_fixture();
    let removal = conversion_source(&fixture, "source.md");
    let mut transaction = Transaction::acquire_with_io(
        &fixture.spec_root,
        InjectedIo::failing_conversion_phase(ConversionPhase::Prepared),
    )
    .unwrap();

    transaction
        .execute_conversion(
            Operation::ImportOpenSpec,
            &[FileWrite {
                path: fixture.spec_root.base().join("converted.md"),
                content: b"converted\n".to_vec(),
            }],
            &[removal],
            &[],
        )
        .unwrap_err();
    let journal = fs::read_to_string(
        fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .join(JOURNAL_FILE),
    )
    .unwrap();
    assert!(journal.contains("scope: repository-root"));
    drop(transaction);
    recover(&fixture);

    let mut transaction = acquire(&fixture.spec_root).unwrap();
    let external = TempDir::new().unwrap();
    let external_error = transaction
        .execute_conversion(
            Operation::ExportOpenSpec,
            &[FileWrite {
                path: external.path().join("outside.md"),
                content: b"outside\n".to_vec(),
            }],
            &[],
            &[],
        )
        .unwrap_err();
    assert!(external_error.message().contains("outside repository"));
    assert!(
        !fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .exists()
    );

    let reserved_error = transaction
        .execute_conversion(
            Operation::ExportOpenSpec,
            &[FileWrite {
                path: fixture.spec_root.base().join(LOCK_FILE),
                content: b"reserved\n".to_vec(),
            }],
            &[],
            &[],
        )
        .unwrap_err();
    assert!(
        reserved_error
            .message()
            .contains("reserved transaction state")
    );
}

#[test]
fn planted_archive_journal_cannot_redirect_state_artifacts() {
    let fixture = fail_at_phase(Phase::Prepared);
    let original = read_archive_journal(&fixture);
    let victim = fixture.canonical_path.clone();

    let mut planted = original.clone();
    planted.active_backup_path = "specs/search/spec.md".to_string();
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(
        error
            .message()
            .contains("active backup path is not transaction-owned")
    );
    assert_eq!(fs::read(&victim).unwrap(), ORIGINAL_CANONICAL);

    let mut planted = original.clone();
    planted.canonical[0].staged_path = "specs/search/spec.md".to_string();
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(
        error
            .message()
            .contains("staged file path is not transaction-owned")
    );
    assert_eq!(fs::read(&victim).unwrap(), ORIGINAL_CANONICAL);

    let mut planted = original;
    planted.canonical[0].backup_path = Some("specs/search/spec.md".to_string());
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(
        error
            .message()
            .contains("file backup path is not transaction-owned")
    );
    assert_eq!(fs::read(victim).unwrap(), ORIGINAL_CANONICAL);
}

#[test]
fn planted_archive_journal_cannot_duplicate_canonical_destinations() {
    let fixture = fail_at_phase(Phase::Prepared);
    let original = read_archive_journal(&fixture);

    let mut planted = original;
    let mut duplicate = planted.canonical[0].clone();
    duplicate.staged_path = duplicate.staged_path.replace("canonical/0", "canonical/1");
    duplicate.backup_path = duplicate
        .backup_path
        .map(|backup| backup.replace("canonical/0", "canonical/1"));
    planted.canonical.push(duplicate);
    write_archive_journal(&fixture, &planted);

    let error = acquire_error(&fixture);

    assert!(error.message().contains("paths overlap"));
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        ORIGINAL_CANONICAL
    );
}

#[test]
fn planted_archive_journal_cannot_redirect_recovery_paths() {
    let fixture = fail_at_phase(Phase::Prepared);
    let original = read_archive_journal(&fixture);
    let specifications = fixture.spec_root.specifications();

    let mut planted = original.clone();
    planted.archive_staged_path = "specs".to_string();
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(error.message().contains("staging path does not match"));
    assert!(specifications.is_dir());
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        ORIGINAL_CANONICAL
    );

    let mut planted = original.clone();
    planted.active_path = "specs/search".to_string();
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(
        error
            .message()
            .contains("active path does not match change")
    );
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        ORIGINAL_CANONICAL
    );

    let mut planted = original.clone();
    planted.archive_path = "specs/search".to_string();
    planted.archive_staged_path = "specs/.search.rune-stage".to_string();
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(
        error
            .message()
            .contains("outside the expected archive subtree")
    );
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        ORIGINAL_CANONICAL
    );

    let mut planted = original;
    planted.canonical[0].path = "changes/change/proposal.md".to_string();
    write_archive_journal(&fixture, &planted);
    let error = acquire_error(&fixture);
    assert!(error.message().contains("outside specifications"));
    assert_eq!(
        fs::read(&fixture.canonical_path).unwrap(),
        ORIGINAL_CANONICAL
    );
    assert!(archive_journal_path(&fixture).is_file());
}

#[test]
fn loaded_journal_rejects_parent_path_escape() {
    let fixture = fail_at_phase(Phase::Prepared);
    let journal_path = fixture
        .spec_root
        .base()
        .join(TRANSACTION_DIRECTORY)
        .join(JOURNAL_FILE);
    let content = fs::read_to_string(&journal_path).unwrap();
    let escaped = content.replacen("active_path: changes/change", "active_path: ../outside", 1);
    fs::write(&journal_path, escaped).unwrap();

    let error = acquire_error(&fixture);

    assert!(error.message().contains("relative to the spec root"));
    assert!(
        fixture
            .spec_root
            .base()
            .join(TRANSACTION_DIRECTORY)
            .is_dir()
    );
}
