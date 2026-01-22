use std::collections::HashMap;
use std::process::Command;

use crate::status::Status;

pub struct DockerClient;

impl DockerClient {
    pub fn docker_info_ok() -> bool {
        Command::new("docker")
            .arg("info")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn docker_cli_ok() -> bool {
        Command::new("docker")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn compose_cli_ok() -> bool {
        Command::new("docker")
            .arg("compose")
            .arg("version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn image_exists(image: &str) -> bool {
        Command::new("docker")
            .arg("image")
            .arg("inspect")
            .arg(image)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn get_status(project: &str) -> Status {
        match Command::new("docker")
            .arg("ps")
            .arg("--filter")
            .arg(format!("label=com.docker.compose.project={}", project))
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
                    let has_running = lines.iter().any(|line| {
                        line.split('\t')
                            .nth(1)
                            .map(|status| status.starts_with("Up"))
                            .unwrap_or(false)
                    });
                    if has_running {
                        Status::Running
                    } else {
                        Status::Stopped
                    }
                }
            }
            Err(_) => Status::Error,
        }
    }

    pub fn get_batch_statuses(service_names: &[String]) -> HashMap<String, Status> {
        let mut statuses = HashMap::new();

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

        let cmd = Command::new("docker")
            .arg("ps")
            .arg("--format")
            .arg("{{.Names}}\t{{.Status}}\t{{.Label \"com.docker.compose.project\"}}")
            .output();

        match cmd {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 3 {
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
                for name in service_names {
                    statuses.insert(name.clone(), Status::Error);
                }
            }
        }

        statuses
    }
}

fn validate_service_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}
