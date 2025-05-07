mod job;
mod scheduler;

use job::{Job, JobKind};
use scheduler::Scheduler;
use std::time::Duration;
use tokio::runtime;
use tracing::{error, info};

use uuid::Uuid;

#[allow(unused)]
pub struct Check {
    check_id: Uuid,
    interval: Duration,
    url: String,
}

/// Get env var as number or panic, with a default number
pub fn env_get_or<T>(env: &'static str, other: T) -> T
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    if let Ok(value) = std::env::var(env) {
        match value.parse::<T>() {
            Ok(parsed_value) => parsed_value,
            Err(err) => {
                error!("Unable to parse environment variable {env} : {err}");
                std::process::exit(1);
            }
        }
    } else {
        other
    }
}

async fn main_process() {
    info!("Agent has started !");
}

/// Start logger
#[inline]
pub fn init_logger() {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_env_var("LOG_LEVEL")
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn main() {
    init_logger();

    let worker_threads = env_get_or("WORKER_THREADS", 1);
    let runtime = match runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            error!("Unable to start tokio runtime : {err}");
            std::process::exit(1);
        }
    };

    runtime.block_on(main_process())
}
