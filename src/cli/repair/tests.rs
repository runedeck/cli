use super::*;
use rune::manifest;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

/// A deployment fixture: a source with a `build/claude` tree and a target
/// with a `.claude` provider directory. Shared with the doctor tests so both
/// commands see the same layout.
pub(crate) struct Fixture {
    pub(crate) root: TempDir,
    pub(crate) source: PathBuf,
    pub(crate) target_base: PathBuf,
    pub(crate) provider_target: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = TempDir::new().unwrap();
        let source = root.path().join("source");
        let target_base = root.path().join("target");
        let provider_target = target_base.join(".claude");
        fs::create_dir_all(source.join("build/claude/skills/Alpha")).unwrap();
        fs::create_dir_all(provider_target.join("skills/Alpha")).unwrap();
        Self {
            root,
            source,
            target_base,
            provider_target,
        }
    }

    pub(crate) fn write_manifest(&self, entries: &[(&str, &str)]) {
        let entries = entries
            .iter()
            .map(|(path, content)| {
                (
                    (*path).to_string(),
                    manifest::ManifestEntry {
                        fingerprint: manifest::content_sha256(content),
                        provenance: None,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let yaml = manifest::write(&entries).unwrap();
        fs::write(self.provider_target.join(".manifest"), yaml).unwrap();
    }

    pub(crate) fn build_file(&self, relative: &str, content: &str) {
        let path = self.source.join("build/claude").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    pub(crate) fn deployed_file(&self, relative: &str, content: &str) {
        let path = self.provider_target.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn repair(&self, dry_run: bool) -> (Vec<RepairAction>, Vec<doctor::TargetReport>) {
        let targets = doctor::discover_targets(&self.target_base, &self.source).unwrap();
        repair_deployment(&targets, &self.source, dry_run).unwrap()
    }
}

#[test]
fn modified_file_is_reported_and_never_repaired() {
    let fixture = Fixture::new();
    fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
    fixture.build_file("skills/Alpha/SKILL.md", "deployed");
    fixture.deployed_file("skills/Alpha/SKILL.md", "user edit");

    let (repairs, remaining) = fixture.repair(false);

    assert_eq!(
        fs::read_to_string(fixture.provider_target.join("skills/Alpha/SKILL.md")).unwrap(),
        "user edit"
    );
    assert!(remaining[0].findings.iter().any(|finding| {
        finding.status == IntegrityStatus::Modified && finding.path == "skills/Alpha/SKILL.md"
    }));
    assert!(repairs.is_empty());
}

#[test]
fn repair_restores_missing_file_from_digest_matching_build() {
    let fixture = Fixture::new();
    fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
    fixture.build_file("skills/Alpha/SKILL.md", "deployed");

    let (repairs, remaining) = fixture.repair(false);

    assert_eq!(
        fs::read_to_string(fixture.provider_target.join("skills/Alpha/SKILL.md")).unwrap(),
        "deployed"
    );
    assert!(
        remaining[0]
            .findings
            .iter()
            .any(|finding| finding.status == IntegrityStatus::Ok)
    );
    assert_eq!(repairs[0].action, "restored");
}

#[test]
fn repair_does_not_restore_source_with_wrong_digest() {
    let fixture = Fixture::new();
    fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
    fixture.build_file("skills/Alpha/SKILL.md", "new build");

    let (repairs, remaining) = fixture.repair(false);

    assert!(
        !fixture
            .provider_target
            .join("skills/Alpha/SKILL.md")
            .exists()
    );
    assert!(
        remaining[0]
            .findings
            .iter()
            .any(|finding| finding.status == IntegrityStatus::Missing)
    );
    assert!(repairs.is_empty());
}

#[test]
fn repair_quarantines_orphan_under_target_trash() {
    let fixture = Fixture::new();
    fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
    fixture.deployed_file("skills/Alpha/SKILL.md", "deployed");
    fixture.deployed_file("rules/Orphan.md", "orphan");

    let (repairs, remaining) = fixture.repair(false);

    assert!(!fixture.provider_target.join("rules/Orphan.md").exists());
    let quarantine = Path::new(&repairs[0].destination);
    assert!(quarantine.is_file());
    assert!(quarantine.starts_with(fixture.provider_target.join(".trash")));
    assert_eq!(fs::read_to_string(quarantine).unwrap(), "orphan");
    assert!(
        !remaining[0]
            .findings
            .iter()
            .any(|finding| finding.status == IntegrityStatus::Orphan)
    );
}

#[test]
fn dry_run_plans_every_write_and_touches_nothing() {
    let fixture = Fixture::new();
    fixture.write_manifest(&[("skills/Alpha/SKILL.md", "deployed")]);
    fixture.build_file("skills/Alpha/SKILL.md", "deployed");
    fixture.deployed_file("rules/Orphan.md", "orphan");

    let (repairs, remaining) = fixture.repair(true);

    assert!(
        !fixture
            .provider_target
            .join("skills/Alpha/SKILL.md")
            .exists()
    );
    assert!(fixture.provider_target.join("rules/Orphan.md").is_file());
    assert!(!fixture.provider_target.join(".trash").exists());
    // Findings sort by path, so the orphan under rules/ precedes skills/.
    let actions: Vec<_> = repairs
        .iter()
        .map(|repair| repair.action.as_str())
        .collect();
    assert_eq!(actions, ["would quarantine", "would restore"]);
    assert!(doctor::target_is_broken(&remaining[0]));
}

#[test]
fn fixture_keeps_temp_directory_alive() {
    let fixture = Fixture::new();
    assert!(fixture.root.path().exists());
}
