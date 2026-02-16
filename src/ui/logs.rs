use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus, LogTab};
use crate::status::Status;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let logs_content = selected_logs(app);
    let title = logs_title(app);
    let border_color = if app.focus == Focus::Logs {
        Color::Blue
    } else {
        Color::White
    };

    if app.log_auto_scroll {
        let total_lines = logs_content.lines.len() as u16;
        let visible_lines = area.height.saturating_sub(2);
        app.log_scroll = total_lines.saturating_sub(visible_lines);
    }

    let logs_widget = Paragraph::new(logs_content)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(Color::Gray))
        .scroll((app.log_scroll, 0));

    frame.render_widget(logs_widget, area);
}

fn selected_logs(app: &App) -> Text<'static> {
    if let Some(index) = app.state.selected() {
        let service = &app.services[index];
        match app.log_tab {
            LogTab::Events => {
                let logs = service.events.lock().unwrap().clone();
                let status = service.status.lock().unwrap().clone();
                let pull_progress = service.pull_progress.lock().unwrap().clone();

                let mut content = if logs.is_empty() {
                    Text::from(vec![Line::from(vec![Span::styled(
                        "No events yet - start the service to see events",
                        Style::default().fg(Color::DarkGray),
                    )])])
                } else {
                    colorize_events(logs)
                };

                if let Some(progress_line) =
                    event_progress_line(&status, pull_progress.as_deref(), app.animation_tick)
                {
                    let mut lines = vec![progress_line, Line::from("")];
                    lines.extend(content.lines);
                    content = Text::from(lines);
                }

                content
            }
            LogTab::LiveLogs => {
                let logs = service.live_logs.lock().unwrap().clone();
                if logs.is_empty() {
                    Text::from(vec![Line::from(vec![Span::styled(
                        "No live logs yet - start the service to see logs",
                        Style::default().fg(Color::DarkGray),
                    )])])
                } else {
                    colorize_logs(logs)
                }
            }
        }
    } else {
        Text::from("Select a service to view logs")
    }
}

