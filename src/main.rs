mod actions;
mod app;
mod config;
mod event_handler;
mod service;
mod status;
mod toast;
mod ui;

use std::io;

use ratatui::{DefaultTerminal, TerminalOptions, Viewport};
use ratatui::prelude::CrosstermBackend;
use ratatui::crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use app::App;
use config::Keybinds;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Initialize terminal without mouse capture
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let terminal = ratatui::Terminal::with_options(
        CrosstermBackend::new(stdout),
        TerminalOptions {
            viewport: Viewport::Fullscreen,
        },
    )?;

    let app_result = run(terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    app_result
}


async fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    let keybinds = Keybinds::load();
    let mut app = App::new(keybinds);
    app.next(); // select first

    loop {
        terminal.draw(|frame| {
            ui::render_ui(frame, &mut app).unwrap();
        })?;

        if !event_handler::handle_events(&mut app).await? {
            break; // Exit the application
        }
    }

    Ok(())
}
