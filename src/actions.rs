use std::process::Command;
use std::thread;
use std::sync::Arc;

use crate::app::App;
use crate::status::{Status, ToastState};
use crate::toast::Toast;

impl App {
    pub fn refresh_statuses(&mut self) {
        // Cache daemon status to avoid repeated checks
        let daemon_running = check_docker_daemon();
        let daemon_changed = daemon_running != self.docker_daemon_running;
        self.docker_daemon_running = daemon_running;

        if !self.docker_daemon_running {
            for service in &mut self.services {
                service.status = Status::DaemonNotRunning;
            }
        } else {
            // Use batch status checking for better performance
            // Always check on first run or when daemon changed or services are in transition
            if self.first_status_check ||
                daemon_changed ||
                self.services.iter().any(|s| matches!(s.status, Status::Starting | Status::Stopping | Status::Pulling)) {
                let service_names: Vec<String> = self.services.iter().map(|s| s.name.clone()).collect();
                let batch_statuses = get_batch_statuses(&service_names);

                for service in &mut self.services {
                    if let Some(actual_status) = batch_statuses.get(&service.name).cloned() {
                        // Handle state transitions based on current state and actual container status
                        match service.status {
                            Status::Pulling => {
                                // If pulling and containers are now running, transition is complete
                                if actual_status == Status::Running {
                                    service.status = Status::Running;
                                }
                                // If still stopped after pulling, check logs for errors
                                else if actual_status == Status::Stopped {
                                    let logs = service.logs.lock().unwrap();
                                    if logs.contains("Pull failed") || logs.contains("Pull output:") && !logs.contains("Up output:") {
                                        service.status = Status::Error;
                                    } else {
                                        // Pull completed but containers not yet started, transition to Starting
                                        service.status = Status::Starting;
                                    }
                                }
                                // Stay in Pulling if still transitioning
                            }
                            Status::Starting => {
                                // If containers are now running, transition complete
                                if actual_status == Status::Running {
                                    service.status = Status::Running;
                                }
                                // If still stopped, check for errors
                                else if actual_status == Status::Stopped {
                                    service.status = Status::Error;
                                }
                                // Stay in Starting if still transitioning
                            }
                            Status::Stopping => {
                                // If containers are now stopped, transition complete
                                if actual_status == Status::Stopped {
                                    service.status = Status::Stopped;
                                }
                                // Stay in Stopping if still transitioning
                            }
                            // For stable states, update to actual status
                            _ => {
                                service.status = actual_status;
                            }
                        }
                    }
                }
                self.first_status_check = false;
            }
        }
    }

