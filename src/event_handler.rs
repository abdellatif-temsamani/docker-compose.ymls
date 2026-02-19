use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, KeyCode, KeyEventKind};

use crate::app::{App, DaemonAction, Focus, LogTab};
use crate::status::{Status, ToastState};

pub async fn handle_events(app: &mut App, poll_timeout: Duration) -> io::Result<bool> {
    app.animation_tick = app.animation_tick.wrapping_add(1);
    if app.daemon_probe_cooldown_ticks > 0 {
        app.daemon_probe_cooldown_ticks = app.daemon_probe_cooldown_ticks.saturating_sub(1);
    }

    let keys = Keys::from_app(app);

    if event::poll(poll_timeout)? {
        let event = event::read()?;
        if let event::Event::Key(key) = event
            && key.kind == KeyEventKind::Press
            && !handle_key(app, key.code, &keys)
        {
            return Ok(false);
        }
    } else {
        refresh_if_transitioning(app);
    }

    update_toast_timer(app);
    app.sync_live_log_listener();
    Ok(true)
}

struct Keys {
    quit: char,
    search: char,
    stop: char,
    start: char,
    daemon: char,
    scroll_down: char,
    scroll_up: char,
    switch_tab_left: char,
    switch_tab_right: char,
    toggle: char,
    refresh: char,
}

