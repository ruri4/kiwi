use anyhow::{Context, Result, anyhow};
use crossterm::{event, execute, terminal};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{Stdout, stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalSession {
    terminal: Tui,
    restored: bool,
}

impl TerminalSession {
    pub fn start() -> Result<Self> {
        terminal::enable_raw_mode().context("failed to enable terminal raw mode")?;
        if let Err(error) = execute!(
            stdout(),
            terminal::EnterAlternateScreen,
            event::EnableMouseCapture
        ) {
            let cleanup = terminal::disable_raw_mode();
            return match cleanup {
                Ok(()) => Err(error).context("failed to enter alternate screen"),
                Err(cleanup_error) => Err(anyhow!(
                    "failed to enter alternate screen: {error}; also failed to disable raw mode: {cleanup_error}"
                )),
            };
        }

        let backend = CrosstermBackend::new(stdout());
        let terminal = match Terminal::new(backend) {
            Ok(value) => value,
            Err(error) => {
                let cleanup = emergency_restore();
                return match cleanup {
                    Ok(()) => Err(error).context("failed to initialize terminal"),
                    Err(cleanup_error) => Err(anyhow!(
                        "failed to initialize terminal: {error}; cleanup also failed: {cleanup_error}"
                    )),
                };
            }
        };
        install_panic_restore();
        Ok(Self {
            terminal,
            restored: false,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut Tui {
        &mut self.terminal
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        let mut errors = Vec::new();
        if let Err(error) = self.terminal.show_cursor() {
            errors.push(format!("show cursor: {error}"));
        }
        if let Err(error) = execute!(
            self.terminal.backend_mut(),
            event::DisableMouseCapture,
            terminal::LeaveAlternateScreen,
        ) {
            errors.push(format!("leave alternate screen: {error}"));
        }
        if let Err(error) = terminal::disable_raw_mode() {
            errors.push(format!("disable raw mode: {error}"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("terminal cleanup failed: {}", errors.join("; ")))
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.restored || std::thread::panicking() {
            return;
        }
        if let Err(error) = self.restore() {
            eprintln!("{error:#}");
        }
    }
}

fn install_panic_restore() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Err(error) = emergency_restore() {
            eprintln!("terminal cleanup during panic failed: {error:#}");
        }
        previous(info);
    }));
}

fn emergency_restore() -> Result<()> {
    let alternate = execute!(
        stdout(),
        event::DisableMouseCapture,
        terminal::LeaveAlternateScreen,
    );
    let raw = terminal::disable_raw_mode();
    match (alternate, raw) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(first), Ok(())) => Err(first).context("leave alternate screen"),
        (Ok(()), Err(second)) => Err(second).context("disable raw mode"),
        (Err(first), Err(second)) => Err(anyhow!(
            "leave alternate screen: {first}; disable raw mode: {second}"
        )),
    }
}
