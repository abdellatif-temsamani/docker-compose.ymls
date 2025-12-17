use crate::status::Status;

#[derive(Clone)]
pub struct Service {
    pub name: String,
    pub status: Status,
}

