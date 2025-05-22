mod http;

use self::http::{HttpJob, execute_http};

use isok_data::models::{CheckKind, CheckResult};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Clone)]
pub enum JobKind {
    Http(HttpJob),
}

#[derive(Clone)]
pub struct Job {
    pub check_id: Uuid,
    pub kind: JobKind,
}

impl Job {
    pub async fn execute(self, tx: UnboundedSender<CheckResult>) {
        match self.kind {
            JobKind::Http(http_job) => execute_http(self.check_id, http_job, tx).await,
        }
    }
}

impl Into<JobKind> for CheckKind {
    fn into(self) -> JobKind {
        match self {
            CheckKind::Http(http_check) => JobKind::Http(HttpJob {
                method: http_check.method,
                url: reqwest::Url::parse(&http_check.url.to_string()).unwrap(),
            }),
        }
    }
}

impl Into<Job> for isok_data::models::Check {
    fn into(self) -> Job {
        Job {
            check_id: self.id,
            kind: self.kind.into(),
        }
    }
}
