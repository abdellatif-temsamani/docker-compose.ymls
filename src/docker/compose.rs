use std::process::{Child, Command, Output, Stdio};

use crate::docker::process::run_capture;

#[derive(Clone)]
pub struct ComposeProject {
    pub dir: String,
}

impl ComposeProject {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let dir = format!("containers/{}", name);
        Self { dir }
    }

    pub fn command(&self) -> Command {
        let mut cmd = Command::new("docker");
        cmd.arg("compose").current_dir(&self.dir);
        cmd
    }

    pub fn pull_cmd(&self) -> Command {
        let mut cmd = self.command();
        cmd.arg("pull");
        cmd
    }

    pub fn up_detached_cmd(&self) -> Command {
        let mut cmd = self.command();
        cmd.arg("up").arg("-d");
        cmd
    }

    pub fn down_cmd(&self) -> Command {
        let mut cmd = self.command();
        cmd.arg("down");
        cmd
    }

    pub fn ps_output(&self) -> std::io::Result<Output> {
        let mut cmd = self.command();
        cmd.arg("ps");
        run_capture(cmd)
    }

    pub fn logs_follow(&self) -> std::io::Result<Child> {
        let mut cmd = self.command();
        cmd.arg("logs")
            .arg("-f")
            .arg("--tail=100")
            .stdout(Stdio::piped());
        cmd.spawn()
    }
}
