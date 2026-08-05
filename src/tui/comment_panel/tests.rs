use super::*;

fn text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn wrapping_matches_tuicr_for_ascii_and_wide_characters() {
    assert_eq!(wrap_segments("hello world", 5), vec!["hello", " worl", "d"]);
    assert_eq!(wrap_segments("a中b文", 3), vec!["a中", "b文"]);
    assert_eq!(wrap_segments("中a", 1), vec!["中", "a"]);
}

#[test]
fn input_box_has_tuicr_border_badge_and_hint() {
    let (lines, cursor) = format_comment_input_lines(
        CommentKind::Issue,
        "fix this",
        4,
        Some(CommentLineRange::new(12, 12)),
        false,
        80,
    );

    assert_eq!(
        text(&lines[0]),
        "    ├── Add [ISSUE] L12 (Tab/S-Tab:type Enter:save Shift-Enter:newline Esc:cancel)"
    );
    assert_eq!(text(&lines[1]), "    │  fix this");
    assert!(text(lines.last().expect("footer")).starts_with("    ╰─"));
    assert_eq!(
        cursor,
        CommentCursorInfo {
            line_offset: 1,
            column: 11
        }
    );
}

#[test]
fn saved_comment_box_wraps_and_renders_kind_badge() {
    let lines = format_comment_lines(
        CommentKind::Suggestion,
        "abcdefghijk",
        Some(CommentLineRange::new(3, 5)),
        18,
    );

    assert_eq!(text(&lines[0]), "    ├── [SUGGESTION] L3-L5 ");
    assert_eq!(text(&lines[1]), "    │  abcdefghi");
    assert_eq!(text(&lines[2]), "    │  jk");
    assert_eq!(text(&lines[3]), "    ╰─────────────");
}

#[test]
fn cursor_tracks_wrapped_wide_text() {
    let (_, cursor) = format_comment_input_lines(CommentKind::Note, "ab中cd", 2, None, true, 12);

    assert_eq!(
        cursor,
        CommentCursorInfo {
            line_offset: 2,
            column: 7
        }
    );
}
