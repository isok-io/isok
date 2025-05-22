use std::{collections::HashMap, time::Duration};

use tokio::sync::mpsc::UnboundedSender;
use tracing::error;
use uuid::Uuid;

use crate::scheduler::Scheduler;
use isok_data::models::{Check, CheckResult};

pub struct AgentState {
    pub checks: HashMap<Uuid, Duration>,
    pub schedulers: HashMap<Duration, Scheduler>,
    pub snd: UnboundedSender<CheckResult>,
}

impl AgentState {
    pub fn new(snd: UnboundedSender<CheckResult>) -> Self {
        AgentState {
            checks: HashMap::new(),
            schedulers: HashMap::new(),
            snd,
        }
    }

    pub async fn insert_check(&mut self, check: Check) {
        let check_id = check.id;
        let check_interval = check.interval;
        if let Some(s) = self.schedulers.get_mut(&check.interval) {
            s.insert(check.into()).await;
        } else {
            let mut s = Scheduler::new(check_interval);
            s.insert(check.into()).await;
            s.start(self.snd.clone());
            self.schedulers.insert(check_interval, s);
        }
        self.checks.insert(check_id, check_interval);
    }

    pub async fn delete_check(&mut self, check_id: Uuid) -> bool {
        if let Some(interval) = self.checks.remove(&check_id) {
            if let Some(s) = self.schedulers.get_mut(&interval) {
                s.delete(check_id).await;
            } else {
                error!("Cannot remove check {check_id} from scheduler : scheduler not found");
            }
            true
        } else {
            false
        }
    }
}
