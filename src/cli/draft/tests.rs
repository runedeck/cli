use super::{DraftKind, create, deployed_provider_dirs, drop, list};
use rune::manifest::drafts::Register;
use std::fs;
use std::path::Path;

/// A consumer with a `.manifest` in each named provider directory.
fn consumer(providers: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for provider in providers {
        let root = dir.path().join(provider);
        fs::create_dir_all(root.join("skills")).unwrap();
        fs::write(
            root.join(".manifest"),
            "skills:\n    Managed:\n        SKILL.md:\n            fingerprint: abc\n",
        )
        .unwrap();
    }
    dir
}

fn exists(root: &Path, relative: &str) -> bool {
    root.join(relative).exists()
}

#[test]
fn draft_skill_lands_in_every_deployed_provider_and_registers() {
    let dir = consumer(&[".claude", ".codex"]);
    fs::create_dir_all(dir.path().join(".gemini")).unwrap(); // no manifest: not deployed
    let written = create(dir.path(), DraftKind::Skill, "ReviewSpec").unwrap();
    assert_eq!(
        written,
        vec![
            ".claude/skills/ReviewSpec/SKILL.md",
            ".codex/skills/ReviewSpec/SKILL.md"
        ]
    );
    assert!(exists(dir.path(), ".claude/skills/ReviewSpec/SKILL.md"));
    assert!(!exists(dir.path(), ".gemini/skills/ReviewSpec/SKILL.md"));
    let register = Register::load(dir.path()).unwrap();
    assert_eq!(
        register.paths(),
        written.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let manifest = fs::read_to_string(dir.path().join(".claude/.manifest")).unwrap();
    assert!(
        !manifest.contains("ReviewSpec"),
        "the manifest never lists a draft"
    );
}

#[test]
fn draft_refuses_a_managed_name() {
    let dir = consumer(&[".claude"]);
    let error = create(dir.path(), DraftKind::Skill, "Managed").unwrap_err();
    assert!(error.to_string().contains("managed rune"), "{error}");
}

#[test]
fn draft_refuses_without_a_deployed_provider() {
    let dir = tempfile::tempdir().unwrap();
    let error = create(dir.path(), DraftKind::Rule, "Foo").unwrap_err();
    assert!(
        error.to_string().contains("run rune install first"),
        "{error}"
    );
}

#[test]
fn draft_refuses_a_path_in_the_name() {
    let dir = consumer(&[".claude"]);
    assert!(create(dir.path(), DraftKind::Skill, "../Foo").is_err());
}

#[test]
fn drop_removes_every_copy_and_the_register_entry() {
    let dir = consumer(&[".claude", ".codex"]);
    create(dir.path(), DraftKind::Skill, "Foo").unwrap();
    create(dir.path(), DraftKind::Rule, "Bar").unwrap();
    let removed = drop(dir.path(), "Foo").unwrap();
    assert_eq!(removed.len(), 2);
    assert!(
        !exists(dir.path(), ".claude/skills/Foo"),
        "empty skill directory is removed"
    );
    assert!(exists(dir.path(), ".claude/rules/Bar.md"));
    assert_eq!(
        Register::load(dir.path()).unwrap().paths(),
        vec![".claude/rules/Bar.md", ".codex/rules/Bar.md"]
    );
    assert!(
        drop(dir.path(), "Foo").is_err(),
        "a second drop finds nothing"
    );
}

#[test]
fn list_marks_a_missing_file() {
    let dir = consumer(&[".claude"]);
    create(dir.path(), DraftKind::Agent, "Critic").unwrap();
    fs::remove_file(dir.path().join(".claude/agents/Critic.md")).unwrap();
    let lines = list(dir.path()).unwrap();
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].starts_with("Critic  agent  0 days"),
        "{}",
        lines[0]
    );
    assert!(lines[0].ends_with("(missing)"), "{}", lines[0]);
}

#[test]
fn only_directories_with_a_manifest_count_as_deployed() {
    let dir = consumer(&[".claude"]);
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    let dirs = deployed_provider_dirs(dir.path()).unwrap();
    assert_eq!(dirs, vec![dir.path().join(".claude")]);
}

#[test]
fn one_name_means_one_kind() {
    let dir = consumer(&[".claude"]);
    create(dir.path(), DraftKind::Skill, "Foo").unwrap();
    let error = create(dir.path(), DraftKind::Rule, "Foo").unwrap_err();
    assert!(
        error.to_string().contains("one name means one kind"),
        "{error}"
    );
    assert!(
        !exists(dir.path(), ".claude/rules/Foo.md"),
        "the refusal wrote nothing"
    );
    assert_eq!(Register::load(dir.path()).unwrap().drafts.len(), 1);
}

#[test]
fn a_refusal_in_the_second_provider_writes_nothing_in_the_first() {
    let dir = consumer(&[".claude", ".codex"]);
    fs::create_dir_all(dir.path().join(".codex/rules")).unwrap();
    fs::write(dir.path().join(".codex/rules/Bar.md"), "hand-written\n").unwrap();
    assert!(create(dir.path(), DraftKind::Rule, "Bar").is_err());
    assert!(!exists(dir.path(), ".claude/rules/Bar.md"));
    assert!(!exists(dir.path(), ".drafts"));
}

#[test]
fn register_paths_stay_inside_the_consumer() {
    let dir = consumer(&[".claude"]);
    fs::write(
        dir.path().join(".drafts"),
        "drafts:\n  - path: ../victim\n    kind: rule\n    name: V\n    created: now\n",
    )
    .unwrap();
    assert!(
        drop(dir.path(), "V").is_err(),
        "an escaping entry never reaches remove_file"
    );
    assert!(list(dir.path()).is_err());
}
