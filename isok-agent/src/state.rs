use std::ops::Deref;
use std::time::Duration;

use tokio::time::Instant;

use crate::jobs::Job;

// Own a job and it's last execution
#[derive(Debug)]
pub struct JobState {
    job: Job,
    next_run: Instant,
}

impl JobState {
    pub(crate) fn new(job: Job) -> JobState {
        JobState {
            next_run: Instant::now(),
            job,
        }
    }

    pub fn next_run(&self) -> Instant {
        self.next_run
    }

    pub fn interval(&self) -> Duration {
        self.job.interval()
    }

    pub fn set_next_run(&mut self, current_time: Instant) {
        self.next_run = current_time + self.interval();
    }
}

impl Deref for JobState {
    type Target = Job;
    fn deref(&self) -> &Self::Target {
        &self.job
    }
}
