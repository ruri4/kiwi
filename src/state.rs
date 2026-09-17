use crate::cli::Config;
use crate::input::Composer;
use crate::network::ChatRequest;
use crate::session::{Session, SessionBrowser, Sessions};
use crate::storage::SessionStorage;
use crate::types::{Message, ReasoningEffort};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};

const MOUSE_SCROLL_LINES: usize = 3;

mod turn;

pub(crate) enum Action {
    Continue,
    Quit,
    Send(ChatRequest),
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
    endpoint: String,
    model: String,
    api_key: Option<String>,
    reasoning_effort: ReasoningEffort,
    sessions: Sessions,
    storage: SessionStorage,
    pending: Option<PendingTurn>,
    status: String,
    status_error: bool,
}

impl AppState {
    pub fn load(config: Config) -> Result<Self> {
        let storage = SessionStorage::discover()?;
        let sessions = storage
            .load()
            .context("failed to restore saved chat sessions")?
            .unwrap_or_else(Sessions::fresh);
        Ok(Self {
            composer: Composer::default(),
            in_flight: false,
            scroll_from_bottom: 0,
            transcript_page: 1,
            endpoint: config.endpoint,
            model: config.model,
            api_key: config.api_key,
            reasoning_effort: config.reasoning_effort,
            sessions,
            storage,
            pending: None,
            status: "ready".to_owned(),
            status_error: false,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
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

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Action::Continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C' | 'q' | 'Q'))
        {
            return Action::Quit;
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
            let outcome = self.sessions.handle_browser_key(key);
            if outcome.active_changed {
                self.composer = Composer::default();
                self.scroll_from_bottom = 0;
                self.status = "ready".to_owned();
                self.status_error = false;
            }
            if outcome.changed {
                self.persist_sessions();
            }
            return Action::Continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r' | 'R'))
        {
            if !self.in_flight {
                self.reasoning_effort = self.reasoning_effort.cycle();
            }
            return Action::Continue;
        }

        match key.code {
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.composer.insert_char('\n');
            }
            KeyCode::Enter if !self.in_flight && !self.composer.is_blank() => {
                if let Some(request) = self.start_request() {
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

    pub fn handle_paste(&mut self, value: &str) {
        if self.sessions.browser_is_open() {
            self.sessions.paste_into_browser(value);
        } else {
            self.composer.insert_str(value);
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
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

    pub(super) fn persist_sessions(&mut self) {
        match self.storage.save(&self.sessions) {
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
}
