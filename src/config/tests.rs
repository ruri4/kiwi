use super::{derive_profile_urls, parse_yaml, read_required_key_from_sources};
use crate::types::ReasoningEffort;
use std::collections::HashMap;

#[test]
fn parses_profiles_and_derives_urls_with_or_without_trailing_slash() {
    let yaml = r#"
default_endpoint: local
endpoints:
  - name: local
    base_url: http://localhost:8000/v1/
    default_model: model-a
  - name: remote
    base_url: https://example.invalid/api/v1
"#;
    let (profiles, default, reasoning) = parse_yaml(yaml, &HashMap::new()).expect("valid config");
    assert_eq!(default, "local");
    assert_eq!(reasoning, ReasoningEffort::Auto);
    let local = profiles.first().expect("local profile");
    let remote = profiles.get(1).expect("remote profile");
    assert_eq!(local.chat_url, "http://localhost:8000/v1/chat/completions");
    assert_eq!(local.models_url, "http://localhost:8000/v1/models");
    assert_eq!(
        remote.chat_url,
        "https://example.invalid/api/v1/chat/completions"
    );
}

#[test]
fn parses_configured_reasoning_effort() {
    let yaml = "reasoning_effort: high\ndefault_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1'}]";
    let (_, _, reasoning) = parse_yaml(yaml, &HashMap::new()).expect("valid config");
    assert_eq!(reasoning, ReasoningEffort::High);
}

#[test]
fn rejects_unknown_fields_duplicates_and_missing_defaults() {
    let unknown =
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', token: bad}]";
    assert!(parse_yaml(unknown, &HashMap::new()).is_err());
    let duplicate = "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1'}, {name: a, base_url: 'https://y/v1'}]";
    assert!(parse_yaml(duplicate, &HashMap::new()).is_err());
    let missing = "default_endpoint: b\nendpoints: [{name: a, base_url: 'https://x/v1'}]";
    assert!(parse_yaml(missing, &HashMap::new()).is_err());
}

#[test]
fn validates_blank_values_and_http_urls() {
    for yaml in [
        "default_endpoint: a\nendpoints: [{name: ' ', base_url: 'https://x/v1'}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'file:///v1'}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', default_model: ' '}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: ' '}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: 'A=B'}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: '1_KEY'}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: 'A-B'}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: 'A.B'}]",
        "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: 'Ä_KEY'}]",
    ] {
        assert!(
            parse_yaml(yaml, &HashMap::new()).is_err(),
            "accepted {yaml}"
        );
    }
}

#[test]
fn resolves_only_referenced_dotenv_credentials() {
    let dotenv = HashMap::from([
        ("REFERENCED_KEY".to_owned(), "dotenv-token".to_owned()),
        (
            "UNREFERENCED_URL".to_owned(),
            "https://ignored.invalid".to_owned(),
        ),
        ("UNREFERENCED_MODEL".to_owned(), "ignored-model".to_owned()),
    ]);
    let yaml = "default_endpoint: a\nendpoints: [{name: a, base_url: 'https://x/v1', api_key_env: REFERENCED_KEY}]";
    let (profiles, _, _) = parse_yaml(yaml, &dotenv).expect("valid config");
    assert_eq!(
        profiles
            .first()
            .expect("referenced profile")
            .api_key
            .as_deref(),
        Some("dotenv-token")
    );
    assert_eq!(
        read_required_key_from_sources("REFERENCED_KEY", Ok("process-token".to_owned()), &dotenv,)
            .expect("process value wins"),
        "process-token"
    );
}

#[test]
fn rejects_empty_or_non_unicode_process_credentials_without_exposing_values() {
    let dotenv = HashMap::from([("TOKEN_KEY".to_owned(), "dotenv-secret".to_owned())]);
    let empty = read_required_key_from_sources("TOKEN_KEY", Ok(String::new()), &dotenv)
        .expect_err("empty process value");
    assert!(empty.to_string().contains("TOKEN_KEY"));
    assert!(!empty.to_string().contains("dotenv-secret"));

    let non_unicode = read_required_key_from_sources(
        "TOKEN_KEY",
        Err(std::env::VarError::NotUnicode(std::ffi::OsString::from(
            "process-secret",
        ))),
        &dotenv,
    )
    .expect_err("non-unicode process value");
    assert!(non_unicode.to_string().contains("TOKEN_KEY"));
    assert!(!non_unicode.to_string().contains("process-secret"));
}

#[test]
fn derives_profile_models_url() {
    assert_eq!(
        derive_profile_urls("https://example.invalid/v1")
            .expect("valid URL")
            .1,
        "https://example.invalid/v1/models"
    );
    assert!(derive_profile_urls("https://user:secret@example.invalid/v1").is_err());
}
