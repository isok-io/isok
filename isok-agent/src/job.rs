mod http;

use std::time::Duration;

use http::execute_http;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Clone)]
pub enum JobKind {
    Http(String),
}

#[derive(Clone)]
pub struct Job {
    pub check_id: Uuid,
    pub kind: JobKind,
}

impl Job {
    pub async fn execute(self, snd: UnboundedSender<JobResult>) {
        match self.kind {
            JobKind::Http(url) => execute_http(self.check_id, url, snd).await,
        }
    }
}

#[derive(Debug)]
pub struct JobResult {
    check_id: Uuid,
    latency: Duration,
}
