use crate::types::ReasoningEffort;
use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use reqwest::Url;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.yaml";

pub(crate) struct Config {
    pub(crate) endpoints: Vec<Endpoint>,
    pub(crate) default_endpoint: String,
    pub(crate) reasoning_effort: ReasoningEffort,
}

#[derive(Clone)]
pub(crate) struct Endpoint {
    pub(crate) name: String,
    pub(crate) chat_url: String,
    pub(crate) models_url: String,
    pub(crate) api_key: Option<String>,
    pub(crate) default_model: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct YamlConfig {
    default_endpoint: String,
    #[serde(default)]
    reasoning_effort: ReasoningEffort,
    endpoints: Vec<YamlEndpoint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct YamlEndpoint {
    name: String,
    base_url: String,
    api_key_env: Option<String>,
    default_model: Option<String>,
}

pub(crate) fn load(config_path: Option<PathBuf>) -> Result<Config> {
    let path = match config_path {
        Some(path) => path,
        None => default_path()?,
    };
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            bail!("configuration file {} does not exist", path.display());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read configuration file {}", path.display()));
        }
    };
    let dotenv = read_dotenv()?;
    let (endpoints, default_endpoint, reasoning_effort) = parse_yaml(&source, &dotenv)
        .with_context(|| format!("invalid configuration {}", path.display()))?;
    Ok(Config {
        endpoints,
        default_endpoint,
        reasoning_effort,
    })
}

fn default_path() -> Result<PathBuf> {
    let directories = ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))
        .context("platform configuration directory is unavailable")?;
    Ok(directories.config_dir().join(CONFIG_FILE))
}

fn parse_yaml(
    source: &str,
    dotenv: &HashMap<String, String>,
) -> Result<(Vec<Endpoint>, String, ReasoningEffort)> {
    let document: YamlConfig =
        serde_saphyr::from_str(source).context("failed to deserialize YAML")?;
    if document.default_endpoint.trim().is_empty() {
        bail!("default_endpoint cannot be blank");
    }
    if document.endpoints.is_empty() {
        bail!("endpoints must contain at least one profile");
    }

    let mut names = HashSet::new();
    let mut endpoints = Vec::with_capacity(document.endpoints.len());
    for profile in document.endpoints {
        if profile.name.trim().is_empty() {
            bail!("endpoint names cannot be blank");
        }
        if !names.insert(profile.name.clone()) {
            bail!("duplicate endpoint name {:?}", profile.name);
        }
        validate_optional_model(profile.default_model.as_deref())?;
        if let Some(name) = profile.api_key_env.as_deref() {
            if name.trim().is_empty() {
                bail!(
                    "api_key_env for endpoint {:?} cannot be blank",
                    profile.name
                );
            }
            if !is_valid_env_name(name) {
                bail!(
                    "api_key_env for endpoint {:?} is not a valid environment variable name",
                    profile.name
                );
            }
        }
        let (chat_url, models_url) = derive_profile_urls(&profile.base_url)
            .with_context(|| format!("invalid base_url for endpoint {:?}", profile.name))?;
        let api_key = profile
            .api_key_env
            .as_deref()
            .map(|name| read_required_key(name, dotenv))
            .transpose()?;
        endpoints.push(Endpoint {
            name: profile.name,
            chat_url,
            models_url,
            api_key,
            default_model: profile.default_model,
        });
    }
    if !names.contains(&document.default_endpoint) {
        bail!(
            "default_endpoint {:?} does not name an endpoint",
            document.default_endpoint
        );
    }
    Ok((
        endpoints,
        document.default_endpoint,
        document.reasoning_effort,
    ))
}

fn validate_optional_model(model: Option<&str>) -> Result<()> {
    if model.is_some_and(|value| value.trim().is_empty()) {
        bail!("model identifiers cannot be blank");
    }
    Ok(())
}

fn is_valid_env_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(
        characters.next(),
        Some(character) if character.is_ascii_alphabetic() || character == '_'
    ) && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn read_dotenv() -> Result<HashMap<String, String>> {
    let entries = match dotenvy::dotenv_iter() {
        Ok(entries) => entries,
        Err(error) if error.not_found() => return Ok(HashMap::new()),
        Err(error) => return Err(error).context("failed to read optional .env file"),
    };
    entries
        .map(|entry| entry.map_err(|_| anyhow::anyhow!("failed to parse optional .env file")))
        .collect()
}

fn read_required_key(name: &str, dotenv: &HashMap<String, String>) -> Result<String> {
    read_required_key_from_sources(name, std::env::var(name), dotenv)
}

fn read_required_key_from_sources(
    name: &str,
    process_value: std::result::Result<String, std::env::VarError>,
    dotenv: &HashMap<String, String>,
) -> Result<String> {
    let value = match process_value {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => dotenv
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("environment variable {name:?} is not set"))?,
        Err(std::env::VarError::NotUnicode(_)) => {
            bail!("environment variable {name:?} is not valid Unicode")
        }
    };
    if value.is_empty() {
        bail!("environment variable {name:?} is empty");
    }
    Ok(value)
}

fn derive_profile_urls(base: &str) -> Result<(String, String)> {
    let mut url = Url::parse(base).context("base URL is not a valid URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("base URL must be an absolute HTTP(S) URL");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("base URL must not contain credentials");
    }
    if url.query().is_some() || url.fragment().is_some() {
        bail!("base URL must not contain a query or fragment");
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&format!("{path}/"));
    let chat = url
        .join("chat/completions")
        .context("failed to derive chat URL")?;
    let models = url.join("models").context("failed to derive models URL")?;
    Ok((chat.to_string(), models.to_string()))
}

#[cfg(test)]
mod tests;
