use std::process::Command;

use crate::service::Service;
use crate::status::{Status, ToastState};
use crate::toast::Toast;

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
    pub daemon_start_mode: bool,
    pub password_input: String,
    pub logs: Vec<String>,
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
            (Some(Toast {
                state: ToastState::Error,
                message: "Docker Compose not found. Services may not work.".to_string(),
            }), 5)
        } else if !docker_command_available {
            (Some(Toast {
                state: ToastState::Error,
                message: "Docker CLI not found.".to_string(),
            }), 5)
        } else if !docker_running {
            (Some(Toast {
                state: ToastState::Warning,
                message: "Docker daemon not running.".to_string(),
            }), 4)
        } else {
            (Some(Toast {
                state: ToastState::Info,
                message: "Welcome to Docker Manager".to_string(),
            }), 3)
        };

        let mut app = Self {
            state: ratatui::widgets::ListState::default(),
            services: service_names
                .into_iter()
                .map(|name| Service {
                    name,
                    status: Status::Error,
                })
                .collect(),
            toast,
            toast_timer,

            search_mode: false,
            search_query: String::new(),
            docker_daemon_running: docker_running,
            docker_command_available,
            daemon_start_mode: false,
            password_input: String::new(),
            logs: vec![],
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
    }
}


