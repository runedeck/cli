//! Inline comment boxes transliterated from tuicr's comment panel.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use rune::review::CommentKind;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::styles;

/// Content prefix used on every comment-body line. Four pad characters, the
/// border, and two more spaces keep comment chrome clear of code/diff gutters.
pub(super) const BORDER_PREFIX: &str = "    │  ";
const BORDER_PREFIX_WIDTH: usize = 7;
const BORDER_PREFIX_COLUMN: u16 = 7;

/// The logical source range to which an inline comment is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CommentLineRange {
    pub(super) start: usize,
    pub(super) end: usize,
}

impl CommentLineRange {
    pub(super) const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    const fn is_single(self) -> bool {
        self.start == self.end
    }
}

/// Split `text` into segments whose display width fits `content_area`.
pub(super) fn wrap_segments(text: &str, content_area: usize) -> Vec<&str> {
    if content_area == 0 || text.width() <= content_area {
        return vec![text];
    }
    let mut segments = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let mut take_bytes = 0usize;
        let mut taken_width = 0usize;
        for character in remaining.chars() {
            let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
            if taken_width + character_width > content_area {
                break;
            }
            taken_width += character_width;
            take_bytes += character.len_utf8();
        }
        // Emit a character wider than the available area so wrapping always
        // makes progress.
        if take_bytes == 0 {
            take_bytes = remaining.chars().next().map_or(0, char::len_utf8);
        }
        let (segment, rest) = remaining.split_at(take_bytes);
        segments.push(segment);
        remaining = rest;
    }
    segments
}

fn push_cursor_spans(
    spans: &mut Vec<Span<'static>>,
    before: &str,
    after: &str,
    cursor_style: Style,
) {
    spans.push(Span::raw(before.to_string()));
    let mut characters = after.chars();
    if let Some(cursor_character) = characters.next() {
        spans.push(Span::styled(cursor_character.to_string(), cursor_style));
        spans.push(Span::raw(characters.as_str().to_string()));
    }
}

/// Position of the terminal cursor within the formatted comment box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CommentCursorInfo {
    pub(super) line_offset: usize,
    pub(super) column: u16,
}

