use std::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Running,
    Stopped,
    Starting,
    Stopping,
    Error,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Running => write!(f, "running"),
            Status::Stopped => write!(f, "stopped"),
            Status::Starting => write!(f, "starting"),
            Status::Stopping => write!(f, "stopping"),
            Status::Error => write!(f, "error"),
        }
    }
}