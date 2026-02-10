use std::io::{BufRead, BufReader};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

type LineCallback = Arc<dyn Fn(&str) + Send + Sync + 'static>;

pub fn run_capture(mut cmd: Command) -> std::io::Result<Output> {
    cmd.output()
}

pub fn run_stream(
    cmd: Command,
    logs: Arc<Mutex<String>>,
    header: Option<&str>,
) -> std::io::Result<bool> {
    run_stream_with_line_callback(cmd, logs, header, None)
}

pub fn run_stream_with_line_callback(
    mut cmd: Command,
    logs: Arc<Mutex<String>>,
    header: Option<&str>,
    on_line: Option<LineCallback>,
) -> std::io::Result<bool> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    if let Some(header) = header {
        let mut logs_lock = logs.lock().unwrap();
        logs_lock.push_str(header);
    }

    if let Some(stdout) = child.stdout.take() {
        let logs_stdout = Arc::clone(&logs);
        let on_line_stdout = on_line.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let mut logs_lock = logs_stdout.lock().unwrap();
                logs_lock.push_str(&line);
                logs_lock.push('\n');
                if let Some(callback) = &on_line_stdout {
                    callback(&line);
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let logs_stderr = Arc::clone(&logs);
        let on_line_stderr = on_line;
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let mut logs_lock = logs_stderr.lock().unwrap();
                logs_lock.push_str(&line);
                logs_lock.push('\n');
                if let Some(callback) = &on_line_stderr {
                    callback(&line);
                }
            }
        });
    }

    let status = child.wait()?;
    Ok(status.success())
}
