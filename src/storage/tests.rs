use super::SessionStorage;
use crate::session::Sessions;
use crate::types::{Message, Role};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("llm-chat-{name}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

#[test]
fn persistence_round_trip_preserves_both_histories_and_active_session() {
    let directory = temporary_directory("round-trip");
    let storage = SessionStorage::at(directory.join("sessions.json"));
    let mut sessions = Sessions::fresh();
    let active = sessions.active_mut().expect("active session");
    active.title = "Saved conversation".to_owned();
    active.transcript = vec![
        Message {
            role: Role::Reasoning,
            content: "thinking".to_owned(),
        },
        Message {
            role: Role::Error,
            content: "visible failure".to_owned(),
        },
    ];
    active.conversation = vec![
        Message {
            role: Role::User,
            content: "successful context".to_owned(),
        },
        Message {
            role: Role::Assistant,
            content: "successful answer".to_owned(),
        },
    ];

    storage.save(&sessions).expect("save sessions");
    let loaded = storage
        .load()
        .expect("load sessions")
        .expect("store exists");
    assert_eq!(loaded.active_id(), sessions.active_id());
    assert_eq!(loaded.items(), sessions.items());
    #[cfg(unix)]
    {
        let mode = fs::metadata(directory.join("sessions.json"))
            .expect("read store metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
    }
    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn malformed_store_is_an_error() {
    let directory = temporary_directory("malformed");
    let path = directory.join("sessions.json");
    fs::write(&path, b"not json").expect("write malformed store");
    let error = SessionStorage::at(path).load();
    assert!(error.is_err_and(|value| value.to_string().contains("malformed session store")));
    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn unsupported_store_version_is_an_error() {
    let directory = temporary_directory("unsupported");
    let path = directory.join("sessions.json");
    fs::write(
        &path,
        br#"{"version":99,"active_session_id":1,"next_session_id":2,"sessions":[]}"#,
    )
    .expect("write unsupported store");
    let error = SessionStorage::at(path).load();
    assert!(error.is_err_and(|value| value.to_string().contains("unsupported session store")));
    fs::remove_dir_all(directory).expect("remove test directory");
}
