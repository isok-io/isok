use std::{future::Future, time::Duration};

use enum_dispatch::enum_dispatch;
use https::HttpsJob;
use isok_data::JobId;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    batch_sender::JobResult,
    jobs::{http::HttpJob, tcp::TcpJob},
};

pub mod http;
pub mod https;
pub mod tcp;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(tag = "type")]
#[enum_dispatch(Execute)]
pub enum JobInnerConfig {
    #[serde(rename = "tcp")]
    Tcp(TcpJob),
    #[serde(rename = "http")]
    Http(HttpJob),
    #[serde(rename = "https")]
    Https(HttpsJob),
}

impl Execute for JobInnerConfig {
    async fn execute(&self, job_result: &mut JobResult) -> Result<(), JobError> {
        match self {
            Self::Tcp(job) => job.execute(job_result).await,
            Self::Http(job) => job.execute(job_result).await,
            Self::Https(job) => job.execute(job_result).await,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Job {
    #[serde(default = "generate_id")]
    id: JobId,
    #[serde(deserialize_with = "deserialize_duration")]
    interval: Duration,
    #[serde(flatten)]
    inner: JobInnerConfig,
    pretty_name: String,
}

fn deserialize_duration<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Duration, D::Error> {
    let s = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(s))
}

fn generate_id() -> JobId {
    tracing::warn!("One of the job doesn't have any ID, generating one");
    JobId::generate()
}

impl Job {
    pub fn new(interval: Duration, job_config: JobInnerConfig, pretty_name: String) -> Self {
        Self {
            id: JobId::generate(),
            interval,
            inner: job_config,
            pretty_name,
        }
    }

    pub fn id(&self) -> JobId {
        self.id.clone()
    }

    pub(crate) fn interval(&self) -> Duration {
        self.interval
    }

    pub(crate) fn pretty_name(&self) -> String {
        self.pretty_name.clone()
    }

    #[tracing::instrument(skip_all, fields(self.id, self.pretty_name))]
    pub(crate) async fn execute(&self, tx: UnboundedSender<JobResult>) -> Result<(), JobError> {
        let mut job_result = JobResult::new(self.id(), self.pretty_name());
        self.inner.execute(&mut job_result).await?;
        if let Err(e) = tx.send(job_result) {
            tracing::error!("Job failed to properly send its result to channel {e}");
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Invalid job config {0}")]
    InvalidJobConfig(String),
    #[error("Unable to execute job {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("DNS config error: {0}")]
    DnsConfigError(#[from] https::DnsResolverError),
    #[error("Root certificates error: {0}")]
    RootCertsError(#[from] https::RootCertsError),
}

pub trait Execute {
    fn execute(
        &self,
        job_result: &mut JobResult,
    ) -> impl Future<Output = Result<(), JobError>> + Send;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use isok_data::JobId;
    use serde::{Deserialize, Serialize};

    use crate::jobs::{http::HttpJob, Job, JobInnerConfig};

    #[test]
    fn test_see_output_of_job() {
        let job = Job {
            id: JobId::generate(),
            interval: Duration::from_secs(10),
            inner: JobInnerConfig::Http(HttpJob::new("https://google.com".to_string())),
            pretty_name: "google".to_string(),
        };
        let str = serde_yaml::to_string(&job).unwrap();
        println!("{}", str);
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct DummyRootJob {
        jobs: Vec<Job>,
    }

    #[test]
    fn test_job_id_serde_generate() {
        let config = r#"
        jobs:
            - type: "http"
              pretty_name: "5s failing endpoint"
              endpoint: "https://my_endpoint.com/api/v1/healthy?system_only=true"
              interval: 5
              headers:
                Authorization: "Bearer..."
            "#;
        let root: DummyRootJob = serde_yaml::from_str(config).unwrap();
        let job = &root.jobs[0];

        assert_eq!(job.id.to_string().len(), 26);
    }

    #[test]
    fn test_job_id_serde_from_ulid() {
        let config = r#"
        jobs:
            - type: "http"
              id: "01ARZ3NDEKTSV4RRWETS2EGZ5M"
              pretty_name: "5s failing endpoint"
              endpoint: "https://my_endpoint.com/api/v1/healthy?system_only=true"
              interval: 5
              headers:
                Authorization: "Bearer..."
            "#;
        let root: DummyRootJob = serde_yaml::from_str(config).unwrap();
        assert_eq!(&root.jobs[0].id.to_string(), "01ARZ3NDEKTSV4RRWETS2EGZ5M");
    }
}
