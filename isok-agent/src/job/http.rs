use reqwest::Method;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;
use tracing::info;
use uuid::Uuid;

use super::JobResult;

pub async fn execute_http(check_id: Uuid, url: String, snd: UnboundedSender<JobResult>) {
    info!("Fetching url : {url}");
    let client = reqwest::Client::new();
    let before = Instant::now();
    let _ = client.request(Method::GET, url).send().await.unwrap();
    let latency = before.elapsed();
    snd.send(JobResult { check_id, latency });
}
