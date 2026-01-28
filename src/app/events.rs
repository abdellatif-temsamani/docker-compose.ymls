use crate::app::state::App;
use crate::docker::events::spawn_project_listener;

impl App {
    pub fn start_event_listeners(&self) {
        if !self.docker_daemon_running {
            return;
        }
        for service in &self.services {
            let service_name = service.name.clone();
            let status_clone = std::sync::Arc::clone(&service.status);
            spawn_project_listener(service_name, status_clone);
        }

        self.start_live_log_listeners();
    }
}
