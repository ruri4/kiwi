use crate::cli::Config;
use crate::input::Composer;
use crate::network::{ChatRequest, NetworkEvent};
use crate::types::{Message, ReasoningEffort, Role};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub(crate) enum Action {
    Continue,
    Quit,
    Send(ChatRequest),
}

struct PendingTurn {
    user: String,
    assistant_index: usize,
    reasoning_index: Option<usize>,
}

pub(crate) struct AppState {
    pub composer: Composer,
    pub transcript: Vec<Message>,
    pub in_flight: bool,
    pub scroll_from_bottom: usize,
    pub transcript_page: usize,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    reasoning_effort: ReasoningEffort,
    conversation: Vec<Message>,
    pending: Option<PendingTurn>,
    status: String,
    status_error: bool,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            composer: Composer::default(),
            transcript: Vec::new(),
            in_flight: false,
            scroll_from_bottom: 0,
            transcript_page: 1,
            endpoint: config.endpoint,
            model: config.model,
            api_key: config.api_key,
            reasoning_effort: config.reasoning_effort,
            conversation: Vec::new(),
            pending: None,
            status: "ready".to_owned(),
            status_error: false,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
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
                return Action::Send(self.start_request());
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

    fn start_request(&mut self) -> ChatRequest {
        let user = self.composer.take();
        self.transcript.push(Message {
            role: Role::User,
            content: user.clone(),
        });
        let assistant_index = self.transcript.len();
        self.transcript.push(Message {
            role: Role::Assistant,
            content: String::new(),
        });
        self.pending = Some(PendingTurn {
            user: user.clone(),
            assistant_index,
            reasoning_index: None,
        });
        self.in_flight = true;
        self.scroll_from_bottom = 0;
        self.status = "streaming".to_owned();
        self.status_error = false;

        let mut messages = self.conversation.clone();
        messages.push(Message {
            role: Role::User,
            content: user,
        });
        ChatRequest {
            endpoint: self.endpoint.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            messages,
            reasoning_effort: self.reasoning_effort,
        }
    }

    pub fn handle_network(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Delta(content) => {
                if content.is_empty() {
                    return;
                }
                let index = self.pending.as_ref().map(|turn| turn.assistant_index);
                let message = index.and_then(|index| self.transcript.get_mut(index));
                if let Some(message) = message {
                    message.content.push_str(&content);
                }
            }
            NetworkEvent::Reasoning(content) => self.append_reasoning(content),
            NetworkEvent::Finished => self.finish_turn(),
            NetworkEvent::Failed(error) => self.fail_turn(error),
        }
    }

    fn append_reasoning(&mut self, content: String) {
        if content.is_empty() {
            return;
        }
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        let index = match pending.reasoning_index {
            Some(index) => index,
            None => {
                let index = pending.assistant_index;
                self.transcript.insert(
                    index,
                    Message {
                        role: Role::Reasoning,
                        content: String::new(),
                    },
                );
                pending.reasoning_index = Some(index);
                pending.assistant_index += 1;
                index
            }
        };
        if let Some(message) = self.transcript.get_mut(index) {
            message.content.push_str(&content);
        }
    }

    fn finish_turn(&mut self) {
        let Some(pending) = self.pending.take() else {
            self.fail_turn("received completion without a pending request".to_owned());
            return;
        };
        let assistant = self
            .transcript
            .get(pending.assistant_index)
            .map(|message| message.content.clone())
            .unwrap_or_default();
        self.in_flight = false;
        if assistant.is_empty() {
            if pending.assistant_index < self.transcript.len() {
                self.transcript.remove(pending.assistant_index);
            }
            self.add_error("endpoint completed without assistant content".to_owned());
            return;
        }
        self.conversation.push(Message {
            role: Role::User,
            content: pending.user,
        });
        self.conversation.push(Message {
            role: Role::Assistant,
            content: assistant,
        });
        self.status = "ready".to_owned();
        self.status_error = false;
    }

    fn fail_turn(&mut self, error: String) {
        self.in_flight = false;
        if let Some(pending) = self.pending.take() {
            let empty = self
                .transcript
                .get(pending.assistant_index)
                .is_some_and(|message| message.content.is_empty());
            if empty && pending.assistant_index < self.transcript.len() {
                self.transcript.remove(pending.assistant_index);
            }
        }
        self.add_error(error);
    }

    fn add_error(&mut self, error: String) {
        self.status = "error".to_owned();
        self.status_error = true;
        self.transcript.push(Message {
            role: Role::Error,
            content: error,
        });
        self.scroll_from_bottom = 0;
    }
}
