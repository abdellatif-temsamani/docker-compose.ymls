use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::thread;

use crate::app::state::App;
use crate::docker::compose::ComposeProject;
use crate::status::Status;

#[derive(serde::Deserialize)]
struct Compose {
    services: std::collections::HashMap<String, serde_yaml::Value>,
}

impl App {
    pub fn populate_initial_logs(&self) {
        if !self.docker_daemon_running {
            return;
        }
        for service in &self.services {
            let service_name = service.name.clone();
            let logs = Arc::clone(&service.logs);
            thread::spawn(move || {
                let project = ComposeProject::new(service_name.clone());
                if let Ok(output) = project.ps_output() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("Up") {
                        let compose_path =
                            format!("containers/{}/docker-compose.yml", service_name);
                        let mut text = String::new();
                        if let Ok(content) = fs::read_to_string(&compose_path)
                            && let Ok(compose) = serde_yaml::from_str::<Compose>(&content) {
                                let services = compose.services.keys().cloned().collect::<Vec<_>>();
                                let network = format!("{}_default", service_name);
                                text = format!("Up output:\nNetwork {} Running\n", network);
                                for svc in services {
                                    text.push_str(&format!("Container {} Running\n", svc));
                                }
                            }
                        let mut logs_lock = logs.lock().unwrap();
                        if logs_lock.is_empty() {
                            logs_lock.push_str(&text);
                        }
                    }
                }
            });
        }
    }

    pub fn sync_live_log_listener(&mut self) {
        if !self.docker_daemon_running {
            self.stop_live_logs_for_all_services();
            return;
        }

        let selected_index = self.state.selected();
        let target_index = selected_index.filter(|&index| {
            self.log_tab == crate::app::LogTab::LiveLogs
                && *self.services[index].status.lock().unwrap() == Status::Running
        });

        for index in 0..self.services.len() {
            if Some(index) != target_index {
                self.stop_live_logs_for_service(index);
            }
        }

        if let Some(index) = target_index {
            self.ensure_live_logs_for_service(index);
        }
    }

    fn ensure_live_logs_for_service(&self, index: usize) {
        let service = &self.services[index];
        if service.logs_child.lock().unwrap().is_some() {
            return;
        }

        let project = ComposeProject::new(service.name.clone());
        let live_logs = Arc::clone(&service.live_logs);
        let logs_child = Arc::clone(&service.logs_child);

        if let Ok(mut child) = project.logs_follow()
            && let Some(stdout) = child.stdout.take()
        {
            *logs_child.lock().unwrap() = Some(child);
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    let mut logs = live_logs.lock().unwrap();
                    logs.push_str(&line);
                    logs.push('\n');
                }

                if let Some(mut child) = logs_child.lock().unwrap().take() {
                    let _ = child.wait();
                }
            });
        }
    }

    fn stop_live_logs_for_service(&self, index: usize) {
        let service = &self.services[index];
        if let Some(mut child) = service.logs_child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if *service.status.lock().unwrap() != Status::Running {
            service.live_logs.lock().unwrap().clear();
        }
    }

    fn stop_live_logs_for_all_services(&self) {
        for index in 0..self.services.len() {
            self.stop_live_logs_for_service(index);
        }
    }

    pub fn kill_all_live_logs(&self) {
        self.stop_live_logs_for_all_services();
    }
}
