use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use log::{error, info, warn};
use slab::Slab;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::task::LocalPoolHandle;
use uuid::Uuid;

pub use isok_data::check::CheckKind;
use isok_data::check::CheckOutput;
use isok_data::check_kinds::http::HttpFields;
pub use isok_data::pulsar_commands::Command;
use isok_data::pulsar_commands::CommandKind;
use isok_data::pulsar_messages::{CheckMessage, CheckResult, CheckType};

use crate::http::{HttpClient, HttpContext, HttpResult};
use crate::magic_pool::MagicPool;

/// Ressources shared between jobs
pub struct JobResources {
    pub http_pool: MagicPool<HttpClient>,
}

impl Default for JobResources {
    fn default() -> Self {
        JobResources {
            http_pool: MagicPool::with_capacity(1000, 20),
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
        resources: &mut JobResources,
        pulsar_sender: mpsc::Sender<CheckMessage>,
        agent_id: String,
    ) {
        let borrowed_id = id.clone();
        let borrowed_req = ctx.clone().into();
        let checkout = resources.http_pool.get();

        let process = async move {
            let http_result = checkout.run(borrowed_req).await;

            info!(
                "Check http {borrowed_id} has been trigerred with status {} and response time {} !",
                http_result.status,
                http_result.request_time.as_millis()
            );

            let check_result: CheckResult<HttpFields> = http_result.into();
            let check_message: CheckMessage = check_result.to_message(borrowed_id, agent_id);

            let _ = pulsar_sender.send(check_message).await;
        };

        info!("Triggering check http {id} at {} ...", ctx.url());
        task_pool.spawn_pinned(|| process);
    }

    /// Execute a job
    pub fn execute(
        &self,
        task_pool: &LocalPoolHandle,
        resources: &mut JobResources,
        pulsar_sender: mpsc::Sender<CheckMessage>,
        agent_id: String,
    ) {
        match &self.kind {
            JobKind::Dummy => Self::execute_dummy(&self.id, task_pool),
            JobKind::Http(ctx) => Self::execute_http(
                &self.id,
                ctx.clone(),
                task_pool,
                resources,
                pulsar_sender,
                agent_id,
            ),
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
    frequency: Duration,
    fill_cursor: usize,
    empty_slot: Vec<usize>,
    jobs: Arc<Mutex<Vec<Slab<Job>>>>,
    _process: JoinHandle<()>,
}

impl JobScheduler {
    pub fn new(
        range: usize,
        wait: Duration,
        resources: Arc<Mutex<JobResources>>,
        pulsar_sender: mpsc::Sender<CheckMessage>,
        task_pool_size: usize,
        agent_id: String,
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
            let pulsar_sender: mpsc::Sender<CheckMessage> = pulsar_sender;
            let mut time_cursor = 0;

            loop {
                let now = SystemTime::now();
                if let Ok(mut jl) = job_list.lock() {
                    for (_, j) in &mut jl[time_cursor] {
                        if let Ok(mut resources) = resources.lock() {
                            j.execute(
                                &task_pool,
                                &mut resources,
                                pulsar_sender.clone(),
                                agent_id.clone(),
                            )
                        }
                    }
                }

                if time_cursor + 1 == range {
                    time_cursor = 0;
                } else {
                    time_cursor += 1;
                }
                std::thread::sleep(wait - now.elapsed().expect("System should have time"));
            }
        };

        Self {
            frequency: Duration::from_secs(range as u64),
            fill_cursor: 0,
            empty_slot: Vec::new(),
            jobs,
            _process: tokio::task::spawn(process),
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

    pub fn remove_job(&mut self, jl: JobLocation, id: Uuid) {
        if let Ok(mut jobs) = self.jobs.lock() {
            let slab = match jobs.get_mut(jl.offset) {
                Some(s) => s,
                None => {
                    error!(
                        "Could not find a job {id} at jobscheduler {} at offset {}",
                        self.frequency.as_secs(),
                        jl.offset
                    );
                    return;
                }
            };

            if slab.contains(jl.position) {
                slab.remove(jl.position);
                self.empty_slot.push(jl.offset);
            } else {
                error!(
                    "Could not find a job {id} at jobscheduler {} at offset {} at position {}",
                    self.frequency.as_secs(),
                    jl.offset,
                    jl.position,
                );
            }
        } else {
            error!("Could not lock job mutex when trying to remove job {id}");
        }
    }
}

/// App main state handling pulsar commands ([`Command`]), storing jobs ([`Job`]) and job ressources ([`JobResources`])
pub struct JobsHandler {
    resources: Arc<Mutex<JobResources>>,
    checks: HashMap<Uuid, JobLocation>,
    jobs: HashMap<Duration, JobScheduler>,
    scheduler_task_pool_size: usize,
    pulsar_sender: mpsc::Sender<CheckMessage>,
    agent_id: String,
}

impl JobsHandler {
    pub fn new(
        resources: JobResources,
        pulsar_sender: mpsc::Sender<CheckMessage>,
        scheduler_task_pool_size: usize,
        agent_id: String,
    ) -> Self {
        Self {
            resources: Arc::new(Mutex::new(resources)),
            checks: HashMap::new(),
            jobs: HashMap::new(),
            scheduler_task_pool_size,
            pulsar_sender,
            agent_id,
        }
    }

    pub fn handle_command(&mut self, cmd: Command) {
        match cmd.kind() {
            CommandKind::Add(a) => self.add_check(&a.check),
            CommandKind::Remove(id) => self.remove_check(id.clone()),
        }
    }

    pub fn add_check(&mut self, c: &CheckOutput) {
        let frequency = Duration::from_secs(c.interval as u64);

        if !self.jobs.contains_key(&frequency) {
            self.jobs.insert(
                frequency,
                JobScheduler::new(
                    frequency.as_secs() as usize,
                    Duration::from_secs(1),
                    Arc::clone(&self.resources),
                    self.pulsar_sender.clone(),
                    self.scheduler_task_pool_size,
                    self.agent_id.clone(),
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

        info!("Check {} successfully added and scheduled !", c.id);
    }

    pub fn remove_check(&mut self, id: Uuid) {
        if !self.checks.contains_key(&id) {
            warn!("Trying to removing unkown check {id}");
            return;
        }

        let jl = self
            .checks
            .remove(&id)
            .expect("Should have a key after checking if key exist");

        self.jobs
            .get_mut(&jl.frequency)
            .expect("Should have key here")
            .remove_job(jl, id);

        info!("Check {} successfully removed !", &id);
    }
}