fn logs_title(app: &App) -> Line<'static> {
    let selected_name = app
        .state
        .selected()
        .and_then(|index| app.services.get(index))
        .map(|service| service.name.clone())
        .unwrap_or_else(|| "none".to_string());

    let mut spans = vec![
        Span::styled(" Logs ", Style::default().fg(Color::White)),
        Span::styled(
            format!("{}", selected_name),
            Style::default().fg(Color::Cyan),
        ),
    ];

    spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
    if app.log_tab == LogTab::Events {
        spans.push(Span::styled(
            "[Events]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled("Events", Style::default().fg(Color::White)));
    }

    spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
    if app.log_tab == LogTab::LiveLogs {
        spans.push(Span::styled(
            "[Live Logs]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled("Live Logs", Style::default().fg(Color::White)));
    }

    if app.focus == Focus::Logs && app.log_auto_scroll {
        spans.push(Span::styled(
            " [AUTO]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn event_progress_line(
    status: &Status,
    pull_progress: Option<&str>,
    tick: u64,
) -> Option<Line<'static>> {
    match status {
        Status::Pulling => {
            let progress = pull_progress.unwrap_or("in progress");
            let bar = progress_bar(tick, parse_progress_percent(progress));
            Some(Line::from(vec![
                Span::styled("[progress] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", status),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled(bar, Style::default().fg(Color::Cyan)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(progress.to_string(), Style::default().fg(Color::White)),
                Span::styled(" ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    pulling_spinner(tick).to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]))
        }
        Status::Starting => {
            let bar = progress_bar(tick, None);
            Some(Line::from(vec![
                Span::styled("[progress] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "starting".to_string(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled(bar, Style::default().fg(Color::Yellow)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    transition_spinner(tick).to_string(),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        }
        Status::Stopping => {
            let bar = progress_bar(tick, None);
            Some(Line::from(vec![
                Span::styled("[progress] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "stopping".to_string(),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled(bar, Style::default().fg(Color::Red)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    transition_spinner(tick).to_string(),
                    Style::default().fg(Color::Red),
                ),
            ]))
        }
        _ => None,
    }
}

fn pulling_spinner(tick: u64) -> &'static str {
    const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    SPINNER[((tick / 2) % SPINNER.len() as u64) as usize]
}

fn transition_spinner(tick: u64) -> &'static str {
    const SPINNER: [&str; 8] = ["-", "\\", "|", "/", "-", "\\", "|", "/"];
    SPINNER[(tick % SPINNER.len() as u64) as usize]
}

fn parse_progress_percent(progress: &str) -> Option<u8> {
    let percent_pos = progress.find('%')?;
    let prefix = &progress[..percent_pos];
    let digits_reversed: String = prefix
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();

    if digits_reversed.is_empty() {
        return None;
    }

    let digits: String = digits_reversed.chars().rev().collect();
    digits.parse::<u8>().ok().map(|percent| percent.min(100))
}

fn progress_bar(tick: u64, percent: Option<u8>) -> String {
    const WIDTH: usize = 24;
    const MARKER_WIDTH: usize = 5;

    if let Some(percent) = percent {
        let filled = (percent as usize * WIDTH) / 100;
        return format!("{}{}", "#".repeat(filled), "-".repeat(WIDTH - filled));
    }

    let mut bar = vec!['-'; WIDTH];
    let offset = (tick as usize) % (WIDTH + MARKER_WIDTH);
    for i in 0..MARKER_WIDTH {
        let idx = offset + i;
        if idx < WIDTH {
            bar[idx] = '#';
        }
    }

    bar.into_iter().collect()
}

fn colorize_logs(logs: String) -> Text<'static> {
    let mut lines = Vec::new();

    for raw_line in logs.lines() {
        let line_str = raw_line.to_string();

        if line_str.starts_with("Pull output:") {
            lines.push(Line::from(vec![Span::styled(
                "Pull output:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if line_str.starts_with("Up output:") {
            lines.push(Line::from(vec![Span::styled(
                "Up output:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if line_str.starts_with("Down output:") {
            lines.push(Line::from(vec![Span::styled(
                "Down output:",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
        } else if line_str.contains("failed")
            || line_str.contains("Failed")
            || line_str.contains("error")
            || line_str.contains("Error")
        {
            lines.push(Line::from(vec![Span::styled(
                line_str,
                Style::default().fg(Color::Red),
            )]));
        } else if line_str.contains("success")
            || line_str.contains("Success")
            || line_str.contains("done")
            || line_str.contains("Done")
        {
            lines.push(Line::from(vec![Span::styled(
                line_str,
                Style::default().fg(Color::Green),
            )]));
        } else if line_str.trim().is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(colorize_runtime_log_line(&line_str));
        }
    }

    Text::from(lines)
}

fn colorize_runtime_log_line(line: &str) -> Line<'static> {
    if let Some((service, body)) = split_service_prefix(line) {
        if let Some((head, marker, tail)) = split_log_marker(body) {
            let marker_level = marker.trim_matches(':').trim().to_ascii_uppercase();
            let mut tail_color = classify_log_body_color(tail);
            if matches!(marker_level.as_str(), "LOG" | "INFO" | "NOTICE" | "*")
                && tail_color == Color::Green
            {
                tail_color = Color::Gray;
            }

            return Line::from(vec![
                Span::styled(
                    format!("{} | ", service),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(head.to_string(), Style::default().fg(Color::DarkGray)),
                Span::styled(" ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    marker.to_string(),
                    Style::default().fg(log_marker_color(marker)),
                ),
                Span::styled(" ", Style::default().fg(Color::DarkGray)),
                Span::styled(tail.to_string(), Style::default().fg(tail_color)),
            ]);
        }

        let body_color = classify_log_body_color(body);

        return Line::from(vec![
            Span::styled(
                format!("{} | ", service),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(body.to_string(), Style::default().fg(body_color)),
        ]);
    }

    Line::from(vec![Span::styled(
        line.to_string(),
        Style::default().fg(classify_log_body_color(line)),
    )])
}

fn split_service_prefix(line: &str) -> Option<(&str, &str)> {
    let (service, body) = line.split_once('|')?;
    if service.trim().is_empty() || body.trim().is_empty() {
        return None;
    }
    Some((service.trim(), body.trim_start()))
}

fn classify_log_body_color(body: &str) -> Color {
    let lower = body.to_ascii_lowercase();

    if lower.contains(" panic")
        || lower.contains(" panicked")
        || lower.contains(" fatal")
        || lower.contains("error")
        || lower.contains("exception")
        || lower.contains("crit")
    {
        return Color::Red;
    }

    if lower.contains("warn")
        || lower.contains("timeout")
        || lower.contains("retry")
        || lower.contains("deprecated")
    {
        return Color::Yellow;
    }

    if lower.contains("debug") || lower.contains("trace") {
        return Color::LightBlue;
    }

    if lower.contains("started")
        || lower.contains("ready")
        || lower.contains("listening")
        || lower.contains("connected")
        || lower.contains("accept connections")
        || lower.contains("created")
        || lower.contains("loaded")
        || lower.contains("ok")
        || lower.contains("success")
    {
        return Color::Green;
    }

    Color::Gray
}

fn split_log_marker(body: &str) -> Option<(&str, &str, &str)> {
    for marker in [
        " TRACE ",
        " DEBUG ",
        " INFO ",
        " NOTICE ",
        " WARN ",
        " WARNING ",
        " ERROR ",
        " ERR ",
        " CRITICAL ",
        " FATAL ",
        " LOG:",
        " WARNING:",
        " ERROR:",
        " FATAL:",
        " * ",
        " # ",
        " - ",
    ] {
        if let Some(index) = body.find(marker) {
            let head = body[..index].trim_end();
            let tail = body[index + marker.len()..].trim_start();
            let symbol = marker.trim();
            if !head.is_empty() && !tail.is_empty() {
                return Some((head, symbol, tail));
            }
        }
    }
    None
}

fn log_marker_color(marker: &str) -> Color {
    match marker
        .trim_matches(':')
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "ERROR" | "ERR" | "FATAL" | "CRITICAL" | "PANIC" => Color::Red,
        "WARN" | "WARNING" | "#" => Color::Yellow,
        "DEBUG" | "TRACE" | "-" => Color::LightBlue,
        "INFO" | "NOTICE" | "LOG" | "*" => Color::Green,
        _ => Color::Gray,
    }
}

fn colorize_events(logs: String) -> Text<'static> {
    let mut lines = Vec::new();

    for raw_line in logs.lines() {
        if raw_line.trim().is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        if let Some(rest) = raw_line.strip_prefix("[event] ") {
            if let Some(runtime_payload) = rest.strip_prefix("runtime ") {
                lines.push(colorize_runtime_event("service", runtime_payload));
                continue;
            }

            if let Some((scope, details)) = rest.split_once(" runtime ") {
                lines.push(colorize_runtime_event(scope, details));
                continue;
            }

            if let Some((scope, action)) = rest.rsplit_once(' ') {
                lines.push(Line::from(vec![
                    Span::styled("[event] ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} ", scope),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        action.to_string(),
                        Style::default().fg(event_action_color(action)),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(Color::Gray),
                )]));
            }
        } else {
            lines.push(Line::from(vec![Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::Gray),
            )]));
        }
    }

    Text::from(lines)
}

fn colorize_runtime_event(scope: &str, details: &str) -> Line<'static> {
    let mut spans = vec![
        Span::styled("[event] ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} ", scope),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("runtime ", Style::default().fg(Color::Yellow)),
    ];

    if let Some(ips_body) = extract_bracket_body(details, "ips=") {
        spans.push(Span::styled("ips=[", Style::default().fg(Color::Blue)));
        spans.extend(colorize_ip_mappings(&ips_body));
        spans.push(Span::styled("]", Style::default().fg(Color::Blue)));
    }

    if let Some(ports_body) = extract_bracket_body(details, "ports=") {
        if !spans.is_empty() {
            spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(Span::styled("ports=[", Style::default().fg(Color::Magenta)));
        spans.extend(colorize_port_mappings(&ports_body));
        spans.push(Span::styled("]", Style::default().fg(Color::Magenta)));
    } else {
        spans.push(Span::styled(
            details.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    Line::from(spans)
}

fn event_action_color(action: &str) -> Color {
    match action {
        "start" | "running (snapshot)" | "health_status: healthy" => Color::Green,
        "create" | "restart" | "unpause" => Color::Yellow,
        "stop" | "destroy" | "pause" | "die" => Color::Red,
        "kill" | "health_status: unhealthy" => Color::LightRed,
        _ => Color::Gray,
    }
}

fn extract_bracket_body(details: &str, key: &str) -> Option<String> {
    let start = details.find(key)? + key.len();
    let rest = &details[start..];
    if !rest.starts_with('[') {
        return None;
    }

    let end = rest.find(']')?;
    Some(rest[1..end].to_string())
}

fn colorize_ip_mappings(mappings: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (idx, mapping) in mappings.split(',').map(str::trim).enumerate() {
        if mapping.is_empty() {
            continue;
        }

        if idx > 0 {
            spans.push(Span::styled(", ", Style::default().fg(Color::DarkGray)));
        }

        if let Some((network, ip)) = mapping.split_once('=') {
            spans.push(Span::styled(
                network.to_string(),
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::styled("=", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                ip.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                mapping.to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    spans
}

fn colorize_port_mappings(mappings: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (idx, mapping) in mappings.split(',').map(str::trim).enumerate() {
        if mapping.is_empty() {
            continue;
        }

        if idx > 0 {
            spans.push(Span::styled(", ", Style::default().fg(Color::DarkGray)));
        }

        if let Some((container_port, host_binding)) = mapping.split_once('=') {
            spans.push(Span::styled(
                container_port.to_string(),
                Style::default().fg(Color::Yellow),
            ));
            spans.push(Span::styled("=", Style::default().fg(Color::DarkGray)));

            if host_binding == "internal" {
                spans.push(Span::styled(
                    "internal".to_string(),
                    Style::default().fg(Color::Gray),
                ));
            } else if let Some((host_ip, host_port)) = host_binding.rsplit_once(':') {
                spans.push(Span::styled(
                    host_ip.to_string(),
                    Style::default().fg(Color::Blue),
                ));
                spans.push(Span::styled(":", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    host_port.to_string(),
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    host_binding.to_string(),
                    Style::default().fg(Color::Blue),
                ));
            }
        } else {
            spans.push(Span::styled(
                mapping.to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    spans
}
