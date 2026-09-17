use super::normalize_title;
use crate::input::Composer;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

#[derive(Debug)]
pub(crate) enum BrowserMode {
    Browse,
    Rename(Composer),
    ConfirmDelete,
}

#[derive(Debug)]
pub(crate) struct SessionBrowser {
    pub(crate) selected: usize,
    pub(crate) mode: BrowserMode,
}

pub(super) enum BrowserCommand {
    None,
    Close,
    Switch(usize),
    Create,
    Rename(usize, String),
    Delete(usize),
}

impl SessionBrowser {
    pub(super) fn handle_key(
        &mut self,
        key: KeyEvent,
        session_count: usize,
        selected_title: &str,
    ) -> BrowserCommand {
        match &mut self.mode {
            BrowserMode::Browse => match key.code {
                KeyCode::Esc => BrowserCommand::Close,
                KeyCode::Up => {
                    self.selected = self.selected.saturating_sub(1);
                    BrowserCommand::None
                }
                KeyCode::Down => {
                    self.selected = self
                        .selected
                        .saturating_add(1)
                        .min(session_count.saturating_sub(1));
                    BrowserCommand::None
                }
                KeyCode::Enter => BrowserCommand::Switch(self.selected),
                KeyCode::Char('n' | 'N') => BrowserCommand::Create,
                KeyCode::Char('r' | 'R') => {
                    self.mode = BrowserMode::Rename(Composer::from_text(selected_title.to_owned()));
                    BrowserCommand::None
                }
                KeyCode::Char('d' | 'D') => {
                    self.mode = BrowserMode::ConfirmDelete;
                    BrowserCommand::None
                }
                _ => BrowserCommand::None,
            },
            BrowserMode::ConfirmDelete => match key.code {
                KeyCode::Char('y' | 'Y') => BrowserCommand::Delete(self.selected),
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.mode = BrowserMode::Browse;
                    BrowserCommand::None
                }
                _ => BrowserCommand::None,
            },
            BrowserMode::Rename(editor) => match key.code {
                KeyCode::Esc => {
                    self.mode = BrowserMode::Browse;
                    BrowserCommand::None
                }
                KeyCode::Enter => {
                    let title = normalize_title(editor.text());
                    if title.is_empty() {
                        BrowserCommand::None
                    } else {
                        self.mode = BrowserMode::Browse;
                        BrowserCommand::Rename(self.selected, title)
                    }
                }
                KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    editor.insert_char(character);
                    BrowserCommand::None
                }
                KeyCode::Left => {
                    editor.left();
                    BrowserCommand::None
                }
                KeyCode::Right => {
                    editor.right();
                    BrowserCommand::None
                }
                KeyCode::Home => {
                    editor.home();
                    BrowserCommand::None
                }
                KeyCode::End => {
                    editor.end();
                    BrowserCommand::None
                }
                KeyCode::Backspace => {
                    editor.backspace();
                    BrowserCommand::None
                }
                KeyCode::Delete => {
                    editor.delete();
                    BrowserCommand::None
                }
                _ => BrowserCommand::None,
            },
        }
    }

    pub(super) fn paste(&mut self, value: &str) {
        let BrowserMode::Rename(editor) = &mut self.mode else {
            return;
        };
        let single_line = value
            .chars()
            .map(|character| {
                if character.is_whitespace() {
                    ' '
                } else {
                    character
                }
            })
            .collect::<String>();
        editor.insert_str(&single_line);
    }

    pub(super) fn handle_mouse(&mut self, kind: MouseEventKind, session_count: usize) {
        match kind {
            MouseEventKind::ScrollUp => self.move_up(),
            MouseEventKind::ScrollDown => self.move_down(session_count),
            _ => {}
        }
    }

    fn move_up(&mut self) {
        if matches!(&self.mode, BrowserMode::Browse) {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    fn move_down(&mut self, session_count: usize) {
        if matches!(&self.mode, BrowserMode::Browse) {
            self.selected = self
                .selected
                .saturating_add(1)
                .min(session_count.saturating_sub(1));
        }
    }
}
