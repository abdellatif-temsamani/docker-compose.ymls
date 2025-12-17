use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::service::Service;
use crate::status::{Status, ToastState};
use crate::toast::Toast;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Focus {
    #[default]
    Services,
    Logs,
}

#[derive(Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum DaemonAction {
    #[default]
    Start,
    Stop,
    Restart,
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
    pub daemon_menu_mode: bool,
    pub daemon_action_selected: DaemonAction,
    pub daemon_start_mode: bool,
    pub password_input: String,
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
                    status: Arc::new(Mutex::new(Status::Stopped)),
                    logs: Arc::new(Mutex::new(String::new())),
                })
                .collect(),
            toast,
            toast_timer,

            search_mode: false,
            search_query: String::new(),
            docker_daemon_running: docker_running,
            docker_command_available,
            docker_compose_available,
            daemon_menu_mode: false,
            daemon_action_selected: DaemonAction::Start,
            daemon_start_mode: false,
            password_input: String::new(),
            focus: Focus::Services,  // Start focused on services
            first_status_check: true,
        };
        app.refresh_statuses(); // Check current statuses
        app.populate_initial_logs(); // Populate logs for running services
        app.start_event_listeners(); // Start listening to docker events
        app
    }

    pub fn populate_initial_logs(&mut self) {
        if !self.docker_daemon_running {
            return; // Can't get logs if docker isn't running
        }

        for service in &mut self.services {
            if *service.status.lock().unwrap() == Status::Running {
                // For running services, show a status summary that looks like startup logs
                let service_name = service.name.clone();
                let container_dir = format!("containers/{}", service_name);
                let logs_clone = Arc::clone(&service.logs);

                // Run this in a thread to not block app startup
                std::thread::spawn(move || {
                    let mut log_content = String::new();

                    // Simulate pull status (check if images exist)
                    log_content.push_str("Pull output:\n");
                    if let Ok(out) = std::process::Command::new("docker-compose")
                        .arg("images")
                        .current_dir(&container_dir)
                        .output()
                    {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        for line in stdout.lines().skip(1) { // Skip header
                            if !line.trim().is_empty() {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if !parts.is_empty() {
                                    log_content.push_str(&format!("{} Pulled\n", parts[0]));
                                }
                            }
                        }
                    }
                    log_content.push('\n');

                    // Show up status (services that are running)
                    log_content.push_str("Up output:\n");
                    if let Ok(out) = std::process::Command::new("docker-compose")
                        .arg("ps")
                        .current_dir(&container_dir)
                        .output()
                    {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        for line in stdout.lines().skip(1) { // Skip header
                            if !line.trim().is_empty() {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if !parts.is_empty() {
                                    let service = parts[0];
                                    log_content.push_str(&format!("Container {}  Running\n", service));
                                }
                            }
                        }
                    }

                    let mut logs = logs_clone.lock().unwrap();
                    *logs = log_content;
                });
            }
        }
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

    pub fn start_event_listeners(&self) {
        for service in &self.services {
            let service_name = service.name.clone();
            let status_clone = Arc::clone(&service.status);

            std::thread::spawn(move || {
                let mut cmd = std::process::Command::new("docker");
                cmd.arg("events")
                    .arg("--filter")
                    .arg(format!("label=com.docker.compose.project={}", service_name))
                    .arg("--format")
                    .arg("{{.Action}}\t{{.Actor.Attributes.name}}");

                match cmd.stdout(std::process::Stdio::piped()).spawn() {
                    Ok(mut child) => {
                        if let Some(stdout) = child.stdout.take() {
                            use std::io::{BufRead, BufReader};
                            let reader = BufReader::new(stdout);
                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    let parts: Vec<&str> = line.split('\t').collect();
                                    if parts.len() >= 2 {
                                        let action = parts[0];
                                        let _container_name = parts[1];

                                        let new_status = match action {
                                            "start" => Status::Running,
                                            "stop" | "die" => Status::Stopped,
                                            "create" => Status::Starting,
                                            "destroy" => Status::Stopped,
                                            _ => continue, // Ignore other events
                                        };

                                        *status_clone.lock().unwrap() = new_status;
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // If docker events fails, fall back to polling
                    }
                }
            });
        }
    }




}
