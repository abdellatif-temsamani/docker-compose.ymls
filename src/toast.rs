use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::status::ToastState;

#[derive(Clone)]
pub struct Toast {
    pub state: ToastState,
    pub message: String,
}

pub fn create_toast_widget(toast: &Toast) -> Paragraph<'_> {
    let (bg_color, fg_color, border_color) = match toast.state {
        ToastState::Success => (Color::Black, Color::Green, Color::Green),
        ToastState::Warning => (Color::Black, Color::Yellow, Color::Yellow),
        ToastState::Error => (Color::Black, Color::Red, Color::Red),
        ToastState::Info => (Color::Black, Color::Blue, Color::Blue),
    };
    Paragraph::new(toast.message.clone())
        .block(
            Block::default()
                .title("Notification")
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default().bg(bg_color).fg(fg_color)),
        )
        .wrap(ratatui::widgets::Wrap { trim: true })
}
