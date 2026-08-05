use super::*;

#[test]
fn rendered_skill_substitutes_version_and_stays_kebab() {
    let content = rendered();
    assert!(!content.contains("${VERSION}"));
    assert!(content.contains(concat!("version: ", env!("CARGO_PKG_VERSION"))));
    assert!(content.starts_with("---\nname: rune\n"));
}

#[test]
fn install_writes_skill_under_the_project_claude_tree() {
    let temp = tempfile::tempdir().expect("tempdir");
    install(Some(temp.path().to_str().expect("utf-8 path")), true).expect("install");
    let written = std::fs::read_to_string(temp.path().join(".claude/skills/rune/SKILL.md"))
        .expect("skill written");
    assert!(written.contains("# rune"));
}
