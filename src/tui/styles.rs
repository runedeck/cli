//! The TUI palette, derived from the resolved theme.
//!
//! Every color the TUI paints comes through here. The five theme tones
//! (accent, good, alert, bad, violet) carry the meaning; the surfaces and
//! text tones follow the theme's light or dark flag, so a light palette gets
//! light panels and dark text without a second setting.

use ratatui::style::{Color, Modifier, Style};
use rune::review::CommentKind;

use crate::cli::theme::{self, ThemeTones, Tone};

/// One resolved TUI palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Palette {
    pub(super) panel_bg: Color,
    pub(super) bg_highlight: Color,
    pub(super) fg_primary: Color,
    pub(super) fg_secondary: Color,
    pub(super) fg_dim: Color,
    pub(super) diff_add: Color,
    pub(super) diff_add_bg: Color,
    pub(super) diff_del: Color,
    pub(super) diff_del_bg: Color,
    pub(super) diff_context: Color,
    pub(super) hunk_header_bg: Color,
    pub(super) border_focused: Color,
    pub(super) border_unfocused: Color,
    pub(super) status_bar_bg: Color,
    pub(super) cursor: Color,
    pub(super) mode_fg: Color,
    pub(super) mode_bg: Color,
    pub(super) accent: Color,
    pub(super) good: Color,
    pub(super) alert: Color,
    pub(super) bad: Color,
    pub(super) violet: Color,
}

fn rgb(tone: Tone) -> Color {
    let (red, green, blue) = tone.rgb;
    Color::Rgb(red, green, blue)
}

impl Palette {
    /// Derive the TUI palette from one theme.
    #[must_use]
    pub(super) fn from_theme(tones: &ThemeTones) -> Self {
        let accent = rgb(tones.accent);
        let good = rgb(tones.good);
        let alert = rgb(tones.alert);
        let bad = rgb(tones.bad);
        let violet = rgb(tones.violet);
        if tones.light {
            Self {
                panel_bg: Color::Rgb(250, 250, 252),
                bg_highlight: Color::Rgb(214, 218, 226),
                fg_primary: Color::Black,
                fg_secondary: Color::Rgb(60, 60, 70),
                fg_dim: Color::Rgb(110, 110, 120),
                diff_add: good,
                diff_add_bg: Color::Rgb(222, 244, 226),
                diff_del: bad,
                diff_del_bg: Color::Rgb(250, 222, 222),
                diff_context: Color::Rgb(70, 70, 70),
                hunk_header_bg: Color::Rgb(236, 236, 240),
                border_focused: accent,
                border_unfocused: Color::Rgb(170, 170, 180),
                status_bar_bg: Color::Rgb(235, 236, 240),
                cursor: alert,
                mode_fg: Color::White,
                mode_bg: accent,
                accent,
                good,
                alert,
                bad,
                violet,
            }
        } else {
            Self {
                panel_bg: Color::Rgb(24, 24, 28),
                bg_highlight: Color::Rgb(70, 70, 70),
                fg_primary: Color::White,
                fg_secondary: Color::Rgb(210, 210, 210),
                fg_dim: Color::Rgb(160, 160, 160),
                diff_add: good,
                diff_add_bg: Color::Rgb(0, 60, 20),
                diff_del: bad,
                diff_del_bg: Color::Rgb(70, 0, 0),
                diff_context: Color::Rgb(200, 200, 200),
                hunk_header_bg: Color::Rgb(42, 42, 46),
                border_focused: accent,
                border_unfocused: Color::Rgb(110, 110, 110),
                status_bar_bg: Color::Rgb(30, 30, 30),
                cursor: alert,
                mode_fg: Color::Black,
                mode_bg: accent,
                accent,
                good,
                alert,
                bad,
                violet,
            }
        }
    }
}

/// The active palette. The theme installs once at dispatch, so this is a
/// cheap derivation on every call.
pub(super) fn palette() -> Palette {
    Palette::from_theme(&theme::current())
}

pub(super) fn fg_primary() -> Color {
    palette().fg_primary
}

pub(super) fn fg_secondary() -> Color {
    palette().fg_secondary
}

pub(super) fn fg_dim() -> Color {
    palette().fg_dim
}

pub(super) fn accent() -> Color {
    palette().accent
}

pub(super) fn good() -> Color {
    palette().good
}

pub(super) fn alert() -> Color {
    palette().alert
}

pub(super) fn bad() -> Color {
    palette().bad
}

pub(super) fn violet() -> Color {
    palette().violet
}

/// Bold heading in the theme's violet, for group labels and keys.
pub(super) fn heading_style() -> Style {
    Style::default().fg(violet()).add_modifier(Modifier::BOLD)
}

/// Inverse highlight for search matches and the picker cursor.
pub(super) fn highlight_style(current: bool) -> Style {
    let palette = palette();
    Style::default()
        .fg(palette.mode_fg)
        .bg(if current {
            palette.violet
        } else {
            palette.alert
        })
        .add_modifier(Modifier::BOLD)
}

/// Visual (line-range) selection in the Code tab.
pub(super) fn visual_selection_style() -> Style {
    let palette = palette();
    Style::default().fg(palette.mode_fg).bg(palette.accent)
}

pub(super) fn selected_style() -> Style {
    let palette = palette();
    Style::default()
        .bg(palette.bg_highlight)
        .fg(palette.fg_primary)
}

pub(super) fn current_line_indicator_style() -> Style {
    Style::default()
        .fg(palette().cursor)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn dim_style() -> Style {
    Style::default().fg(fg_dim())
}

pub(super) fn diff_add_style() -> Style {
    let palette = palette();
    Style::default()
        .fg(palette.diff_add)
        .bg(palette.diff_add_bg)
}

pub(super) fn diff_del_style() -> Style {
    let palette = palette();
    Style::default()
        .fg(palette.diff_del)
        .bg(palette.diff_del_bg)
}

pub(super) fn diff_context_style() -> Style {
    Style::default().fg(palette().diff_context)
}

pub(super) fn diff_hunk_header_style() -> Style {
    let palette = palette();
    Style::default()
        .fg(palette.fg_dim)
        .bg(palette.hunk_header_bg)
}

pub(super) fn file_header_style() -> Style {
    Style::default()
        .fg(fg_primary())
        .add_modifier(Modifier::BOLD)
}

pub(super) fn border_style(focused: bool) -> Style {
    let palette = palette();
    Style::default().fg(if focused {
        palette.border_focused
    } else {
        palette.border_unfocused
    })
}

pub(super) fn panel_style() -> Style {
    let palette = palette();
    Style::default().bg(palette.panel_bg).fg(palette.fg_primary)
}

pub(super) fn status_bar_style() -> Style {
    let palette = palette();
    Style::default()
        .bg(palette.status_bar_bg)
        .fg(palette.fg_primary)
}

pub(super) fn mode_style() -> Style {
    let palette = palette();
    Style::default()
        .fg(palette.mode_fg)
        .bg(palette.mode_bg)
        .add_modifier(Modifier::BOLD)
}

/// Comment kinds map onto the theme tones: notes are informational (accent),
/// suggestions are stylistic (violet), issues block (bad), praise is good.
pub(super) fn comment_type_style(kind: CommentKind) -> Style {
    let palette = palette();
    let color = match kind {
        CommentKind::Note => palette.accent,
        CommentKind::Suggestion => palette.violet,
        CommentKind::Issue => palette.bad,
        CommentKind::Praise => palette.good,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub(super) fn comment_border_style() -> Style {
    file_header_style()
}

#[cfg(test)]
mod tests;
