use super::*;
use crate::spec::transaction::ConversionPhase;
use std::cell::Cell;
use tempfile::TempDir;

#[test]
fn conversion_report_output_is_frozen() {
    let report = ConversionReport {
        converted: 12,
        destination: "docs".to_string(),
        recovered: true,
    };
    let json = format!("{}\n", render_report_json(&report).unwrap());
    assert_report_fixture(&json, "tests/fixtures/output/conversion-report.json");
    let sheet = crate::sheet::Sheet::detect();
    assert_report_fixture(
        &render_report(&report, &sheet),
        "tests/fixtures/output/conversion-report.txt",
    );
}

fn assert_report_fixture(rendered: &str, relative: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if std::env::var_os("RUNE_UPDATE_FIXTURES").is_some() {
        fs::write(&path, rendered).unwrap();
        return;
    }
    let fixture = fs::read_to_string(&path).unwrap();
    assert_eq!(
        rendered, fixture,
        "{relative} no longer matches; regenerate with RUNE_UPDATE_FIXTURES=1 if intentional"
    );
}

#[test]
fn reserved_state_paths_agree_with_the_transaction_layer() {
    assert!(is_reserved_state_path(TRANSACTION_DIRECTORY));
    assert!(is_reserved_state_path(&format!(
        "{TRANSACTION_DIRECTORY}/{}",
        crate::spec::transaction::JOURNAL_FILE
    )));
    assert!(is_reserved_state_path(INTEROP_DIRECTORY));
    assert!(is_reserved_state_path(&format!(
        "{INTEROP_DIRECTORY}/manifest.yaml"
    )));
    assert!(!is_reserved_state_path("changes/add-widget/proposal.md"));
    assert!(!is_reserved_state_path(".rune-transaction-notes.md"));
    assert_eq!(LOCK_FILE, crate::spec::transaction::LOCK_FILE);
}

const PROJECT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interop/project.md"
));
const OPEN_SPEC_CONFIG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interop/config.yaml"
));
const SCHEMA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interop/change-schema.yaml"
));

fn configure_spec_root(repository: &Path, selected: &str) {
    let config = format!("spec:\n    root: {selected}\n");
    fs::write(repository.join("config.yaml"), config).unwrap();
    crate::spec::set_root_config_lookup(test_config_lookup);
}

fn test_config_lookup(repository: &Path) -> Result<Option<String>, String> {
    let path = repository.join("config.yaml");
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let value: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|error| error.to_string())?;
    Ok(value
        .get("spec")
        .and_then(|spec| spec.get("root"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string))
}

fn write_bytes(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn seed_openspec(repository: &Path) {
    write_bytes(
        repository,
        "openspec/changes/add-widget/proposal.md",
        b"# Widget proposal\n",
    );
    write_bytes(
        repository,
        "openspec/specs/widgets/spec.md",
        b"# Widget specification\n",
    );
    write_bytes(repository, "openspec/project.md", PROJECT);
    write_bytes(repository, "openspec/config.yaml", OPEN_SPEC_CONFIG);
    write_bytes(repository, "openspec/schemas/change.yaml", SCHEMA);
    write_bytes(repository, "openspec/assets/payload.bin", &[0, 1, 127, 255]);
}

#[test]
fn exact_round_trip_preserves_direct_and_top_level_files() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());
    write_bytes(repository.path(), "docs/unrelated.txt", b"leave me\n");

    import_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(
        fs::read(
            repository
                .path()
                .join("docs/changes/add-widget/proposal.md")
        )
        .unwrap(),
        b"# Widget proposal\n"
    );
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("docs/.interop/openspec/files/project.md")
        )
        .unwrap(),
        PROJECT
    );
    assert!(!repository.path().join("openspec/project.md").exists());
    assert!(
        repository
            .path()
            .join("docs/.interop/openspec/manifest.yaml")
            .is_file()
    );

    export_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(
        fs::read(repository.path().join("openspec/project.md")).unwrap(),
        PROJECT
    );
    assert_eq!(
        fs::read(repository.path().join("openspec/config.yaml")).unwrap(),
        OPEN_SPEC_CONFIG
    );
    assert_eq!(
        fs::read(repository.path().join("openspec/schemas/change.yaml")).unwrap(),
        SCHEMA
    );
    assert_eq!(
        fs::read(repository.path().join("openspec/assets/payload.bin")).unwrap(),
        [0, 1, 127, 255]
    );
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("openspec/changes/add-widget/proposal.md")
        )
        .unwrap(),
        b"# Widget proposal\n"
    );
    assert_eq!(
        fs::read(repository.path().join("openspec/specs/widgets/spec.md")).unwrap(),
        b"# Widget specification\n"
    );
    assert!(
        !repository
            .path()
            .join("openspec/.interop/openspec/manifest.yaml")
            .exists()
    );
    assert!(
        !repository
            .path()
            .join("docs/changes/add-widget/proposal.md")
            .exists()
    );
    assert!(
        !repository
            .path()
            .join("docs/.interop/openspec/manifest.yaml")
            .exists()
    );
    assert_eq!(
        fs::read(repository.path().join("docs/unrelated.txt")).unwrap(),
        b"leave me\n"
    );
}

