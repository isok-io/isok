use rand::Rng;
use rand::seq::IteratorRandom;
use slab::Slab;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::usize;
use tokio::sync::RwLock;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time;
use uuid::Uuid;

use crate::job::{Job, JobResult};

#[derive(Debug)]
pub struct JobLocation {
    /// Offset in the Vec of jobs
    offset: usize,
    /// Index in the Slab of jobs at an offset
    index: usize,
}

type InsertFunction = fn(
    job: Job,
    &mut Vec<Slab<Job>>,
    &mut (Duration, usize),
) -> (JobLocation, Option<SchedulerFunction>);

#[derive(Debug)]
pub enum SchedulerFunction {
    Divide,
    BestOfN,
    BestOfK,
}

impl SchedulerFunction {
    pub const fn insert_function(self) -> InsertFunction {
        match self {
            SchedulerFunction::Divide => Self::divide,
            SchedulerFunction::BestOfN => Self::best_of_n,
            SchedulerFunction::BestOfK => Self::best_of_k,
        }
    }

    fn divide(
        job: Job,
        jobs: &mut Vec<Slab<Job>>,
        loop_interval_iter: &mut (Duration, usize),
    ) -> (JobLocation, Option<SchedulerFunction>) {
        let mut next_scheduler_function = None;

        let (interval, iter) = loop_interval_iter;

        let mut slab = Slab::with_capacity(1);
        let index = slab.insert(job);

        if jobs.len() == jobs.capacity() - 1 && interval.as_millis() <= 500 {
            if jobs.len() < 16 {
                next_scheduler_function = Some(SchedulerFunction::BestOfN);
            } else {
                next_scheduler_function = Some(SchedulerFunction::BestOfK);
            }
        } else if jobs.len() == jobs.capacity() {
            jobs.reserve_exact(jobs.capacity());
            *interval = interval.div_f64(2.0);
            *iter = jobs.capacity();
        }

        let offset = jobs.len();
        jobs.push(slab);
        (JobLocation { offset, index }, next_scheduler_function)
    }

    fn best_of_n(
        job: Job,
        jobs: &mut Vec<Slab<Job>>,
        _loop_interval_iter: &mut (Duration, usize),
    ) -> (JobLocation, Option<SchedulerFunction>) {
        let best = jobs.iter_mut().enumerate().min_by_key(|(_i, s)| s.len());

        if let Some((offset, best_slab)) = best {
            let index = best_slab.insert(job);

            (JobLocation { offset, index }, None)
        } else {
            let offset = rand::rng().random_range(0..jobs.len());
            let index = jobs[offset].insert(job);

            (JobLocation { offset, index }, None)
        }
    }

    fn best_of_k(
        job: Job,
        jobs: &mut Vec<Slab<Job>>,
        _loop_interval_iter: &mut (Duration, usize),
    ) -> (JobLocation, Option<SchedulerFunction>) {
        // K c'est une constante ptn
        const K: usize = 10;
        let mut rng = rand::rng();
        let best = jobs
            .iter_mut()
            .enumerate()
            .choose_multiple(&mut rng, K)
            .into_iter()
            .min_by_key(|(_i, s)| s.len());

        if let Some((offset, best_slab)) = best {
            let index = best_slab.insert(job);

            (JobLocation { offset, index }, None)
        } else {
            let offset = rand::rng().random_range(0..jobs.len());
            let index = jobs[offset].insert(job);

            (JobLocation { offset, index }, None)
        }
    }
}

pub struct Scheduler {
    // TODO : use this
    #[allow(unused)]
    interval: Duration,
    jobs: Arc<RwLock<Vec<Slab<Job>>>>,
    jobs_locations: HashMap<Uuid, JobLocation>,
    loop_interval_iter: Arc<RwLock<(Duration, usize)>>,
    insert_function: InsertFunction,
}

impl Scheduler {
    pub fn new(interval: Duration) -> Self {
        const INITINAL_ITER: usize = 1;
        Scheduler {
            interval,
            jobs: Arc::new(RwLock::new(Vec::with_capacity(INITINAL_ITER))),
            jobs_locations: HashMap::new(),
            loop_interval_iter: Arc::new(RwLock::new((interval, INITINAL_ITER))),
            insert_function: (SchedulerFunction::Divide).insert_function(),
        }
    }

    #[inline]
    pub fn start(&mut self, snd: UnboundedSender<JobResult>) -> JoinHandle<()> {
        tokio::spawn(Self::job_loop(
            self.jobs.clone(),
            self.loop_interval_iter.clone(),
            snd,
        ))
    }

    pub async fn job_loop(
        jobs: Arc<RwLock<Vec<Slab<Job>>>>,
        loop_interval_iter: Arc<RwLock<(Duration, usize)>>,
        snd: UnboundedSender<JobResult>,
    ) {
        loop {
            let (interval, iter) = *loop_interval_iter.read().await;
            let mut ticker = time::interval(interval);

            ticker.tick().await;
            for offset in 0..iter {
                tokio::spawn(Self::launch_jobs(jobs.clone(), offset, snd.clone()));
                ticker.tick().await;
            }
        }
    }

    async fn launch_jobs(
        jobs: Arc<RwLock<Vec<Slab<Job>>>>,
        offset: usize,
        snd: UnboundedSender<JobResult>,
    ) {
        let jobs = jobs.read().await;

        if let Some(jobs_at_offset) = jobs.get(offset) {
            for (_index, job) in jobs_at_offset {
                tokio::spawn(job.clone().execute(snd.clone()));
            }
        }
    }

    pub async fn insert(&mut self, job: Job) {
        let jobs = &mut *self.jobs.write().await;
        let loop_interval_inter = &mut *self.loop_interval_iter.write().await;

        let job_uuid = job.check_id;
        let (job_location, sf) = (self.insert_function)(job, jobs, loop_interval_inter);

        self.jobs_locations.insert(job_uuid, job_location);
        if let Some(sf) = sf {
            self.insert_function = sf.insert_function();
        }
    }

    pub async fn delete(&mut self, check_id: Uuid) {
        if let Some(location) = self.jobs_locations.get(&check_id) {
            let jobs = &mut *self.jobs.write().await;
            if let Some(slab) = jobs.get_mut(location.offset) {
                slab.remove(location.index);
                self.jobs_locations.remove(&check_id);
            }
        }
    }
}
