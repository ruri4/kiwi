use crossterm::event::{KeyCode, KeyEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PickerKind {
    Endpoint,
    Model,
}

pub(crate) struct Picker {
    pub(crate) kind: PickerKind,
    pub(crate) items: Vec<String>,
    pub(crate) selected: usize,
}

pub(crate) enum PickerEvent {
    None,
    Close,
    Select(String),
}

impl Picker {
    pub(crate) fn new(kind: PickerKind, items: Vec<String>, current: Option<&str>) -> Self {
        let selected = current
            .and_then(|current| items.iter().position(|item| item == current))
            .unwrap_or(0);
        Self {
            kind,
            items,
            selected,
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> PickerEvent {
        match key.code {
            KeyCode::Esc => PickerEvent::Close,
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                PickerEvent::None
            }
            KeyCode::Down => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.items.len().saturating_sub(1));
                PickerEvent::None
            }
            KeyCode::Enter => self
                .items
                .get(self.selected)
                .cloned()
                .map_or(PickerEvent::None, PickerEvent::Select),
            _ => PickerEvent::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Picker, PickerEvent, PickerKind};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn picker_navigation_is_bounded_and_selects_unicode_safely() {
        let mut picker = Picker::new(
            PickerKind::Model,
            vec!["alpha".to_owned(), "界-model".to_owned()],
            Some("alpha"),
        );
        assert!(matches!(
            picker.handle_key(key(KeyCode::Up)),
            PickerEvent::None
        ));
        assert_eq!(picker.selected, 0);
        picker.handle_key(key(KeyCode::Down));
        picker.handle_key(key(KeyCode::Down));
        assert_eq!(picker.selected, 1);
        assert!(matches!(
            picker.handle_key(key(KeyCode::Enter)),
            PickerEvent::Select(value) if value == "界-model"
        ));
    }
}
