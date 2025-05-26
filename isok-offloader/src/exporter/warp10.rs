use crate::config::Warp10Config;
use crate::exporter::Exporter;
use async_trait::async_trait;
use isok_data::models::CheckResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::pin;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::{Mutex, MutexGuard, watch};
use tokio::task::JoinSet;
use tracing::{debug, info, trace};

pub struct Warp10Exporter {
    client: reqwest::Client,
    config: Warp10Config,
    result: Mutex<Vec<CheckResult>>,
    res_tx: UnboundedSender<(CheckResult, i32, i64)>,
    offsets_tx: Sender<HashMap<i32, i64>>,
    offsets_rx: Mutex<Receiver<HashMap<i32, i64>>>,
}

impl Warp10Exporter {
    pub fn new(
        config: Warp10Config,
        js: &mut JoinSet<crate::errors::Result<()>>,
        shutdown_rx: Receiver<()>,
    ) -> Arc<Self> {
        let (res_tx, res_rx) = unbounded_channel();
        let (offsets_tx, offsets_rx) = watch::channel(Default::default());

        let s = Arc::new(Self {
            client: Default::default(),
            result: Mutex::new(Vec::with_capacity(config.batch_size)),
            res_tx,
            config,
            offsets_rx: Mutex::new(offsets_rx),
            offsets_tx,
        });

        js.spawn(s.clone().worker(res_rx, shutdown_rx));

        s
    }

    async fn worker(
        self: Arc<Self>,
        mut res_rx: UnboundedReceiver<(CheckResult, i32, i64)>,
        mut shutdown_rx: Receiver<()>,
    ) -> crate::errors::Result<()> {
        let mut offsets = HashMap::new();
        let itv = tokio::time::interval(Duration::from_millis(self.config.batch_interval));
        pin!(itv);

        loop {
            tokio::select! {
                _ = itv.tick() => {
                    let mut results = self.result.lock().await;
                    if !results.is_empty() {
                        debug!(reason = "tick", "Flushing {} results", results.len());
                        self.flush_results(&mut results, &offsets).await;
                    }
                }
                Some((result, partition, offset)) = res_rx.recv() => {
                    trace!(?result, "got new result");
                    let mut results = self.result.lock().await;
                    results.push(result);
                    offsets.insert(partition, offset);
                    if results.len() >= self.config.batch_size {
                        debug!(reason = "batch_full", "Flushing {} results", results.len());
                        self.flush_results(&mut results, &offsets).await;
                    }
                }
                _ = shutdown_rx.changed() => {
                    info!("Shutting down");
                    let mut results = self.result.lock().await;
                    if !results.is_empty() {
                        debug!(reason = "shutdown", "Flushing {} results", results.len());
                        self.flush_results(&mut results, &offsets).await;
                        // Wait to let the consumer commit its offset, not the cleanest way to do
                        // An improvement would be for the consumer to notify the exporter
                        info!("Waiting 10s before shutting down");
                        let mut itv = tokio::time::interval(Duration::from_secs(10));
                        itv.tick().await;
                        itv.tick().await;

                    }
                    break;
                }
            }
        }

        Ok(())
    }

    async fn flush_results(
        self: &Arc<Self>,
        results: &mut MutexGuard<'_, Vec<CheckResult>>,
        offsets: &HashMap<i32, i64>,
    ) {
        // Send results here
        results.clear();
        _ = self.offsets_tx.send(offsets.clone());
    }
}

#[async_trait]
impl Exporter for Arc<Warp10Exporter> {
    async fn send_result(&self, result: CheckResult, partition: i32, offset: i64) {
        _ = self.res_tx.send((result, partition, offset));
    }

    async fn get_commited(&self) -> HashMap<i32, i64> {
        let mut rx = self.offsets_rx.lock().await;
        _ = rx.changed().await;
        let x = rx.borrow().clone();
        x
    }
}
