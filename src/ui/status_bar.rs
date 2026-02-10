use chrono::Local;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::status::Status;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let now = Local::now();
    let daemon_dot = if app.docker_daemon_running {
        "●"
    } else {
        "○"
    };

    let running_services = app
        .services
        .iter()
        .filter(|service| *service.status.lock().unwrap() == Status::Running)
        .count();
    let total_services = app.services.len();

    let docker_status_text = if app.docker_daemon_running {
        format!("{} Daemon: running", daemon_dot)
    } else {
        format!("{} Daemon: stopped", daemon_dot)
    };
    let docker_status_color = if app.docker_daemon_running {
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
        "Compose: ok"
    } else {
        "Compose: n/a"
    };
    let docker_compose_color = if app.docker_compose_available {
        Color::Green
    } else {
        Color::Red
    };

    let status_line = Line::from(vec![
        Span::styled(
            "docker-manager",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default().fg(Color::DarkGray)),
        Span::styled("v0.1.0", Style::default().fg(Color::Gray)),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", now.format("%H:%M:%S")),
            Style::default().fg(Color::Gray),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(docker_status_text, Style::default().fg(docker_status_color)),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(docker_cli_text, Style::default().fg(docker_cli_color)),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            docker_compose_text,
            Style::default().fg(docker_compose_color),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("Services: {}/{} running", running_services, total_services),
            Style::default().fg(Color::White),
        ),
    ]);

    let status_bar = Paragraph::new(status_line).block(
        Block::default()
            .title(" Overview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(status_bar, area);
}
