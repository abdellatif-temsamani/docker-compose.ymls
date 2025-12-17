use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};

use crate::app::App;
use crate::status::ToastState;
use crate::toast::Toast;

/// Handle keyboard events (mouse support disabled)
pub async fn handle_events(app: &mut App) -> io::Result<bool> {
    if event::poll(Duration::from_secs(1))? {
        let event = event::read()?;
        if let event::Event::Key(key) = event
            && key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') if !app.search_mode && !app.daemon_start_mode => {
                        return Ok(false); // Exit the application
                    }
                    KeyCode::Char('/') if !app.search_mode && !app.daemon_start_mode => {
                        app.search_mode = true;
                        app.search_query.clear();
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if app.focus == crate::app::Focus::Logs {
                            app.scroll_logs_half_page_down();
                        }
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if app.focus == crate::app::Focus::Logs {
                            app.scroll_logs_half_page_up();
                        }
                    }
                    KeyCode::Char('g') if !app.search_mode && !app.daemon_start_mode => {
                        if app.focus == crate::app::Focus::Logs {
                            app.scroll_logs_to_top();
                        }
                    }
                    KeyCode::Char('G') if !app.search_mode && !app.daemon_start_mode => {
                        if app.focus == crate::app::Focus::Logs {
                            app.scroll_logs_to_bottom();
                        }
                    }
                    KeyCode::Char('d') if !app.search_mode && !app.daemon_start_mode => {
                        if app.focus == crate::app::Focus::Services {
                            app.stop_service();
                        }
                    }
                    KeyCode::Char('D') if !app.search_mode && !app.daemon_start_mode => {
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
                        // Focus switching with h/l keys
                        KeyCode::Char('h') => {
                            app.focus_services();
                            app.toast = Some(Toast {
                                state: ToastState::Info,
                                message: "Focus: Services - use J/K to navigate".to_string(),
                            });
                            app.toast_timer = 2;
                        }
                        KeyCode::Char('l') => {
                            app.focus_logs();
                            app.toast = Some(Toast {
                                state: ToastState::Info,
                                message: "Focus: Logs - use J/K to scroll".to_string(),
                            });
                            app.toast_timer = 2;
                        }
                        // Navigation or scrolling based on focus
                        KeyCode::Char('j') | KeyCode::Down => {
                            if app.focus == crate::app::Focus::Logs {
                                app.scroll_logs_down();
                            } else {
                                app.next();
                                app.refresh_logs();
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if app.focus == crate::app::Focus::Logs {
                                app.scroll_logs_up();
                            } else {
                                app.previous();
                                app.refresh_logs();
                            }
                        }

                        KeyCode::Tab => {
                            if app.focus == crate::app::Focus::Services {
                                app.next();
                                app.refresh_logs();
                            }
                        }
                        KeyCode::BackTab => {
                            if app.focus == crate::app::Focus::Services {
                                app.previous();
                                app.refresh_logs();
                            }
                        }
                         KeyCode::Char(' ') => {
                             if app.focus == crate::app::Focus::Services {
                                 app.toggle_service();
                             }
                         }
                         KeyCode::Char('r') => {
                            app.refresh_statuses();
                            app.refresh_logs();
                            app.toast = Some(Toast {
                                state: ToastState::Info,
                                message: "Refreshed statuses".to_string(),
                            });
                            app.toast_timer = 3;
                        }
                    _ => {}
                }
            }
        }
    } else {
        // Timeout: auto-refresh statuses
        app.refresh_statuses();
        app.refresh_logs();
    }

    // Handle toast timer
    if app.toast_timer > 0 {
        app.toast_timer = app.toast_timer.saturating_sub(1);
        if app.toast_timer == 0 {
            app.toast = None;
        }
    }

    Ok(true) // Continue running
}