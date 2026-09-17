use crate::sse::{SseDecoder, SseItem};
use crate::types::{Message, ReasoningEffort};
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::header::ACCEPT;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

const ERROR_BODY_LIMIT: usize = 8 * 1024;

#[derive(Clone)]
pub struct ChatRequest {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub messages: Vec<Message>,
    pub reasoning_effort: ReasoningEffort,
}

#[derive(Clone)]
pub struct ModelsRequest {
    pub endpoint: String,
    pub api_key: Option<String>,
}

#[derive(Debug)]
pub enum NetworkEvent {
    Delta(String),
    Reasoning(String),
    Finished,
    Failed(String),
    Models(Vec<String>),
    ModelsFailed(String),
}

pub async fn discover_models(
    client: reqwest::Client,
    request: ModelsRequest,
    sender: UnboundedSender<NetworkEvent>,
) {
    let event = match run_discovery(client, &request).await {
        Ok(models) => NetworkEvent::Models(models),
        Err(error) => NetworkEvent::ModelsFailed(redact_key(
            &format!("{error:#}"),
            request.api_key.as_deref(),
        )),
    };
    let _receiver_closed = sender.send(event).is_err();
}

async fn run_discovery(client: reqwest::Client, request: &ModelsRequest) -> Result<Vec<String>> {
    let mut builder = client.get(&request.endpoint);
    if let Some(api_key) = &request.api_key {
        builder = builder.bearer_auth(api_key);
    }
    let response = builder
        .send()
        .await
        .context("model discovery request failed")?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("model discovery returned HTTP {status}");
    }
    let bytes = response
        .bytes()
        .await
        .context("failed to read model discovery response")?;
    parse_models(&bytes).context("failed to decode model discovery response")
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

fn parse_models(bytes: &[u8]) -> Result<Vec<String>> {
    let response: ModelsResponse = serde_json::from_slice(bytes)?;
    let mut models = response
        .data
        .into_iter()
        .map(|model| model.id)
        .filter(|id| !id.trim().is_empty())
        .collect::<Vec<_>>();
    models.sort_unstable();
    models.dedup();
    Ok(models)
}

pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .context("failed to construct HTTP client")
}

pub async fn stream_chat(
    client: reqwest::Client,
    request: ChatRequest,
    sender: UnboundedSender<NetworkEvent>,
) {
    if let Err(error) = run_stream(client, &request, &sender).await {
        let message = redact_key(&format!("{error:#}"), request.api_key.as_deref());
        let _receiver_closed = sender.send(NetworkEvent::Failed(message)).is_err();
    }
}

async fn run_stream(
    client: reqwest::Client,
    request: &ChatRequest,
    sender: &UnboundedSender<NetworkEvent>,
) -> Result<()> {
    let messages = request
        .messages
        .iter()
        .filter_map(|message| {
            message
                .role
                .api_name()
                .map(|role| json!({ "role": role, "content": message.content }))
        })
        .collect::<Vec<_>>();
    let mut payload = json!({
        "model": request.model,
        "messages": messages,
        "stream": true
    });
    if let Some(effort) = request.reasoning_effort.api_value() {
        payload["reasoning_effort"] = json!(effort);
    }

    let mut builder = client
        .post(&request.endpoint)
        .header(ACCEPT, "text/event-stream")
        .json(&payload);
    if let Some(api_key) = &request.api_key {
        builder = builder.bearer_auth(api_key);
    }
    let response = builder.send().await.context("chat request failed")?;
    let status = response.status();
    let mut stream = response.bytes_stream();

    if !status.is_success() {
        let mut body = Vec::new();
        let mut truncated = false;
        while body.len() < ERROR_BODY_LIMIT {
            let Some(chunk) = stream.next().await else {
                break;
            };
            let chunk = chunk.context("failed while reading HTTP error response")?;
            let remaining = ERROR_BODY_LIMIT - body.len();
            if chunk.len() > remaining {
                let prefix = chunk
                    .get(..remaining)
                    .context("failed to bound HTTP error response")?;
                body.extend_from_slice(prefix);
                truncated = true;
                break;
            }
            body.extend_from_slice(&chunk);
        }
        let mut detail = String::from_utf8_lossy(&body).trim().to_owned();
        if truncated {
            detail.push_str("… [truncated]");
        }
        let detail = redact_key(&detail, request.api_key.as_deref());
        if detail.is_empty() {
            anyhow::bail!("endpoint returned HTTP {status}");
        }
        anyhow::bail!("endpoint returned HTTP {status}: {detail}");
    }

    let mut decoder = SseDecoder::default();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("stream read failed")?;
        if forward(decoder.feed(&chunk)?, sender) {
            return Ok(());
        }
    }
    if forward(decoder.finish()?, sender) {
        return Ok(());
    }
    let _receiver_closed = sender.send(NetworkEvent::Finished).is_err();
    Ok(())
}

fn forward(items: Vec<SseItem>, sender: &UnboundedSender<NetworkEvent>) -> bool {
    for item in items {
        let event = match item {
            SseItem::Content(content) => NetworkEvent::Delta(content),
            SseItem::Reasoning(content) => NetworkEvent::Reasoning(content),
            SseItem::Done => NetworkEvent::Finished,
        };
        let done = matches!(&event, NetworkEvent::Finished);
        if sender.send(event).is_err() || done {
            return true;
        }
    }
    false
}

fn redact_key(message: &str, api_key: Option<&str>) -> String {
    match api_key.filter(|key| !key.is_empty()) {
        Some(key) => message.replace(key, "[REDACTED]"),
        None => message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_models;

    #[test]
    fn model_list_parser_sorts_deduplicates_and_ignores_blank_ids() {
        let models =
            parse_models(br#"{"data":[{"id":"zeta"},{"id":" "},{"id":"alpha"},{"id":"zeta"}]}"#)
                .expect("valid response");
        assert_eq!(models, ["alpha", "zeta"]);
    }

    #[test]
    fn model_list_parser_rejects_nonstandard_shapes() {
        assert!(parse_models(br#"{"models":["a"]}"#).is_err());
        assert!(parse_models(br#"{"data":[{"name":"a"}]}"#).is_err());
    }
}
