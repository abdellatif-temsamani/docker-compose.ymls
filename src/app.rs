use std::collections::VecDeque;
use std::fs;
use std::process::Command;

use crate::service::Service;
use crate::status::Status;

#[derive(Default)]
pub struct App {
    pub state: ratatui::widgets::ListState,
    pub services: Vec<Service>,
    pub last_actions: VecDeque<String>,

    pub search_mode: bool,
    pub search_query: String,
    pub docker_daemon_running: bool,
    pub docker_command_available: bool,
    pub daemon_start_mode: bool,
    pub password_input: String,
    pub logs: Vec<String>,
}

fn check_docker_available() -> bool {
    Command::new("docker-compose")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
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
        let mut app = Self {
            state: ratatui::widgets::ListState::default(),
            services: service_names
                .into_iter()
                .map(|name| Service {
                    name,
                    status: Status::Error,
                })
                .collect(),
            last_actions: {
                let mut dq = VecDeque::new();
                if !check_docker_available() {
                    dq.push_back(
                        "Warning: Docker Compose not found. Services may not work.".to_string(),
                    );
                }
                if !docker_command_available {
                    dq.push_back("Warning: Docker CLI not found.".to_string());
                }
                if !docker_running {
                    dq.push_back("Warning: Docker daemon not running.".to_string());
                }
                dq.push_back("Welcome to Docker Manager".to_string());
                dq
            },

            search_mode: false,
            search_query: String::new(),
            docker_daemon_running: docker_running,
            docker_command_available,
            daemon_start_mode: false,
            password_input: String::new(),
            logs: vec![],
        };
        app.load_states(); // Load saved states
        app.refresh_statuses(); // Check current statuses
        app.refresh_logs(); // Load logs for selected
        // Add initial status to output
        for service in &app.services {
            app.last_actions
                .push_back(format!("{}: {}", service.name, service.status));
            if app.last_actions.len() > 20 {
                app.last_actions.pop_front();
            }
        }
        app
    }

    pub fn refresh_statuses(&mut self) {
        self.docker_daemon_running = check_docker_daemon();
        if !self.docker_daemon_running {
            for service in &mut self.services {
                service.status = Status::DaemonNotRunning;
            }
        } else {
            for service in &mut self.services {
                let current_status = get_status(service.name.clone());
                // Only update if transitioning to expected status or not in transition
                if (service.status == Status::Starting && current_status == Status::Running)
                    || (service.status == Status::Stopping && current_status == Status::Stopped)
                    || (service.status != Status::Starting && service.status != Status::Stopping)
                {
                    service.status = current_status;
                }
            }
        }
    }

    pub fn load_states(&mut self) {
        if let Ok(content) = fs::read_to_string("states.json")
            && let Ok(saved_services) =
                serde_json::from_str::<Vec<crate::service::Service>>(&content)
            {
                for saved in saved_services {
                    if let Some(service) = self.services.iter_mut().find(|s| s.name == saved.name) {
                        service.status = saved.status;
                    }
                }
            }
    }

    pub fn start_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.docker_daemon_running {
                self.last_actions
                    .push_back("Cannot start service: Docker daemon not running".to_string());
                if self.last_actions.len() > 20 {
                    self.last_actions.pop_front();
                }
                return;
            }
            let service = &mut self.services[i];
            let current_status = get_status(service.name.clone());
            if current_status == Status::Running {
                self.last_actions
                    .push_back(format!("{} already running", service.name));
                if self.last_actions.len() > 20 {
                    self.last_actions.pop_front();
                }
                return;
            }
            service.status = Status::Starting;
            let _ = Command::new("docker-compose")
                .arg("up")
                .arg("-d")
                .arg("--quiet-pull")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .current_dir(format!("containers/{}", service.name))
                .spawn();
            self.last_actions
                .push_back(format!("Starting {}", service.name));
            if self.last_actions.len() > 20 {
                self.last_actions.pop_front();
            }
        }
    }

    pub fn stop_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.docker_daemon_running {
                self.last_actions
                    .push_back("Cannot stop service: Docker daemon not running".to_string());
                if self.last_actions.len() > 20 {
                    self.last_actions.pop_front();
                }
                return;
            }
            let service = &mut self.services[i];
            let current_status = get_status(service.name.clone());
            if current_status != Status::Running {
                self.last_actions
                    .push_back(format!("{} not running", service.name));
                if self.last_actions.len() > 20 {
                    self.last_actions.pop_front();
                }
                return;
            }
            service.status = Status::Stopping;
            let _ = Command::new("docker-compose")
                .arg("down")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .current_dir(format!("containers/{}", service.name))
                .spawn();
            self.last_actions
                .push_back(format!("Stopping {}", service.name));
            if self.last_actions.len() > 20 {
                self.last_actions.pop_front();
            }
        }
    }

    pub fn toggle_service(&mut self) {
        if let Some(i) = self.state.selected() {
            let service = &self.services[i];
            if service.status == Status::Running {
                self.stop_service();
            } else {
                self.start_service();
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

    pub fn refresh_logs(&mut self) {
        if !self.docker_daemon_running {
            self.logs = vec!["Docker daemon not running".to_string()];
            return;
        }
        if let Some(i) = self.state.selected() {
            let service = &self.services[i];
            match Command::new("docker-compose")
                .arg("logs")
                .arg("--tail")
                .arg("10")
                .arg(&service.name)
                .current_dir(format!("containers/{}", service.name))
                .output()
            {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    self.logs = if stdout.trim().is_empty() {
                        vec!["No logs available".to_string()]
                    } else {
                        stdout.lines().map(|s| s.to_string()).collect()
                    };
                }
                Err(_) => {
                    self.logs = vec!["Failed to get logs".to_string()];
                }
            }
        } else {
            self.logs = vec![];
        }
    }

    pub fn start_daemon(&mut self) {
        if self.password_input.is_empty() {
            return;
        }
        match Command::new("sudo")
            .arg("-S")
            .arg("systemctl")
            .arg("start")
            .arg("docker")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                if let Some(stdin) = child.stdin.as_mut() {
                    use std::io::Write;
                    writeln!(stdin, "{}", self.password_input).ok();
                }
                match child.wait() {
                    Ok(status) if status.success() => {
                        self.last_actions
                            .push_back("Docker daemon started".to_string());
                        self.docker_daemon_running = true;
                        self.refresh_statuses();
                    }
                    _ => {
                        let error_msg = if let Some(stderr) = child.stderr.as_mut() {
                            use std::io::Read;
                            let mut buf = String::new();
                            stderr.read_to_string(&mut buf).ok();
                            if buf.trim().is_empty() {
                                "Failed to start Docker daemon".to_string()
                            } else {
                                format!("Failed to start Docker daemon: {}", buf.trim())
                            }
                        } else {
                            "Failed to start Docker daemon".to_string()
                        };
                        self.last_actions.push_back(error_msg);
                    }
                }
            }
            Err(e) => {
                self.last_actions
                    .push_back(format!("Failed to start Docker daemon: {}", e));
            }
        }
        self.password_input.clear();
        self.daemon_start_mode = false;
        if self.last_actions.len() > 20 {
            self.last_actions.pop_front();
        }
    }
}

fn get_status(name: String) -> Status {
    match Command::new("docker")
        .arg("ps")
        .arg("--filter")
        .arg(format!("label=com.docker.compose.project={}", name))
        .arg("--format")
        .arg("{{.Names}}\t{{.Status}}")
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = stdout.trim().lines().collect();
            if lines.is_empty() {
                Status::Stopped
            } else {
                // Check if any container is running
                let mut has_running = false;
                for line in lines {
                    if let Some(status_part) = line.split('\t').nth(1)
                        && status_part.starts_with("Up") {
                            has_running = true;
                            break;
                        }
                }
                if has_running {
                    Status::Running
                } else {
                    Status::Stopped
                }
            }
        }
        Err(_e) => Status::Error,
    }
}
