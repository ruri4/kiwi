use crate::config::{Config, Endpoint};
use crate::input::Composer;
use crate::network::{ChatRequest, ModelsRequest, NetworkEvent};
use crate::picker::Picker;
use crate::session::{Session, SessionBrowser, Sessions};
use crate::storage::{GlobalSelection, SessionStorage};
use crate::types::{Message, ReasoningEffort};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};

const MOUSE_SCROLL_LINES: usize = 3;

mod turn;

pub(crate) enum Action {
    Continue,
    Quit,
    Send(ChatRequest),
    Discover(ModelsRequest),
}

struct PendingTurn {
    user: String,
    assistant_index: usize,
    reasoning_index: Option<usize>,
    pending_whitespace: String,
}

pub(crate) struct AppState {
    pub composer: Composer,
    pub in_flight: bool,
    pub scroll_from_bottom: usize,
    pub transcript_page: usize,
    endpoints: Vec<Endpoint>,
    active_endpoint: usize,
    model: Option<String>,
    models: Vec<String>,
    discovering: bool,
    picker: Option<Picker>,
    reasoning_effort: ReasoningEffort,
    agents: crate::agents::InstructionContext,
    sessions: Sessions,
    storage: SessionStorage,
    pending: Option<PendingTurn>,
    status: String,
    status_error: bool,
}

impl AppState {
    pub fn load(config: Config) -> Result<Self> {
        let agents = crate::agents::discover()?;
        let storage = SessionStorage::discover()?;
        let stored = storage
            .load()
            .context("failed to restore saved chat sessions")?;
        let (sessions, saved) = stored
            .map(|stored| (stored.sessions, stored.selection))
            .unwrap_or_else(|| (Sessions::fresh(), None));
        let active_endpoint = saved
            .as_ref()
            .and_then(|selection| {
                config
                    .endpoints
                    .iter()
                    .position(|endpoint| endpoint.name == selection.endpoint)
            })
            .or_else(|| {
                config
                    .endpoints
                    .iter()
                    .position(|endpoint| endpoint.name == config.default_endpoint)
            })
            .context("configured default endpoint is unavailable")?;
        let endpoint = config
            .endpoints
            .get(active_endpoint)
            .context("active endpoint is unavailable")?;
        let saved_model = saved.as_ref().and_then(|selection| {
            if selection.endpoint == endpoint.name {
                selection.model.clone()
            } else {
                None
            }
        });
        let model = saved_model.or_else(|| endpoint.default_model.clone());
        Ok(Self {
            composer: Composer::default(),
            in_flight: false,
            scroll_from_bottom: 0,
            transcript_page: 1,
            endpoints: config.endpoints,
            active_endpoint,
            model,
            models: Vec::new(),
            discovering: false,
            picker: None,
            reasoning_effort: config.reasoning_effort,
            agents,
            sessions,
            storage,
            pending: None,
            status: "ready".to_owned(),
            status_error: false,
        })
    }

    pub fn begin_startup_discovery(&mut self) -> Option<ModelsRequest> {
        self.begin_discovery()
    }

    pub fn network_active(&self) -> bool {
        self.in_flight || self.discovering
    }

    pub fn endpoint_name(&self) -> &str {
        self.active_profile()
            .map_or("unavailable", |item| &item.name)
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or("none")
    }

    pub fn reasoning_effort(&self) -> ReasoningEffort {
        self.reasoning_effort
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn status_is_error(&self) -> bool {
        self.status_error
    }

    pub fn active_session_name(&self) -> &str {
        self.sessions.active_title()
    }

    pub fn active_session_id(&self) -> u64 {
        self.sessions.active_id()
    }

    pub fn transcript(&self) -> &[Message] {
        self.sessions
            .active()
            .map(|session| session.transcript.as_slice())
            .unwrap_or(&[])
    }

    pub fn sessions(&self) -> &[Session] {
        self.sessions.items()
    }

    pub fn session_browser(&self) -> Option<&SessionBrowser> {
        self.sessions.browser()
    }

    pub fn picker(&self) -> Option<&Picker> {
        self.picker.as_ref()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Action::Continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C' | 'q' | 'Q'))
        {
            return Action::Quit;
        }
        if self.picker.is_some() {
            return self.handle_picker_key(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s' | 'S'))
        {
            if !self.in_flight {
                self.sessions.toggle_browser();
            }
            return Action::Continue;
        }
        if self.sessions.browser_is_open() {
            self.handle_session_key(key);
            return Action::Continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('e' | 'E'))
        {
            if !self.network_active() {
                self.open_endpoint_picker();
            }
            return Action::Continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('m' | 'M'))
        {
            return self.open_model_picker();
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r' | 'R'))
        {
            if !self.in_flight {
                self.reasoning_effort = self.reasoning_effort.cycle();
            }
            return Action::Continue;
        }
        self.handle_composer_key(key)
    }

    pub fn handle_paste(&mut self, value: &str) {
        if self.picker.is_some() {
            return;
        }
        if self.sessions.browser_is_open() {
            self.sessions.paste_into_browser(value);
        } else {
            self.composer.insert_str(value);
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.picker.is_some() {
            return;
        }
        if self.sessions.browser_is_open() {
            self.sessions.handle_browser_mouse(mouse.kind);
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_from_bottom =
                    self.scroll_from_bottom.saturating_add(MOUSE_SCROLL_LINES);
            }
            MouseEventKind::ScrollDown => {
                self.scroll_from_bottom =
                    self.scroll_from_bottom.saturating_sub(MOUSE_SCROLL_LINES);
            }
            _ => {}
        }
    }

    pub fn handle_network(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Delta(content) => self.append_delta(content),
            NetworkEvent::Reasoning(content) => self.append_reasoning(content),
            NetworkEvent::Finished => self.finish_turn(),
            NetworkEvent::Failed(error) => self.fail_turn(error),
            NetworkEvent::Models(models) => self.finish_discovery(models),
            NetworkEvent::ModelsFailed(error) => self.fail_discovery(error),
        }
    }

    pub(super) fn persist_sessions(&mut self) {
        let selection = self.selection();
        match self.storage.save(&self.sessions, Some(&selection)) {
            Ok(()) if self.status.starts_with("save error:") => {
                self.status = "ready".to_owned();
                self.status_error = false;
            }
            Ok(()) => {}
            Err(error) => {
                self.status = format!("save error: {error:#}");
                self.status_error = true;
            }
        }
    }

    fn selection(&self) -> GlobalSelection {
        GlobalSelection {
            endpoint: self.endpoint_name().to_owned(),
            model: self.model.clone(),
        }
    }

    fn active_profile(&self) -> Option<&Endpoint> {
        self.endpoints.get(self.active_endpoint)
    }
}

mod selection;
