mod agents;
mod app;
mod cli;
mod config;
mod input;
mod network;
mod picker;
mod session;
mod sse;
mod state;
mod storage;
mod terminal;
mod types;
mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run(cli::load()?).await
}
