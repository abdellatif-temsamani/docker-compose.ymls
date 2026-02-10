use crate::status::Status;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Service {
    pub name: String,
    pub status: Arc<Mutex<Status>>,
    pub pull_progress: Arc<Mutex<Option<String>>>,
    pub events: Arc<Mutex<String>>,
    pub logs: Arc<Mutex<String>>,
    pub live_logs: Arc<Mutex<String>>,
    pub logs_child: Arc<Mutex<Option<std::process::Child>>>,
}
