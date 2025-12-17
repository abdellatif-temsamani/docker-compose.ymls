use std::io;

use chrono::prelude::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation},
};

use crate::app::App;
use crate::service::Service;
use crate::status::Status;
use crate::toast::create_toast_widget;

/// Render the UI to the terminal frame
pub fn render_ui(frame: &mut ratatui::Frame, app: &mut App) -> io::Result<()> {
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
    let services_title = if app.focus == crate::app::Focus::Services {
        "Docker Services [FOCUSED]"
    } else {
        "Docker Services"
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(services_title)
                .borders(Borders::ALL)
                .border_style(if app.focus == crate::app::Focus::Services {
                    Style::default().fg(Color::Blue)
                } else {
                    Style::default().fg(Color::White)
                }),
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
                .border_style(Style::default().fg(Color::White)),
        );
    frame.render_widget(status_bar, chunks[clock_start]);

    if app.search_mode {
        let search = Paragraph::new(format!("/{}", app.search_query))
            .block(
                Block::default()
                    .title("Search")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
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
        if let Some(mut logs_guard) = crate::actions::lock_logs(&app.logs) {
            if let Some(i) = app.state.selected() {
                let service_name = &app.services[i].name;
                if let Some(buf) = logs_guard.get_mut(service_name) {
                    if app.focus == crate::app::Focus::Logs {
                        // In logs focus, get all logs and let paragraph handle scrolling
                        let all_logs = buf.get_recent_logs(usize::MAX); // Get all available logs
                        if all_logs.is_empty() {
                            ("No logs yet - start the service to see activity".to_string(), 1)
                        } else {
                            let total_lines = all_logs.len() as u16;
                            (all_logs.join("\n"), total_lines)
                        }
                    } else {
                        // Services focus - show recent logs in viewport mode
                        let viewport_log_count = app.log_viewport_height.saturating_sub(5) as usize;
                        let logs = buf.get_recent_logs(viewport_log_count);
                        if logs.is_empty() {
                            ("No logs yet - start the service to see activity".to_string(), 1)
                        } else {
                            // Add 5 empty lines at the bottom
                            let mut display_logs = logs;
                            for _ in 0..5 {
                                display_logs.push(String::new());
                            }
                            (display_logs.join("\n"), display_logs.len() as u16)
                        }
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

    // Only reset scroll position when not focused on logs
    if app.focus != crate::app::Focus::Logs {
        app.log_scroll_position = 0;
        app.log_scrollbar_state = app.log_scrollbar_state.position(0);
    }

    let title = if app.focus == crate::app::Focus::Logs {
        let max_scroll = app.log_total_lines.saturating_sub(app.log_viewport_height);
        let current_pos = app.log_scroll_position + 1;
        let max_pos = max_scroll + 1;
        format!("Container Logs [FOCUSED] (Scroll: {}/{})", current_pos, max_pos)
    } else {
        "Container Logs".to_string()
    };

    let logs_border_color = if app.focus == crate::app::Focus::Logs {
        Color::Blue
    } else {
        Color::White
    };

    let logs_widget = Paragraph::new(logs_text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(logs_border_color)),
        )
        .style(Style::default().fg(Color::White))
        .scroll((app.log_scroll_position, 0))
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(logs_widget, logs_rect);

    // Update scrollbar state with content length
    app.log_scrollbar_state = app.log_scrollbar_state.content_length(log_line_count as usize);

    // Render scrollbar if there are more lines than can be displayed
    if log_line_count > app.log_viewport_height as u16 {
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            logs_rect,
            &mut app.log_scrollbar_state,
        );
    }

    let help_text = match app.focus {
        crate::app::Focus::Logs => Line::from(vec![
            Span::styled("j/k:", Style::default().fg(Color::Cyan)),
            Span::styled("scroll logs ", Style::default().fg(Color::White)),
            Span::styled("gg/G:", Style::default().fg(Color::Magenta)),
            Span::styled("top/bottom ", Style::default().fg(Color::White)),
            Span::styled("Ctrl+d/u:", Style::default().fg(Color::Yellow)),
            Span::styled("half page ", Style::default().fg(Color::White)),
            Span::styled("h/l:", Style::default().fg(Color::Magenta)),
            Span::styled("switch focus ", Style::default().fg(Color::White)),

            Span::styled("r:", Style::default().fg(Color::Red)),
            Span::styled("refresh ", Style::default().fg(Color::White)),
            Span::styled("q:", Style::default().fg(Color::Red)),
            Span::styled("quit", Style::default().fg(Color::White)),
        ]),
        crate::app::Focus::Services => Line::from(vec![
            Span::styled("j/k/↑↓/Tab:", Style::default().fg(Color::Yellow)),
            Span::styled("nav ", Style::default().fg(Color::White)),
            Span::styled("space:", Style::default().fg(Color::Green)),
            Span::styled("toggle ", Style::default().fg(Color::White)),

            Span::styled("/:", Style::default().fg(Color::Blue)),
            Span::styled("search ", Style::default().fg(Color::White)),
            Span::styled("h/l:", Style::default().fg(Color::Magenta)),
            Span::styled("switch focus ", Style::default().fg(Color::White)),
            Span::styled("r:", Style::default().fg(Color::Red)),
            Span::styled("refresh ", Style::default().fg(Color::White)),
            Span::styled("d:", Style::default().fg(Color::Red)),
            Span::styled("stop ", Style::default().fg(Color::White)),
            Span::styled("D:", Style::default().fg(Color::Yellow)),
            Span::styled("daemon ", Style::default().fg(Color::White)),
            Span::styled("q:", Style::default().fg(Color::Red)),
            Span::styled("quit", Style::default().fg(Color::White)),
        ]),
    };
    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Controls")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
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

    Ok(())
}