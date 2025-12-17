use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, KeyCode, KeyEventKind};

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
                    KeyCode::Char('q') if !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        return Ok(false); // Exit the application
                    }
                    KeyCode::Char('/') if !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        app.search_mode = true;
                        app.search_query.clear();
                    }

                    KeyCode::Char('s') if !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        if app.focus == crate::app::Focus::Services {
                            app.stop_service();
                        }
                    }
                    KeyCode::Char('d') if !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        app.daemon_menu_mode = true;
                        app.daemon_action_selected = crate::app::DaemonAction::Start;
                    }
                    KeyCode::Esc => {
                        if app.search_mode || app.daemon_start_mode || app.daemon_menu_mode {
                            app.search_mode = false;
                            app.daemon_start_mode = false;
                            app.daemon_menu_mode = false;
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
                        } else if app.daemon_menu_mode {
                            app.daemon_menu_mode = false;
                            app.daemon_start_mode = true;
                            app.password_input.clear();
                        } else if app.daemon_start_mode {
                            app.execute_daemon_action();
                        }
                    }
                    _ if app.search_mode => match key.code {
                        KeyCode::Char(c) => app.search_query.push(c),
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        _ => {}
                    },
                     _ if app.daemon_menu_mode => match key.code {
                         KeyCode::Char('j') | KeyCode::Down => {
                             app.daemon_action_selected = match app.daemon_action_selected {
                                 crate::app::DaemonAction::Start => crate::app::DaemonAction::Stop,
                                 crate::app::DaemonAction::Stop => crate::app::DaemonAction::Restart,
                                 crate::app::DaemonAction::Restart => crate::app::DaemonAction::Start,
                             };
                         }
                         KeyCode::Char('k') | KeyCode::Up => {
                             app.daemon_action_selected = match app.daemon_action_selected {
                                 crate::app::DaemonAction::Start => crate::app::DaemonAction::Restart,
                                 crate::app::DaemonAction::Stop => crate::app::DaemonAction::Start,
                                 crate::app::DaemonAction::Restart => crate::app::DaemonAction::Stop,
                             };
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
                             app.focus = crate::app::Focus::Services;
                         }
                         KeyCode::Char('l') => {
                             app.focus = crate::app::Focus::Logs;
                         }

                         // Navigation
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.next();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.previous();
                        }

                        KeyCode::Tab => {
                            if app.focus == crate::app::Focus::Services {
                                app.next();
                            }
                        }
                        KeyCode::BackTab => {
                            if app.focus == crate::app::Focus::Services {
                                app.previous();
                            }
                        }
                         KeyCode::Char(' ') => {
                             if app.focus == crate::app::Focus::Services {
                                 app.toggle_service();
                             }
                         }
                         KeyCode::Char('r') => {
                            app.refresh_statuses();
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
        // Timeout: do nothing, events handle status updates
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