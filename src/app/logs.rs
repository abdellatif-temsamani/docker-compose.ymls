use std::fs;
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
                        if let Ok(content) = fs::read_to_string(&compose_path) {
                            if let Ok(compose) = serde_yaml::from_str::<Compose>(&content) {
                                let services = compose.services.keys().cloned().collect::<Vec<_>>();
                                let network = format!("{}_default", service_name);
                                text = format!("Up output:\nNetwork {} Running\n", network);
                                for svc in services {
                                    text.push_str(&format!("Container {} Running\n", svc));
                                }
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

    pub fn start_live_log_listeners(&self) {
        if !self.docker_daemon_running {
            return;
        }
        for service in &self.services {
            let service_name_logs = service.name.clone();
            let live_logs_clone = Arc::clone(&service.live_logs);
            let logs_child_clone = Arc::clone(&service.logs_child);
            let status_clone = Arc::clone(&service.status);
            thread::spawn(move || {
                let project = ComposeProject::new(service_name_logs);

                loop {
                    loop {
                        match project.ps_output() {
                            Ok(output) => {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                if stdout.contains("Up") {
                                    break;
                                }
                            }
                            Err(_) => {
                                // If ps fails, wait and retry
                            }
                        }
                        thread::sleep(std::time::Duration::from_secs(1));
                    }

                    match project.logs_follow() {
                        Ok(mut child) => {
                            let stdout = child.stdout.take();
                            *logs_child_clone.lock().unwrap() = Some(child);
                            if let Some(stdout) = stdout {
                                use std::io::{BufRead, BufReader};
                                let reader = BufReader::new(stdout);
                                for line in reader.lines().map_while(Result::ok) {
                                    if *status_clone.lock().unwrap() != Status::Running {
                                        if let Some(mut child) =
                                            logs_child_clone.lock().unwrap().take()
                                        {
                                            let _ = child.kill();
                                            let _ = child.wait();
                                        }
                                        live_logs_clone.lock().unwrap().clear();
                                        break;
                                    }
                                    let mut logs = live_logs_clone.lock().unwrap();
                                    logs.push_str(&line);
                                    logs.push('\n');
                                }
                            }
                        }
                        Err(_) => {
                            thread::sleep(std::time::Duration::from_secs(1));
                        }
                    }
                }
            });
        }
    }
}
