use std::time::Duration;

use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use isok_data::JobId;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::batch_sender::JobResult;
use crate::jobs::http::HttpJob;
use crate::jobs::tcp::TcpJob;

pub mod http;
pub mod tcp;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(tag = "type")]
#[enum_dispatch(Execute)]
pub enum JobInnerConfig {
    #[serde(rename = "tcp")]
    Tcp(TcpJob),
    #[serde(rename = "http")]
    Http(HttpJob),
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

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
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
        match &self.inner {
            JobInnerConfig::Tcp(job) => job.execute(&mut job_result).await,
            JobInnerConfig::Http(job) => job.execute(&mut job_result).await,
        }?;

        if let Err(e) = tx.send(job_result) {
            tracing::error!("Job failed to properly send its result to channel {}", e);
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
}

#[async_trait]
#[enum_dispatch]
pub trait Execute {
    async fn execute(&self, job_result: &mut JobResult) -> Result<(), JobError>;
}

#[cfg(test)]
mod tests {
    use crate::jobs::{Job, JobInnerConfig};
    use std::time::Duration;

    use crate::jobs::http::HttpJob;
    use isok_data::JobId;
    use serde::{Deserialize, Serialize};

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
