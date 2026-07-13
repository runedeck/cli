use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{
    EditorEventHandler, EditorMode, EditorState, EditorStatusLine, EditorTheme, EditorView, Index2,
    LineNumbers, Lines,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
};

pub(super) enum EditorAction {
    Continue,
    Save,
    Discard,
}

pub(super) struct FileEditor {
    path: PathBuf,
    original: String,
    state: EditorState,
    events: EditorEventHandler,
    command: Option<String>,
    discard_armed: bool,
}

impl FileEditor {
    pub(super) fn open(path: PathBuf, line: Option<usize>) -> Result<Self, String> {
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
            original,
            state,
            events: EditorEventHandler::default(),
            command: None,
            discard_armed: false,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
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

    pub(super) fn mode_label(&self) -> String {
        if let Some(command) = &self.command {
            return format!(":{command}");
        }
        if self.discard_armed {
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
        if self.command.is_some() {
            return self.command_key(key);
        }
        if self.state.mode == EditorMode::Normal {
            if key.code == KeyCode::Char(':') {
                self.discard_armed = false;
                self.command = Some(String::new());
                return EditorAction::Continue;
            }
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                return self.request_discard();
            }
        }
        self.discard_armed = false;
        self.events.on_key_event(key, &mut self.state);
        EditorAction::Continue
    }

    fn command_key(&mut self, key: KeyEvent) -> EditorAction {
        match key.code {
            KeyCode::Enter => {
                let command = self.command.take().unwrap_or_default();
                match command.trim() {
                    "w" | "wq" | "x" => EditorAction::Save,
                    "q" => self.request_discard(),
                    "q!" => EditorAction::Discard,
                    _ => EditorAction::Continue,
                }
            }
            KeyCode::Esc => {
                self.command = None;
                EditorAction::Continue
            }
            KeyCode::Backspace => {
                if let Some(command) = self.command.as_mut() {
                    if command.is_empty() {
                        self.command = None;
                    } else {
                        command.pop();
                    }
                }
                EditorAction::Continue
            }
            KeyCode::Char(character) if key.modifiers.is_empty() => {
                if let Some(command) = self.command.as_mut() {
                    command.push(character);
                }
                EditorAction::Continue
            }
            _ => EditorAction::Continue,
        }
    }

    fn request_discard(&mut self) -> EditorAction {
        if !self.is_dirty() || self.discard_armed {
            return EditorAction::Discard;
        }
        self.command = None;
        self.discard_armed = true;
        EditorAction::Continue
    }

    pub(super) fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let dirty = if self.is_dirty() { "*" } else { "" };
        let title = format!(" Edit{dirty} · {} ", self.display_path());
        let line = Style::default().fg(Color::Gray);
        let mode = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let theme = EditorTheme::default()
            .base(Style::default())
            .cursor_style(Style::default().fg(Color::Black).bg(Color::White))
            .selection_style(Style::default().fg(Color::Black).bg(Color::Yellow))
            .line_numbers_style(Style::default().fg(Color::DarkGray))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_editor_requires_a_second_normal_mode_quit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Rule.md");
        std::fs::write(&path, "old").unwrap();
        let mut editor = FileEditor::open(path, None).unwrap();
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
