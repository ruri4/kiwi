use super::{AppState, PendingTurn};
use crate::network::{ChatRequest, NetworkEvent};
use crate::types::{Message, Role};

impl AppState {
    pub(super) fn start_request(&mut self) -> Option<ChatRequest> {
        if self.sessions.active().is_none() {
            self.add_error("active session is unavailable".to_owned());
            return None;
        }
        let user = self.composer.take();
        self.sessions.name_first_message(&user);
        let conversation = self.sessions.active()?.conversation.clone();
        let transcript = &mut self.sessions.active_mut()?.transcript;
        transcript.push(Message {
            role: Role::User,
            content: user.clone(),
        });
        let assistant_index = transcript.len();
        transcript.push(Message {
            role: Role::Assistant,
            content: String::new(),
        });
        self.pending = Some(PendingTurn {
            user: user.clone(),
            assistant_index,
            reasoning_index: None,
            pending_whitespace: String::new(),
        });
        self.in_flight = true;
        self.scroll_from_bottom = 0;
        self.status = "streaming".to_owned();
        self.status_error = false;

        let mut messages = conversation;
        messages.push(Message {
            role: Role::User,
            content: user,
        });
        Some(ChatRequest {
            endpoint: self.endpoint.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            messages,
            reasoning_effort: self.reasoning_effort,
        })
    }

    pub fn handle_network(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Delta(content) => self.append_delta(content),
            NetworkEvent::Reasoning(content) => self.append_reasoning(content),
            NetworkEvent::Finished => self.finish_turn(),
            NetworkEvent::Failed(error) => self.fail_turn(error),
        }
    }

    fn append_delta(&mut self, content: String) {
        if content.is_empty() {
            return;
        }
        let (Some(pending), Some(session)) = (self.pending.as_mut(), self.sessions.active_mut())
        else {
            return;
        };
        let Some(message) = session.transcript.get_mut(pending.assistant_index) else {
            return;
        };
        append_trimmed_delta(
            &mut message.content,
            &mut pending.pending_whitespace,
            &content,
        );
    }

    fn append_reasoning(&mut self, content: String) {
        if content.is_empty() {
            return;
        }
        let (Some(pending), Some(session)) = (self.pending.as_mut(), self.sessions.active_mut())
        else {
            return;
        };
        let index = pending.reasoning_index.unwrap_or_else(|| {
            let index = pending.assistant_index;
            session.transcript.insert(
                index,
                Message {
                    role: Role::Reasoning,
                    content: String::new(),
                },
            );
            pending.reasoning_index = Some(index);
            pending.assistant_index += 1;
            index
        });
        if let Some(message) = session.transcript.get_mut(index) {
            message.content.push_str(&content);
        }
    }

    fn finish_turn(&mut self) {
        let Some(pending) = self.pending.take() else {
            self.fail_turn("received completion without a pending request".to_owned());
            return;
        };
        self.in_flight = false;
        let assistant = self
            .sessions
            .active()
            .and_then(|session| session.transcript.get(pending.assistant_index))
            .map(|message| message.content.clone())
            .unwrap_or_default();
        if assistant.is_empty() {
            if let Some(session) = self.sessions.active_mut()
                && pending.assistant_index < session.transcript.len()
            {
                session.transcript.remove(pending.assistant_index);
            }
            self.add_error("endpoint completed without assistant content".to_owned());
            self.persist_sessions();
            return;
        }
        if let Some(session) = self.sessions.active_mut() {
            session.conversation.push(Message {
                role: Role::User,
                content: pending.user,
            });
            session.conversation.push(Message {
                role: Role::Assistant,
                content: assistant,
            });
        }
        self.status = "ready".to_owned();
        self.status_error = false;
        self.persist_sessions();
    }

    fn fail_turn(&mut self, error: String) {
        self.in_flight = false;
        if let Some(pending) = self.pending.take()
            && let Some(session) = self.sessions.active_mut()
        {
            let empty = session
                .transcript
                .get(pending.assistant_index)
                .is_some_and(|message| message.content.is_empty());
            if empty && pending.assistant_index < session.transcript.len() {
                session.transcript.remove(pending.assistant_index);
            }
        }
        self.add_error(error);
        self.persist_sessions();
    }

    fn add_error(&mut self, error: String) {
        self.status = "error".to_owned();
        self.status_error = true;
        if let Some(session) = self.sessions.active_mut() {
            session.transcript.push(Message {
                role: Role::Error,
                content: error,
            });
        }
        self.scroll_from_bottom = 0;
    }
}

fn append_trimmed_delta(output: &mut String, pending_whitespace: &mut String, content: &str) {
    for character in content.chars() {
        if character.is_whitespace() {
            if !output.is_empty() {
                pending_whitespace.push(character);
            }
        } else {
            output.push_str(pending_whitespace);
            pending_whitespace.clear();
            output.push(character);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::append_trimmed_delta;

    fn stream(chunks: &[&str]) -> (String, String) {
        let (mut output, mut pending) = (String::new(), String::new());
        for chunk in chunks {
            append_trimmed_delta(&mut output, &mut pending, chunk);
        }
        (output, pending)
    }

    #[test]
    fn trims_stream_whitespace_across_chunks() {
        let (output, pending) = stream(&[" \n", "\u{2003}answer", "\u{a0}\t"]);
        assert_eq!(output, "answer");
        assert_eq!(pending, "\u{a0}\t");
    }

    #[test]
    fn preserves_internal_whitespace_across_chunks() {
        let (output, pending) = stream(&["alpha \u{2003}", "\n beta"]);
        assert_eq!(output, "alpha \u{2003}\n beta");
        assert!(pending.is_empty());
    }

    #[test]
    fn all_whitespace_chunks_leave_output_and_pending_empty() {
        let (output, pending) = stream(&[" \n", "\u{2003}\u{a0}\t"]);
        assert!(output.is_empty());
        assert!(pending.is_empty());
    }
}
