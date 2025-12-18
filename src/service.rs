use std::sync::{Arc, Mutex};
use crate::status::Status;

#[derive(Clone)]
pub struct Service {
    pub name: String,
    pub status: Arc<Mutex<Status>>,
    pub logs: Arc<Mutex<String>>,
    pub live_logs: Arc<Mutex<String>>,
    pub logs_child: Arc<Mutex<Option<std::process::Child>>>,
}

