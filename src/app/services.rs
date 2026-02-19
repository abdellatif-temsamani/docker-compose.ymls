use std::fs;
use std::sync::Arc;
use std::thread;

use serde_yaml;

use crate::app::state::App;
use crate::docker::client::DockerClient;
use crate::docker::compose::ComposeProject;
use crate::docker::daemon;
use crate::docker::process::{run_stream, run_stream_with_line_callback};
use crate::status::{Status, ToastState};

impl App {
    pub fn refresh_statuses(&mut self) {
        const DAEMON_PROBE_COOLDOWN_TICKS: u8 = 60;

        let should_probe_daemon = self.first_status_check
            || self.daemon_probe_cooldown_ticks == 0
            || !self.docker_daemon_running;
        let daemon_running = if should_probe_daemon {
            self.daemon_probe_cooldown_ticks = DAEMON_PROBE_COOLDOWN_TICKS;
            daemon::docker_service_active() && DockerClient::docker_info_ok()
        } else {
            self.docker_daemon_running
        };
        let daemon_changed = daemon_running != self.docker_daemon_running;
        self.docker_daemon_running = daemon_running;
        let has_transitioning_services = self.services.iter().any(|service| {
            matches!(
                *service.status.lock().unwrap(),
                Status::Pulling | Status::Starting | Status::Stopping
            )
        });

        if !self.docker_daemon_running {
            self.event_listener_running = false;
            for service in &mut self.services {
                *service.status.lock().unwrap() = Status::DaemonNotRunning;
                *service.pull_progress.lock().unwrap() = None;
            }
        } else if self.first_status_check || daemon_changed || has_transitioning_services {
            let service_names: Vec<String> = self.services.iter().map(|s| s.name.clone()).collect();
            let batch_statuses = DockerClient::get_batch_statuses(&service_names);

            for service in &mut self.services {
                if let Some(actual_status) = batch_statuses.get(&service.name).cloned() {
                    let mut status_lock = service.status.lock().unwrap();
                    match *status_lock {
                        Status::Pulling => {
                            if actual_status == Status::Running {
                                *service.pull_progress.lock().unwrap() = None;
                                *status_lock = Status::Running;
                            }
                        }
                        Status::Starting => {
                            if actual_status == Status::Running {
                                *service.pull_progress.lock().unwrap() = None;
                                *status_lock = Status::Running;
                            }
                        }
                        Status::Stopping => {
                            if actual_status == Status::Stopped
                                && DockerClient::all_containers_stopped(&service.name)
                            {
                                *service.pull_progress.lock().unwrap() = None;
                                *status_lock = Status::Stopped;
                            }
                        }
                        _ => {
                            *status_lock = actual_status;
                        }
                    }
                }
            }
            self.first_status_check = false;
        }

        if self.docker_daemon_running && !self.event_listener_running {
            self.start_event_listeners();
        }
    }

    pub fn start_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !daemon::docker_service_active() {
                self.set_toast(
                    ToastState::Error,
                    "Cannot start service: Docker service not running",
                    5,
                );
                return;
            }
            if !self.docker_daemon_running {
                self.set_toast(
                    ToastState::Error,
                    "Cannot start service: Docker daemon not responding",
                    5,
                );
                return;
            }

