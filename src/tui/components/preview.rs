use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use commands::view::ArtifactView;

use super::super::{app::DetailTab, styles};

const DETAIL_TAB_COUNT: usize = 6;

/// Display rows the lines occupy when word-wrapped to `width`. Counts each
/// line as at least one row plus one per full width beyond it; wide glyphs
/// count as one column, close enough to keep the last line reachable.
pub(in super::super) fn wrapped_rows(lines: &[Line<'_>], width: u16) -> usize {
    if width == 0 {
        return lines.len();
    }
    let width = usize::from(width);
    lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(width))
        .sum()
}

/// Rendered lines for one (tab, width) combination, rebuilt only when either
/// changes so scrolling a large file stays cheap.
#[derive(Debug, Clone)]
struct PreviewPane {
    tab: DetailTab,
    width: u16,
    lines: Vec<Line<'static>>,
    windowed: bool,
    /// Rendered-row to source-line mapping. Code is gutter-wrapped before it
    /// reaches this component, while its cursor remains a logical source line.
    logical_rows: Vec<usize>,
}

/// Logical selection and viewport origin retained independently for each tab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewPosition {
    pub cursor: usize,
    pub scroll: u16,
}

/// Full-window scrollable view of the selected artifact. Opened from the
/// detail pane with Enter; shows the same rich tabs as the pane (digits
/// switch), so zooming never loses highlighting or layout.
#[derive(Debug, Clone)]
pub struct ArtifactPreview {
    artifact: ArtifactView,
    active_tab: DetailTab,
    positions: [PreviewPosition; DETAIL_TAB_COUNT],
    pane: Option<PreviewPane>,
}

impl ArtifactPreview {
    /// Opens a preview at the same logical line and viewport origin as the
    /// detail pane. The position remains tab-local while the preview is open.
    #[must_use]
    pub fn from_artifact_at(
        artifact: &ArtifactView,
        tab: DetailTab,
        cursor: usize,
        scroll: u16,
    ) -> Self {
        let mut positions = [PreviewPosition::default(); DETAIL_TAB_COUNT];
        positions[tab as usize] = PreviewPosition { cursor, scroll };
        Self {
            artifact: artifact.clone(),
            active_tab: tab,
            positions,
            pane: None,
        }
    }

    #[must_use]
    pub fn artifact(&self) -> &ArtifactView {
        &self.artifact
    }

    /// Rebinds an open preview after a scan without discarding any tab's
    /// logical position. Rendered lines are invalidated for the fresh data.
    pub fn replace_artifact(&mut self, artifact: &ArtifactView) {
        self.artifact = artifact.clone();
        self.pane = None;
    }

    #[cfg(test)]
    #[must_use]
    pub fn scroll(&self) -> u16 {
        self.position(self.active_tab).scroll
    }

