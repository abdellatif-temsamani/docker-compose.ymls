use std::process::Command;

use ratatui::widgets::ScrollbarState;

use crate::service::Service;
use crate::status::{Status, ToastState};
use crate::toast::Toast;

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Services,
    Logs,
}

impl Default for Focus {
    fn default() -> Self {
        Focus::Services
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub service: String,
    pub message: String,
}

#[derive(Clone)]
pub struct LogBuffer {
    entries: Vec<LogEntry>,
    max_capacity: usize,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_capacity: 500, // Reduced from 1000 to save memory
        }
    }
}

impl LogBuffer {
    pub fn add_entry(&mut self, service: String, message: String) {
        let entry = LogEntry { service, message };

        self.entries.push(entry);

        // Maintain capacity by removing oldest entries if we exceed max
        if self.entries.len() > self.max_capacity {
            let excess = self.entries.len() - self.max_capacity;
            self.entries.drain(0..excess);
        }
    }

    pub fn get_recent_logs(&mut self, limit: usize) -> Vec<String> {
        // Trim buffer to maintain only the most recent logs we need
        if self.entries.len() > limit * 2 {
            // Keep more than requested for smooth scrolling, but not too many
            let excess = self.entries.len() - (limit * 2);
            self.entries.drain(0..excess);
        }

        let start = self.entries.len().saturating_sub(limit);

        self.entries[start..]
            .iter()
            .map(|entry| format!("{}: {}", entry.service, entry.message))
            .collect()
    }
}

#[derive(Default)]
pub struct App {
    pub state: ratatui::widgets::ListState,
    pub services: Vec<Service>,
    pub toast: Option<Toast>,
    pub toast_timer: u32,

    pub search_mode: bool,
    pub search_query: String,
    pub docker_daemon_running: bool,
    pub docker_command_available: bool,
    pub docker_compose_available: bool,
    pub daemon_start_mode: bool,
    pub password_input: String,
    pub logs: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, LogBuffer>>>,
    pub log_scroll_position: u16,

    pub log_viewport_height: u16, // Height of the logs viewport (for scroll calculations)
    pub log_total_lines: u16,     // Total number of log lines (for scroll calculations)
    pub log_scrollbar_state: ScrollbarState, // Scrollbar state for logs
    pub focus: Focus,             // Current focus area
    pub first_status_check: bool, // Track if this is the first status check
}

fn check_docker_daemon() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn check_docker_command() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn check_docker_compose_available() -> bool {
    Command::new("docker-compose")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn get_service_names() -> Vec<String> {
    // Scan containers/ for directories containing docker-compose.yml
    match std::fs::read_dir("containers/") {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .filter_map(|dir| {
                let compose_path = dir.path().join("docker-compose.yml");
                if compose_path.exists() {
                    dir.file_name().to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => vec![
            "adminer".to_string(),
            "mysql".to_string(),
            "phpmyadmin".to_string(),
            "postgres".to_string(),
            "redis".to_string(),
        ], // fallback
    }
}

impl App {
    pub fn new() -> Self {
        let service_names = get_service_names();

        let docker_running = check_docker_daemon();
        let docker_command_available = check_docker_command();
        let docker_compose_available = check_docker_compose_available();

        let (toast, toast_timer) = if !docker_compose_available {
            (
                Some(Toast {
                    state: ToastState::Error,
                    message: "Docker Compose not found. Services may not work.".to_string(),
                }),
                5,
            )
        } else if !docker_command_available {
            (
                Some(Toast {
                    state: ToastState::Error,
                    message: "Docker CLI not found.".to_string(),
                }),
                5,
            )
        } else if !docker_running {
            (
                Some(Toast {
                    state: ToastState::Warning,
                    message: "Docker daemon not running.".to_string(),
                }),
                4,
            )
        } else {
            (
                Some(Toast {
                    state: ToastState::Info,
                    message: "Welcome to Docker Manager".to_string(),
                }),
                3,
            )
        };

        let mut app = Self {
            state: ratatui::widgets::ListState::default(),
            services: service_names
                .into_iter()
                .map(|name| Service {
                    name,
                    status: Status::Stopped,
                })
                .collect(),
            toast,
            toast_timer,

            search_mode: false,
            search_query: String::new(),
            docker_daemon_running: docker_running,
            docker_command_available,
            docker_compose_available,
            daemon_start_mode: false,
            password_input: String::new(),
            logs: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            log_scroll_position: 0,
            log_viewport_height: 10, // Default, will be updated in draw
            log_total_lines: 0,      // Will be updated in draw
            log_scrollbar_state: ScrollbarState::default(),
            focus: Focus::Services,  // Start focused on services
            first_status_check: true,
        };
        app.refresh_statuses(); // Check current statuses
        app.refresh_logs(); // Load logs for selected
        app
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.services.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        // Reset scroll position when switching services
        self.log_scroll_position = 0;
        self.log_scrollbar_state = self.log_scrollbar_state.position(0);
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.services.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        // Reset scroll position when switching services
        self.log_scroll_position = 0;
        self.log_scrollbar_state = self.log_scrollbar_state.position(0);
    }

    pub fn focus_services(&mut self) {
        self.focus = Focus::Services;
    }

    pub fn focus_logs(&mut self) {
        self.focus = Focus::Logs;
    }

    pub fn scroll_logs_up(&mut self) {
        if self.focus == Focus::Logs && self.log_scroll_position > 0 {
            self.log_scroll_position = self.log_scroll_position.saturating_sub(1);
            self.log_scrollbar_state = self.log_scrollbar_state.position(self.log_scroll_position as usize);
        }
    }

    pub fn scroll_logs_down(&mut self) {
        if self.focus == Focus::Logs {
            let max_scroll = self.log_total_lines.saturating_sub(self.log_viewport_height);
            if self.log_scroll_position < max_scroll {
                self.log_scroll_position += 1;
                self.log_scrollbar_state = self.log_scrollbar_state.position(self.log_scroll_position as usize);
            }
        }
    }

    pub fn scroll_logs_half_page_down(&mut self) {
        if self.focus == Focus::Logs {
            let half_page = self.log_viewport_height / 2;
            let max_scroll = self.log_total_lines.saturating_sub(self.log_viewport_height);
            let new_position = (self.log_scroll_position as u16 + half_page).min(max_scroll);
            self.log_scroll_position = new_position;
            self.log_scrollbar_state = self.log_scrollbar_state.position(self.log_scroll_position as usize);
        }
    }

    pub fn scroll_logs_half_page_up(&mut self) {
        if self.focus == Focus::Logs {
            let half_page = self.log_viewport_height / 2;
            let new_position = self.log_scroll_position.saturating_sub(half_page);
            self.log_scroll_position = new_position;
            self.log_scrollbar_state = self.log_scrollbar_state.position(self.log_scroll_position as usize);
        }
    }

    pub fn scroll_logs_to_top(&mut self) {
        if self.focus == Focus::Logs {
            self.log_scroll_position = 0;
            self.log_scrollbar_state = self.log_scrollbar_state.position(0);
        }
    }

    pub fn scroll_logs_to_bottom(&mut self) {
        if self.focus == Focus::Logs {
            let max_scroll = self.log_total_lines.saturating_sub(self.log_viewport_height);
            self.log_scroll_position = max_scroll;
            self.log_scrollbar_state = self.log_scrollbar_state.position(max_scroll as usize);
        }
    }
}
