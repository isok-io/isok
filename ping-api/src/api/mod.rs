pub mod auth;
pub mod checks;
pub mod errors;
pub mod routes;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::DbHandler;
use crate::pulsar::PulsarClient;

pub struct ApiHandler {
    pub db: DbHandler,
    pub pulsar_client: PulsarClient,
}

#[derive(Clone)]
pub struct ApiHandlerState(Arc<Mutex<ApiHandler>>);

impl ApiHandlerState {
    pub fn new(handler: ApiHandler) -> Self {
        Self(Arc::new(Mutex::new(handler)))
    }
}

impl Deref for ApiHandlerState {
    type Target = Arc<Mutex<ApiHandler>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ApiHandlerState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
