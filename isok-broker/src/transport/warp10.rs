use std::collections::HashMap;

use isok_data::broker_rpc::CheckResult;
use reqwest::Url;

use crate::config::Warp10Config;
use crate::transport::{ResultTransport, TransportError};

static WARP10_TOKEN_HEADER: &str = "X-Warp10-Token";
static WARP10_UPDATE_ENDPOINT: &str = "/api/v0/update";
static WARP10_ERROR_HEADER: &str = "X-Warp10-Error";

pub struct Warp10MetricsTransport {
    client: reqwest::Client,
    base_url: Url,
    write_token: String,
    /// Extra labels to add to the metrics
    extra_labels: HashMap<String, String>,
}

impl Warp10MetricsTransport {
    pub fn try_new(config: Warp10Config) -> Result<Self, TransportError> {
        if config.endpoint.scheme() == "http" && !config.insecure {
            return Err(TransportError::InsecureParams(
                "Warp10 endpoint is using HTTP, but the broker insecure usage must be allowed"
                    .to_string(),
            ));
        }

        let client = reqwest::Client::default();

        Ok(Self {
            client,
            extra_labels: config.labels,
            base_url: config.endpoint,
            write_token: config.write_token,
        })
    }

    fn handle_global_error(err: reqwest::Error) -> Result<(), TransportError> {
        match err.status() {
            Some(status) if status.is_client_error() => {
                Err(TransportError::BatchFatalError(err.to_string()))
            }
            _ => {
                tracing::error!(error = ?err, "Received server error while sending metrics to Warp10");
                Err(TransportError::ServiceUnhealthy)
            }
        }
    }

    fn handle_response_status(&self, response: reqwest::Response) -> Result<(), TransportError> {
        match response {
            response if response.status().is_success() => {
                tracing::debug!("Successfully sent metrics to Warp10",);
                Ok(())
            }
            response if response.status().is_client_error() => {
                let error = response.headers().get(WARP10_ERROR_HEADER);
                tracing::error!(
                    error = ?error,
                    status_code = ?response.status(),
                    "Failed to send metrics to Warp10, received client error",
                );
                Err(TransportError::BatchFatalError(
                    "Warp10 client error".to_string(),
                ))
            }
            response => {
                let error = response.headers().get(WARP10_ERROR_HEADER);
                tracing::error!(
                    status_code = ?response.status(),
                    error = ?error,
                    "Failed to send metrics to Warp10, received server error",
                );
                Err(TransportError::ServiceUnhealthy)
            }
        }
    }

    async fn post_metrics(&self, metrics: String) -> Result<reqwest::Response, reqwest::Error> {
        self.client
            .post(self.base_url.join(WARP10_UPDATE_ENDPOINT).unwrap())
            .header(WARP10_TOKEN_HEADER, &self.write_token)
            .body(metrics)
            .send()
            .await
    }

    #[tracing::instrument(skip_all, fields(metrics = tracing::field::Empty))]
    async fn send_batch(&self, batch: &[CheckResult]) -> Result<(), TransportError> {
        let metrics = batch
            .iter()
            .map(|r| r.warp10_serialize(&self.extra_labels))
            .fold(String::new(), |acc, cur| {
                if acc.is_empty() {
                    cur
                } else {
                    (acc + "\n") + &cur
                }
            });
        let metrics_count = metrics.lines().count();
        tracing::Span::current().record("metrics_count", &metrics_count);

        match self.post_metrics(metrics).await {
            Ok(response) => self.handle_response_status(response),
            // That case might happen if network is down, or if the server is unreachable
            Err(err) => Self::handle_global_error(err),
        }
    }
}

impl ResultTransport for Warp10MetricsTransport {
    async fn process_result(&self, result: &CheckResult) -> Result<(), TransportError> {
        self.send_batch(&[result.clone()]).await
    }

    async fn health_check(&self) -> Result<(), TransportError> {
        self.client
            .get(self.base_url.join(WARP10_UPDATE_ENDPOINT).unwrap())
            .header(WARP10_TOKEN_HEADER, &self.write_token)
            .send()
            .await
            .map_err(|_| TransportError::ServiceUnhealthy)?;
        Ok(())
    }
}
