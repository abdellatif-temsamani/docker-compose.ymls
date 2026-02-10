mod app;
mod config;
mod docker;
mod event_handler;
mod service;
mod status;
mod toast;
mod ui;

use std::io;
use std::time::Duration;

use ratatui::crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::CrosstermBackend;
use ratatui::{DefaultTerminal, TerminalOptions, Viewport};

use app::App;
use config::Keybinds;

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

fn install_panic_hook() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        previous_hook(panic_info);
    }));
}

#[tokio::main]
async fn main() -> io::Result<()> {
    install_panic_hook();

    // Initialize terminal without mouse capture
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _terminal_cleanup = TerminalCleanup;

    let terminal = ratatui::Terminal::with_options(
        CrosstermBackend::new(stdout),
        TerminalOptions {
            viewport: Viewport::Fullscreen,
        },
    )?;

    run(terminal).await
}

async fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    const FRAME_DURATION: Duration = Duration::from_millis(33);

    let keybinds = Keybinds::load();
    let mut app = App::new(keybinds);
    app.next(); // select first

    loop {
        let mut render_error: Option<io::Error> = None;
        terminal.draw(|frame| {
            if let Err(err) = ui::render_ui(frame, &mut app) {
                render_error = Some(err);
            }
        })?;

        if let Some(err) = render_error {
            return Err(err);
        }

        if !event_handler::handle_events(&mut app, FRAME_DURATION).await? {
            break; // Exit the application
        }
    }

    Ok(())
}
