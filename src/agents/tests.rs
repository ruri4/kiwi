use super::load_from_with;
use std::io;
use std::path::Path;

fn missing_file() -> io::Result<Vec<u8>> {
    Err(io::Error::from(io::ErrorKind::NotFound))
}

#[test]
fn missing_files_produce_no_sources() {
    let context = load_from_with(Path::new("/project/src"), |_| missing_file())
        .expect("missing AGENTS.md files should be ignored");

    assert!(context.sources.is_empty());
}

#[test]
fn sources_are_loaded_from_outermost_to_nearest() {
    let context = load_from_with(Path::new("/project/src/nested"), |path| match path {
        path if path == Path::new("/AGENTS.md") => Ok(b"outer".to_vec()),
        path if path == Path::new("/project/AGENTS.md") => Ok(b"parent".to_vec()),
        path if path == Path::new("/project/src/nested/AGENTS.md") => Ok(b"nearest".to_vec()),
        _ => missing_file(),
    })
    .expect("fixture files should load");

    let contents: Vec<&str> = context
        .sources
        .iter()
        .map(|source| source.content.as_str())
        .collect();
    assert_eq!(contents, vec!["outer", "parent", "nearest"]);
}

#[test]
fn request_framing_states_precedence_and_places_user_last() {
    let context = load_from_with(Path::new("/project/src"), |path| match path {
        path if path == Path::new("/AGENTS.md") => Ok(b"outer rule".to_vec()),
        path if path == Path::new("/project/src/AGENTS.md") => Ok(b"near rule".to_vec()),
        _ => missing_file(),
    })
    .expect("fixture files should load");
    let request = context.for_request("do this explicitly");

    assert!(request.contains("later, nearer AGENTS.md file overrides conflicting"));
    assert!(request.contains("explicit user request below takes precedence over every AGENTS.md"));
    let Some(outer) = request.find("outer rule") else {
        panic!("outer source is missing");
    };
    let Some(near) = request.find("near rule") else {
        panic!("near source is missing");
    };
    let Some(user) = request.find("do this explicitly") else {
        panic!("explicit request is missing");
    };
    assert!(outer < near);
    assert!(near < user);
}

#[test]
fn invalid_utf8_reports_path_without_content() {
    let error = match load_from_with(Path::new("/project"), |path| {
        if path == Path::new("/project/AGENTS.md") {
            Ok(vec![0xff, b's', b'e', b'c', b'r', b'e', b't'])
        } else {
            missing_file()
        }
    }) {
        Ok(_) => panic!("invalid UTF-8 should fail discovery"),
        Err(error) => error,
    };
    let message = format!("{error:#}");

    assert!(message.contains("/project/AGENTS.md"));
    assert!(!message.contains("secret"));
}

#[test]
fn read_failures_report_path_without_file_contents() {
    let error = match load_from_with(Path::new("/project"), |path| {
        if path == Path::new("/project/AGENTS.md") {
            Err(io::Error::other("fixture read failure"))
        } else {
            missing_file()
        }
    }) {
        Ok(_) => panic!("read failures should fail discovery"),
        Err(error) => error,
    };
    let message = format!("{error:#}");

    assert!(message.contains("/project/AGENTS.md"));
    assert!(message.contains("fixture read failure"));
}
