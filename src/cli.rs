use crate::config::{self, Config};
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "kiwi",
    version,
    about = "Chat with OpenAI-compatible streaming endpoints",
    after_help = "Endpoint, model, and reasoning settings come from YAML. API keys are read from the process environment or referenced .env entries."
)]
struct Cli {
    /// YAML endpoint profile configuration (or KIWI_CONFIG)
    #[arg(long, env = "KIWI_CONFIG", value_name = "PATH")]
    config: Option<PathBuf>,
}

pub(crate) fn load() -> Result<Config> {
    let cli = Cli::parse();
    config::load(cli.config).context("failed to load configuration")
}
