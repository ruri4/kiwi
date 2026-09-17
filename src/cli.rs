use anyhow::{Context, Result, bail};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "llm-chat",
    version,
    about = "Chat with a Modal-hosted OpenAI-compatible streaming endpoint",
    after_help = "Authentication is read only from LLM_CHAT_API_KEY; it is never accepted as a command-line option.\nThe endpoint must be the complete chat-completions URL, typically ending in /v1/chat/completions."
)]
struct Cli {
    /// Exact OpenAI-compatible chat-completions URL (or LLM_CHAT_ENDPOINT)
    #[arg(long, env = "LLM_CHAT_ENDPOINT", value_name = "URL", required = true)]
    endpoint: String,

    /// Model identifier sent to the endpoint (or LLM_CHAT_MODEL)
    #[arg(
        long,
        env = "LLM_CHAT_MODEL",
        value_name = "MODEL",
        default_value = "llm"
    )]
    model: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        if let Err(error) = dotenvy::dotenv()
            && !error.not_found()
        {
            return Err(error).context("failed to load optional .env file");
        }

        let cli = Cli::parse();
        if cli.endpoint.trim().is_empty() {
            bail!("--endpoint/LLM_CHAT_ENDPOINT cannot be empty");
        }
        if cli.model.trim().is_empty() {
            bail!("--model/LLM_CHAT_MODEL cannot be empty");
        }

        let api_key = match std::env::var("LLM_CHAT_API_KEY") {
            Ok(value) if value.is_empty() => None,
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => return Err(error).context("LLM_CHAT_API_KEY is not valid Unicode"),
        };

        Ok(Self {
            endpoint: cli.endpoint,
            model: cli.model,
            api_key,
        })
    }
}
