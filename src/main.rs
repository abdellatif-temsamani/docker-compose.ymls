mod actions;
mod app;
mod service;
mod status;
mod toast;

use std::io;

use chrono::prelude::*;
use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{self, KeyCode, KeyEventKind, poll},
        terminal,
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation},
};
use std::time::Duration;

use app::App;
use service::Service;
use status::Status;
use toast::create_toast_widget;

fn main() -> io::Result<()> {
    let (width, height) = terminal::size()?;
    if width < 40 || height < 12 {
        eprintln!(
            "Terminal too small. Minimum size: 40x12. Current: {}x{}",
            width, height
        );
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
            // Responsive layout based on terminal height
            let frame_height = frame.area().height;
            let controls_height = if frame_height < 15 {
                1 // Minimum 1 line for controls on very small terminals
            } else if frame_height < 20 {
                2 // 2 lines on small terminals
            } else if frame_height < 30 {
                3 // 3 lines on medium terminals
            } else {
                4 // 4 lines on large terminals
            };

            let chunks = if app.search_mode {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Min(5),
                        Constraint::Min(controls_height),
                    ])
                    .split(frame.area())
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(controls_height),
                        Constraint::Length(controls_height),
                    ])
                    .split(frame.area())
            };

            let filtered_services: Vec<&Service> =
                if app.search_mode && !app.search_query.is_empty() {
                    app.services
                        .iter()
                        .filter(|s| s.name.contains(&app.search_query))
                        .collect()
                } else {
                    app.services.iter().collect()
                };

            let items: Vec<ListItem> = filtered_services
                .iter()
                .map(|service| {
                    let style = match service.status {
                        Status::Starting => Style::default().fg(Color::Yellow),
                        Status::Stopping => Style::default().fg(Color::Red),
                        Status::Pulling => Style::default().fg(Color::Cyan),
                        Status::Running => Style::default().fg(Color::Green),
                        Status::Stopped => Style::default().fg(Color::Gray),
                        Status::Error => Style::default().fg(Color::White),
                        Status::DaemonNotRunning => Style::default().fg(Color::White),
                    };
                    let display_text = if service.status == Status::Pulling {
                        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                        let spinner_idx = (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() / 100) % spinner_chars.len() as u128;
                        format!("{}: {} {}", service.name, service.status, spinner_chars[spinner_idx as usize])
                    } else {
                        format!("{}: {}", service.name, service.status)
                    };
                    ListItem::new(display_text).style(style)
                })
                .collect();

            let clock_start = 0;
            let list_start = if app.search_mode {
                2
            } else {
                1
            };
            let help_start = if app.search_mode {
                3
            } else {
                2
            };

            let highlight_style = if let Some(i) = app.state.selected() {
                let status = &app.services[i].status;
                if *status == Status::Starting || *status == Status::Stopping || *status == Status::Pulling {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Black).bg(Color::Blue)
                }
            } else {
                Style::default().fg(Color::Black).bg(Color::Blue)
            };
            let list = List::new(items)
                .block(
                    Block::default()
                        .title("Docker Services")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue)),
                )
                .style(Style::default().fg(Color::White))
                .highlight_style(highlight_style)
                .highlight_symbol(">> ");

            // Compact status bar combining time and docker status
            let now = Local::now();
            let docker_status_text = if app.docker_daemon_running {
                "● Running"
            } else {
                "● Stopped"
            };
            let docker_color = if app.docker_daemon_running {
                Color::Green
            } else {
                Color::Red
            };
            let docker_cli_text = if app.docker_command_available {
                "● Docker CLI OK"
            } else {
                "● Docker CLI N/A"
            };
            let docker_cli_color = if app.docker_command_available {
                Color::Green
            } else {
                Color::Red
            };

            let docker_compose_text = if app.docker_compose_available {
                "● Compose OK"
            } else {
                "● Compose N/A"
            };
            let docker_compose_color = if app.docker_compose_available {
                Color::Green
            } else {
                Color::Red
            };

            let status_line = Line::from(vec![
                Span::styled(format!("{} ", now.format("%H:%M:%S")), Style::default().fg(Color::White)),
                Span::styled("| ", Style::default().fg(Color::Gray)),
                Span::styled(docker_status_text, Style::default().fg(docker_color)),
                Span::styled(" | ", Style::default().fg(Color::Gray)),
                Span::styled(docker_cli_text, Style::default().fg(docker_cli_color)),
                Span::styled(" | ", Style::default().fg(Color::Gray)),
                Span::styled(docker_compose_text, Style::default().fg(docker_compose_color)),
            ]);

            let status_bar = Paragraph::new(status_line)
                .block(
                    Block::default()
                        .title("Status")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue)),
                );
            frame.render_widget(status_bar, chunks[clock_start]);

            if app.search_mode {
                let search = Paragraph::new(format!("/{}", app.search_query))
                    .block(
                        Block::default()
                            .title("Search")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Blue)),
                    )
                    .style(Style::default().fg(Color::White));
                frame.render_widget(search, chunks[1]);
            }

            // Responsive horizontal split based on terminal width
            let frame_width = frame.area().width;
            let services_percentage = if frame_width < 80 {
                25 // Narrow terminals: smaller services panel
            } else if frame_width < 120 {
                30 // Medium terminals: standard split
            } else {
                35 // Wide terminals: larger services panel for better readability
            };

            let [list_rect, logs_rect] =
                Layout::horizontal([Constraint::Percentage(services_percentage), Constraint::Percentage(100 - services_percentage)])
                    .areas(chunks[list_start]);
            frame.render_stateful_widget(list, list_rect, &mut app.state);

            let (logs_text, log_line_count) = {
                // Use safe locking for logs display
                if let Some(logs_guard) = crate::actions::lock_logs(&app.logs) {
                    if let Some(i) = app.state.selected() {
                        let service_name = &app.services[i].name;
                        if let Some(buf) = logs_guard.get(service_name) {
                            let logs = buf.get_recent_logs(200); // Get more logs for scrolling
                            if logs.is_empty() {
                                ("No logs yet - start the service to see activity".to_string(), 1)
                            } else {
                                (logs.join("\n"), logs.len() as u16)
                            }
                        } else {
                            ("No logs yet - start the service to see activity".to_string(), 1)
                        }
                    } else {
                        ("Select a service to view logs".to_string(), 1)
                    }
                } else {
                    ("Unable to access logs".to_string(), 1)
                }
            };

            // Calculate dimensions for scrolling
            let logs_height = logs_rect.height.saturating_sub(2); // Subtract borders
            app.log_viewport_height = logs_height; // Store for key handling
            app.log_total_lines = log_line_count; // Store for key handling

            // Update scrollbar content length (matching Ratatui example pattern)
            app.log_scrollbar_state = app.log_scrollbar_state.content_length(log_line_count as usize);

            let logs_widget = Paragraph::new(logs_text)
                .block(
                    Block::default()
                        .title("Container Logs")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green)),
                )
                .style(Style::default().fg(Color::White))
                .wrap(ratatui::widgets::Wrap { trim: true })
                .scroll((app.log_scroll_position, 0));
            frame.render_widget(logs_widget, logs_rect);

            // Render scrollbar (matching Ratatui example pattern)
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓")),
                logs_rect,
                &mut app.log_scrollbar_state,
            );

            let help_text = Line::from(vec![
                Span::styled("j/k/↑↓/Tab:", Style::default().fg(Color::Yellow)),
                Span::styled("nav ", Style::default().fg(Color::White)),
                Span::styled("space:", Style::default().fg(Color::Green)),
                Span::styled("toggle ", Style::default().fg(Color::White)),
                Span::styled("J/K:", Style::default().fg(Color::Cyan)),
                Span::styled("scroll ", Style::default().fg(Color::White)),
                Span::styled("PgUp/Dn:", Style::default().fg(Color::Magenta)),
                Span::styled("page ", Style::default().fg(Color::White)),
                Span::styled("Home/End:", Style::default().fg(Color::Magenta)),
                Span::styled("top/bottom ", Style::default().fg(Color::White)),
                Span::styled("/:", Style::default().fg(Color::Blue)),
                Span::styled("search ", Style::default().fg(Color::White)),
                Span::styled("r:", Style::default().fg(Color::Red)),
                Span::styled("refresh ", Style::default().fg(Color::White)),
                Span::styled("d:", Style::default().fg(Color::Yellow)),
                Span::styled("daemon ", Style::default().fg(Color::White)),
                Span::styled("q:", Style::default().fg(Color::Red)),
                Span::styled("quit", Style::default().fg(Color::White)),
            ]);
            let help = Paragraph::new(help_text)
                .block(
                    Block::default()
                        .title("Controls")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Gray)),
                )
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(help, chunks[help_start]);

            if app.daemon_start_mode {
                let password_display = "*".repeat(app.password_input.len());
                let password_input = Paragraph::new(format!("Password: {}", password_display))
                    .block(
                        Block::default()
                            .title("Start Docker Daemon")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Yellow))
                            .style(Style::default().bg(Color::Black)),
                    )
                    .style(Style::default().fg(Color::White).bg(Color::Black));

                // Center the password input as an overlay
                let input_width = 50;
                let input_height = 5;
                let input_area = Rect {
                    x: (frame.area().width.saturating_sub(input_width)) / 2,
                    y: (frame.area().height.saturating_sub(input_height)) / 2,
                    width: input_width.min(frame.area().width),
                    height: input_height.min(frame.area().height),
                };
                frame.render_widget(password_input, input_area);
            }

            if let Some(toast) = &app.toast {
                let toast_width = 50;
                let toast_height = 3;
                let toast_area = Rect {
                    x: frame.area().width.saturating_sub(toast_width),
                    y: 0,
                    width: toast_width.min(frame.area().width),
                    height: toast_height,
                };
                let toast_widget = create_toast_widget(toast);
                frame.render_widget(toast_widget, toast_area);
            }

        })?;

        if poll(Duration::from_secs(1))? {
            if let event::Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') if !app.search_mode && !app.daemon_start_mode => {
                            return Ok(());
                        }
                        KeyCode::Char('/') if !app.search_mode && !app.daemon_start_mode => {
                            app.search_mode = true;
                            app.search_query.clear();
                        }
                        KeyCode::Char('d') if !app.search_mode && !app.daemon_start_mode => {
                            app.daemon_start_mode = true;
                            app.password_input.clear();
                        }
                        KeyCode::Esc => {
                            if app.search_mode || app.daemon_start_mode {
                                app.search_mode = false;
                                app.daemon_start_mode = false;
                                app.search_query.clear();
                                app.password_input.clear();
                                app.state.select(Some(0));
                            }
                        }
                        KeyCode::Enter => {
                            if app.search_mode {
                                if let Some(index) = app.services.iter().position(|s| {
                                    s.name
                                        .to_lowercase()
                                        .starts_with(&app.search_query.to_lowercase())
                                }) {
                                    app.state.select(Some(index));
                                }
                                app.search_mode = false;
                                app.search_query.clear();
                            } else if app.daemon_start_mode {
                                app.start_daemon();
                            }
                        }
                        _ if app.search_mode => match key.code {
                            KeyCode::Char(c) => app.search_query.push(c),
                            KeyCode::Backspace => {
                                app.search_query.pop();
                            }
                            _ => {}
                        },
                        _ if app.daemon_start_mode => match key.code {
                            KeyCode::Char(c) => app.password_input.push(c),
                            KeyCode::Backspace => {
                                app.password_input.pop();
                            }
                            _ => {}
                        },
                        _ => match key.code {
                             // Log scrolling shortcuts (must come before navigation)
                              KeyCode::Char('K') => { // Shift+K (uppercase)
                                  app.log_scroll_position = app.log_scroll_position.saturating_sub(1);
                                  app.log_scrollbar_state = app.log_scrollbar_state.position(app.log_scroll_position as usize);
                              }
                              KeyCode::Char('J') => { // Shift+J (uppercase)
                                  app.log_scroll_position = app.log_scroll_position.saturating_add(1);
                                  app.log_scrollbar_state = app.log_scrollbar_state.position(app.log_scroll_position as usize);
                              }
                              // Page up/down for larger scrolls
                              KeyCode::PageUp => {
                                  app.log_scroll_position = app.log_scroll_position.saturating_sub(10);
                                  app.log_scrollbar_state = app.log_scrollbar_state.position(app.log_scroll_position as usize);
                              }
                              KeyCode::PageDown => {
                                  app.log_scroll_position = app.log_scroll_position.saturating_add(10);
                                  app.log_scrollbar_state = app.log_scrollbar_state.position(app.log_scroll_position as usize);
                              }
                              // Home/End for quick navigation
                              KeyCode::Home => {
                                  app.log_scroll_position = 0;
                                  app.log_scrollbar_state = app.log_scrollbar_state.position(0);
                              }
                              KeyCode::End => {
                                  // Scroll to bottom - use saturating_sub like in example
                                  app.log_scroll_position = app.log_total_lines.saturating_sub(app.log_viewport_height);
                                  app.log_scrollbar_state = app.log_scrollbar_state.position(app.log_scroll_position as usize);
                              }
                            // Navigation (must come after scrolling shortcuts)
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next();
                                app.refresh_logs();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous();
                                app.refresh_logs();
                            }
                            KeyCode::Tab => {
                                app.next();
                                app.refresh_logs();
                            }
                            KeyCode::BackTab => {
                                app.previous();
                                app.refresh_logs();
                            }
                            KeyCode::Char(' ') => app.toggle_service(),
                            KeyCode::Char('r') => {
                                app.refresh_statuses();
                                app.refresh_logs();
                                app.toast = Some(crate::toast::Toast {
                                    state: crate::status::ToastState::Info,
                                    message: "Refreshed statuses".to_string(),
                                });
                                app.toast_timer = 3;
                            }
                            _ => {}
                        },
                    }
                }
        } else {
            // Timeout: auto-refresh statuses
            app.refresh_statuses();
            app.refresh_logs();
        }

        if app.toast_timer > 0 {
            app.toast_timer = app.toast_timer.saturating_sub(1);
            if app.toast_timer == 0 {
                app.toast = None;
            }
        }
    }
}
