use log::{error, info, warn};
use slab::Slab;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::task::LocalPoolHandle;
use uuid::Uuid;

pub use ping_data::check::CheckKind;
use ping_data::check::CheckOutput;
use crate::redis::{RedisClient, RedisContext};
pub use ping_data::pulsar_commands::Command;
use ping_data::pulsar_commands::CommandKind;

use crate::http::{HttpClient, HttpContext, HttpResult};
use crate::magic_pool::MagicPool;
use crate::warp10::Warp10Data;

/// Ressources shared between jobs
pub struct JobRessources {
    pub http_pool: MagicPool<HttpClient>,
    pub redis_client: RedisClient,
}

impl Default for JobRessources {
    fn default() -> Self {
        JobRessources {
            http_pool: MagicPool::with_cappacity(1000, 20),
            redis_client: RedisClient::new("redis://127.0.0.1/"),
        }
    }
}

/// Different job contexts, mapped from [`CheckKind`]
#[derive(Debug, Clone)]
pub enum JobKind {
    Http(HttpContext),
    Redis(RedisContext),
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
                None => {
                    let res = HttpResult {
                        datetime: OffsetDateTime::now_utc(),
                        request_time: Duration::from_millis(i64::MAX as u64),
                        status: 500,
                    };

                    for d in res.data(borowed_id.clone()) {
                        warp10_snd.send(d).await;
                    }

                    info!("Check http {borowed_id} has been trigerred and timed out.");
                }
            };
        };

        info!("Triggering check http {id} at {} ...", ctx.url());
        task_pool.spawn_pinned(|| process);
    }
/// Execute a redis job
    fn execute_redis(
        id: &Uuid,
        ctx: &RedisContext,
        task_pool: &LocalPoolHandle,
        resources: &JobRessources,
        warp10_snd: mpsc::Sender<warp10::Data>,
    ) {
        let borrowed_id = id.clone();
        let borrowed_ctx = ctx.clone();
        let redis_client = &resources.redis_client;
    
        let process = async move {
            let redis_result = redis_client.send_redis(&borrowed_ctx).await;
            
            match redis_result {
                Some(res) => {
                    for d in res.data(borrowed_id) {
                        let _ =warp10_snd.send(d).await;
                    }
                    info!(
                        "Redis check {borrowed_id} triggered with success: {} and response time: {} ms",
                        res.success, res.process_time.as_millis()
                    );
                }
                None => {
                    info!("Redis check {borrowed_id} failed or timed out.");
                }
            };
        };
    
        info!("Triggering Redis check {id} ");
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
            JobKind::Http(ctx) => Self::execute_http(&self.id, ctx.clone(), task_pool, resources, warp10_snd.clone()),
            JobKind::Redis(ctx) => Self::execute_redis(&self.id, ctx, task_pool, resources, warp10_snd.clone()),
        }
        
    }
}

impl From<CheckOutput> for Job {
    fn from(value: CheckOutput) -> Self {
        let kind = match value.kind {
            CheckKind::Http(http) => JobKind::Http(HttpContext::from(http)),
            CheckKind::Redis(redis) => JobKind::Redis(RedisContext::from(redis)),
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
                let now = SystemTime::now();
                if let Ok(mut jl) = job_list.lock() {
                    for (_, j) in &mut jl[time_cursor] {
                        if let Ok(mut resources) = resources.lock() {
                            j.execute(&task_pool, &mut resources, warp10_snd.clone())
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
            CommandKind::Remove(id) => self.remove_check(id.clone()),
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
