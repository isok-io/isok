use std::sync::Arc;

use tokio::sync::Mutex;

use crate::db::DbHandler;
use crate::pulsar::PulsarClient;

pub mod auth;
pub mod checks;
pub mod errors;
pub mod routes;

#[derive(Clone)]
pub struct ServerState {
    pub db: Arc<DbHandler>,
    pub pulsar_client: Arc<Mutex<PulsarClient>>,
}
