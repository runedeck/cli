use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{
    EditorEventHandler, EditorMode, EditorState, EditorStatusLine, EditorTheme, EditorView, Index2,
    LineNumbers, Lines,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders},
};

use super::modal_editor::{ModalAction, ModalState};

use super::styles;

pub(super) enum EditorAction {
    Continue,
    Save,
    SaveAndClose,
    Discard,
}

pub(super) struct FileEditor {
    path: PathBuf,
    artifact_key: Option<String>,
    original: String,
    state: EditorState,
    events: EditorEventHandler,
    modal: ModalState,
}

impl FileEditor {
    pub(super) fn open(
        path: PathBuf,
        line: Option<usize>,
        artifact_key: Option<String>,
    ) -> Result<Self, String> {
        let original = std::fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let mut state = EditorState::new(Lines::from(original.as_str()));
        let row = line
            .unwrap_or(1)
            .saturating_sub(1)
            .min(state.lines.len().saturating_sub(1));
        state.cursor = Index2::new(row, 0);
        Ok(Self {
            path,
            artifact_key,
            original,
            state,
            events: EditorEventHandler::default(),
            modal: ModalState::default(),
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn artifact_key(&self) -> Option<&str> {
        self.artifact_key.as_deref()
    }

    pub(super) fn display_path(&self) -> String {
        let components = self
            .path
            .components()
            .rev()
            .take(3)
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        components.into_iter().rev().collect::<Vec<_>>().join("/")
    }

    pub(super) fn text(&self) -> String {
        self.state.lines.to_string()
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.text() != self.original
    }

    pub(super) fn mark_saved(&mut self) {
        self.original = self.text();
    }

    pub(super) fn mode_label(&self) -> String {
        if let Some(command) = self.modal.command() {
            return format!(":{command}");
        }
        if self.modal.discard_armed() {
            return "Esc/q again to discard".to_string();
        }
        match self.state.mode {
            EditorMode::Normal => "NORMAL",
            EditorMode::Insert => "INSERT",
            EditorMode::Visual => "VISUAL",
            EditorMode::Search => "SEARCH",
        }
        .to_string()
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> EditorAction {
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return EditorAction::Save;
        }
        if self.modal.command().is_some() {
            let dirty = self.is_dirty();
            return editor_action(&self.modal.command_key(key, dirty));
        }
        if self.state.mode == EditorMode::Normal {
            if key.code == KeyCode::Char(':') {
                self.modal.begin_command();
                return EditorAction::Continue;
            }
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                let dirty = self.is_dirty();
                return editor_action(&self.modal.request_discard(dirty));
            }
        }
        self.modal.clear_discard();
        self.events.on_key_event(key, &mut self.state);
        EditorAction::Continue
    }

    pub(super) fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let dirty = if self.is_dirty() { "*" } else { "" };
        let title = format!(" Edit{dirty} · {} ", self.display_path());
        let line = Style::default().fg(styles::fg_secondary());
        let mode = Style::default()
            .fg(styles::palette().mode_fg)
            .bg(styles::accent())
            .add_modifier(Modifier::BOLD);
        let theme = EditorTheme::default()
            .base(Style::default())
            .cursor_style(
                Style::default()
                    .fg(styles::palette().panel_bg)
                    .bg(styles::fg_primary()),
            )
            .selection_style(styles::highlight_style(false))
            .line_numbers_style(Style::default().fg(styles::fg_dim()))
            .status_line(
                EditorStatusLine::default()
                    .style_line(line)
                    .style_mode(mode),
            )
            .block(Block::default().title(title).borders(Borders::ALL));
        frame.render_widget(
            EditorView::new(&mut self.state)
                .theme(theme)
                .line_numbers(LineNumbers::Absolute)
                .wrap(false),
            area,
        );
    }
}

fn editor_action(action: &ModalAction) -> EditorAction {
    match action {
        ModalAction::Continue | ModalAction::Unknown(_) => EditorAction::Continue,
        ModalAction::Save => EditorAction::Save,
        ModalAction::SaveAndClose => EditorAction::SaveAndClose,
        ModalAction::Discard => EditorAction::Discard,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_editor_requires_a_second_normal_mode_quit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Rule.md");
        std::fs::write(&path, "old").unwrap();
        let mut editor = FileEditor::open(path, None, None).unwrap();
        editor.events.on_key_event(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            &mut editor.state,
        );
        editor.events.on_key_event(
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            &mut editor.state,
        );
        editor.events.on_key_event(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut editor.state,
        );

        assert!(matches!(
            editor.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            EditorAction::Continue
        ));
        assert!(matches!(
            editor.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            EditorAction::Discard
        ));
    }
}
