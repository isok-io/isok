use super::Exporter;
use async_trait::async_trait;
use isok_data::models::CheckResult;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::{Mutex, watch};
use tracing::info;

pub struct StdoutExporter {
    tx: Sender<HashMap<i32, i64>>,
    rx: Arc<Mutex<Receiver<HashMap<i32, i64>>>>,
}

impl StdoutExporter {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(HashMap::new());

        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}

#[async_trait]
impl Exporter for StdoutExporter {
    async fn send_result(&self, result: CheckResult, partition: i32, offset: i64) {
        info!("{result:?}");
        self.tx.send_modify(|map| {
            map.insert(partition, offset);
        });
    }

    async fn get_commited(&self) -> HashMap<i32, i64> {
        let mut rx = self.rx.lock().await;
        _ = rx.changed().await;
        let x = rx.borrow().clone();
        x
    }
}
