use std::{collections::HashMap, time::Duration};

use uuid::Uuid;

use crate::scheduler::Scheduler;

pub struct State {
    checks: HashMap<Uuid, Duration>,
    schedulers: HashMap<Duration, Scheduler>,
}

impl State {}