impl Keys {
    fn from_app(app: &App) -> Self {
        Self {
            quit: app.keybinds.app.quit.chars().next().unwrap_or('q'),
            search: app.keybinds.app.search.chars().next().unwrap_or('/'),
            stop: app.keybinds.services.stop.chars().next().unwrap_or('s'),
            start: app.keybinds.services.start.chars().next().unwrap_or('S'),
            daemon: app.keybinds.app.daemon_menu.chars().next().unwrap_or('d'),
            scroll_down: app.keybinds.app.scroll_down.chars().next().unwrap_or('j'),
            scroll_up: app.keybinds.app.scroll_up.chars().next().unwrap_or('k'),
            switch_tab_left: app
                .keybinds
                .app
                .switch_tab_left
                .chars()
                .next()
                .unwrap_or('['),
            switch_tab_right: app
                .keybinds
                .app
                .switch_tab_right
                .chars()
                .next()
                .unwrap_or(']'),
            toggle: app.keybinds.services.toggle.chars().next().unwrap_or(' '),
            refresh: app.keybinds.app.refresh.chars().next().unwrap_or('r'),
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode, keys: &Keys) -> bool {
    if matches!(code, KeyCode::Char(c) if c == keys.quit) && !in_overlay_mode(app) {
        return false;
    }

    if matches!(code, KeyCode::Char(c) if c == keys.search)
        && !in_overlay_mode(app)
        && app.focus == Focus::Services
    {
        app.search_mode = true;
        app.search_query.clear();
        return true;
    }

    if matches!(code, KeyCode::Char(c) if c == keys.stop) && !in_overlay_mode(app) {
        if app.focus == Focus::Services {
            if selected_service_transitioning(app) {
                app.set_toast(ToastState::Info, "Service is busy, wait for transition", 2);
            } else {
                app.stop_service();
            }
        }
        return true;
    }

    if matches!(code, KeyCode::Char(c) if c == keys.start) && !in_overlay_mode(app) {
        if app.focus == Focus::Services {
            if selected_service_transitioning(app) {
                app.set_toast(ToastState::Info, "Service is busy, wait for transition", 2);
            } else {
                app.start_service();
            }
        }
        return true;
    }

    if matches!(code, KeyCode::Char(c) if c == keys.daemon) && !in_overlay_mode(app) {
        app.daemon_menu_mode = true;
        app.daemon_action_selected = DaemonAction::Start;
        return true;
    }

    match code {
        KeyCode::Esc => {
            if in_overlay_mode(app) {
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
                select_searched_service(app);
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
        _ if app.search_mode => match code {
            KeyCode::Char(c) => app.search_query.push(c),
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            _ => {}
        },
        _ if app.daemon_menu_mode => match code {
            KeyCode::Char(c) if c == keys.scroll_down => daemon_next(app),
            KeyCode::Down => daemon_next(app),
            KeyCode::Char(c) if c == keys.scroll_up => daemon_previous(app),
            KeyCode::Up => daemon_previous(app),
            _ => {}
        },
        _ if app.daemon_start_mode => match code {
            KeyCode::Char(c) => app.password_input.push(c),
            KeyCode::Backspace => {
                app.password_input.pop();
            }
            _ => {}
        },
        _ => handle_normal_mode(app, code, keys),
    }

    true
}

fn handle_normal_mode(app: &mut App, code: KeyCode, keys: &Keys) {
    match code {
        KeyCode::Char(c) if c == keys.scroll_down => move_down(app),
        KeyCode::Down => move_down(app),
        KeyCode::Char(c) if c == keys.scroll_up => move_up(app),
        KeyCode::Up => move_up(app),
        KeyCode::Tab => {
            if app.focus == Focus::Services {
                app.next();
            }
        }
        KeyCode::BackTab => {
            if app.focus == Focus::Services {
                app.previous();
            }
        }
        KeyCode::Char(c) if c == keys.toggle => {
            if app.focus == Focus::Services {
                if selected_service_transitioning(app) {
                    app.set_toast(ToastState::Info, "Service is busy, wait for transition", 2);
                } else {
                    app.toggle_service();
                }
            } else if app.focus == Focus::Logs {
                app.log_auto_scroll = !app.log_auto_scroll;
            }
        }
        KeyCode::Char(c) if c == keys.refresh => {
            app.refresh_statuses();
            app.set_toast(ToastState::Info, "Refreshed statuses", 3);
        }
        KeyCode::Char(c) if c == keys.switch_tab_left => toggle_log_tab(app),
        KeyCode::Char(c) if c == keys.switch_tab_right => toggle_log_tab(app),
        _ => {}
    }
}

fn in_overlay_mode(app: &App) -> bool {
    app.search_mode || app.daemon_start_mode || app.daemon_menu_mode
}

fn selected_service_transitioning(app: &App) -> bool {
    app.state
        .selected()
        .map(|index| {
            matches!(
                *app.services[index].status.lock().unwrap(),
                Status::Pulling | Status::Starting | Status::Stopping
            )
        })
        .unwrap_or(false)
}

fn select_searched_service(app: &mut App) {
    if let Some(index) = app.services.iter().position(|service| {
        service
            .name
            .to_lowercase()
            .starts_with(&app.search_query.to_lowercase())
    }) {
        app.state.select(Some(index));
    }
}

fn daemon_next(app: &mut App) {
    app.daemon_action_selected = match app.daemon_action_selected {
        DaemonAction::Start => DaemonAction::Stop,
        DaemonAction::Stop => DaemonAction::Restart,
        DaemonAction::Restart => DaemonAction::Start,
    };
}

fn daemon_previous(app: &mut App) {
    app.daemon_action_selected = match app.daemon_action_selected {
        DaemonAction::Start => DaemonAction::Restart,
        DaemonAction::Stop => DaemonAction::Start,
        DaemonAction::Restart => DaemonAction::Stop,
    };
}

fn move_down(app: &mut App) {
    if app.focus == Focus::Services {
        app.next();
    } else {
        app.log_scroll += 1;
        app.log_auto_scroll = false;
    }
}

fn move_up(app: &mut App) {
    if app.focus == Focus::Services {
        app.previous();
    } else {
        app.log_scroll = app.log_scroll.saturating_sub(1);
        app.log_auto_scroll = false;
    }
}

fn toggle_log_tab(app: &mut App) {
    app.log_tab = match app.log_tab {
        LogTab::Events => LogTab::LiveLogs,
        LogTab::LiveLogs => LogTab::Events,
    };

    if app.log_tab == LogTab::LiveLogs {
        app.log_auto_scroll = true;
    }
}

fn refresh_if_transitioning(app: &mut App) {
    const STATUS_REFRESH_COOLDOWN_TICKS: u8 = 24;

    if app.status_refresh_cooldown_ticks > 0 {
        app.status_refresh_cooldown_ticks = app.status_refresh_cooldown_ticks.saturating_sub(1);
        return;
    }

    let needs_refresh = app.services.iter().any(|service| {
        matches!(
            *service.status.lock().unwrap(),
            Status::Pulling | Status::Starting | Status::Stopping
        )
    });

    if needs_refresh {
        app.refresh_statuses();
        app.status_refresh_cooldown_ticks = STATUS_REFRESH_COOLDOWN_TICKS;
    }
}

fn update_toast_timer(app: &mut App) {
    const TOAST_TICKS_PER_SECOND: u8 = 30;

    if app.toast_timer > 0 {
        app.toast_tick_accumulator = app.toast_tick_accumulator.saturating_add(1);
        if app.toast_tick_accumulator >= TOAST_TICKS_PER_SECOND {
            app.toast_tick_accumulator = 0;
            app.toast_timer = app.toast_timer.saturating_sub(1);
            if app.toast_timer == 0 {
                app.toast = None;
            }
        }
    }
}
