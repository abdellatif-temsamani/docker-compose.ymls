use std::fs;
use std::sync::Arc;
use std::thread;

use serde_yaml;

use crate::app::state::App;
use crate::docker::client::DockerClient;
use crate::docker::compose::ComposeProject;
use crate::docker::daemon;
use crate::docker::process::run_stream;
use crate::status::{Status, ToastState};

impl App {
    pub fn refresh_statuses(&mut self) {
        let daemon_running = daemon::docker_service_active() && DockerClient::docker_info_ok();
        let daemon_changed = daemon_running != self.docker_daemon_running;
        self.docker_daemon_running = daemon_running;

        if !self.docker_daemon_running {
            for service in &mut self.services {
                *service.status.lock().unwrap() = Status::DaemonNotRunning;
            }
        } else if self.first_status_check || daemon_changed {
            let service_names: Vec<String> = self.services.iter().map(|s| s.name.clone()).collect();
            let batch_statuses = DockerClient::get_batch_statuses(&service_names);

            for service in &mut self.services {
                if let Some(actual_status) = batch_statuses.get(&service.name).cloned() {
                    let mut status_lock = service.status.lock().unwrap();
                    match *status_lock {
                        Status::Pulling => {
                            if actual_status == Status::Running {
                                *status_lock = Status::Running;
                            } else if actual_status == Status::Stopped {
                                let logs = service.logs.lock().unwrap();
                                if logs.contains("Pull failed")
                                    || logs.contains("Pull output:") && !logs.contains("Up output:")
                                {
                                    *status_lock = Status::Error;
                                } else {
                                    *status_lock = Status::Starting;
                                }
                            }
                        }
                        Status::Starting => {
                            if actual_status == Status::Running {
                                *status_lock = Status::Running;
                            } else if actual_status == Status::Stopped {
                                *status_lock = Status::Error;
                            }
                        }
                        Status::Stopping => {
                            if actual_status == Status::Stopped {
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

            *service.status.lock().unwrap() = Status::Pulling;

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let project = ComposeProject::new(service_name.clone());

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
                            }
                        }
                    }
                }

                let pull_success = if skip_pull {
                    true
                } else {
                    match run_stream(
                        project.pull_cmd(),
                        Arc::clone(&logs),
                        Some("Pull output:\n"),
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
                    return;
                }

                if let Err(e) = run_stream(
                    project.up_detached_cmd(),
                    Arc::clone(&logs),
                    Some("Up output:\n"),
                ) {
                    let mut logs_lock = logs.lock().unwrap();
                    logs_lock.push_str(&format!("Up failed: {}\n", e));
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

            *service.status.lock().unwrap() = Status::Stopping;

            *service.live_logs.lock().unwrap() = String::new();
            if let Some(mut child) = service.logs_child.lock().unwrap().take() {
                let _ = child.kill();
            }

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let project = ComposeProject::new(service_name);

            thread::spawn(move || {
                if let Err(e) = run_stream(
                    project.down_cmd(),
                    Arc::clone(&logs),
                    Some("Down output:\n"),
                ) {
                    let mut logs_lock = logs.lock().unwrap();
                    logs_lock.push_str(&format!("Down failed: {}\n", e));
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