    #[cfg(test)]
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.position(self.active_tab).cursor
    }

    #[must_use]
    pub fn active_tab(&self) -> DetailTab {
        self.active_tab
    }

    #[must_use]
    pub fn position(&self, tab: DetailTab) -> PreviewPosition {
        self.positions[tab as usize]
    }

    pub fn set_position(&mut self, tab: DetailTab, cursor: usize, scroll: u16) {
        self.positions[tab as usize] = PreviewPosition { cursor, scroll };
    }

    #[must_use]
    pub fn needs_rebuild(&self, tab: DetailTab, width: u16) -> bool {
        self.pane
            .as_ref()
            .is_none_or(|pane| pane.tab != tab || pane.width != width)
    }

    pub fn set_lines(
        &mut self,
        tab: DetailTab,
        width: u16,
        lines: Vec<Line<'static>>,
        windowed: bool,
    ) {
        self.active_tab = tab;
        let logical_rows = if tab == DetailTab::Code {
            code_logical_rows(&lines)
        } else {
            (0..lines.len()).collect()
        };
        self.pane = Some(PreviewPane {
            tab,
            width,
            lines,
            windowed,
            logical_rows,
        });
        let total = self.pane.as_ref().map_or(0, |pane| {
            if pane.windowed {
                pane.logical_rows
                    .last()
                    .map_or(0, |logical| logical.saturating_add(1))
            } else {
                wrapped_rows(&pane.lines, pane.width)
            }
        });
        let position = &mut self.positions[tab as usize];
        position.cursor = position.cursor.min(total.saturating_sub(1));
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let position = &mut self.positions[self.active_tab as usize];
        position.scroll = position.scroll.saturating_add(amount);
        position.cursor = position.cursor.saturating_add(usize::from(amount));
    }

    pub fn scroll_up(&mut self, amount: u16) {
        let position = &mut self.positions[self.active_tab as usize];
        position.scroll = position.scroll.saturating_sub(amount);
        position.cursor = position.cursor.saturating_sub(usize::from(amount));
    }

    pub fn scroll_to_top(&mut self) {
        self.positions[self.active_tab as usize] = PreviewPosition::default();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.positions[self.active_tab as usize] = PreviewPosition {
            cursor: usize::MAX,
            scroll: u16::MAX,
        };
    }

    /// Reconciles a tab-local cursor with the rendered viewport. Geometry
    /// changes move the viewport around the cursor, never the cursor itself
    /// (unless the rebuilt content is shorter).
    fn reconcile_viewport(&mut self, total: usize, viewport: usize) {
        let viewport = viewport.max(1);
        let position = &mut self.positions[self.active_tab as usize];
        position.cursor = position.cursor.min(total.saturating_sub(1));
        let max_scroll = total.saturating_sub(viewport);
        let mut scroll = usize::from(position.scroll).min(max_scroll);
        if position.cursor < scroll {
            scroll = position.cursor;
        } else if position.cursor >= scroll.saturating_add(viewport) {
            scroll = position.cursor.saturating_add(1).saturating_sub(viewport);
        }
        position.scroll = u16::try_from(scroll.min(max_scroll)).unwrap_or(u16::MAX);
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, pending_count: Option<usize>) {
        let tab_label = self
            .pane
            .as_ref()
            .map_or("Preview", |pane| pane.tab.label());
        let title = format!(
            " {} · {} · {} ",
            self.artifact.name, tab_label, self.artifact.relative_path
        );
        let footer = if let Some(count) = pending_count {
            format!(" count: {count} — j/k repeat, Esc cancel ")
        } else if matches!(self.active_tab, DetailTab::Code | DetailTab::Diff) {
            " p/c/d/v/f/i tabs · 0-9 count · j/k · ␣/b page · g/G ends · Esc close ".to_string()
        } else {
            " 1-6 tabs · j/k · ␣/b page · g/G ends · Esc close ".to_string()
        };
        let block = Block::default()
            .title(title)
            .title_bottom(Line::from(Span::styled(footer, styles::dim_style())))
            .borders(Borders::ALL)
            .border_style(styles::border_style(true))
            .style(styles::panel_style());
        let inner = block.inner(area);
        let viewport = usize::from(inner.height.max(1));

        let Some((windowed, total)) = self.pane.as_ref().map(|pane| {
            let total = if pane.windowed {
                pane.logical_rows
                    .last()
                    .map_or(0, |logical| logical.saturating_add(1))
            } else {
                wrapped_rows(&pane.lines, inner.width)
            };
            (pane.windowed, total)
        }) else {
            frame.render_widget(Paragraph::new("building preview...").block(block), area);
            return;
        };
        // Wrapped content occupies more display rows than logical lines;
        // clamp against the wrapped estimate or the tail becomes unreachable.
        self.reconcile_viewport(total, viewport);
        let position = self.position(self.active_tab);
        let position_title = Line::from(Span::styled(
            format!(" {}/{} ", position.cursor.saturating_add(1), total.max(1)),
            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
        ));
        let pane = self.pane.as_ref().expect("preview pane was checked above");

        if windowed {
            let (render_scroll, render_cursor) = if pane.tab == DetailTab::Code {
                code_render_position(pane, position, viewport)
            } else {
                (usize::from(position.scroll), position.cursor)
            };
            if pane.tab == DetailTab::Code {
                let logical_scroll = pane.logical_rows.get(render_scroll).copied().unwrap_or(0);
                self.positions[self.active_tab as usize].scroll =
                    u16::try_from(logical_scroll).unwrap_or(u16::MAX);
            }
            let mut window: Vec<Line<'static>> = pane
                .lines
                .iter()
                .skip(render_scroll)
                .take(viewport)
                .cloned()
                .collect();
            if matches!(pane.tab, DetailTab::Code | DetailTab::Diff)
                && let Some(line) = window.get_mut(render_cursor.saturating_sub(render_scroll))
            {
                if pane.tab == DetailTab::Diff {
                    mark_diff_cursor_line(line, true);
                } else {
                    line.style = styles::selected_style();
                }
            }
            frame.render_widget(
                Paragraph::new(Text::from(window)).block(block.title_top(position_title)),
                area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(Text::from(pane.lines.clone()))
                    .wrap(Wrap { trim: false })
                    .scroll((position.scroll, 0))
                    .block(block.title_top(position_title)),
                area,
            );
        }
    }
}

