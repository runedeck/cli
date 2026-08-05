//! tuicr's TUI style palette, adapted to rune's fixed-color interface.

use ratatui::style::{Color, Modifier, Style};
use rune::review::CommentKind;

pub(super) const PANEL_BG: Color = Color::Rgb(24, 24, 28);
pub(super) const BG_HIGHLIGHT: Color = Color::Rgb(70, 70, 70);
pub(super) const FG_PRIMARY: Color = Color::White;
pub(super) const FG_SECONDARY: Color = Color::Rgb(210, 210, 210);
pub(super) const FG_DIM: Color = Color::Rgb(160, 160, 160);
pub(super) const DIFF_ADD: Color = Color::Rgb(80, 220, 120);
pub(super) const DIFF_ADD_BG: Color = Color::Rgb(0, 60, 20);
pub(super) const DIFF_DEL: Color = Color::Rgb(240, 90, 90);
pub(super) const DIFF_DEL_BG: Color = Color::Rgb(70, 0, 0);
pub(super) const DIFF_CONTEXT: Color = Color::Rgb(200, 200, 200);
pub(super) const BORDER_FOCUSED: Color = Color::Rgb(90, 200, 255);
pub(super) const BORDER_UNFOCUSED: Color = Color::Rgb(110, 110, 110);
pub(super) const STATUS_BAR_BG: Color = Color::Rgb(30, 30, 30);
pub(super) const CURSOR_COLOR: Color = Color::Rgb(255, 210, 90);
pub(super) const MODE_FG: Color = Color::Black;
pub(super) const MODE_BG: Color = Color::Rgb(90, 200, 255);

pub(super) fn selected_style() -> Style {
    Style::default().bg(BG_HIGHLIGHT).fg(FG_PRIMARY)
}

pub(super) fn current_line_indicator_style() -> Style {
    Style::default()
        .fg(CURSOR_COLOR)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn dim_style() -> Style {
    Style::default().fg(FG_DIM)
}

pub(super) fn diff_add_style() -> Style {
    Style::default().fg(DIFF_ADD).bg(DIFF_ADD_BG)
}

pub(super) fn diff_del_style() -> Style {
    Style::default().fg(DIFF_DEL).bg(DIFF_DEL_BG)
}

pub(super) fn diff_context_style() -> Style {
    Style::default().fg(DIFF_CONTEXT)
}

pub(super) fn diff_hunk_header_style() -> Style {
    Style::default().fg(FG_DIM).bg(Color::Rgb(42, 42, 46))
}

pub(super) fn file_header_style() -> Style {
    Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD)
}

pub(super) fn border_style(focused: bool) -> Style {
    Style::default().fg(if focused {
        BORDER_FOCUSED
    } else {
        BORDER_UNFOCUSED
    })
}

pub(super) fn panel_style() -> Style {
    Style::default().bg(PANEL_BG).fg(FG_PRIMARY)
}

pub(super) fn status_bar_style() -> Style {
    Style::default().bg(STATUS_BAR_BG).fg(FG_PRIMARY)
}

pub(super) fn mode_style() -> Style {
    Style::default()
        .fg(MODE_FG)
        .bg(MODE_BG)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn comment_type_style(kind: CommentKind) -> Style {
    let color = match kind {
        CommentKind::Note => Color::Rgb(90, 170, 255),
        CommentKind::Suggestion => Color::Rgb(90, 220, 240),
        CommentKind::Issue => Color::Rgb(240, 90, 90),
        CommentKind::Praise => Color::Rgb(80, 220, 120),
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub(super) fn comment_border_style() -> Style {
    file_header_style()
}

#[cfg(test)]
mod tests;
