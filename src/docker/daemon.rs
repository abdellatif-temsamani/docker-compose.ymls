use std::io::{Read, Write};
use std::process::{Command, Stdio};

pub fn docker_service_active() -> bool {
    Command::new("systemctl")
        .arg("is-active")
        .arg("docker.service")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn start(password: &str) -> Result<(), String> {
    run_systemctl(password, "start", &["docker"])
}

pub fn stop(password: &str) -> Result<(), String> {
    run_systemctl(password, "stop", &["docker.service", "docker.socket"])
}

pub fn restart(password: &str) -> Result<(), String> {
    run_systemctl(password, "restart", &["docker.service", "docker.socket"])
}

fn run_systemctl(password: &str, action: &str, units: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new("sudo");
    cmd.arg("-S")
        .arg("systemctl")
        .arg(action)
        .args(units)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(stdin) = child.stdin.as_mut() {
        writeln!(stdin, "{}", password).ok();
    }

    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        _ => {
            let error_msg = if let Some(stderr) = child.stderr.as_mut() {
                let mut buf = String::new();
                stderr.read_to_string(&mut buf).ok();
                if buf.trim().is_empty() {
                    format!("Failed to {} Docker daemon", action)
                } else {
                    format!("Failed to {} Docker daemon: {}", action, buf.trim())
                }
            } else {
                format!("Failed to {} Docker daemon", action)
            };
            Err(error_msg)
        }
    }
}
