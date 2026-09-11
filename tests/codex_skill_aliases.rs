use rune::skill_readiness::{self, SkillReadiness};
use std::fs;
use std::path::{Path, PathBuf};

const SKILL: &str = "---\nname: AliasCanary\ndescription: Identify a harmless alias fixture.\n---\nRead guide.md.\n";

fn write_skill(workspace: &Path, folder: &str) -> PathBuf {
    let skill = workspace.join(".agents/skills").join(folder);
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), SKILL).unwrap();
    fs::write(skill.join("guide.md"), "Harmless companion.\n").unwrap();
    skill
}

fn inspect(workspace: &Path) -> SkillReadiness {
    skill_readiness::inspect(
        workspace,
        &[(workspace.join(".agents/skills"), "repository".into())],
        "alias fixture".into(),
    )
}

fn assert_duplicate_paths(report: &SkillReadiness, first: &Path, second: &Path) {
    assert_eq!(report.skills.len(), 2, "{report:#?}");
    let duplicates: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.code == "CSI001_DUPLICATE_NAME")
        .collect();
    assert_eq!(duplicates.len(), 1, "{report:#?}");
    assert_eq!(duplicates[0].identity.as_deref(), Some("AliasCanary"));
    let mut expected = vec![
        first.join("SKILL.md").display().to_string(),
        second.join("SKILL.md").display().to_string(),
    ];
    expected.sort();
    assert_eq!(duplicates[0].paths, expected);
    assert!(!report.static_valid);
    assert_eq!(report.native_discovery, "unverified");
    assert!(!report.accepted);
}

#[cfg(unix)]
#[test]
fn sibling_directory_aliases_remain_distinct_without_native_evidence() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().canonicalize().unwrap();
    let original = write_skill(&workspace, "original");
    let alias = original.parent().unwrap().join("alias");
    symlink("original", &alias).unwrap();
    assert_eq!(alias.canonicalize().unwrap(), original);

    let report = inspect(&workspace);
    // Static candidates retain logical paths until native evidence resolves discovery.
    assert_duplicate_paths(&report, &original, &alias);
}

#[test]
fn distinct_physical_same_name_skills_remain_duplicates() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().canonicalize().unwrap();
    let first = write_skill(&workspace, "first");
    let second = write_skill(&workspace, "second");
    assert_ne!(
        first.canonicalize().unwrap(),
        second.canonicalize().unwrap()
    );
    assert_eq!(
        fs::read(first.join("SKILL.md")).unwrap(),
        fs::read(second.join("SKILL.md")).unwrap()
    );

    let report = inspect(&workspace);
    assert_duplicate_paths(&report, &first, &second);
    assert_eq!(
        report.skills[0].bundle.digest,
        report.skills[1].bundle.digest
    );
}
