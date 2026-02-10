use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::status::Status;

pub struct ProjectEventTargets {
    pub status: Arc<Mutex<Status>>,
    pub events: Arc<Mutex<String>>,
    pub pull_progress: Arc<Mutex<Option<String>>>,
}

pub fn spawn_projects_listener(project_targets: HashMap<String, ProjectEventTargets>) {
    thread::spawn(move || {
        seed_initial_events(&project_targets);

        loop {
            let mut cmd = std::process::Command::new("docker");
            cmd.arg("events")
                .arg("--filter")
                .arg("type=container")
                .arg("--filter")
                .arg("label=com.docker.compose.project")
                .arg("--format")
                .arg("{{.Action}}\t{{index .Actor.Attributes \"com.docker.compose.project\"}}\t{{index .Actor.Attributes \"name\"}}\t{{index .Actor.Attributes \"exitCode\"}}");

            match cmd.stdout(Stdio::piped()).spawn() {
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            handle_event_line(&line, &project_targets);
                        }
                    }
                    let _ = child.wait();
                }
                Err(_) => {
                    // Docker events stream unavailable, retry shortly.
                }
            }

            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn seed_initial_events(project_targets: &HashMap<String, ProjectEventTargets>) {
    for (project, target) in project_targets {
        let containers = list_project_containers(project);
        if containers.is_empty() {
            continue;
        }

        for container_name in containers {
            append_event_log(
                &target.events,
                project,
                &container_name,
                "running (snapshot)",
            );
            append_runtime_details(&target.events, &container_name);
        }
    }
}

fn handle_event_line(line: &str, project_targets: &HashMap<String, ProjectEventTargets>) {
    let mut parts = line.splitn(4, '\t');
    let action = parts.next().unwrap_or("").trim();
    let project = normalize_template_value(parts.next().unwrap_or("").trim());
    let container_name = parts.next().unwrap_or("").trim();
    let exit_code = parts.next().unwrap_or("").trim();

    if action.is_empty() {
        return;
    }

    let project = if project.is_empty() {
        resolve_project_from_container(container_name).unwrap_or_default()
    } else {
        project
    };

    if project.is_empty() {
        return;
    }

    if let Some(target) = project_targets.get(&project) {
        append_event_log(&target.events, &project, container_name, action);
        if matches!(action, "start" | "restart" | "unpause") {
            append_runtime_details(&target.events, container_name);
        }

        let mut status = target.status.lock().unwrap();
        let currently_stopping = matches!(*status, Status::Stopping);
        let next_status = match action {
            "create" | "restart" | "unpause" => Some(Status::Starting),
            "start" => Some(Status::Running),
            "stop" | "destroy" | "pause" => Some(Status::Stopped),
            "die" | "kill" => {
                if currently_stopping || matches!(*status, Status::Stopped) || exit_code == "0" {
                    Some(Status::Stopped)
                } else {
                    Some(Status::Error)
                }
            }
            _ if action.starts_with("health_status: ") => {
                if action.ends_with("healthy") {
                    Some(Status::Running)
                } else if action.ends_with("unhealthy") {
                    Some(Status::Error)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(next_status) = next_status {
            if next_status != Status::Pulling {
                *target.pull_progress.lock().unwrap() = None;
            }

            if *status != Status::Pulling || matches!(next_status, Status::Running | Status::Error)
            {
                *status = next_status;
            }
        }
    }
}

fn normalize_template_value(value: &str) -> String {
    if value.is_empty() || value == "<no value>" {
        String::new()
    } else {
        value.to_string()
    }
}

fn resolve_project_from_container(container_name: &str) -> Option<String> {
    if container_name.is_empty() {
        return None;
    }

    docker_inspect_field(
        container_name,
        "{{index .Config.Labels \"com.docker.compose.project\"}}",
    )
}

fn append_runtime_details(logs: &Arc<Mutex<String>>, container_name: &str) {
    if container_name.is_empty() {
        return;
    }

    let ips = docker_inspect_field(
        container_name,
        "{{range $k, $v := .NetworkSettings.Networks}}{{$k}}={{$v.IPAddress}} {{end}}",
    )
    .unwrap_or_else(|| "unknown".to_string());

    let ports = docker_inspect_field(
        container_name,
        "{{range $p, $v := .NetworkSettings.Ports}}{{$p}}={{if $v}}{{(index $v 0).HostIp}}:{{(index $v 0).HostPort}}{{else}}internal{{end}} {{end}}",
    )
    .unwrap_or_else(|| "none".to_string());

    let ips = normalize_runtime_value(&ips, "pending");
    let ports = normalize_runtime_value(&ports, "none");

    let mut logs_lock = logs.lock().unwrap();
    logs_lock.push_str(&format!(
        "[event] {} runtime ips=[{}] ports=[{}]\n",
        container_name, ips, ports
    ));
}

fn list_project_containers(project: &str) -> Vec<String> {
    let output = Command::new("docker")
        .arg("ps")
        .arg("--filter")
        .arg(format!("label=com.docker.compose.project={}", project))
        .arg("--format")
        .arg("{{.Names}}")
        .output();

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn docker_inspect_field(container_name: &str, template: &str) -> Option<String> {
    let output = Command::new("docker")
        .arg("inspect")
        .arg("--format")
        .arg(template)
        .arg(container_name)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(", ")
}

fn normalize_runtime_value(value: &str, fallback: &str) -> String {
    let normalized = normalize_whitespace(value);
    if normalized.is_empty()
        || normalized.eq_ignore_ascii_case("unknown")
        || normalized.eq_ignore_ascii_case("none")
        || normalized.eq_ignore_ascii_case("invalid, IP")
        || normalized.eq_ignore_ascii_case("invalid IP")
        || normalized.eq_ignore_ascii_case("<no, value>")
        || normalized.eq_ignore_ascii_case("<no value>")
    {
        fallback.to_string()
    } else {
        normalized
    }
}

fn append_event_log(logs: &Arc<Mutex<String>>, project: &str, container_name: &str, action: &str) {
    let mut logs_lock = logs.lock().unwrap();
    let scope = if container_name.is_empty() {
        project
    } else {
        container_name
    };
    logs_lock.push_str(&format!("[event] {} {}\n", scope, action));
}
