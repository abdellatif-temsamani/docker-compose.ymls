use std::fmt;

#[derive(Clone, PartialEq)]
pub enum Status {
    Running,
    Stopped,
    Starting,
    Stopping,
    Pulling,
    Error,
    DaemonNotRunning,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Running => write!(f, "running"),
            Status::Stopped => write!(f, "stopped"),
            Status::Starting => write!(f, "starting"),
            Status::Stopping => write!(f, "stopping"),
            Status::Pulling => write!(f, "pulling images"),
            Status::Error => write!(f, "error"),
            Status::DaemonNotRunning => write!(f, "daemon not running"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToastState {
    Success,
    Warning,
    Error,
    Info,
}

