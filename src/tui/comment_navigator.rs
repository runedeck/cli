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
    pub(super) viewport_height: usize,
}

impl CommentNavigatorState {
    pub(super) fn selected(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub(super) fn select(&mut self, index: usize) {
        self.list_state.select(Some(index));
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
    state.viewport_height = usize::from(inner.height);

    let rows = items
        .iter()
        .map(|item| render_comment_row(item, usize::from(inner.width)))
        .map(ListItem::new)
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

#[cfg(test)]
mod tests {
    use super::wrap_segments;

    #[test]
    fn wrap_segments_uses_terminal_display_width() {
        assert_eq!(wrap_segments("a中b文", 3), vec!["a中", "b文"]);
    }
}