#[test]
fn native_export_does_not_synthesize_project_file() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    write_bytes(
        repository.path(),
        "docs/specs/widgets/spec.md",
        b"# Widget specification\n",
    );

    export_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(
        fs::read(repository.path().join("openspec/specs/widgets/spec.md")).unwrap(),
        b"# Widget specification\n"
    );
    assert!(!repository.path().join("openspec/project.md").exists());
}

#[test]
fn manifest_records_classification_and_hashes() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());

    import_openspec(&repository.path().to_string_lossy(), true).unwrap();

    let spec_root = resolve_spec_root(repository.path()).unwrap();
    let manifest = load_manifest(&spec_root).unwrap();
    assert_eq!(manifest.version, MANIFEST_VERSION);
    let project = manifest
        .entries
        .iter()
        .find(|entry| entry.path == "project.md")
        .unwrap();
    assert_eq!(project.classification, Classification::File);
    assert_eq!(
        project.sha256,
        transaction::sha256_file(&spec_root.base().join(MIRROR_DIRECTORY).join("project.md"))
            .unwrap()
    );
    assert!(manifest.entries.iter().any(|entry| {
        entry.path == "changes/add-widget/proposal.md"
            && entry.classification == Classification::Change
    }));
}

#[test]
fn manifest_validation_rejects_unsupported_and_malformed_ownership() {
    let manifest_path = Path::new("manifest.yaml");
    let valid_entry = ManifestEntry {
        path: "project.md".to_string(),
        classification: Classification::File,
        sha256: "0".repeat(64),
    };
    let mut manifest = Manifest {
        version: MANIFEST_VERSION,
        entries: vec![valid_entry.clone()],
    };

    manifest.version = MANIFEST_VERSION + 1;
    assert!(
        validate_manifest(&manifest, manifest_path)
            .unwrap_err()
            .message()
            .contains("unsupported")
    );

    manifest.version = MANIFEST_VERSION;
    manifest.entries[0].path = "../project.md".to_string();
    assert!(
        validate_manifest(&manifest, manifest_path)
            .unwrap_err()
            .message()
            .contains("relative")
    );

    manifest.entries[0].path = "/project.md".to_string();
    assert!(
        validate_manifest(&manifest, manifest_path)
            .unwrap_err()
            .message()
            .contains("relative")
    );

    manifest.entries[0] = valid_entry.clone();
    manifest.entries[0].classification = Classification::Change;
    assert!(
        validate_manifest(&manifest, manifest_path)
            .unwrap_err()
            .message()
            .contains("does not own")
    );

    manifest.entries[0] = valid_entry.clone();
    manifest.entries[0].sha256 = "invalid".to_string();
    assert!(
        validate_manifest(&manifest, manifest_path)
            .unwrap_err()
            .message()
            .contains("SHA-256")
    );

    manifest.entries = vec![valid_entry.clone(), valid_entry];
    assert!(
        validate_manifest(&manifest, manifest_path)
            .unwrap_err()
            .message()
            .contains("duplicate OpenSpec ownership")
    );
}

