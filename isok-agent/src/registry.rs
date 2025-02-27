use std::path::PathBuf;
use std::time::Duration;

use dashmap::DashMap;
use figment::providers::{Format, Yaml};
use figment::Figment;
use isok_data::JobPrettyName;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::batch_sender::JobResult;
use crate::errors::Result;
use crate::jobs::Job;
use crate::state::JobState;

pub struct JobRegistry {
    jobs: DashMap<JobPrettyName, JobState>,
}

impl JobRegistry {
    pub fn from_configuration_file(path_buf: PathBuf) -> Result<JobRegistry> {
        #[derive(Debug, Deserialize)]
        struct Wrapper {
            checks: Vec<Job>,
        }

        let mut registry = JobRegistry::new();

        let jobs = Figment::new()
            .merge(Yaml::file(path_buf))
            .extract::<Wrapper>()?;

        for job in jobs.checks {
            registry.append(job);
        }

        Ok(registry)
    }

    pub fn from_static_config(jobs: Vec<Job>) -> Result<JobRegistry> {
        let mut registry = JobRegistry::new();

        for job in jobs {
            registry.append(job);
        }

        Ok(registry)
    }
}

impl JobRegistry {
    pub(crate) fn new() -> JobRegistry {
        JobRegistry {
            jobs: Default::default(),
        }
    }

    fn append(&mut self, job: Job) {
        self.jobs
            .insert(JobPrettyName::new(job.pretty_name()), JobState::new(job));
    }

    pub(crate) async fn execute(&self, tx: UnboundedSender<JobResult>) {
        loop {
            let current_time = Instant::now();

            for mut job in self.jobs.iter_mut() {
                if job.next_run() > current_time {
                    continue;
                }
                tracing::debug!(job_name = ?job.key(), "Job execution");

                let next_run = current_time + job.interval();
                job.set_next_run(next_run);

                if let Err(error) = job.execute(tx.clone()).await {
                    // TODO: job should be dequeued
                    tracing::error!("Job failed to execute: {error}");
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
