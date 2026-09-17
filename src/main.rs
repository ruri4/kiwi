mod app;
mod cli;
mod input;
mod network;
mod sse;
mod state;
mod terminal;
mod types;
mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run(cli::Config::load()?).await
}