    pub fn start_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.docker_daemon_running {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: "Cannot start service: Docker daemon not running".to_string(),
                });
                self.toast_timer = 5;
                return;
            }
            let service = &mut self.services[i];
            let current_status = get_status(service.name.clone());
            if current_status == Status::Running {
                self.toast = Some(Toast {
                    state: ToastState::Warning,
                    message: format!("{} already running", service.name),
                });
                self.toast_timer = 4;
                return;
            }

            // Set initial status to Pulling
            service.status = Status::Pulling;

            // Clone necessary data for the thread
            let service_name = service.name.clone();
            let service_name_for_toast = service_name.clone();
            let container_dir = format!("containers/{}", service_name);
            let logs = Arc::clone(&service.logs);

            // Spawn a thread to handle the service startup process
            thread::spawn(move || {
                // Clear previous logs
                {
                    let mut logs_lock = logs.lock().unwrap();
                    logs_lock.clear();
                }

                // Phase 1: Pull images
                let pull_success = match Command::new("docker-compose")
                    .arg("pull")
                    .current_dir(&container_dir)
                    .output()
                {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let output = format!("Pull output:\n{}{}\n", stdout, stderr);
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&output);
                        out.status.success()
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Pull failed: {}\n", e));
                        false
                    }
                };

                if !pull_success {
                    // Set status to Error if pull failed
                    // Note: We can't directly modify service.status here as it's not in scope
                    // The refresh_statuses() will handle detecting this via logs or lack of transition
                    return;
                }

                // Phase 2: Start containers (transition to Starting status)
                // Note: Status transition to Starting happens in refresh_statuses()

                match Command::new("docker-compose")
                    .arg("up")
                    .arg("-d")
                    .current_dir(&container_dir)
                    .output()
                {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let output = format!("Up output:\n{}{}\n", stdout, stderr);
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&output);
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Up failed: {}\n", e));
                    }
                }
            });

            self.toast = Some(Toast {
                state: ToastState::Success,
                message: format!("Starting {}", service_name_for_toast),
            });
            self.toast_timer = 3;
        }
    }

    pub fn stop_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.docker_daemon_running {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: "Cannot stop service: Docker daemon not running".to_string(),
                });
                self.toast_timer = 5;
                return;
            }
            let service = &mut self.services[i];
            let current_status = get_status(service.name.clone());
            if current_status != Status::Running {
                self.toast = Some(Toast {
                    state: ToastState::Warning,
                    message: format!("{} not running", service.name),
                });
                self.toast_timer = 4;
                return;
            }
            service.status = Status::Stopping;

            // Clone the service name for the thread
            let service_name = service.name.clone();
            let service_name_for_toast = service_name.clone();
            let container_dir = format!("containers/{}", service_name);
            let logs = Arc::clone(&service.logs);

            // Spawn a thread to handle the service shutdown
            thread::spawn(move || {
                match Command::new("docker-compose")
                    .arg("down")
                    .current_dir(&container_dir)
                    .output()
                {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let output = format!("Down output:\n{}{}\n", stdout, stderr);
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&output);
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Down failed: {}\n", e));
                    }
                }
            });

            self.toast = Some(Toast {
                state: ToastState::Success,
                message: format!("Stopping {}", service_name_for_toast),
            });
            self.toast_timer = 3;
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
                        self.toast = Some(Toast {
                            state: ToastState::Success,
                            message: "Docker daemon started".to_string(),
                        });
                        self.toast_timer = 3;
                        // Refresh daemon status after operation
                        std::thread::sleep(std::time::Duration::from_millis(500));
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
                        self.toast = Some(Toast {
                            state: ToastState::Error,
                            message: error_msg,
                        });
                        self.toast_timer = 5;
                    }
                }
            }
            Err(e) => {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: format!("Failed to start Docker daemon: {}", e),
                });
                self.toast_timer = 5;
            }
        }
        self.password_input.clear();
        self.daemon_start_mode = false;
    }

    pub fn stop_all_services(&mut self) -> Result<(), String> {
        // Stop all running services before stopping daemon
        let running_services: Vec<String> = self.services.iter()
            .filter(|s| s.status == Status::Running)
            .map(|s| s.name.clone())
            .collect();

        if running_services.is_empty() {
            return Ok(());
        }

        for service_name in running_services {
            let container_dir = format!("containers/{}", service_name);
            match Command::new("docker-compose")
                .arg("down")
                .current_dir(&container_dir)
                .output()
            {
                Ok(out) => {
                    if !out.status.success() {
                        return Err(format!("Failed to stop service {}", service_name));
                    }
                }
                Err(e) => {
                    return Err(format!("Error stopping service {}: {}", service_name, e));
                }
            }
        }

        // Refresh statuses after stopping services
        self.refresh_statuses();
        Ok(())
    }

    pub fn restart_daemon(&mut self) {
        if self.password_input.is_empty() {
            return;
        }

        // First stop all running services
        match self.stop_all_services() {
            Ok(_) => {
                // Show progress message
                self.toast = Some(Toast {
                    state: ToastState::Info,
                    message: "Stopping services before restart...".to_string(),
                });
                self.toast_timer = 2;
            }
            Err(e) => {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: format!("Failed to stop services: {}", e),
                });
                self.toast_timer = 5;
                self.password_input.clear();
                self.daemon_start_mode = false;
                return;
            }
        }

        // Restart both docker.service and docker.socket
        let result = Command::new("sudo")
            .arg("-S")
            .arg("systemctl")
            .arg("restart")
            .arg("docker.service")
            .arg("docker.socket")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match result {
            Ok(mut child) => {
                if let Some(stdin) = child.stdin.as_mut() {
                    use std::io::Write;
                    writeln!(stdin, "{}", self.password_input).ok();
                }
                match child.wait() {
                    Ok(status) if status.success() => {
                        self.toast = Some(Toast {
                            state: ToastState::Success,
                            message: "Docker daemon restarted (services stopped first)".to_string(),
                        });
                        self.toast_timer = 4;
                        // Refresh daemon status after operation
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        self.refresh_statuses();
                    }
                    _ => {
                        let error_msg = if let Some(stderr) = child.stderr.as_mut() {
                            use std::io::Read;
                            let mut buf = String::new();
                            stderr.read_to_string(&mut buf).ok();
                            if buf.trim().is_empty() {
                                "Failed to restart Docker daemon".to_string()
                            } else {
                                format!("Failed to restart Docker daemon: {}", buf.trim())
                            }
                        } else {
                            "Failed to restart Docker daemon".to_string()
                        };
                        self.toast = Some(Toast {
                            state: ToastState::Error,
                            message: error_msg,
                        });
                        self.toast_timer = 5;
                    }
                }
            }
            Err(e) => {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: format!("Failed to restart Docker daemon: {}", e),
                });
                self.toast_timer = 5;
            }
        }
        self.password_input.clear();
        self.daemon_start_mode = false;
    }

    pub fn stop_daemon(&mut self) {
        if self.password_input.is_empty() {
            return;
        }

        // First stop all running services
        match self.stop_all_services() {
            Ok(_) => {
                // Show progress message
                self.toast = Some(Toast {
                    state: ToastState::Info,
                    message: "Stopping services...".to_string(),
                });
                self.toast_timer = 2;
            }
            Err(e) => {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: format!("Failed to stop services: {}", e),
                });
                self.toast_timer = 5;
                self.password_input.clear();
                self.daemon_start_mode = false;
                return;
            }
        }

        // Stop both docker.service and docker.socket
        let result = Command::new("sudo")
            .arg("-S")
            .arg("systemctl")
            .arg("stop")
            .arg("docker.service")
            .arg("docker.socket")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match result {
            Ok(mut child) => {
                if let Some(stdin) = child.stdin.as_mut() {
                    use std::io::Write;
                    writeln!(stdin, "{}", self.password_input).ok();
                }
                match child.wait() {
                    Ok(status) if status.success() => {
                        self.toast = Some(Toast {
                            state: ToastState::Success,
                            message: "Docker daemon stopped (services stopped first)".to_string(),
                        });
                        self.toast_timer = 4;
                        // Refresh daemon status after operation
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        self.refresh_statuses();
                    }
                    _ => {
                        let error_msg = if let Some(stderr) = child.stderr.as_mut() {
                            use std::io::Read;
                            let mut buf = String::new();
                            stderr.read_to_string(&mut buf).ok();
                            if buf.trim().is_empty() {
                                "Failed to stop Docker daemon".to_string()
                            } else {
                                format!("Failed to stop Docker daemon: {}", buf.trim())
                            }
                        } else {
                            "Failed to stop Docker daemon".to_string()
                        };
                        self.toast = Some(Toast {
                            state: ToastState::Error,
                            message: error_msg,
                        });
                        self.toast_timer = 5;
                    }
                }
            }
            Err(e) => {
                self.toast = Some(Toast {
                    state: ToastState::Error,
                    message: format!("Failed to stop Docker daemon: {}", e),
                });
                self.toast_timer = 5;
            }
        }
        self.password_input.clear();
        self.daemon_start_mode = false;
    }

    pub fn execute_daemon_action(&mut self) {
        use crate::app::DaemonAction;
        match self.daemon_action_selected {
            DaemonAction::Start => self.start_daemon(),
            DaemonAction::Stop => self.stop_daemon(),
            DaemonAction::Restart => self.restart_daemon(),
        }
    }
}
fn check_docker_daemon() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
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
                let has_running = lines.iter().any(|line|
                    line.split('\t').nth(1)
                        .map(|status| status.starts_with("Up"))
                        .unwrap_or(false)
                );
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

