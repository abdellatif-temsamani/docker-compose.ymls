mod app;
mod status;
mod service;

use std::io;

use ratatui::{
    crossterm::{event::{self, KeyCode, KeyEventKind, poll}, terminal},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    DefaultTerminal,
};
use std::time::Duration;
use chrono::prelude::*;

use app::App;
use service::Service;
use status::Status;









fn main() -> io::Result<()> {
    let (width, height) = terminal::size()?;
    if width < 50 || height < 10 {
        eprintln!("Terminal too small. Minimum size: 50x10. Current: {}x{}", width, height);
        std::process::exit(1);
    }
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let app_result = run(terminal);
    ratatui::restore();
    app_result
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    app.next(); // select first

    loop {

        terminal.draw(|frame| {
            let chunks = if app.search_mode {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(5), Constraint::Percentage(20), Constraint::Percentage(20)])
                    .split(frame.area())
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Percentage(60), Constraint::Percentage(20), Constraint::Percentage(20)])
                    .split(frame.area())
            };

            let filtered_services: Vec<&Service> = if app.search_mode && !app.search_query.is_empty() {
                app.services.iter().filter(|s| s.name.contains(&app.search_query)).collect()
            } else {
                app.services.iter().collect()
            };

            let items: Vec<ListItem> = filtered_services
                .iter()
                .map(|service| {
                    let style = match service.status {
                        Status::Starting => Style::default().fg(Color::Yellow),
                        Status::Stopping => Style::default().fg(Color::Red),
                        Status::Running => Style::default().fg(Color::Green),
                        Status::Stopped => Style::default().fg(Color::Gray),
                        Status::Error => Style::default().fg(Color::White),
                        Status::DaemonNotRunning => Style::default().fg(Color::White),
                    };
                    ListItem::new(format!("{}: {}", service.name, service.status)).style(style)
                })
                .collect();

            let clock_start = 0;
            let list_start = if app.search_mode { 2 } else { 1 };
            let help_start = if app.search_mode { 3 } else { 2 };
            let output_start = if app.search_mode { 4 } else { 3 };

            let top_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)])
                .split(chunks[clock_start]);

            let highlight_style = if let Some(i) = app.state.selected() {
                let status = &app.services[i].status;
                if *status == Status::Starting || *status == Status::Stopping {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Black).bg(Color::Blue)
                }
            } else {
                Style::default().fg(Color::Black).bg(Color::Blue)
            };
            let list = List::new(items)
                .block(Block::default().title("Docker Services").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(Color::White))
                .highlight_style(highlight_style)
                .highlight_symbol(">> ");

            let now = Local::now();
            let clock = Paragraph::new(format!("{}", now.format("%H:%M:%S")))
                .block(Block::default().title("Time").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(Color::White));
            frame.render_widget(clock, top_chunks[0]);

            let docker_status_text = if app.docker_daemon_running { "Running" } else { "Not Running" };
            let docker_color = if app.docker_daemon_running { Color::Green } else { Color::Red };
            let docker_status = Paragraph::new(format!("Docker: {}", docker_status_text))
                .block(Block::default().title("Daemon").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(docker_color));
            frame.render_widget(docker_status, top_chunks[1]);

            let docker_cli_text = if app.docker_command_available { "Available" } else { "Not Available" };
            let docker_cli_color = if app.docker_command_available { Color::Green } else { Color::Red };
            let docker_cli = Paragraph::new(format!("CLI: {}", docker_cli_text))
                .block(Block::default().title("Docker CLI").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(docker_cli_color));
            frame.render_widget(docker_cli, top_chunks[2]);

            if app.search_mode {
                let search = Paragraph::new(format!("/{}", app.search_query))
                    .block(Block::default().title("Search").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                    .style(Style::default().fg(Color::White));
                frame.render_widget(search, chunks[1]);
            }

            frame.render_stateful_widget(list, chunks[list_start], &mut app.state);

            let help = Paragraph::new("j/k/arrows: navigate | tab: cycle | space: toggle start/stop | r: refresh | auto-refresh every 1s | q: quit")
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(Color::White))
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(help, chunks[help_start]);

            let output_text = app.last_actions.iter().cloned().collect::<Vec<_>>().join("\n");
            let output = Paragraph::new(output_text)
                .block(Block::default().title("Output").borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .style(Style::default().fg(Color::White))
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(output, chunks[output_start]);
        })?;

        if poll(Duration::from_secs(1))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('/') => {
                            app.search_mode = true;
                            app.search_query.clear();
                        },
                        KeyCode::Esc => {
                            app.search_mode = false;
                            app.search_query.clear();
                            app.state.select(Some(0)); // reset to first
                        },
                        KeyCode::Enter => {
                            if let Some(index) = app.services.iter().position(|s| s.name.to_lowercase().starts_with(&app.search_query.to_lowercase())) {
                                app.state.select(Some(index));
                            }
                            app.search_mode = false;
                            app.search_query.clear();
                        },
                        _ => {
                            if app.search_mode {
                                match key.code {
                                    KeyCode::Char(c) => app.search_query.push(c),
                                    KeyCode::Backspace => { app.search_query.pop(); },
                                    _ => {}
                                }
                            } else {
                                match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                                    KeyCode::Tab => app.next(),
                                    KeyCode::BackTab => app.previous(),
                                    KeyCode::Char(' ') => app.toggle_service(),
                                    KeyCode::Char('r') => {
                                        app.refresh_statuses();
                                        app.last_actions.push_back("Refreshed statuses".to_string());
                                        if app.last_actions.len() > 20 {
                                            app.last_actions.pop_front();
                                        }
                                    },
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Timeout: auto-refresh statuses
            app.refresh_statuses();
            app.last_actions.push_back("Auto-refreshed statuses".to_string());
            if app.last_actions.len() > 20 {
                app.last_actions.pop_front();
            }
        }
    }
}
