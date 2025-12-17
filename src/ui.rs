use std::io;

use chrono::prelude::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::App;
use crate::service::Service;
use crate::status::Status;
use crate::toast::create_toast_widget;

fn colorize_logs(logs: String) -> Text<'static> {
    let mut lines = Vec::new();

    for line in logs.lines() {
        let line_str = line.to_string();
        if line_str.starts_with("Pull output:") {
            lines.push(Line::from(vec![
                Span::styled("Pull output:", Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)),
            ]));
        } else if line_str.starts_with("Up output:") {
            lines.push(Line::from(vec![
                Span::styled("Up output:", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
            ]));
        } else if line_str.starts_with("Down output:") {
            lines.push(Line::from(vec![
                Span::styled("Down output:", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
            ]));
        } else if line_str.contains("failed") || line_str.contains("Failed") || line_str.contains("error") || line_str.contains("Error") {
            lines.push(Line::from(vec![
                Span::styled(line_str, Style::default().fg(Color::Red)),
            ]));
        } else if line_str.contains("success") || line_str.contains("Success") || line_str.contains("done") || line_str.contains("Done") {
            lines.push(Line::from(vec![
                Span::styled(line_str, Style::default().fg(Color::Green)),
            ]));
        } else if line_str.trim().is_empty() {
            lines.push(Line::from(""));
        } else {
            // Default color for other log lines
            lines.push(Line::from(vec![
                Span::styled(line_str, Style::default().fg(Color::White)),
            ]));
        }
    }

    Text::from(lines)
}

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
            let status = service.status.lock().unwrap().clone();
            let style = match status {
                Status::Starting => Style::default().fg(Color::Yellow),
                Status::Stopping => Style::default().fg(Color::Red),
                Status::Pulling => Style::default().fg(Color::Cyan),
                Status::Running => Style::default().fg(Color::Green),
                Status::Stopped => Style::default().fg(Color::Gray),
                Status::Error => Style::default().fg(Color::White),
                Status::DaemonNotRunning => Style::default().fg(Color::White),
            };
            let display_text = if status == Status::Pulling {
                let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let spinner_idx = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() / 100) % spinner_chars.len() as u128;
                format!("{}: {} {}", service.name, status, spinner_chars[spinner_idx as usize])
            } else {
                format!("{}: {}", service.name, status)
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
        let status = app.services[i].status.lock().unwrap().clone();
        if status == Status::Starting || status == Status::Stopping || status == Status::Pulling {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::Black).bg(Color::Blue)
        }
    } else {
        Style::default().fg(Color::Black).bg(Color::Blue)
    };
    let services_title = if app.focus == crate::app::Focus::Services {
        Line::from(vec![
            Span::styled("Docker Services ", Style::default().fg(Color::White)),
            Span::styled("[FOCUSED]", Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
        ])
    } else {
        Line::from("Docker Services")
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

    let logs_content = if let Some(i) = app.state.selected() {
        let service = &app.services[i];
        let logs_string = service.logs.lock().unwrap().clone();
        if logs_string.is_empty() {
            Text::from("No startup logs yet - start the service to see docker-compose output")
        } else {
            colorize_logs(logs_string)
        }
    } else {
        Text::from("Select a service to view logs")
    };

    let logs_title = if app.focus == crate::app::Focus::Logs {
        Line::from(vec![
            Span::styled("Container Logs ", Style::default().fg(Color::White)),
            Span::styled("[FOCUSED]", Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
        ])
    } else {
        Line::from("Container Logs")
    };

    let logs_border_color = if app.focus == crate::app::Focus::Logs {
        Color::Blue
    } else {
        Color::White
    };

    let logs_widget = Paragraph::new(logs_content)
        .block(
            Block::default()
                .title(logs_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(logs_border_color)),
        )
        .style(Style::default().fg(Color::White))
        .wrap(ratatui::widgets::Wrap { trim: true })
        .scroll((app.log_scroll, 0));
    frame.render_widget(logs_widget, logs_rect);



    let nav_label = if app.focus == crate::app::Focus::Services { "nav " } else { "scroll " };
    let mut help_spans = vec![
        Span::styled("j/k/↑↓:", Style::default().fg(Color::Yellow)),
        Span::styled(nav_label, Style::default().fg(Color::White)),
    ];

    if app.focus == crate::app::Focus::Services {
        help_spans.extend(vec![
            Span::styled("Tab:", Style::default().fg(Color::Yellow)),
            Span::styled("nav ", Style::default().fg(Color::White)),
            Span::styled(if app.keybinds.services.toggle == " " { "space:".to_string() } else { format!("{}:", app.keybinds.services.toggle) }, Style::default().fg(Color::Green)),
            Span::styled("toggle ", Style::default().fg(Color::White)),
            Span::styled(format!("{}:", app.keybinds.services.start), Style::default().fg(Color::Green)),
            Span::styled("start ", Style::default().fg(Color::White)),
            Span::styled(format!("{}:", app.keybinds.services.stop), Style::default().fg(Color::Red)),
            Span::styled("stop ", Style::default().fg(Color::White)),
            Span::styled(format!("{}:", app.keybinds.app.refresh), Style::default().fg(Color::Red)),
            Span::styled("refresh ", Style::default().fg(Color::White)),
            Span::styled(format!("{}:", app.keybinds.app.focus_logs), Style::default().fg(Color::Magenta)),
            Span::styled("focus logs ", Style::default().fg(Color::White)),
        ]);
    } else {
        // Focus on Logs
        help_spans.extend(vec![
            Span::styled(format!("{}:", app.keybinds.app.focus_services), Style::default().fg(Color::Magenta)),
            Span::styled("focus services ", Style::default().fg(Color::White)),
        ]);
    }

    help_spans.extend(vec![
        Span::styled(format!("{}:", app.keybinds.app.search), Style::default().fg(Color::Blue)),
        Span::styled("search ", Style::default().fg(Color::White)),
        Span::styled(format!("{}:", app.keybinds.app.daemon_menu), Style::default().fg(Color::Yellow)),
        Span::styled("daemon ", Style::default().fg(Color::White)),
        Span::styled(format!("{}:", app.keybinds.app.quit), Style::default().fg(Color::Red)),
        Span::styled("quit", Style::default().fg(Color::White)),
    ]);

    let help_text = Line::from(help_spans);
    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Controls")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(help, chunks[help_start]);

    if app.daemon_menu_mode {
        // Clear and overlay the entire screen
        frame.render_widget(Clear, frame.area());
        let bg_overlay = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(bg_overlay, frame.area());

        let menu_items = ["Start Docker Daemon",
            "Stop Docker Daemon",
            "Restart Docker Daemon"];

        let menu_height = menu_items.len() as u16 + 4; // +4 for borders and title
        let menu_width = 30;

        let mut list_items = Vec::new();
        for (i, item) in menu_items.iter().enumerate() {
            let style = if i == app.daemon_action_selected as usize {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(Color::White).add_modifier(ratatui::style::Modifier::BOLD)
            };
            list_items.push(ratatui::widgets::ListItem::new(*item).style(style));
        }

        let daemon_menu = ratatui::widgets::List::new(list_items)
            .block(
                Block::default()
                    .title("Docker Daemon Control")
                    .title_style(Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD))
                    .style(Style::default().bg(Color::Black)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Black));

        let menu_area = Rect {
            x: (frame.area().width.saturating_sub(menu_width)) / 2,
            y: (frame.area().height.saturating_sub(menu_height)) / 2,
            width: menu_width.min(frame.area().width),
            height: menu_height.min(frame.area().height),
        };
        frame.render_widget(daemon_menu, menu_area);
    } else if app.daemon_start_mode {
        // Clear and overlay the entire screen
        frame.render_widget(Clear, frame.area());
        let bg_overlay = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(bg_overlay, frame.area());

        let action_text = match app.daemon_action_selected {
            crate::app::DaemonAction::Start => "Start Docker Daemon",
            crate::app::DaemonAction::Stop => "Stop Docker Daemon",
            crate::app::DaemonAction::Restart => "Restart Docker Daemon",
        };

        let password_display = "*".repeat(app.password_input.len());
        let password_input = Paragraph::new(format!("Password: {}", password_display))
            .block(
                Block::default()
                    .title(action_text)
                    .title_style(Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD))
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