fn validate_service_name(name: &str) -> bool {
    // Only allow alphanumeric characters, hyphens, and underscores
    name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn get_batch_statuses(service_names: &[String]) -> std::collections::HashMap<String, Status> {
    let mut statuses = std::collections::HashMap::new();

    // Initialize all as stopped and validate names
    for name in service_names {
        if !validate_service_name(name) {
            statuses.insert(name.clone(), Status::Error);
        } else {
            statuses.insert(name.clone(), Status::Stopped);
        }
    }

    if service_names.is_empty() {
        return statuses;
    }

    // Single docker ps call for all services
    match Command::new("docker")
        .arg("ps")
        .arg("--format")
        .arg("{{.Names}}\t{{.Status}}\t{{.Label \"com.docker.compose.project\"}}")
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 3 {
                    let _container_name = parts[0];
                    let status_str = parts[1];
                    let project_name = parts[2];

                    if service_names.contains(&project_name.to_string()) {
                        let status = if status_str.starts_with("Up") {
                            Status::Running
                        } else {
                            Status::Stopped
                        };
                        statuses.insert(project_name.to_string(), status);
                    }
                }
            }
        }
        Err(_) => {
            // On error, mark all as error status
            for name in service_names {
                statuses.insert(name.clone(), Status::Error);
            }
        }
    }

    statuses
}