/// Format a tuicr-style inline comment editor and its terminal cursor position.
pub(super) fn format_comment_input_lines(
    comment_kind: CommentKind,
    buffer: &str,
    cursor_pos: usize,
    line_range: Option<CommentLineRange>,
    is_editing: bool,
    width: usize,
) -> (Vec<Line<'static>>, CommentCursorInfo) {
    let type_style = styles::comment_type_style(comment_kind);
    let border_style = styles::comment_border_style();
    let cursor_style = Style::default()
        .fg(styles::palette().cursor)
        .add_modifier(Modifier::UNDERLINED);

    let action = if is_editing { "Edit" } else { "Add" };
    let line_info = line_range.map_or_else(String::new, |range| {
        if range.is_single() {
            format!("L{} ", range.start)
        } else {
            format!("L{}-L{} ", range.start, range.end)
        }
    });

    let content_area = width.saturating_sub(BORDER_PREFIX_WIDTH + 2);
    let mut result = Vec::new();
    let mut cursor_line_offset = 1usize;
    let mut cursor_column = BORDER_PREFIX_COLUMN;

    let top_corner = if line_range.is_some() { '├' } else { '╭' };
    let top_prefix = format!("    {top_corner}── ");
    result.push(Line::from(vec![
        Span::styled(top_prefix, border_style),
        Span::styled(format!("{action} "), styles::dim_style()),
        Span::styled(format!("[{}] ", comment_kind.label()), type_style),
        Span::styled(line_info, styles::dim_style()),
        Span::styled(
            "(Tab/S-Tab:type Enter:save Shift-Enter:newline Esc:cancel)",
            styles::dim_style(),
        ),
    ]));

    if buffer.is_empty() {
        result.push(Line::from(vec![
            Span::styled(BORDER_PREFIX, border_style),
            Span::styled(" ", cursor_style),
            Span::styled("Type your comment...", styles::dim_style()),
        ]));
    } else {
        let buffer_lines = buffer.split('\n').collect::<Vec<_>>();
        let mut byte_offset = 0usize;
        let mut total_visual_lines = 0usize;

        for (line_index, text) in buffer_lines.iter().enumerate() {
            let line_start = byte_offset;
            let line_end = byte_offset + text.len();
            let is_last_logical = line_index + 1 == buffer_lines.len();
            let cursor_on_this_line = cursor_pos >= line_start
                && (cursor_pos <= line_end || (is_last_logical && cursor_pos == buffer.len()));
            let segments = wrap_segments(text, content_area);
            let mut segment_byte_start = 0usize;

            for (segment_index, segment) in segments.iter().enumerate() {
                let segment_start = line_start + segment_byte_start;
                let segment_end = segment_start + segment.len();
                let is_last_segment = segment_index + 1 == segments.len();
                let cursor_in_segment = cursor_on_this_line
                    && cursor_pos >= segment_start
                    && (cursor_pos < segment_end || is_last_segment);
                let mut line_spans = vec![Span::styled(BORDER_PREFIX, border_style)];

                if cursor_in_segment {
                    let cursor_in_segment = (cursor_pos - segment_start).min(segment.len());
                    let (before, after) = segment.split_at(cursor_in_segment);
                    cursor_line_offset = 1 + total_visual_lines;
                    cursor_column = BORDER_PREFIX_COLUMN
                        .saturating_add(u16::try_from(before.width()).unwrap_or(u16::MAX));
                    push_cursor_spans(&mut line_spans, before, after, cursor_style);
                } else {
                    line_spans.push(Span::raw((*segment).to_string()));
                }

                result.push(Line::from(line_spans));
                total_visual_lines += 1;
                segment_byte_start += segment.len();
            }

            byte_offset = line_end + 1;
        }
    }

    result.push(Line::from(vec![Span::styled(
        "    ╰".to_string() + &"─".repeat(width.saturating_sub(5)),
        border_style,
    )]));

    (
        result,
        CommentCursorInfo {
            line_offset: cursor_line_offset,
            column: cursor_column,
        },
    )
}

/// Format a persisted comment as a tuicr-style inline box.
pub(super) fn format_comment_lines(
    comment_kind: CommentKind,
    content: &str,
    line_range: Option<CommentLineRange>,
    width: usize,
) -> Vec<Line<'static>> {
    let type_style = styles::comment_type_style(comment_kind);
    let border_style = styles::comment_border_style();
    let badge_text = format!("[{}] ", comment_kind.label());
    let badge_width = badge_text.width();
    let line_info = line_range.map_or_else(String::new, |range| {
        if range.is_single() {
            format!("L{} ", range.start)
        } else {
            format!("L{}-L{} ", range.start, range.end)
        }
    });
    let content_area = width.saturating_sub(BORDER_PREFIX_WIDTH + 2);
    let top_corner = if line_range.is_some() { '├' } else { '╭' };
    let top_prefix = format!("    {top_corner}── ");
    let top_fill = width.saturating_sub(8 + badge_width + line_info.width());

    let mut result = vec![Line::from(vec![
        Span::styled(top_prefix, border_style),
        Span::styled(badge_text, type_style),
        Span::styled(line_info, styles::dim_style()),
        Span::styled("─".repeat(top_fill), border_style),
    ])];

    for line in content.split('\n') {
        for segment in wrap_segments(line, content_area) {
            result.push(Line::from(vec![
                Span::styled(BORDER_PREFIX, border_style),
                Span::raw(segment.to_string()),
            ]));
        }
    }

    result.push(Line::from(vec![Span::styled(
        "    ╰".to_string() + &"─".repeat(width.saturating_sub(5)),
        border_style,
    )]));
    result
}

#[cfg(test)]
mod tests;
