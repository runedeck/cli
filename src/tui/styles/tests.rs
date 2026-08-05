use super::*;

#[test]
fn status_bar_uses_tuicr_default_dark_palette() {
    let style = status_bar_style();

    assert_eq!(style.bg, Some(Color::Rgb(30, 30, 30)));
    assert_eq!(style.fg, Some(Color::White));
}

#[test]
fn comment_badges_use_tuicr_kind_colors() {
    assert_eq!(
        comment_type_style(CommentKind::Issue).fg,
        Some(Color::Rgb(240, 90, 90))
    );
    assert_eq!(
        comment_type_style(CommentKind::Note).fg,
        Some(Color::Rgb(90, 170, 255))
    );
    assert_eq!(
        comment_type_style(CommentKind::Suggestion).fg,
        Some(Color::Rgb(90, 220, 240))
    );
    assert_eq!(
        comment_type_style(CommentKind::Praise).fg,
        Some(Color::Rgb(80, 220, 120))
    );
}
