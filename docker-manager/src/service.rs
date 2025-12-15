use crate::status::Status;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub status: Status,
}