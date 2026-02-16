use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub fn docker_service_active() -> bool {
    Command::new("systemctl")
        .arg("is-active")
        .arg("docker.service")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn start(password: &str) -> Result<(), String> {
    run_systemctl(password, "start", &["docker.service", "docker.socket"])?;
    ensure_daemon_state(true, "start")
}

pub fn stop(password: &str) -> Result<(), String> {
    run_systemctl(password, "stop", &["docker.service", "docker.socket"])?;
    ensure_daemon_state(false, "stop")
}

pub fn restart(password: &str) -> Result<(), String> {
    run_systemctl(password, "restart", &["docker.service", "docker.socket"])?;
    ensure_daemon_state(true, "restart")
}

fn run_systemctl(password: &str, action: &str, units: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new("sudo");
    cmd.arg("-S")
        .arg("-p")
        .arg("")
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

    let mut stderr = child.stderr.take();

    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        _ => {
            let error_msg = if let Some(stderr) = stderr.as_mut() {
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

fn ensure_daemon_state(expected_active: bool, action: &str) -> Result<(), String> {
    const MAX_RETRIES: usize = 20;
    const RETRY_DELAY_MS: u64 = 100;

    for _ in 0..MAX_RETRIES {
        if docker_service_active() == expected_active {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
    }

    let expectation = if expected_active {
        "running"
    } else {
        "stopped"
    };
    Err(format!(
        "Docker daemon did not become {} after {}",
        expectation, action
    ))
}
