use crate::Result;
use crate::config::Warp10Config;
use crate::errors::Error;
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
use tracing::{debug, error, info, trace, warn};

const WARP10_TOKEN_HEADER: &str = "X-Warp10-Token";
const WARP10_UPDATE_ENDPOINT: &str = "api/v0/update";
const WARP10_ERROR_HEADER: &str = "X-Warp10-Error";

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
        js: &mut JoinSet<Result<()>>,
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
    ) -> Result<()> {
        let mut offsets = HashMap::new();
        let itv = tokio::time::interval(Duration::from_millis(self.config.batch_interval));
        pin!(itv);

        loop {
            tokio::select! {
                _ = itv.tick() => {
                    let mut results = self.result.lock().await;
                    if !results.is_empty() {
                        debug!(reason = "tick", "Flushing {} results", results.len());
                        self.flush_results(&mut results, &offsets).await?;
                    }
                }
                Some((result, partition, offset)) = res_rx.recv() => {
                    trace!(?result, "got new result");
                    let mut results = self.result.lock().await;
                    results.push(result);
                    offsets.insert(partition, offset);
                    if results.len() >= self.config.batch_size {
                        debug!(reason = "batch_full", "Flushing {} results", results.len());
                        self.flush_results(&mut results, &offsets).await?;
                    }
                }
                _ = shutdown_rx.changed() => {
                    info!("Shutting down");
                    let mut results = self.result.lock().await;
                    if !results.is_empty() {
                        debug!(reason = "shutdown", "Flushing {} results", results.len());
                        self.flush_results(&mut results, &offsets).await?;
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
    ) -> Result<()> {
        let res = results.as_slice();
        for i in 0..15 {
            if i > 0 {
                warn!("Try {}/15", i + 1);
            }

            match self.send_batch(res).await {
                Ok(_) => {
                    break;
                }
                Err(error) => {
                    if i == 14 {
                        error!(?error, "Failed to send batch results to Warp10");
                        return Err(error);
                    }
                    warn!(
                        ?error,
                        "Failed to send batch results to Warp10, retrying in {} seconds",
                        4 + i
                    );
                    let mut itv = tokio::time::interval(Duration::from_secs(4 + i));
                    itv.tick().await;
                    itv.tick().await;
                }
            }
        }
        debug!("Sent {} results", results.len());
        results.clear();
        _ = self.offsets_tx.send(offsets.clone());
        Ok(())
    }

    async fn post_metrics(
        &self,
        metrics: String,
    ) -> std::result::Result<reqwest::Response, reqwest::Error> {
        let endpoint = format!("{}{WARP10_UPDATE_ENDPOINT}", self.config.endpoint);
        self.client
            .post(endpoint)
            .header(WARP10_TOKEN_HEADER, &self.config.write_token)
            .body(metrics)
            .send()
            .await
    }

    async fn send_batch(&self, batch: &[CheckResult]) -> Result<()> {
        let metrics = batch
            .iter()
            .map(|r| r.warp10_serialize())
            .fold(String::new(), |acc, cur| {
                if acc.is_empty() {
                    cur
                } else {
                    (acc + "\n") + &cur
                }
            });

        match self.post_metrics(metrics).await {
            Ok(response) => self.handle_response_status(response),
            // That case might happen if network is down, or if the server is unreachable
            Err(err) => Self::handle_global_error(err),
        }
    }

    fn handle_global_error(err: reqwest::Error) -> Result<()> {
        match err.status() {
            Some(status) if status.is_client_error() => Err(Error::Warp10),
            _ => {
                error!(error = ?err, "Received server error while sending metrics to Warp10");
                Err(Error::Warp10)
            }
        }
    }

    fn handle_response_status(&self, response: reqwest::Response) -> Result<()> {
        match response {
            response if response.status().is_success() => {
                debug!("Successfully sent metrics to Warp10",);
                Ok(())
            }
            response if response.status().is_client_error() => {
                let error = response.headers().get(WARP10_ERROR_HEADER);
                tracing::error!(
                    error = ?error,
                    status_code = ?response.status(),
                    "Failed to send metrics to Warp10, received client error",
                );
                Err(Error::Warp10)
            }
            response => {
                let error = response.headers().get(WARP10_ERROR_HEADER);
                tracing::error!(
                    status_code = ?response.status(),
                    error = ?error,
                    "Failed to send metrics to Warp10, received server error",
                );
                Err(Error::Warp10)
            }
        }
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
