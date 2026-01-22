use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::status::Status;

pub fn spawn_project_listener(project: String, status: Arc<Mutex<Status>>) {
    thread::spawn(move || {
        let mut cmd = std::process::Command::new("docker");
        cmd.arg("events")
            .arg("--filter")
            .arg(format!("label=com.docker.compose.project={}", project))
            .arg("--format")
            .arg("{{.Action}}\t{{.Actor.Attributes.name}}");

        match cmd.stdout(std::process::Stdio::piped()).spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().flatten() {
                        let parts: Vec<&str> = line.split('\t').collect();
                        if parts.len() >= 2 {
                            let action = parts[0];
                            let new_status = match action {
                                "start" => Status::Running,
                                "stop" | "die" => Status::Stopped,
                                "create" => Status::Starting,
                                "destroy" => Status::Stopped,
                                _ => continue,
                            };

                            *status.lock().unwrap() = new_status;
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