#[test]
fn tampered_mirror_refuses_export_without_source_deletion() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());
    import_openspec(&repository.path().to_string_lossy(), true).unwrap();
    let mirror = repository
        .path()
        .join("docs/.interop/openspec/files/project.md");
    fs::write(&mirror, b"tampered\n").unwrap();

    let error = export_openspec(&repository.path().to_string_lossy(), true).unwrap_err();

    assert!(error.message().contains("hash mismatch"), "{error}");
    assert!(mirror.is_file());
    assert!(
        repository
            .path()
            .join("docs/changes/add-widget/proposal.md")
            .is_file()
    );
    assert!(!repository.path().join("openspec/project.md").exists());
}

#[test]
fn duplicate_manifest_ownership_refuses_export_without_source_deletion() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());
    import_openspec(&repository.path().to_string_lossy(), true).unwrap();
    let manifest_path = repository
        .path()
        .join("docs/.interop/openspec/manifest.yaml");
    let mut manifest = load_manifest(&resolve_spec_root(repository.path()).unwrap()).unwrap();
    manifest.entries.push(manifest.entries[0].clone());
    fs::write(&manifest_path, serialize_manifest(&manifest).unwrap()).unwrap();

    let error = export_openspec(&repository.path().to_string_lossy(), true).unwrap_err();

    assert!(
        error.message().contains("duplicate OpenSpec ownership"),
        "{error}"
    );
    assert!(
        repository
            .path()
            .join("docs/changes/add-widget/proposal.md")
            .is_file()
    );
    assert!(!repository.path().join("openspec/project.md").exists());
}

#[test]
fn import_collision_refuses_before_source_mutation() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());
    write_bytes(
        repository.path(),
        "docs/changes/add-widget/proposal.md",
        b"existing\n",
    );

    let error = import_openspec(&repository.path().to_string_lossy(), true).unwrap_err();

    assert!(error.message().contains("refuses to overwrite"), "{error}");
    assert!(repository.path().join("openspec/project.md").is_file());
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("docs/changes/add-widget/proposal.md")
        )
        .unwrap(),
        b"existing\n"
    );
}

#[cfg(unix)]
#[test]
fn import_symlink_refuses_before_source_mutation() {
    let repository = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    fs::create_dir_all(repository.path().join("openspec/changes")).unwrap();
    write_bytes(outside.path(), "proposal.md", b"outside\n");
    std::os::unix::fs::symlink(
        outside.path().join("proposal.md"),
        repository.path().join("openspec/changes/proposal.md"),
    )
    .unwrap();

    let error = import_openspec(&repository.path().to_string_lossy(), true).unwrap_err();

    assert!(error.message().contains("symlink"), "{error}");
    assert!(outside.path().join("proposal.md").is_file());
}

#[test]
fn custom_and_openspec_selected_roots_preserve_ownership() {
    let custom = TempDir::new().unwrap();
    configure_spec_root(custom.path(), "artifacts/specifications");
    seed_openspec(custom.path());
    import_openspec(&custom.path().to_string_lossy(), true).unwrap();
    assert!(
        custom
            .path()
            .join("artifacts/specifications/changes/add-widget/proposal.md")
            .is_file()
    );
    export_openspec(&custom.path().to_string_lossy(), true).unwrap();
    assert_eq!(
        fs::read(custom.path().join("openspec/project.md")).unwrap(),
        PROJECT
    );

    let selected_openspec = TempDir::new().unwrap();
    configure_spec_root(selected_openspec.path(), "openspec");
    seed_openspec(selected_openspec.path());
    import_openspec(&selected_openspec.path().to_string_lossy(), true).unwrap();
    assert!(
        selected_openspec
            .path()
            .join("openspec/changes/add-widget/proposal.md")
            .is_file()
    );
    assert!(
        selected_openspec
            .path()
            .join("openspec/.interop/openspec/files/project.md")
            .is_file()
    );
    export_openspec(&selected_openspec.path().to_string_lossy(), true).unwrap();
    assert_eq!(
        fs::read(selected_openspec.path().join("openspec/project.md")).unwrap(),
        PROJECT
    );
}

