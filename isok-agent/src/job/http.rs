use std::time::Duration;

use chrono::Utc;
use isok_data::models::{
    CheckMetrics, CheckResult, CheckResultDetails, CheckStatus, HttpCheckResult,
};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{Instant, timeout};
use uuid::Uuid;

#[derive(Clone)]
pub struct HttpJob {
    pub url: reqwest::Url,
    pub method: reqwest::Method,
}

pub async fn execute_http(check_id: Uuid, http_check: HttpJob, tx: UnboundedSender<CheckResult>) {
    let client = reqwest::Client::new();
    let run_at = Utc::now();
    let before = Instant::now();
    let response = timeout(
        Duration::from_secs(1),
        client.request(http_check.method, http_check.url).send(),
    )
    .await;
    let latency = before.elapsed();

    let (status, error, details) = match response {
        Ok(Ok(response)) => (
            CheckStatus::Reachable,
            None,
            Some(CheckResultDetails::Http(HttpCheckResult {
                status_code: response.status(),
            })),
        ),
        Ok(Err(err)) => (
            CheckStatus::Unreachable,
            Some(format!("internal error : {err}")),
            None,
        ),
        Err(_elapsed) => (
            CheckStatus::Timeout,
            Some("request timed out".to_string()),
            None,
        ),
    };
    _ = tx.send(CheckResult {
        id: check_id,
        run_at,
        metrics: CheckMetrics { latency },
        status,
        details,
        error,
        zone: unsafe { crate::ZONE.expect("Should be here at this point") },
        agent_id: unsafe {
            #[allow(static_mut_refs)]
            crate::AGENT_ID
                .clone()
                .expect("Should be here at this point")
        },
    });
}
