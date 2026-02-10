use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, help_area: Rect) {
    let controls = controls_line(app);
    let border_color = if app.search_mode {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let controls_widget = Paragraph::new(controls)
        .block(
            Block::default()
                .title(" Controls ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(controls_widget, help_area);
}

fn controls_line(app: &App) -> Line<'static> {
    let app_keys = &app.keybinds.app;
    let service_keys = &app.keybinds.services;
    let log_keys = &app.keybinds.logs;

    let mut spans = Vec::new();
    push_key(&mut spans, "Quit", app_keys.quit.clone(), Color::Red);
    spans.push(sep());
    push_key(&mut spans, "Refresh", app_keys.refresh.clone(), Color::Cyan);
    spans.push(sep());
    push_key(&mut spans, "Search", app_keys.search.clone(), Color::Yellow);
    spans.push(sep());
    push_key(
        &mut spans,
        "Daemon",
        app_keys.daemon_menu.clone(),
        Color::Magenta,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Stop",
        service_keys.stop.clone(),
        Color::LightRed,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Start",
        service_keys.start.clone(),
        Color::LightGreen,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Toggle",
        service_keys.toggle.clone(),
        Color::Blue,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Down",
        app_keys.scroll_down.clone(),
        Color::LightBlue,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Up",
        app_keys.scroll_up.clone(),
        Color::LightBlue,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Auto",
        log_keys.toggle_auto_scroll.clone(),
        Color::Green,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Tab<-",
        app_keys.switch_tab_left.clone(),
        Color::LightYellow,
    );
    spans.push(sep());
    push_key(
        &mut spans,
        "Tab->",
        app_keys.switch_tab_right.clone(),
        Color::LightYellow,
    );

    if app.search_mode {
        spans.push(sep());
        spans.push(Span::styled(
            "Search: Enter=select Esc=cancel",
            Style::default().fg(Color::Yellow),
        ));
    }

    Line::from(spans)
}

fn push_key(spans: &mut Vec<Span<'static>>, label: &str, value: String, color: Color) {
    spans.push(Span::styled(
        format!("{} ", label),
        Style::default().fg(color),
    ));
    spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled(
        value,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
}

fn sep() -> Span<'static> {
    Span::styled(" · ", Style::default().fg(Color::DarkGray))
}
