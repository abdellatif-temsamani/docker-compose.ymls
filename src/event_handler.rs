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
                let quit_key = app.keybinds.app.quit.chars().next().unwrap_or('q');
                let search_key = app.keybinds.app.search.chars().next().unwrap_or('/');
                let stop_key = app.keybinds.services.stop.chars().next().unwrap_or('s');
                let start_key = app.keybinds.services.start.chars().next().unwrap_or('S');
                let daemon_key = app.keybinds.app.daemon_menu.chars().next().unwrap_or('d');
                let scroll_down_key = app.keybinds.app.scroll_down.chars().next().unwrap_or('j');
                let scroll_up_key = app.keybinds.app.scroll_up.chars().next().unwrap_or('k');
                let switch_tab_left_key = app.keybinds.logs.switch_tab_left.chars().next().unwrap_or('[');
                let switch_tab_right_key = app.keybinds.logs.switch_tab_right.chars().next().unwrap_or(']');

                match key.code {
                    KeyCode::Char(c) if c == quit_key && !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        return Ok(false); // Exit the application
                    }
                    KeyCode::Char(c) if c == search_key && !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode && app.focus == crate::app::Focus::Services => {
                        app.search_mode = true;
                        app.search_query.clear();
                    }

                    KeyCode::Char(c) if c == stop_key && !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        if app.focus == crate::app::Focus::Services {
                            app.stop_service();
                        }
                    }
                    KeyCode::Char(c) if c == start_key && !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
                        if app.focus == crate::app::Focus::Services {
                            app.start_service();
                        }
                    }
                    KeyCode::Char(c) if c == daemon_key && !app.search_mode && !app.daemon_start_mode && !app.daemon_menu_mode => {
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
                          KeyCode::Char(c) if c == scroll_down_key => {
                              app.daemon_action_selected = match app.daemon_action_selected {
                                  crate::app::DaemonAction::Start => crate::app::DaemonAction::Stop,
                                  crate::app::DaemonAction::Stop => crate::app::DaemonAction::Restart,
                                  crate::app::DaemonAction::Restart => crate::app::DaemonAction::Start,
                              };
                          }
                          KeyCode::Down => {
                              app.daemon_action_selected = match app.daemon_action_selected {
                                  crate::app::DaemonAction::Start => crate::app::DaemonAction::Stop,
                                  crate::app::DaemonAction::Stop => crate::app::DaemonAction::Restart,
                                  crate::app::DaemonAction::Restart => crate::app::DaemonAction::Start,
                              };
                          }
                          KeyCode::Char(c) if c == scroll_up_key => {
                              app.daemon_action_selected = match app.daemon_action_selected {
                                  crate::app::DaemonAction::Start => crate::app::DaemonAction::Restart,
                                  crate::app::DaemonAction::Stop => crate::app::DaemonAction::Start,
                                  crate::app::DaemonAction::Restart => crate::app::DaemonAction::Stop,
                              };
                          }
                          KeyCode::Up => {
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
                          // Focus switching
                          KeyCode::Char(c) if c == app.keybinds.app.focus_services.chars().next().unwrap_or('h') => {
                              app.focus = crate::app::Focus::Services;
                          }
                          KeyCode::Char(c) if c == app.keybinds.app.focus_logs.chars().next().unwrap_or('l') => {
                              app.focus = crate::app::Focus::Logs;
                          }

                          // Navigation
                          KeyCode::Char(c) if c == scroll_down_key => {
                              if app.focus == crate::app::Focus::Services {
                                  app.next();
                              } else {
                                  app.log_scroll += 1;
                                  app.log_auto_scroll = false;
                              }
                          }
                          KeyCode::Down => {
                              if app.focus == crate::app::Focus::Services {
                                  app.next();
                              } else {
                                  app.log_scroll += 1;
                                  app.log_auto_scroll = false;
                              }
                          }
                          KeyCode::Char(c) if c == scroll_up_key => {
                              if app.focus == crate::app::Focus::Services {
                                  app.previous();
                              } else {
                                  app.log_scroll = app.log_scroll.saturating_sub(1);
                                  app.log_auto_scroll = false;
                              }
                          }
                          KeyCode::Up => {
                              if app.focus == crate::app::Focus::Services {
                                  app.previous();
                              } else {
                                  app.log_scroll = app.log_scroll.saturating_sub(1);
                                  app.log_auto_scroll = false;
                              }
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
                           KeyCode::Char(c) if c == app.keybinds.services.toggle.chars().next().unwrap_or(' ') => {
                               if app.focus == crate::app::Focus::Services {
                                   app.toggle_service();
                               } else if app.focus == crate::app::Focus::Logs {
                                   app.log_auto_scroll = !app.log_auto_scroll;
                               }
                           }
                            KeyCode::Char(c) if c == app.keybinds.app.refresh.chars().next().unwrap_or('r') => {
                              app.refresh_statuses();
                              app.toast = Some(Toast {
                                  state: ToastState::Info,
                                  message: "Refreshed statuses".to_string(),
                              });
                              app.toast_timer = 3;
                          }
                          KeyCode::Char(c) if c == switch_tab_left_key && app.focus == crate::app::Focus::Logs => {
                              app.log_tab = match app.log_tab {
                                  crate::app::LogTab::Events => crate::app::LogTab::LiveLogs,
                                  crate::app::LogTab::LiveLogs => crate::app::LogTab::Events,
                              };
                              if app.log_tab == crate::app::LogTab::LiveLogs {
                                  app.log_auto_scroll = true;
                              }
                          }
                          KeyCode::Char(c) if c == switch_tab_right_key && app.focus == crate::app::Focus::Logs => {
                              app.log_tab = match app.log_tab {
                                  crate::app::LogTab::Events => crate::app::LogTab::LiveLogs,
                                  crate::app::LogTab::LiveLogs => crate::app::LogTab::Events,
                              };
                              if app.log_tab == crate::app::LogTab::LiveLogs {
                                  app.log_auto_scroll = true;
                              }
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