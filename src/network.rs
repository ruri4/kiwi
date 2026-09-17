use crate::sse::{SseDecoder, SseItem};
use crate::types::Message;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::header::ACCEPT;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

const ERROR_BODY_LIMIT: usize = 8 * 1024;

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub messages: Vec<Message>,
}

#[derive(Debug)]
pub enum NetworkEvent {
    Delta(String),
    Finished,
    Failed(String),
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
    let payload = json!({
        "model": request.model,
        "messages": messages,
        "stream": true
    });

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
