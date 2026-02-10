use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, DaemonAction};

pub fn render(frame: &mut Frame, app: &App) {
    if app.daemon_menu_mode {
        render_daemon_menu(frame, app);
    }

    if app.daemon_start_mode {
        render_password_prompt(frame, app);
    }

    if let Some(toast) = &app.toast {
        let area = Rect {
            x: frame.area().width.saturating_sub(51),
            y: 1,
            width: 50,
            height: 3,
        };
        frame.render_widget(
            crate::toast::create_toast_widget(toast, app.animation_tick),
            area,
        );
    }
}

fn render_daemon_menu(frame: &mut Frame, app: &App) {
    frame.render_widget(Clear, frame.area());

    let area = centered_rect(50, 9, frame.area());
    let actions = [
        DaemonAction::Start,
        DaemonAction::Stop,
        DaemonAction::Restart,
    ];
    let items: Vec<ListItem> = actions
        .iter()
        .map(|action| ListItem::new(action_label(*action)))
        .collect();

    let selected_index = match app.daemon_action_selected {
        DaemonAction::Start => 0,
        DaemonAction::Stop => 1,
        DaemonAction::Restart => 2,
    };

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(selected_index));

    let list = List::new(items)
        .block(
            Block::default()
                .title("Docker Daemon")
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_password_prompt(frame: &mut Frame, app: &App) {
    frame.render_widget(Clear, frame.area());

    let title = match app.daemon_action_selected {
        DaemonAction::Start => "Start Docker Daemon",
        DaemonAction::Stop => "Stop Docker Daemon",
        DaemonAction::Restart => "Restart Docker Daemon",
    };
    let area = centered_rect(50, 5, frame.area());
    let password_mask = "*".repeat(app.password_input.chars().count());

    let prompt = Paragraph::new(password_mask)
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        );

    let prompt = prompt.block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );

    frame.render_widget(prompt, area);
}

fn action_label(action: DaemonAction) -> &'static str {
    match action {
        DaemonAction::Start => "Start Docker Daemon",
        DaemonAction::Stop => "Stop Docker Daemon",
        DaemonAction::Restart => "Restart Docker Daemon",
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [_, vertical, _] = Layout::vertical([
        Constraint::Length(area.height.saturating_sub(height) / 2),
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .areas(area);

    let [_, centered, _] = Layout::horizontal([
        Constraint::Length(area.width.saturating_sub(width) / 2),
        Constraint::Length(width),
        Constraint::Min(0),
    ])
    .areas(vertical);

    centered
}
