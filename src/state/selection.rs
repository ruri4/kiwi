use super::{Action, AppState};
use crate::network::ModelsRequest;
use crate::picker::{Picker, PickerEvent, PickerKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl AppState {
    pub(super) fn begin_discovery(&mut self) -> Option<ModelsRequest> {
        let profile = self.active_profile()?;
        let endpoint = profile.models_url.clone();
        let api_key = profile.api_key.clone();
        self.discovering = true;
        self.status = "discovering models".to_owned();
        self.status_error = false;
        Some(ModelsRequest { endpoint, api_key })
    }

    pub(super) fn handle_session_key(&mut self, key: KeyEvent) {
        let outcome = self.sessions.handle_browser_key(key);
        if outcome.active_changed {
            self.composer = crate::input::Composer::default();
            self.scroll_from_bottom = 0;
            self.status = "ready".to_owned();
            self.status_error = false;
        }
        if outcome.changed {
            self.persist_sessions();
        }
    }

    pub(super) fn handle_composer_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.composer.insert_char('\n');
            }
            KeyCode::Enter if !self.network_active() && !self.composer.is_blank() => {
                if self.model.is_none() {
                    self.status = "no model available; press ctrl+m to retry discovery".to_owned();
                    self.status_error = true;
                } else if let Some(request) = self.start_request() {
                    return Action::Send(request);
                }
            }
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.composer.insert_char(character);
            }
            KeyCode::Left => self.composer.left(),
            KeyCode::Right => self.composer.right(),
            KeyCode::Home => self.composer.home(),
            KeyCode::End => self.composer.end(),
            KeyCode::Backspace => self.composer.backspace(),
            KeyCode::Delete => self.composer.delete(),
            KeyCode::PageUp => {
                self.scroll_from_bottom = self
                    .scroll_from_bottom
                    .saturating_add(self.transcript_page.max(1));
            }
            KeyCode::PageDown => {
                self.scroll_from_bottom = self
                    .scroll_from_bottom
                    .saturating_sub(self.transcript_page.max(1));
            }
            _ => {}
        }
        Action::Continue
    }

    pub(super) fn open_endpoint_picker(&mut self) {
        let items = self
            .endpoints
            .iter()
            .map(|endpoint| endpoint.name.clone())
            .collect();
        self.picker = Some(Picker::new(
            PickerKind::Endpoint,
            items,
            Some(self.endpoint_name()),
        ));
    }

    pub(super) fn open_model_picker(&mut self) -> Action {
        if self.network_active() {
            return Action::Continue;
        }
        if self.models.is_empty() {
            if let Some(request) = self.begin_discovery() {
                return Action::Discover(request);
            }
            self.status = "model discovery is unavailable for this endpoint".to_owned();
            self.status_error = true;
            return Action::Continue;
        }
        self.picker = Some(Picker::new(
            PickerKind::Model,
            self.models.clone(),
            self.model.as_deref(),
        ));
        Action::Continue
    }

    pub(super) fn handle_picker_key(&mut self, key: KeyEvent) -> Action {
        let Some(picker) = self.picker.as_mut() else {
            return Action::Continue;
        };
        let kind = picker.kind;
        match picker.handle_key(key) {
            PickerEvent::None => Action::Continue,
            PickerEvent::Close => {
                self.picker = None;
                Action::Continue
            }
            PickerEvent::Select(value) => {
                self.picker = None;
                match kind {
                    PickerKind::Endpoint => self.select_endpoint(&value),
                    PickerKind::Model => {
                        self.model = Some(value);
                        self.status = "ready".to_owned();
                        self.status_error = false;
                        self.persist_sessions();
                        Action::Continue
                    }
                }
            }
        }
    }

    fn select_endpoint(&mut self, name: &str) -> Action {
        let Some(index) = self
            .endpoints
            .iter()
            .position(|endpoint| endpoint.name == name)
        else {
            self.status = "selected endpoint is unavailable".to_owned();
            self.status_error = true;
            return Action::Continue;
        };
        if index == self.active_endpoint {
            return Action::Continue;
        }
        self.active_endpoint = index;
        self.models.clear();
        self.model = self
            .active_profile()
            .and_then(|profile| profile.default_model.clone());
        let discovery = self.begin_discovery();
        self.persist_sessions();
        discovery.map_or(Action::Continue, Action::Discover)
    }

    pub(super) fn finish_discovery(&mut self, models: Vec<String>) {
        self.discovering = false;
        self.models = models;
        if self.models.is_empty() {
            self.status = "model discovery returned no models".to_owned();
            self.status_error = true;
            return;
        }
        let current_valid = self
            .model
            .as_ref()
            .is_some_and(|model| self.models.contains(model));
        if !current_valid {
            let default = self
                .active_profile()
                .and_then(|profile| profile.default_model.as_ref())
                .filter(|model| self.models.contains(model))
                .cloned();
            self.model = default.or_else(|| self.models.first().cloned());
        }
        self.status = format!("ready; {} models", self.models.len());
        self.status_error = false;
        self.persist_sessions();
    }

    pub(super) fn fail_discovery(&mut self, error: String) {
        self.discovering = false;
        self.status = if self.status.starts_with("save error:") {
            format!("{}; model discovery failed: {error}", self.status)
        } else {
            format!("model discovery failed: {error}")
        };
        self.status_error = true;
    }
}
