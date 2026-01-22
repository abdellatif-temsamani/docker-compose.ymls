use std::thread;

use crate::app::state::{App, DaemonAction};
use crate::docker::compose::ComposeProject;
use crate::docker::daemon;
use crate::docker::process::run_capture;
use crate::status::ToastState;

impl App {
    pub fn start_daemon(&mut self) {
        if self.password_input.is_empty() {
            return;
        }

        match daemon::start(&self.password_input) {
            Ok(()) => {
                self.set_toast(ToastState::Success, "Docker daemon started", 3);
                thread::sleep(std::time::Duration::from_millis(500));
                self.refresh_statuses();
            }
            Err(error_msg) => {
                self.set_toast(ToastState::Error, error_msg, 5);
            }
        }

        self.password_input.clear();
        self.daemon_start_mode = false;
    }

    pub fn stop_all_services(&mut self) -> Result<(), String> {
        let running_services: Vec<String> = self
            .services
            .iter()
            .filter(|s| *s.status.lock().unwrap() == crate::status::Status::Running)
            .map(|s| s.name.clone())
            .collect();

        if running_services.is_empty() {
            return Ok(());
        }

        for service_name in running_services {
            let project = ComposeProject::new(service_name.clone());
            let cmd = project.down_cmd();
            match run_capture(cmd) {
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

        self.refresh_statuses();
        Ok(())
    }

    pub fn restart_daemon(&mut self) {
        if self.password_input.is_empty() {
            return;
        }

        match self.stop_all_services() {
            Ok(_) => {
                self.set_toast(ToastState::Info, "Stopping services before restart...", 2);
            }
            Err(e) => {
                self.set_toast(
                    ToastState::Error,
                    format!("Failed to stop services: {}", e),
                    5,
                );
                self.password_input.clear();
                self.daemon_start_mode = false;
                return;
            }
        }

        match daemon::restart(&self.password_input) {
            Ok(()) => {
                self.set_toast(
                    ToastState::Success,
                    "Docker daemon restarted (services stopped first)",
                    4,
                );
                thread::sleep(std::time::Duration::from_millis(500));
                self.refresh_statuses();
            }
            Err(error_msg) => {
                self.set_toast(ToastState::Error, error_msg, 5);
            }
        }

        self.password_input.clear();
        self.daemon_start_mode = false;
    }

    pub fn stop_daemon(&mut self) {
        if self.password_input.is_empty() {
            return;
        }

        match self.stop_all_services() {
            Ok(_) => {
                self.set_toast(ToastState::Info, "Stopping services...", 2);
            }
            Err(e) => {
                self.set_toast(
                    ToastState::Error,
                    format!("Failed to stop services: {}", e),
                    5,
                );
                self.password_input.clear();
                self.daemon_start_mode = false;
                return;
            }
        }

        match daemon::stop(&self.password_input) {
            Ok(()) => {
                self.set_toast(
                    ToastState::Success,
                    "Docker daemon stopped (services stopped first)",
                    4,
                );
                thread::sleep(std::time::Duration::from_millis(500));
                self.refresh_statuses();
            }
            Err(error_msg) => {
                self.set_toast(ToastState::Error, error_msg, 5);
            }
        }

        self.password_input.clear();
        self.daemon_start_mode = false;
    }

    pub fn execute_daemon_action(&mut self) {
        match self.daemon_action_selected {
            DaemonAction::Start => self.start_daemon(),
            DaemonAction::Stop => self.stop_daemon(),
            DaemonAction::Restart => self.restart_daemon(),
        }
    }
}