#[derive(Default)]
struct FailAfterDestinationsComplete;

impl TransactionIo for FailAfterDestinationsComplete {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }

    fn conversion_phase_persisted(&self, phase: ConversionPhase) -> io::Result<()> {
        if phase == ConversionPhase::DestinationsComplete {
            Err(io::Error::other("injected destination completion failure"))
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct FailAtCleanupPhase;

impl TransactionIo for FailAtCleanupPhase {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }

    fn conversion_phase_persisted(&self, phase: ConversionPhase) -> io::Result<()> {
        if phase == ConversionPhase::Cleanup {
            Err(io::Error::other("injected cleanup failure"))
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct FailAfterSourceRemoval {
    failed: Cell<bool>,
}

impl TransactionIo for FailAfterSourceRemoval {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }

    fn source_removed(&self, _path: &Path) -> io::Result<()> {
        if self.failed.replace(true) {
            Ok(())
        } else {
            Err(io::Error::other("injected source removal failure"))
        }
    }
}

#[test]
fn import_recovery_finishes_source_removal_through_shared_transaction() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());

    let error = import_openspec_with_io(
        &repository.path().to_string_lossy(),
        None,
        FailAfterSourceRemoval::default(),
    )
    .unwrap_err();
    assert!(error.message().contains("injected source removal failure"));

    let spec_root = resolve_spec_root(repository.path()).unwrap();
    drop(transaction::acquire(&spec_root).unwrap());

    assert!(!repository.path().join("openspec/project.md").exists());
    assert!(
        repository
            .path()
            .join("docs/changes/add-widget/proposal.md")
            .is_file()
    );
    assert!(
        repository
            .path()
            .join("docs/.interop/openspec/manifest.yaml")
            .is_file()
    );
}

#[test]
fn unconfigured_export_retry_recovers_the_transaction_owning_root() {
    let repository = TempDir::new().unwrap();
    let proposal = repository
        .path()
        .join("docs/changes/add-widget/proposal.md");
    fs::create_dir_all(proposal.parent().unwrap()).unwrap();
    fs::write(&proposal, PROJECT).unwrap();

    let error = export_openspec_with_io(
        &repository.path().to_string_lossy(),
        FailAfterDestinationsComplete,
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("injected destination completion failure")
    );
    assert!(proposal.is_file());
    assert!(
        repository
            .path()
            .join("openspec/changes/add-widget/proposal.md")
            .is_file()
    );
    assert!(
        repository
            .path()
            .join("docs/.rune-transaction/journal.yaml")
            .is_file()
    );

    export_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert!(!repository.path().join("docs/changes").exists());
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("openspec/changes/add-widget/proposal.md")
        )
        .unwrap(),
        PROJECT
    );
    assert!(!repository.path().join("docs/.rune-transaction").exists());
    assert_eq!(
        resolve_spec_root(repository.path()).unwrap().layout(),
        crate::spec::SpecLayout::OpenSpec
    );
}

#[test]
fn export_recovery_finishes_source_removal_through_shared_transaction() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());
    import_openspec(&repository.path().to_string_lossy(), true).unwrap();

    let error = export_openspec_with_io(
        &repository.path().to_string_lossy(),
        FailAfterSourceRemoval::default(),
    )
    .unwrap_err();
    assert!(error.message().contains("injected source removal failure"));

    let spec_root = resolve_spec_root(repository.path()).unwrap();
    drop(transaction::acquire(&spec_root).unwrap());

    assert_eq!(
        fs::read(repository.path().join("openspec/project.md")).unwrap(),
        PROJECT
    );
    assert!(
        !repository
            .path()
            .join("docs/changes/add-widget/proposal.md")
            .exists()
    );
    assert!(
        !repository
            .path()
            .join("docs/.interop/openspec/manifest.yaml")
            .exists()
    );
}

