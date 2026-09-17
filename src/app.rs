use crate::config::Config;
use crate::network::{self, NetworkEvent};
use crate::state::{Action, AppState};
use crate::terminal::TerminalSession;
use crate::ui;
use anyhow::{Context, Result, anyhow};
use crossterm::event::{Event, EventStream};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub(crate) async fn run(config: Config) -> Result<()> {
    let client = network::client()?;
    let state = AppState::load(config)?;
    let mut terminal = TerminalSession::start()?;
    let result = run_loop(terminal.terminal_mut(), state, client).await;
    let cleanup = terminal.restore();
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(anyhow!(
            "{error:#}; terminal cleanup also failed: {cleanup_error:#}"
        )),
    }
}

async fn run_loop(
    terminal: &mut crate::terminal::Tui,
    mut state: AppState,
    client: reqwest::Client,
) -> Result<()> {
    let mut terminal_events = EventStream::new();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut task: Option<JoinHandle<()>> = None;
    if let Some(request) = state.begin_startup_discovery() {
        let client = client.clone();
        let sender = sender.clone();
        task = Some(tokio::spawn(async move {
            network::discover_models(client, request, sender).await;
        }));
    }

    loop {
        terminal
            .draw(|frame| ui::draw(frame, &mut state))
            .context("failed to draw terminal UI")?;

        tokio::select! {
            event = terminal_events.next() => {
                let event = event.ok_or_else(|| anyhow!("terminal event stream ended"))??;
                match event {
                    Event::Key(key) => match state.handle_key(key) {
                        Action::Continue => {}
                        Action::Quit => break,
                        Action::Send(request) => {
                            let client = client.clone();
                            let sender = sender.clone();
                            task = Some(tokio::spawn(async move {
                                network::stream_chat(client, request, sender).await;
                            }));
                        }
                        Action::Discover(request) => {
                            let client = client.clone();
                            let sender = sender.clone();
                            task = Some(tokio::spawn(async move {
                                network::discover_models(client, request, sender).await;
                            }));
                        }
                    },
                    Event::Paste(value) => state.handle_paste(&value),
                    Event::Mouse(mouse) => state.handle_mouse(mouse),
                    Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => {}
                }
            }
            event = receiver.recv(), if state.network_active() => {
                let event = event.ok_or_else(|| anyhow!("network event channel closed"))?;
                let finished = matches!(
                    &event,
                    NetworkEvent::Finished
                        | NetworkEvent::Failed(_)
                        | NetworkEvent::Models(_)
                        | NetworkEvent::ModelsFailed(_)
                );
                state.handle_network(event);
                if finished {
                    reap_task(&mut task).await?;
                }
            }
        }
    }

    stop_task(&mut task).await
}

async fn reap_task(task: &mut Option<JoinHandle<()>>) -> Result<()> {
    if let Some(handle) = task.take() {
        handle.await.context("network task failed")?;
    }
    Ok(())
}

async fn stop_task(task: &mut Option<JoinHandle<()>>) -> Result<()> {
    if let Some(handle) = task.take() {
        handle.abort();
        match handle.await {
            Ok(()) => {}
            Err(error) if error.is_cancelled() => {}
            Err(error) => return Err(error).context("network task failed during shutdown"),
        }
    }
    Ok(())
}
