use crate::types::{Message, Role};
use anyhow::{Result, bail};
use crossterm::event::{KeyEvent, MouseEventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod browser;
use browser::BrowserCommand;
pub(crate) use browser::{BrowserMode, SessionBrowser};

pub(crate) const UNTITLED: &str = "Untitled";
const TITLE_CHAR_LIMIT: usize = 48;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Session {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) manual_title: bool,
    pub(crate) transcript: Vec<Message>,
    pub(crate) conversation: Vec<Message>,
}

impl Session {
    fn empty(id: u64) -> Self {
        Self {
            id,
            title: UNTITLED.to_owned(),
            manual_title: false,
            transcript: Vec::new(),
            conversation: Vec::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct BrowserOutcome {
    pub(crate) changed: bool,
    pub(crate) active_changed: bool,
}

pub(crate) struct Sessions {
    items: Vec<Session>,
    active_id: u64,
    next_id: u64,
    browser: Option<SessionBrowser>,
}

impl Sessions {
    pub(crate) fn fresh() -> Self {
        Self {
            items: vec![Session::empty(1)],
            active_id: 1,
            next_id: 2,
            browser: None,
        }
    }

    pub(crate) fn restore(items: Vec<Session>, active_id: u64, next_id: u64) -> Result<Self> {
        if items.is_empty() {
            bail!("session store contains no sessions");
        }
        let mut ids = HashSet::new();
        for session in &items {
            if !ids.insert(session.id) {
                bail!("session store contains duplicate id {}", session.id);
            }
            if session.title.trim().is_empty() {
                bail!("session {} has a blank title", session.id);
            }
            if session
                .conversation
                .iter()
                .any(|message| !matches!(message.role, Role::User | Role::Assistant))
            {
                bail!("session {} has a non-API conversation message", session.id);
            }
        }
        if !ids.contains(&active_id) {
            bail!("active session id {active_id} does not exist");
        }
        let largest_id = items.iter().map(|session| session.id).max().unwrap_or(0);
        if next_id <= largest_id {
            bail!("next session id {next_id} is not greater than existing ids");
        }
        Ok(Self {
            items,
            active_id,
            next_id,
            browser: None,
        })
    }

    pub(crate) fn items(&self) -> &[Session] {
        &self.items
    }

    pub(crate) fn active_id(&self) -> u64 {
        self.active_id
    }

    pub(crate) fn next_id(&self) -> u64 {
        self.next_id
    }

    pub(crate) fn active(&self) -> Option<&Session> {
        self.items
            .iter()
            .find(|session| session.id == self.active_id)
    }

    pub(crate) fn active_mut(&mut self) -> Option<&mut Session> {
        self.items
            .iter_mut()
            .find(|session| session.id == self.active_id)
    }

    pub(crate) fn active_title(&self) -> &str {
        self.active()
            .map(|session| session.title.as_str())
            .unwrap_or(UNTITLED)
    }

    pub(crate) fn browser(&self) -> Option<&SessionBrowser> {
        self.browser.as_ref()
    }

    pub(crate) fn browser_is_open(&self) -> bool {
        self.browser.is_some()
    }

    pub(crate) fn toggle_browser(&mut self) {
        if self.browser.is_some() {
            self.browser = None;
            return;
        }
        let selected = self
            .items
            .iter()
            .position(|session| session.id == self.active_id)
            .unwrap_or(0);
        self.browser = Some(SessionBrowser {
            selected,
            mode: BrowserMode::Browse,
        });
    }

    pub(crate) fn name_first_message(&mut self, message: &str) {
        if let Some(session) = self.active_mut()
            && !session.manual_title
            && session.title == UNTITLED
        {
            session.title = automatic_title(message);
        }
    }

    pub(crate) fn handle_browser_key(&mut self, key: KeyEvent) -> BrowserOutcome {
        let selected_title = self
            .browser
            .as_ref()
            .and_then(|browser| self.items.get(browser.selected))
            .map(|session| session.title.clone())
            .unwrap_or_default();
        let command = match self.browser.as_mut() {
            Some(browser) => browser.handle_key(key, self.items.len(), &selected_title),
            None => BrowserCommand::None,
        };
        self.apply_browser_command(command)
    }

    pub(crate) fn paste_into_browser(&mut self, value: &str) {
        if let Some(browser) = self.browser.as_mut() {
            browser.paste(value);
        }
    }

    pub(crate) fn handle_browser_mouse(&mut self, kind: MouseEventKind) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        browser.handle_mouse(kind, self.items.len());
    }

    fn apply_browser_command(&mut self, command: BrowserCommand) -> BrowserOutcome {
        let mut outcome = BrowserOutcome::default();
        match command {
            BrowserCommand::None => {}
            BrowserCommand::Close => self.browser = None,
            BrowserCommand::Switch(index) => {
                if let Some(session) = self.items.get(index) {
                    outcome.active_changed = session.id != self.active_id;
                    self.active_id = session.id;
                    outcome.changed = outcome.active_changed;
                }
            }
            BrowserCommand::Create => {
                let id = self.next_id;
                let Some(next_id) = self.next_id.checked_add(1) else {
                    return outcome;
                };
                self.next_id = next_id;
                self.items.push(Session::empty(id));
                self.active_id = id;
                if let Some(browser) = self.browser.as_mut() {
                    browser.selected = self.items.len().saturating_sub(1);
                }
                outcome.changed = true;
                outcome.active_changed = true;
            }
            BrowserCommand::Rename(index, title) => {
                if let Some(session) = self.items.get_mut(index) {
                    session.title = title;
                    session.manual_title = true;
                    outcome.changed = true;
                }
            }
            BrowserCommand::Delete(index) => {
                outcome = self.delete(index);
            }
        }
        outcome
    }

    fn delete(&mut self, index: usize) -> BrowserOutcome {
        let Some(deleted) = self.items.get(index) else {
            return BrowserOutcome::default();
        };
        let deleted_id = deleted.id;
        let deleted_active = deleted.id == self.active_id;
        self.items.remove(index);
        if self.items.is_empty() {
            let (id, next_id) = self
                .next_id
                .checked_add(1)
                .map_or((deleted_id, self.next_id), |next_id| {
                    (self.next_id, next_id)
                });
            self.next_id = next_id;
            self.items.push(Session::empty(id));
            self.active_id = id;
        } else if deleted_active {
            let replacement = index.min(self.items.len().saturating_sub(1));
            if let Some(session) = self.items.get(replacement) {
                self.active_id = session.id;
            }
        }
        if let Some(browser) = self.browser.as_mut() {
            browser.selected = index.min(self.items.len().saturating_sub(1));
            browser.mode = BrowserMode::Browse;
        }
        BrowserOutcome {
            changed: true,
            active_changed: deleted_active,
        }
    }
}

pub(crate) fn automatic_title(message: &str) -> String {
    let normalized = normalize_title(message);
    let mut characters = normalized.chars();
    let title = characters
        .by_ref()
        .take(TITLE_CHAR_LIMIT)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{title}…")
    } else {
        title
    }
}

pub(super) fn normalize_title(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
