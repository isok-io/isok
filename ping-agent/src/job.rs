use log::info;
use slab::Slab;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::task::LocalPoolHandle;
use uuid::Uuid;

pub use ping_data::check::CheckKind;
use ping_data::check::CheckOutput;

pub use ping_data::pulsar_commands::Command;
use ping_data::pulsar_commands::CommandKind;

use crate::http::{HttpClient, HttpContext};
use crate::magic_pool::MagicPool;
use crate::warp10::Warp10Data;

/// Ressources shared between jobs
pub struct JobRessources {
    pub http_pool: MagicPool<HttpClient>,
}

impl Default for JobRessources {
    fn default() -> Self {
        JobRessources {
            http_pool: MagicPool::new(10),
        }
    }
}

/// Different job contexts, mapped from [`CheckKind`]
#[derive(Debug, Clone)]
pub enum JobKind {
    Http(HttpContext),
    Dummy,
}

/// Everything you need to execute a job
#[derive(Debug, Clone)]
pub struct Job {
    id: Uuid,
    kind: JobKind,
}

impl Job {
    /// Execute a dummy job (for test only)
    fn execute_dummy(id: &Uuid, task_pool: &LocalPoolHandle) {
        let borowed_id = id.clone();
        let process = async move {
            info!("Check dummy {borowed_id} has been trigerred !");
        };

        info!("Triggering check {id}...");
        task_pool.spawn_pinned(|| process);
    }

    /// Execute a http job
    fn execute_http(
        id: &Uuid,
        ctx: HttpContext,
        task_pool: &LocalPoolHandle,
        resources: &mut JobRessources,
        warp10_snd: mpsc::Sender<warp10::Data>,
    ) {
        let borowed_id = id.clone();
        let borowed_req = ctx.clone().into();
        let checkout = resources.http_pool.get();
        let process = async move {
            let http_result = checkout.send(borowed_req).await;

            match http_result {
                Some(res) => {
                    for d in res.data(borowed_id.clone()) {
                        warp10_snd.send(d).await;
                    }
                    info!(
                        "Check http {borowed_id} has been trigerred with status {} and response time {} !",
                        res.status
                        , res.request_time.as_millis()
                    );
                }
                None => todo!(),
            };
        };

        info!("Triggering check http {id} at {} ...", ctx.url());
        task_pool.spawn_pinned(|| process);
    }

    /// Execute a job
    pub fn execute(
        &self,
        task_pool: &LocalPoolHandle,
        resources: &mut JobRessources,
        warp10_snd: mpsc::Sender<warp10::Data>,
    ) {
        match &self.kind {
            JobKind::Dummy => Self::execute_dummy(&self.id, task_pool),
            JobKind::Http(ctx) => {
                Self::execute_http(&self.id, ctx.clone(), task_pool, resources, warp10_snd)
            }
        }
    }
}

impl From<CheckOutput> for Job {
    fn from(value: CheckOutput) -> Self {
        let kind = match value.kind {
            CheckKind::Http(http) => JobKind::Http(HttpContext::from(http)),
            _ => JobKind::Dummy,
        };

        Self { id: value.id, kind }
    }
}

pub struct JobLocation {
    frequency: Duration,
    offset: usize,
    position: usize,
}

pub struct JobScheduler {
    fill_cursor: usize,
    empty_slot: Vec<usize>,
    jobs: Arc<Mutex<Vec<Slab<Job>>>>,
    process: JoinHandle<()>,
}

impl JobScheduler {
    pub fn new(
        range: usize,
        wait: Duration,
        resources: Arc<Mutex<JobRessources>>,
        warp10_snd: mpsc::Sender<warp10::Data>,
        task_pool_size: usize,
    ) -> Self {
        let jobs = {
            let mut res: Vec<Slab<Job>> = Vec::with_capacity(range);

            for _ in 0..range {
                res.push(Slab::new())
            }
            res
        };

        let jobs = Arc::new(Mutex::new(jobs));
        let job_list = Arc::clone(&jobs);

        let process = async move {
            let task_pool = LocalPoolHandle::new(task_pool_size);
            let warp10_snd: mpsc::Sender<warp10::Data> = warp10_snd;
            let mut time_cursor = 0;

            loop {
                if let Ok(mut jl) = job_list.lock() {
                    for (_, j) in &mut jl[time_cursor] {
                        if let Ok(mut resources) = resources.lock() {
                            j.execute(&task_pool, &mut resources, warp10_snd.clone())
                        }
                    }
                }

                std::thread::sleep(wait);
                if time_cursor + 1 == range {
                    time_cursor = 0;
                } else {
                    time_cursor += 1;
                }
            }
        };

        Self {
            fill_cursor: 0,
            empty_slot: Vec::new(),
            jobs,
            process: tokio::task::spawn(process),
        }
    }

    pub fn add_job(&mut self, job: Job) -> (usize, usize) {
        let mut cursor = self.fill_cursor;
        let mut position = 0;
        if let Ok(mut jobs) = self.jobs.lock() {
            if self.empty_slot.is_empty() {
                position = jobs[cursor].insert(job);
                if self.fill_cursor + 1 == jobs.len() {
                    self.fill_cursor = 0;
                } else {
                    self.fill_cursor += 1;
                }
            } else {
                cursor = self.empty_slot.pop().unwrap();
                position = jobs[cursor].insert(job);
            }
        }

        (cursor, position)
    }
}

/// App main state handling pulsar commands ([`Command`]), storing jobs ([`Job`]) and job ressources ([`JobRessources`])
pub struct JobsHandler {
    resources: Arc<Mutex<JobRessources>>,
    checks: HashMap<Uuid, JobLocation>,
    jobs: HashMap<Duration, JobScheduler>,
    scheduler_task_pool_size: usize,
    warp10_snd: mpsc::Sender<warp10::Data>,
}

impl JobsHandler {
    pub fn new(
        resources: JobRessources,
        warp10_snd: mpsc::Sender<warp10::Data>,
        scheduler_task_pool_size: usize,
    ) -> Self {
        Self {
            resources: Arc::new(Mutex::new(resources)),
            checks: HashMap::new(),
            jobs: HashMap::new(),
            scheduler_task_pool_size,
            warp10_snd,
        }
    }

    pub fn handle_command(&mut self, cmd: Command) {
        match cmd.kind() {
            CommandKind::Add(a) => self.add_check(&a.check),
            CommandKind::Remove(r) => todo!(),
        }
    }

    pub fn add_check(&mut self, c: &CheckOutput) {
        let frequency = c.interval;

        if !self.jobs.contains_key(&frequency) {
            self.jobs.insert(
                frequency,
                JobScheduler::new(
                    frequency.as_secs() as usize,
                    Duration::from_secs(1),
                    Arc::clone(&self.resources),
                    self.warp10_snd.clone(),
                    self.scheduler_task_pool_size,
                ),
            );
        }

        let (offset, position) = self
            .jobs
            .get_mut(&frequency)
            .unwrap()
            .add_job(Job::from(c.clone()));

        self.checks.insert(
            c.id,
            JobLocation {
                frequency,
                offset,
                position,
            },
        );
    }
}
