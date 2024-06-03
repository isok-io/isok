use crate::check::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HttpFields {
    pub status_code: u16,
}

impl HttpFields {
    pub fn new(status_code: u16) -> Self {
        Self { status_code }
    }
}