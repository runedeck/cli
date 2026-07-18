use super::obsidian::{from_obsidian, to_obsidian};
use super::parse::{parse_line, render_line};
use super::*;

#[test]
fn parse_and_render_round_trip_open_and_done_items() {
    let open = "(A) 2026-07-18 fix consumer validate +rune @cli due:2026-07-20";
    let done = "x 2026-07-18 2026-07-17 ship the release +release";

    let open_item = parse_line(open);
    assert_eq!(open_item.priority, Some('A'));
    assert_eq!(open_item.creation_date.as_deref(), Some("2026-07-18"));
    assert_eq!(open_item.projects, vec!["rune"]);
    assert_eq!(open_item.contexts, vec!["cli"]);
    assert_eq!(
        open_item.extensions,
        vec![("due".to_string(), "2026-07-20".to_string())]
    );
    assert_eq!(render_line(&open_item), open);

    let done_item = parse_line(done);
    assert!(done_item.done);
    assert_eq!(done_item.completion_date.as_deref(), Some("2026-07-18"));
    assert_eq!(done_item.creation_date.as_deref(), Some("2026-07-17"));
    assert_eq!(render_line(&done_item), done);
}

#[test]
fn urls_are_not_mistaken_for_extensions() {
    let item = parse_line("read https://example.com/guide +docs");
    assert!(item.extensions.is_empty(), "{:?}", item.extensions);
    assert_eq!(item.projects, vec!["docs"]);
}

#[test]
fn obsidian_round_trip_preserves_representable_fields() {
    let source = "(A) 2026-07-18 fix consumer validate +rune @cli due:2026-07-20";
    let item = parse_line(source);

    let markdown = to_obsidian(&item);
    assert!(markdown.starts_with("- [ ]"), "{markdown}");
    assert!(markdown.contains("#rune"), "{markdown}");
    assert!(markdown.contains("📅 2026-07-20"), "{markdown}");
    assert!(markdown.contains('⏫'), "{markdown}");

    let back = from_obsidian(&markdown).unwrap();
    assert_eq!(back.priority, Some('A'));
    assert_eq!(back.creation_date.as_deref(), Some("2026-07-18"));
    assert_eq!(back.projects, vec!["rune"]);
    assert_eq!(back.contexts, vec!["cli"]);
    assert_eq!(
        back.extensions,
        vec![("due".to_string(), "2026-07-20".to_string())]
    );
}

#[test]
fn add_do_and_ls_operate_on_the_repo_todo_file() {
    let temp = tempfile::tempdir().unwrap();
    execute_at(
        temp.path(),
        Some(TodoAction::Add {
            text: "(B) write the walkthrough +docs".to_string(),
        }),
        true,
    )
    .unwrap();
    execute_at(
        temp.path(),
        Some(TodoAction::Add {
            text: "sharpen the sigil +brand".to_string(),
        }),
        true,
    )
    .unwrap();
    execute_at(temp.path(), Some(TodoAction::Do { position: 1 }), true).unwrap();

    let written = std::fs::read_to_string(temp.path().join("TODO.txt")).unwrap();
    assert!(written.starts_with("x "), "{written}");
    assert!(written.contains("write the walkthrough"), "{written}");
    assert!(written.contains("sharpen the sigil"), "{written}");
    assert!(written.ends_with('\n'));

    let missing = execute_at(temp.path(), Some(TodoAction::Do { position: 9 }), true).unwrap_err();
    assert!(missing.to_string().contains("no task 9"), "{missing}");
}