#[test]
fn round_trip_without_opaque_files_survives_the_absent_mirror_directory() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "openspec");
    write_bytes(
        repository.path(),
        "openspec/changes/add-widget/proposal.md",
        b"proposal\n",
    );
    write_bytes(
        repository.path(),
        "openspec/specs/widget/spec.md",
        b"specification\n",
    );

    import_openspec(&repository.path().to_string_lossy(), true).unwrap();
    assert!(
        !repository
            .path()
            .join("openspec/.interop/openspec/files")
            .exists()
    );

    let status = export_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(status, 0);
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("openspec/changes/add-widget/proposal.md")
        )
        .unwrap(),
        b"proposal\n"
    );
    assert!(
        !repository
            .path()
            .join("openspec/.interop/openspec/manifest.yaml")
            .exists()
    );
}

#[test]
fn export_retry_after_crash_recovery_reports_completed_conversion() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());
    import_openspec(&repository.path().to_string_lossy(), true).unwrap();

    let error = export_openspec_with_io(
        &repository.path().to_string_lossy(),
        FailAfterSourceRemoval::default(),
    )
    .unwrap_err();
    assert!(error.message().contains("injected source removal failure"));

    let status = export_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(status, 0);
    assert_eq!(
        fs::read(repository.path().join("openspec/project.md")).unwrap(),
        PROJECT
    );
    assert!(
        !repository
            .path()
            .join("docs/changes/add-widget/proposal.md")
            .exists()
    );
    assert!(!repository.path().join("docs/.rune-transaction").exists());
    assert!(
        !repository
            .path()
            .join("docs/.interop/openspec/manifest.yaml")
            .exists()
    );
}

#[test]
fn unconfigured_export_retry_after_completed_conversion_reports_success() {
    let repository = TempDir::new().unwrap();
    let proposal = repository
        .path()
        .join("docs/changes/add-widget/proposal.md");
    fs::create_dir_all(proposal.parent().unwrap()).unwrap();
    fs::write(&proposal, PROJECT).unwrap();

    let error = export_openspec_with_io(&repository.path().to_string_lossy(), FailAtCleanupPhase)
        .unwrap_err();
    assert!(error.message().contains("injected cleanup failure"));
    assert!(
        repository
            .path()
            .join("docs/.rune-transaction/journal.yaml")
            .is_file()
    );

    let status = export_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(status, 0);
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("openspec/changes/add-widget/proposal.md")
        )
        .unwrap(),
        PROJECT
    );
    assert!(!repository.path().join("docs/.rune-transaction").exists());
    assert_eq!(
        resolve_spec_root(repository.path()).unwrap().layout(),
        crate::spec::SpecLayout::OpenSpec
    );
}

#[test]
fn import_retry_after_crash_recovery_reports_success() {
    let repository = TempDir::new().unwrap();
    configure_spec_root(repository.path(), "docs");
    seed_openspec(repository.path());

    let error = import_openspec_with_io(
        &repository.path().to_string_lossy(),
        None,
        FailAfterSourceRemoval::default(),
    )
    .unwrap_err();
    assert!(error.message().contains("injected source removal failure"));

    let status = import_openspec(&repository.path().to_string_lossy(), true).unwrap();

    assert_eq!(status, 0);
    assert_eq!(
        fs::read(
            repository
                .path()
                .join("docs/changes/add-widget/proposal.md")
        )
        .unwrap(),
        b"# Widget proposal\n"
    );
    assert!(
        repository
            .path()
            .join("docs/.interop/openspec/manifest.yaml")
            .is_file()
    );
    assert!(!repository.path().join("openspec/project.md").exists());
}
