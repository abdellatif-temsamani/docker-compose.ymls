use std::process::Command;
use std::thread;
use std::sync::Arc;

// Helper function for safe mutex locking
pub fn lock_logs(logs: &std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, crate::app::LogBuffer>>>) -> Option<std::sync::MutexGuard<'_, std::collections::HashMap<String, crate::app::LogBuffer>>> {
    match logs.lock() {
        Ok(guard) => Some(guard),
        Err(_) => {
            eprintln!("Warning: Failed to lock logs mutex");
            None
        }
    }
}

use crate::app::{App, LogBuffer};
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
                if let Some(current_status) = batch_statuses.get(&service.name).cloned() {
                    // Only update if transitioning to expected status or not in transition
                    if (service.status == Status::Starting && current_status == Status::Running)
                        || (service.status == Status::Stopping && current_status == Status::Stopped)
                        || (service.status == Status::Pulling && current_status == Status::Running)
                        || (service.status != Status::Starting && service.status != Status::Stopping && service.status != Status::Pulling)
                    {
                        service.status = current_status;
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
            service.status = Status::Pulling;

            // Clone the service name and logs for the thread
            let service_name = service.name.clone();
            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&self.logs);
            let container_dir = format!("containers/{}", service_name);

            // Spawn a thread to handle the service startup
            thread::spawn(move || {
                // First, pull images and capture logs
                if let Some(mut logs_guard) = lock_logs(&logs) {
                    logs_guard.entry(service_name.clone())
                        .or_insert_with(|| LogBuffer::default())
                        .add_entry(service_name.clone(), "Pulling images...".to_string());
                }
                match Command::new("docker-compose")
                    .arg("pull")
                    .current_dir(&container_dir)
                    .output()
                    .map_err(|e| format!("Failed to execute docker-compose pull: {}", e))
                {
                    Ok(out) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            let service_buffer = logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default());
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            for line in stdout.lines().chain(stderr.lines()) {
                                if !line.trim().is_empty() {
                                    // Parse and enhance pull progress messages with more detail
                                    let enhanced_line = if line.contains("Status: Downloaded") {
                                        format!("✅ {}", line)
                                    } else if line.contains("Status: Image is up to date") {
                                        format!("📋 {}", line)
                                    } else if line.contains("Pulling") && line.contains("fs layer") {
                                        format!("📥 {}", line)
                                    } else if line.contains("Extracting") {
                                        format!("📦 {}", line)
                                    } else if line.contains("Pull complete") {
                                        format!("✅ {}", line)
                                    } else if line.contains("Downloaded") {
                                        format!("⬇️ {}", line)
                                    } else if line.contains("Download") {
                                        format!("⬇️ {}", line)
                                    } else {
                                        format!("pull: {}", line)
                                    };
                                    service_buffer.add_entry(service_name.clone(), enhanced_line);
                                }
                            }
                            if out.status.success() {
                                service_buffer.add_entry(service_name.clone(), "Pull completed successfully".to_string());
                                // Note: Status transition from Pulling to Starting happens via refresh_statuses
                            } else {
                                service_buffer.add_entry(service_name.clone(), "Pull failed - aborting start".to_string());
                                return; // Don't proceed with starting if pull failed
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default())
                                .add_entry(service_name.clone(), format!("Pull error: {}", e));
                        }
                    }
                }

                // Then start the service and capture initial logs
                if let Some(mut logs_guard) = lock_logs(&logs) {
                    logs_guard.entry(service_name.clone())
                        .or_insert_with(|| LogBuffer::default())
                        .add_entry(service_name.clone(), "Starting service...".to_string());
                }
                match Command::new("docker-compose")
                    .arg("up")
                    .arg("-d")
                    .current_dir(&container_dir)
                    .output()
                    .map_err(|e| format!("Failed to execute docker-compose up: {}", e))
                {
                    Ok(out) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            let service_buffer = logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default());
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            for line in stdout.lines().chain(stderr.lines()) {
                                if !line.trim().is_empty() {
                                    service_buffer.add_entry(service_name.clone(), format!("up: {}", line));
                                }
                            }
                            if out.status.success() {
                                service_buffer.add_entry(service_name.clone(), "Service started successfully".to_string());
                            } else {
                                service_buffer.add_entry(service_name.clone(), "Service start failed".to_string());
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default())
                                .add_entry(service_name.clone(), format!("Start error: {}", e));
                        }
                    }
                }
                // Note: Status transition from Pulling to Starting happens via refresh_statuses
                match Command::new("docker-compose")
                    .arg("up")
                    .arg("-d")
                    .current_dir(&container_dir)
                    .output()
                    .map_err(|e| format!("Failed to execute docker-compose up: {}", e))
                {
                    Ok(out) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            let service_buffer = logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default());
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            for line in stdout.lines().chain(stderr.lines()) {
                                if !line.trim().is_empty() {
                                    service_buffer.add_entry(service_name.clone(), format!("up: {}", line));
                                }
                            }
                            if out.status.success() {
                                service_buffer.add_entry(service_name.clone(), "Service started successfully".to_string());
                            } else {
                                service_buffer.add_entry(service_name.clone(), "Service start failed".to_string());
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default())
                                .add_entry(service_name.clone(), format!("Start error: {}", e));
                        }
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

            // Clone the service name and logs for the thread
            let service_name = service.name.clone();
            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&self.logs);
            let container_dir = format!("containers/{}", service_name);

            // Spawn a thread to handle the service shutdown
            thread::spawn(move || {
                if let Some(mut logs_guard) = lock_logs(&logs) {
                    logs_guard.entry(service_name.clone())
                        .or_insert_with(|| LogBuffer::default())
                        .add_entry(service_name.clone(), "Stopping service...".to_string());
                }
                match Command::new("docker-compose")
                    .arg("down")
                    .current_dir(&container_dir)
                    .output()
                    .map_err(|e| format!("Failed to execute docker-compose down: {}", e))
                {
                    Ok(out) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            let service_buffer = logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default());
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            for line in stdout.lines().chain(stderr.lines()) {
                                if !line.trim().is_empty() {
                                    service_buffer.add_entry(service_name.clone(), format!("down: {}", line));
                                }
                            }
                            if out.status.success() {
                                service_buffer.add_entry(service_name.clone(), "Service stopped successfully".to_string());
                            } else {
                                service_buffer.add_entry(service_name.clone(), "Service stop failed".to_string());
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(mut logs_guard) = lock_logs(&logs) {
                            logs_guard.entry(service_name.clone())
                                .or_insert_with(|| LogBuffer::default())
                                .add_entry(service_name.clone(), format!("Stop error: {}", e));
                        }
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

    pub fn refresh_logs(&mut self) {
        if !self.docker_daemon_running {
            // Add system message to all existing service buffers
            if let Some(mut logs_guard) = lock_logs(&self.logs) {
                for (_service_name, buffer) in logs_guard.iter_mut() {
                    buffer.add_entry("system".to_string(), "Docker daemon not running".to_string());
                }
            }
            return;
        }
        if let Some(i) = self.state.selected() {
            let service = &self.services[i];
            match Command::new("docker-compose")
                .arg("logs")
                .arg("--tail")
                .arg("20")
                .current_dir(format!("containers/{}", service.name))
                .output()
            {
                Ok(out) => {
                    if let Some(mut logs_guard) = lock_logs(&self.logs) {
                        let service_buffer = logs_guard.entry(service.name.clone())
                            .or_insert_with(|| LogBuffer::default());
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);

                        // Add runtime logs if available
                        for line in stdout.lines().chain(stderr.lines()) {
                            if !line.trim().is_empty() {
                                service_buffer.add_entry(service.name.clone(), format!("runtime: {}", line));
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(mut logs_guard) = lock_logs(&self.logs) {
                        let service_buffer = logs_guard.entry(service.name.clone())
                            .or_insert_with(|| LogBuffer::default());
                        service_buffer.add_entry(service.name.clone(), format!("Failed to get logs: {}", e));
                    }
                }
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