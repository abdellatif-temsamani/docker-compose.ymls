use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, Focus};
use crate::service::Service;
use crate::status::Status;

pub fn render(frame: &mut Frame, app: &mut App, list_area: Rect, search_area: Option<Rect>) {
    if let Some(search_area) = search_area {
        let cursor = "_";
        let query = if app.search_query.is_empty() {
            "type to filter services".to_string()
        } else {
            format!("{}{}", app.search_query, cursor)
        };

        let search = Paragraph::new(format!("/{}", query))
            .block(
                Block::default()
                    .title(" Search ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .style(if app.search_query.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            });
        frame.render_widget(search, search_area);
    }

    let filtered_services: Vec<&Service> =
        if app.focus == Focus::Services && app.search_mode && !app.search_query.is_empty() {
            app.services
                .iter()
                .filter(|service| service.name.contains(&app.search_query))
                .collect()
        } else {
            app.services.iter().collect()
        };

    let items: Vec<ListItem> = filtered_services
        .iter()
        .map(|service| {
            let status = service.status.lock().unwrap().clone();
            let style = status_style(&status);
            let indicator = status_indicator(&status, app.animation_tick);
            let line = format!("{} {}  {}", indicator, service.name, status);
            ListItem::new(line).style(style)
        })
        .collect();

    let running_count = app
        .services
        .iter()
        .filter(|service| *service.status.lock().unwrap() == Status::Running)
        .count();
    let title = services_title(app.focus, running_count, app.services.len());

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(if app.focus == Focus::Services {
                    Style::default().fg(Color::Blue)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(selected_style(app))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, list_area, &mut app.state);
}

fn status_style(status: &Status) -> Style {
    match status {
        Status::Starting => Style::default().fg(Color::Yellow),
        Status::Stopping => Style::default().fg(Color::Red),
        Status::Pulling => Style::default().fg(Color::Cyan),
        Status::Running => Style::default().fg(Color::Green),
        Status::Stopped => Style::default().fg(Color::Gray),
        Status::Error => Style::default().fg(Color::White),
        Status::DaemonNotRunning => Style::default().fg(Color::White),
    }
}

fn services_title(_focus: Focus, running_count: usize, total_count: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(" Services ", Style::default().fg(Color::White)),
        Span::styled(
            format!("{}/{} running", running_count, total_count),
            Style::default().fg(Color::Green),
        ),
    ])
}

fn selected_style(app: &App) -> Style {
    if let Some(index) = app.state.selected() {
        let status = app.services[index].status.lock().unwrap().clone();
        if matches!(
            status,
            Status::Starting | Status::Stopping | Status::Pulling
        ) {
            let bg = if (app.animation_tick / 3).is_multiple_of(2) {
                Color::Yellow
            } else {
                Color::LightYellow
            };
            return Style::default()
                .fg(Color::Black)
                .bg(bg)
                .add_modifier(Modifier::BOLD);
        }
    }

    Style::default()
        .fg(Color::Black)
        .bg(Color::Blue)
        .add_modifier(Modifier::BOLD)
}

fn status_indicator(status: &Status, tick: u64) -> &'static str {
    match status {
        Status::Running => "●",
        Status::Pulling => "◌",
        Status::Starting => {
            const FRAMES: [&str; 4] = ["◜", "◠", "◝", "◞"];
            FRAMES[((tick / 2) % FRAMES.len() as u64) as usize]
        }
        Status::Stopping => {
            const FRAMES: [&str; 4] = ["◟", "◡", "◞", "◜"];
            FRAMES[((tick / 2) % FRAMES.len() as u64) as usize]
        }
        Status::Stopped => "○",
        Status::Error => "✖",
        Status::DaemonNotRunning => "○",
    }
}
