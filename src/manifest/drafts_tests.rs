use super::{DRAFTS_FILE, Draft, Register, parse, render};
use chrono::{TimeZone, Utc};

fn draft(path: &str, name: &str) -> Draft {
    Draft {
        path: path.to_string(),
        kind: "skill".to_string(),
        name: name.to_string(),
        created: Draft::stamp(Utc.with_ymd_and_hms(2026, 9, 20, 12, 0, 0).unwrap()),
    }
}

#[test]
fn empty_text_is_an_empty_register() {
    assert_eq!(parse("").unwrap(), Register::default());
    assert_eq!(parse("   \n").unwrap(), Register::default());
}

#[test]
fn render_then_parse_round_trips() {
    let mut register = Register::default();
    register
        .add(draft(".claude/skills/Foo/SKILL.md", "Foo"))
        .unwrap();
    register
        .add(draft(".codex/skills/Foo/SKILL.md", "Foo"))
        .unwrap();
    let text = render(&register).unwrap();
    assert_eq!(parse(&text).unwrap(), register);
}

#[test]
fn duplicate_path_is_refused() {
    let mut register = Register::default();
    register
        .add(draft(".claude/skills/Foo/SKILL.md", "Foo"))
        .unwrap();
    let error = register
        .add(draft(".claude/skills/Foo/SKILL.md", "Foo"))
        .unwrap_err();
    assert!(error.contains("already a registered draft"), "{error}");
}

#[test]
fn remove_name_drops_every_provider_copy() {
    let mut register = Register::default();
    register
        .add(draft(".claude/skills/Foo/SKILL.md", "Foo"))
        .unwrap();
    register
        .add(draft(".codex/skills/Foo/SKILL.md", "Foo"))
        .unwrap();
    register.add(draft(".claude/rules/Bar.md", "Bar")).unwrap();
    let removed = register.remove_name("Foo");
    assert_eq!(removed.len(), 2);
    assert_eq!(register.paths(), vec![".claude/rules/Bar.md"]);
}

#[test]
fn age_is_whole_days() {
    let entry = draft(".claude/skills/Foo/SKILL.md", "Foo");
    let now = Utc.with_ymd_and_hms(2026, 9, 23, 11, 0, 0).unwrap();
    assert_eq!(entry.age_days(now), Some(2));
    assert_eq!(entry.created, "2026-09-20T12:00:00Z");
}

#[test]
fn load_of_missing_file_is_empty_and_save_of_empty_removes_it() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Register::load(dir.path()).unwrap(), Register::default());
    let mut register = Register::default();
    register
        .add(draft(".claude/skills/Foo/SKILL.md", "Foo"))
        .unwrap();
    register.save(dir.path()).unwrap();
    assert!(dir.path().join(DRAFTS_FILE).exists());
    Register::default().save(dir.path()).unwrap();
    assert!(!dir.path().join(DRAFTS_FILE).exists());
}
