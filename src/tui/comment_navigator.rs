use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use unicode_width::UnicodeWidthStr;

use rune::review::CommentKind;

use super::{comment_panel::wrap_segments, styles};

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
        .style(styles::panel_style())
        .border_style(styles::border_style(focused));
    let inner = block.inner(area);
    state.viewport_height = usize::from(inner.height);

    let rows = items
        .iter()
        .map(|item| render_comment_row(item, usize::from(inner.width)))
        .map(ListItem::new)
        .collect::<Vec<_>>();
    let list = List::new(rows)
        .highlight_style(styles::selected_style())
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
        Span::styled(kind, styles::comment_type_style(item.kind)),
        Span::raw(" "),
        Span::styled(location, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::raw(first_segment),
    ])
}
