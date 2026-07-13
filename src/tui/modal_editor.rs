use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModalAction {
    Continue,
    Save,
    SaveAndClose,
    Discard,
    Unknown(String),
}

/// Shared Vim-style command line and dirty-discard confirmation state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ModalState {
    command: Option<String>,
    discard_armed: bool,
}

impl ModalState {
    pub(super) fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub(super) const fn discard_armed(&self) -> bool {
        self.discard_armed
    }

    pub(super) fn begin_command(&mut self) {
        self.discard_armed = false;
        self.command = Some(String::new());
    }

    pub(super) fn clear_discard(&mut self) {
        self.discard_armed = false;
    }

    pub(super) fn command_key(&mut self, key: KeyEvent, dirty: bool) -> ModalAction {
        match key.code {
            KeyCode::Enter => {
                let command = self.command.take().unwrap_or_default();
                match command.trim() {
                    "w" => ModalAction::Save,
                    "wq" | "x" => ModalAction::SaveAndClose,
                    "q" => self.request_discard(dirty),
                    "q!" => ModalAction::Discard,
                    "" => ModalAction::Continue,
                    other => ModalAction::Unknown(other.to_string()),
                }
            }
            KeyCode::Esc => {
                self.command = None;
                ModalAction::Continue
            }
            KeyCode::Backspace => {
                if let Some(command) = self.command.as_mut() {
                    if command.is_empty() {
                        self.command = None;
                    } else {
                        command.pop();
                    }
                }
                ModalAction::Continue
            }
            KeyCode::Char(character) if key.modifiers.is_empty() => {
                if let Some(command) = self.command.as_mut() {
                    command.push(character);
                }
                ModalAction::Continue
            }
            _ => ModalAction::Continue,
        }
    }

    pub(super) fn request_discard(&mut self, dirty: bool) -> ModalAction {
        if !dirty || self.discard_armed {
            return ModalAction::Discard;
        }
        self.command = None;
        self.discard_armed = true;
        ModalAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn dirty_quit_arms_then_discards() {
        let mut state = ModalState::default();

        assert_eq!(state.request_discard(true), ModalAction::Continue);
        assert!(state.discard_armed());
        assert_eq!(state.request_discard(true), ModalAction::Discard);
    }

    #[test]
    fn command_line_dispatches_shared_write_and_quit_commands() {
        for (command, expected) in [
            ("w", ModalAction::Save),
            ("wq", ModalAction::SaveAndClose),
            ("x", ModalAction::SaveAndClose),
            ("q!", ModalAction::Discard),
        ] {
            let mut state = ModalState::default();
            state.begin_command();
            for character in command.chars() {
                assert_eq!(
                    state.command_key(key(KeyCode::Char(character)), true),
                    ModalAction::Continue
                );
            }
            assert_eq!(state.command_key(key(KeyCode::Enter), true), expected);
        }
    }
}
