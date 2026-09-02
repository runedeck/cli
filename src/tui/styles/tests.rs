use super::*;
use crate::cli::theme::{DEFAULT_DARK, DEFAULT_LIGHT, named};

fn tone(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(red, green, blue) => (red, green, blue),
        other => panic!("expected an RGB color, got {other:?}"),
    }
}

#[test]
fn dark_palette_keeps_the_dark_surfaces() {
    let palette = Palette::from_theme(&named(DEFAULT_DARK).unwrap());

    assert_eq!(palette.status_bar_bg, Color::Rgb(30, 30, 30));
    assert_eq!(palette.bg_highlight, Color::Rgb(70, 70, 70));
    assert_eq!(palette.fg_primary, Color::White);
    assert_eq!(palette.mode_fg, Color::Black);
}

#[test]
fn light_palette_flips_surfaces_and_text() {
    let palette = Palette::from_theme(&named(DEFAULT_LIGHT).unwrap());

    assert_eq!(palette.fg_primary, Color::Black);
    assert_eq!(palette.mode_fg, Color::White);
    let (red, green, blue) = tone(palette.panel_bg);
    assert!(red > 200 && green > 200 && blue > 200, "light panel");
    let (red, green, blue) = tone(palette.status_bar_bg);
    assert!(red > 200 && green > 200 && blue > 200, "light status bar");
}

#[test]
fn semantic_colors_follow_the_theme_tones() {
    let tones = named("nord").unwrap();
    let palette = Palette::from_theme(&tones);

    assert_eq!(tone(palette.border_focused), tones.accent.rgb);
    assert_eq!(tone(palette.mode_bg), tones.accent.rgb);
    assert_eq!(tone(palette.cursor), tones.alert.rgb);
    assert_eq!(tone(palette.diff_add), tones.good.rgb);
    assert_eq!(tone(palette.diff_del), tones.bad.rgb);
}

#[test]
fn comment_badges_use_theme_tones() {
    let tones = crate::cli::theme::current();

    assert_eq!(
        tone(comment_type_style(CommentKind::Issue).fg.unwrap()),
        tones.bad.rgb
    );
    assert_eq!(
        tone(comment_type_style(CommentKind::Note).fg.unwrap()),
        tones.accent.rgb
    );
    assert_eq!(
        tone(comment_type_style(CommentKind::Suggestion).fg.unwrap()),
        tones.violet.rgb
    );
    assert_eq!(
        tone(comment_type_style(CommentKind::Praise).fg.unwrap()),
        tones.good.rgb
    );
}
