use std::collections::HashMap;

use crate::app::state::App;
use crate::docker::events::{spawn_projects_listener, ProjectEventTargets};

impl App {
    pub fn start_event_listeners(&mut self) {
        if self.event_listener_running {
            return;
        }

        if !self.docker_daemon_running {
            return;
        }

        let mut project_targets = HashMap::new();
        for service in &self.services {
            project_targets.insert(
                service.name.clone(),
                ProjectEventTargets {
                    status: std::sync::Arc::clone(&service.status),
                    events: std::sync::Arc::clone(&service.events),
                    pull_progress: std::sync::Arc::clone(&service.pull_progress),
                },
            );
        }

        spawn_projects_listener(project_targets);
        self.event_listener_running = true;

        for service in &self.services {
            let mut events = service.events.lock().unwrap();
            if events.is_empty() {
                events.push_str("[event] listener attached\n");
            }
        }
    }
}
