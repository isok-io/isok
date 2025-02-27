use std::ops::Deref;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

pub mod broker_rpc;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct JobId(Ulid);

impl JobId {
    pub fn generate() -> Self {
        Self(Ulid::new())
    }
}

impl From<Ulid> for JobId {
    fn from(value: Ulid) -> Self {
        Self(value)
    }
}

impl Deref for JobId {
    type Target = Ulid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobPrettyName {
    inner: String,
}

impl JobPrettyName {
    pub fn new(inner: String) -> Self {
        Self { inner }
    }
}

impl Deref for JobPrettyName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
