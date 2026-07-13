use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use commands::review::CommentKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommentNavigatorItem {
    pub(super) key: (String, String, usize),
    pub(super) kind: CommentKind,
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) text: String,
}

#[derive(Default)]
pub(super) struct CommentNavigatorState {
    pub(super) list_state: ListState,
    pub(super) scroll_x: usize,
    pub(super) viewport_width: usize,
    pub(super) viewport_height: usize,
    pub(super) max_content_width: usize,
}

impl CommentNavigatorState {
    pub(super) fn selected(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub(super) fn select(&mut self, index: usize) {
        self.list_state.select(Some(index));
    }

    pub(super) fn scroll_left(&mut self, columns: usize) {
        self.scroll_x = self.scroll_x.saturating_sub(columns);
    }

    pub(super) fn scroll_right(&mut self, columns: usize) {
        let max_scroll = self.max_content_width.saturating_sub(self.viewport_width);
        self.scroll_x = self.scroll_x.saturating_add(columns).min(max_scroll);
    }
}

pub(super) fn render_comment_navigator(
    frame: &mut Frame<'_>,
    state: &mut CommentNavigatorState,
    area: Rect,
    items: &[CommentNavigatorItem],
    focused: bool,
) {
    let title = format!(" Comments · {} ", items.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused {
            Color::Cyan
        } else {
            Color::DarkGray
        }));
    let inner = block.inner(area);
    state.viewport_width = usize::from(inner.width);
    state.viewport_height = usize::from(inner.height);

    let row_lines = items
        .iter()
        .map(|item| render_comment_row(item, usize::from(inner.width)))
        .collect::<Vec<_>>();
    state.max_content_width = row_lines.iter().map(line_width).max().unwrap_or_default();
    let max_scroll = state
        .max_content_width
        .saturating_sub(usize::from(inner.width));
    state.scroll_x = state.scroll_x.min(max_scroll);
    let rows = row_lines
        .into_iter()
        .map(|line| ListItem::new(apply_horizontal_scroll(line, state.scroll_x)))
        .collect::<Vec<_>>();
    let list = List::new(rows)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(block);
    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_comment_row(item: &CommentNavigatorItem, width: usize) -> Line<'static> {
    let kind = format!("[{}]", item.kind.label());
    let location = format!("{}:{}", item.path, item.line);
    let fixed_width = kind.width() + location.width() + 3;
    let text_width = width.saturating_sub(fixed_width);
    let first_segment = wrap_segments(&item.text, text_width)
        .first()
        .copied()
        .unwrap_or_default()
        .to_string();
    Line::from(vec![
        Span::styled(kind, comment_kind_style(item.kind)),
        Span::raw(" "),
        Span::styled(location, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::raw(first_segment),
    ])
}

fn comment_kind_style(kind: CommentKind) -> Style {
    let color = match kind {
        CommentKind::Issue => Color::Red,
        CommentKind::Note => Color::Blue,
        CommentKind::Suggestion => Color::Yellow,
        CommentKind::Praise => Color::Green,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn line_width(line: &Line<'_>) -> usize {
    line.spans.iter().map(|span| span.content.width()).sum()
}

/// Split text by display width, transliterated from tuicr's comment panel.
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
        if take_bytes == 0 {
            take_bytes = remaining.chars().next().map_or(0, char::len_utf8);
        }
        let (segment, rest) = remaining.split_at(take_bytes);
        segments.push(segment);
        remaining = rest;
    }
    segments
}

/// Apply horizontal scroll while retaining the kind marker, matching tuicr.
fn apply_horizontal_scroll(line: Line<'static>, scroll_x: usize) -> Line<'static> {
    if scroll_x == 0 || line.spans.is_empty() {
        return line;
    }
    let mut spans = line.spans;
    let marker = spans.remove(0);
    let mut characters_to_skip = scroll_x;
    let mut visible = vec![marker];
    for span in spans {
        let content = span.content.into_owned();
        let character_count = content.chars().count();
        if characters_to_skip >= character_count {
            characters_to_skip -= character_count;
        } else if characters_to_skip > 0 {
            let content = content.chars().skip(characters_to_skip).collect::<String>();
            characters_to_skip = 0;
            visible.push(Span::styled(content, span.style));
        } else {
            visible.push(Span::styled(content, span.style));
        }
    }
    Line::from(visible)
}

#[cfg(test)]
mod tests {
    use super::wrap_segments;

    #[test]
    fn wrap_segments_uses_terminal_display_width() {
        assert_eq!(wrap_segments("a中b文", 3), vec!["a中", "b文"]);
    }
}