            let service_name = self.services[i].name.clone();
            let current_status = DockerClient::get_status(&service_name);
            if current_status == Status::Running {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} already running", service_name),
                    4,
                );
                return;
            }

            let service = &mut self.services[i];

            if matches!(
                *service.status.lock().unwrap(),
                Status::Pulling | Status::Starting | Status::Stopping
            ) {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} is busy, wait for operation to finish", service_name),
                    3,
                );
                return;
            }

            *service.status.lock().unwrap() = Status::Pulling;
            *service.pull_progress.lock().unwrap() = Some("queued".to_string());

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let status = Arc::clone(&service.status);
            let pull_progress = Arc::clone(&service.pull_progress);
            let project = ComposeProject::new(service_name.clone());
            let service_name_for_status = service_name.clone();

            thread::spawn(move || {
                {
                    let mut logs_lock = logs.lock().unwrap();
                    logs_lock.clear();
                }

                let compose_path = format!("containers/{}/docker-compose.yml", service_name);
                let mut skip_pull = false;
                if let Ok(content) = fs::read_to_string(&compose_path) {
                    if let Ok(compose) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                        if let Some(services) = compose.get("services").and_then(|s| s.as_mapping())
                        {
                            let mut all_images_exist = true;
                            for (_service_name, service_def) in services {
                                if let Some(image) =
                                    service_def.get("image").and_then(|i| i.as_str())
                                {
                                    if !DockerClient::image_exists(image) {
                                        all_images_exist = false;
                                        break;
                                    }
                                }
                            }
                            if all_images_exist {
                                skip_pull = true;
                                let mut logs_lock = logs.lock().unwrap();
                                logs_lock.push_str("All images already present, skipping pull.\n");
                                *pull_progress.lock().unwrap() = Some("cached".to_string());
                            }
                        }
                    }
                }

                let pull_success = if skip_pull {
                    true
                } else {
                    let progress_callback = {
                        let pull_progress = Arc::clone(&pull_progress);
                        Arc::new(move |line: &str| {
                            if let Some(progress) = extract_pull_progress(line) {
                                *pull_progress.lock().unwrap() = Some(progress);
                            }
                        })
                    };

                    match run_stream_with_line_callback(
                        project.pull_cmd(),
                        Arc::clone(&logs),
                        Some("Pull output:\n"),
                        Some(progress_callback),
                    ) {
                        Ok(success) => success,
                        Err(e) => {
                            let mut logs_lock = logs.lock().unwrap();
                            logs_lock.push_str(&format!("Pull failed: {}\n", e));
                            false
                        }
                    }
                };

                if !pull_success {
                    *pull_progress.lock().unwrap() = None;
                    *status.lock().unwrap() = Status::Error;
                    return;
                }

                *pull_progress.lock().unwrap() = None;
                *status.lock().unwrap() = Status::Starting;

                match run_stream(
                    project.up_detached_cmd(),
                    Arc::clone(&logs),
                    Some("Up output:\n"),
                ) {
                    Ok(true) => {
                        let actual_status = DockerClient::get_status(&service_name_for_status);
                        if actual_status == Status::Running {
                            *status.lock().unwrap() = Status::Running;
                        }
                    }
                    Ok(false) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str("Up failed: command exited with non-zero status\n");
                        *status.lock().unwrap() = Status::Error;
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Up failed: {}\n", e));
                        *status.lock().unwrap() = Status::Error;
                    }
                }
            });

            self.set_toast(
                ToastState::Success,
                format!("Starting {}", service_name_for_toast),
                3,
            );
        }
    }

    pub fn stop_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !daemon::docker_service_active() {
                self.set_toast(
                    ToastState::Error,
                    "Cannot stop service: Docker service not running",
                    5,
                );
                return;
            }
            if !self.docker_daemon_running {
                self.set_toast(
                    ToastState::Error,
                    "Cannot stop service: Docker daemon not responding",
                    5,
                );
                return;
            }

            let service_name = self.services[i].name.clone();
            let current_status = DockerClient::get_status(&service_name);
            if current_status != Status::Running {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} not running", service_name),
                    4,
                );
                return;
            }

            let service = &mut self.services[i];

            if matches!(
                *service.status.lock().unwrap(),
                Status::Pulling | Status::Starting | Status::Stopping
            ) {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} is busy, wait for operation to finish", service_name),
                    3,
                );
                return;
            }

            *service.status.lock().unwrap() = Status::Stopping;
            *service.pull_progress.lock().unwrap() = None;

            *service.live_logs.lock().unwrap() = String::new();
            if let Some(mut child) = service.logs_child.lock().unwrap().take() {
                let _ = child.kill();
            }

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let status = Arc::clone(&service.status);
            let project = ComposeProject::new(service_name);

            thread::spawn(move || {
                match run_stream(
                    project.down_cmd(),
                    Arc::clone(&logs),
                    Some("Down output:\n"),
                ) {
                    Ok(true) => {
                        *status.lock().unwrap() = Status::Stopped;
                    }
                    Ok(false) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str("Down failed: command exited with non-zero status\n");
                        *status.lock().unwrap() = Status::Error;
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Down failed: {}\n", e));
                        *status.lock().unwrap() = Status::Error;
                    }
                }
            });

            self.set_toast(
                ToastState::Success,
                format!("Stopping {}", service_name_for_toast),
                3,
            );
        }
    }

    pub fn toggle_service(&mut self) {
        if let Some(i) = self.state.selected() {
            let service = &self.services[i];
            if *service.status.lock().unwrap() == Status::Running {
                self.stop_service();
            } else {
                self.start_service();
            }
        }
    }
}

fn extract_pull_progress(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let rhs = trimmed
        .split_once(": ")
        .map(|(_, rhs)| rhs)
        .unwrap_or(trimmed);

    if let Some(percent) = rhs.split_whitespace().find(|token| token.ends_with('%')) {
        return Some(percent.to_string());
    }

    if let Some((done, total)) = extract_size_ratio(rhs) {
        if total > 0.0 {
            let phase = if rhs.contains("Extracting") {
                "Extracting"
            } else {
                "Downloading"
            };
            let percent = ((done / total) * 100.0).round().clamp(0.0, 100.0) as u8;
            return Some(format!("{} {}%", phase, percent));
        }
    }

    for keyword in [
        "Waiting",
        "Pulling fs layer",
        "Downloading",
        "Extracting",
        "Download complete",
        "Pull complete",
        "Already exists",
    ] {
        if rhs.contains(keyword) {
            return Some(keyword.to_string());
        }
    }

    None
}

fn extract_size_ratio(text: &str) -> Option<(f64, f64)> {
    for token in text.split_whitespace() {
        let cleaned =
            token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '/');
        if let Some((left, right)) = cleaned.split_once('/') {
            let done = parse_size_to_bytes(left)?;
            let total = parse_size_to_bytes(right)?;
            return Some((done, total));
        }
    }

    None
}

fn parse_size_to_bytes(token: &str) -> Option<f64> {
    let cleaned = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
    if cleaned.is_empty() {
        return None;
    }

    let mut split_idx = cleaned.len();
    for (idx, ch) in cleaned.char_indices() {
        if !(ch.is_ascii_digit() || ch == '.') {
            split_idx = idx;
            break;
        }
    }

    let number = cleaned[..split_idx].parse::<f64>().ok()?;
    let unit = cleaned[split_idx..].to_ascii_lowercase();

    let multiplier = match unit.as_str() {
        "" | "b" => 1.0,
        "kb" => 1_000.0,
        "mb" => 1_000_000.0,
        "gb" => 1_000_000_000.0,
        "tb" => 1_000_000_000_000.0,
        "kib" => 1_024.0,
        "mib" => 1_048_576.0,
        "gib" => 1_073_741_824.0,
        "tib" => 1_099_511_627_776.0,
        _ => return None,
    };

    Some(number * multiplier)
}