/// Marks the selected Diff row without collapsing a wrapped continuation's
/// combined indicator/gutter span. The first display cell is always reserved
/// for the cursor, matching the inline Diff renderer.
pub(in crate::tui) fn mark_diff_cursor_line(line: &mut Line<'static>, focused: bool) {
    let Some(first) = line.spans.first_mut() else {
        return;
    };
    let marker = if focused { "▶" } else { " " };
    let original = first.content.to_string();
    let original_style = first.style;
    let remainder = original.chars().skip(1).collect::<String>();
    *first = Span::styled(marker, styles::current_line_indicator_style());
    if !remainder.is_empty() {
        line.spans
            .insert(1, Span::styled(remainder, original_style));
    }
    if focused {
        line.style = styles::selected_style();
    }
}

fn code_logical_rows(lines: &[Line<'_>]) -> Vec<usize> {
    let mut logical = 0usize;
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let continuation = line
                .spans
                .first()
                .is_some_and(|span| span.content.contains('↪'));
            if index > 0 && !continuation {
                logical = logical.saturating_add(1);
            }
            logical
        })
        .collect()
}

fn code_render_position(
    pane: &PreviewPane,
    position: PreviewPosition,
    viewport: usize,
) -> (usize, usize) {
    let cursor_row = pane
        .logical_rows
        .iter()
        .position(|logical| *logical == position.cursor)
        .unwrap_or_else(|| pane.lines.len().saturating_sub(1));
    let mut scroll_row = pane
        .logical_rows
        .iter()
        .position(|logical| *logical >= usize::from(position.scroll))
        .unwrap_or_else(|| pane.lines.len().saturating_sub(viewport));
    let max_scroll = pane.lines.len().saturating_sub(viewport);
    scroll_row = scroll_row.min(max_scroll);
    if cursor_row < scroll_row {
        scroll_row = cursor_row;
    } else if cursor_row >= scroll_row.saturating_add(viewport) {
        scroll_row = cursor_row.saturating_add(1).saturating_sub(viewport);
    }
    (scroll_row.min(max_scroll), cursor_row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(count: usize) -> Vec<Line<'static>> {
        (0..count)
            .map(|index| Line::from(format!("line {index}")))
            .collect()
    }

    #[test]
    fn opening_at_detail_position_keeps_cursor_visible_after_resize() {
        let mut preview =
            ArtifactPreview::from_artifact_at(&ArtifactView::default(), DetailTab::Diff, 73, 68);
        preview.set_lines(DetailTab::Diff, 80, lines(100), true);

        preview.reconcile_viewport(100, 10);
        assert_eq!(preview.cursor(), 73);
        assert_eq!(preview.scroll(), 68);

        preview.reconcile_viewport(100, 4);
        assert_eq!(preview.cursor(), 73);
        assert_eq!(preview.scroll(), 70);
        assert!(usize::from(preview.scroll()) <= preview.cursor());
        assert!(preview.cursor() < usize::from(preview.scroll()) + 4);
    }

    #[test]
    fn switching_tabs_restores_each_tabs_logical_position() {
        let mut preview =
            ArtifactPreview::from_artifact_at(&ArtifactView::default(), DetailTab::Diff, 18, 15);
        preview.set_lines(DetailTab::Diff, 80, lines(50), true);
        preview.scroll_down(2);
        let diff_position = preview.position(DetailTab::Diff);

        preview.set_lines(DetailTab::Code, 80, lines(50), true);
        preview.scroll_down(7);
        assert_eq!(preview.position(DetailTab::Code).cursor, 7);

        preview.set_lines(DetailTab::Diff, 80, lines(50), true);
        assert_eq!(preview.position(DetailTab::Diff), diff_position);
        assert_eq!(preview.cursor(), 20);
        assert_eq!(preview.scroll(), 17);
    }

    #[test]
    fn shorter_rebuilt_content_clamps_cursor_and_keeps_it_visible() {
        let mut preview =
            ArtifactPreview::from_artifact_at(&ArtifactView::default(), DetailTab::Code, 90, 85);
        preview.set_lines(DetailTab::Code, 80, lines(12), true);
        preview.reconcile_viewport(12, 5);

        assert_eq!(preview.cursor(), 11);
        assert_eq!(preview.scroll(), 7);
    }

    #[test]
    fn scrolling_moves_logical_cursor_that_caller_can_restore_on_close() {
        let mut preview =
            ArtifactPreview::from_artifact_at(&ArtifactView::default(), DetailTab::Diff, 20, 16);
        preview.set_lines(DetailTab::Diff, 80, lines(100), true);
        preview.scroll_down(3);

        assert_eq!(
            preview.position(preview.active_tab()),
            PreviewPosition {
                cursor: 23,
                scroll: 19,
            }
        );
    }

    #[test]
    fn replacing_artifact_preserves_all_tab_positions() {
        let mut preview =
            ArtifactPreview::from_artifact_at(&ArtifactView::default(), DetailTab::Diff, 20, 16);
        preview.set_lines(DetailTab::Diff, 80, lines(100), true);
        preview.set_position(DetailTab::Code, 7, 4);

        let artifact = ArtifactView {
            name: "fresh".to_string(),
            ..ArtifactView::default()
        };
        preview.replace_artifact(&artifact);

        assert_eq!(preview.artifact().name, "fresh");
        assert!(preview.needs_rebuild(DetailTab::Diff, 80));
        assert_eq!(
            preview.position(DetailTab::Diff),
            PreviewPosition {
                cursor: 20,
                scroll: 16,
            }
        );
        assert_eq!(
            preview.position(DetailTab::Code),
            PreviewPosition {
                cursor: 7,
                scroll: 4,
            }
        );
    }

    #[test]
    fn wrapped_code_keeps_source_line_cursor_on_its_logical_line() {
        let mut preview =
            ArtifactPreview::from_artifact_at(&ArtifactView::default(), DetailTab::Code, 2, 0);
        preview.set_lines(
            DetailTab::Code,
            30,
            vec![
                Line::from("  1 first"),
                Line::from("     ↪ continuation one"),
                Line::from("     ↪ continuation two"),
                Line::from("  2 second"),
                Line::from("  3 selected"),
            ],
            true,
        );

        let pane = preview.pane.as_ref().unwrap();
        assert_eq!(pane.logical_rows, vec![0, 0, 0, 1, 2]);
        assert_eq!(
            code_render_position(pane, preview.position(DetailTab::Code), 3),
            (2, 4)
        );
        assert_eq!(preview.cursor(), 2);
    }
}
