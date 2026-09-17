use super::{Session, Sessions, UNTITLED, automatic_title};
use crate::types::{Message, Role};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn automatic_names_collapse_whitespace_and_truncate_by_characters() {
    assert_eq!(
        automatic_title("  hello\n\twide   world  "),
        "hello wide world"
    );
    let unicode = "界".repeat(60);
    let title = automatic_title(&unicode);
    assert_eq!(title.chars().count(), 49);
    assert!(title.ends_with('…'));
    assert_eq!(title.trim_end_matches('…'), "界".repeat(48));

    let mut sessions = Sessions::fresh();
    sessions.name_first_message(&unicode);
    let first_title = sessions.active_title().to_owned();
    sessions.name_first_message("a later message");
    assert_eq!(sessions.active_title(), first_title);
}

#[test]
fn manual_name_is_not_replaced_by_first_message() {
    let mut sessions = Sessions::fresh();
    let active = sessions.active_mut().expect("fresh active session");
    active.title = "My chat".to_owned();
    active.manual_title = true;
    sessions.name_first_message("automatic candidate");
    assert_eq!(sessions.active_title(), "My chat");
}

#[test]
fn deleting_the_only_session_creates_an_untitled_replacement() {
    let mut sessions = Sessions::fresh();
    sessions.toggle_browser();
    let pending = sessions.handle_browser_key(key(KeyCode::Char('d')));
    assert!(!pending.changed);
    let outcome = sessions.handle_browser_key(key(KeyCode::Char('y')));
    assert!(outcome.changed);
    assert!(outcome.active_changed);
    assert_eq!(sessions.items().len(), 1);
    assert_eq!(sessions.active_title(), UNTITLED);
    assert_ne!(sessions.active_id(), 1);
}

#[test]
fn browser_rename_accepts_unicode_paste_and_marks_name_manual() {
    let mut sessions = Sessions::fresh();
    sessions.toggle_browser();
    sessions.handle_browser_key(key(KeyCode::Char('r')));
    sessions.paste_into_browser("  🌙 chat");
    let outcome = sessions.handle_browser_key(key(KeyCode::Enter));
    assert!(outcome.changed);
    assert_eq!(sessions.active_title(), "Untitled 🌙 chat");
    sessions.name_first_message("must not replace the manual name");
    assert_eq!(sessions.active_title(), "Untitled 🌙 chat");
}

#[test]
fn restore_rejects_non_api_conversation_roles() {
    let mut session = Session::empty(4);
    session.conversation.push(Message {
        role: Role::Error,
        content: "not API context".to_owned(),
    });
    let result = Sessions::restore(vec![session], 4, 5);
    assert!(result.is_err_and(|error| error.to_string().contains("non-API")));
}
