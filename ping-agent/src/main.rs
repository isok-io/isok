pub mod http;
pub mod icmp;
pub mod job;
pub mod pulsar_client;
pub mod tcp;

pub use env_logger::{Builder as Logger, Env};
pub use log::{error, info};
pub use std::str::FromStr;
pub use tokio::runtime;

fn env_panic(env: &'static str) -> impl Fn(std::env::VarError) -> String {
    move |e| {
        panic!("{env} is not set ({})", e);
    }
}

fn env_get(env: &'static str) -> String {
    std::env::var(env).unwrap_or_else(env_panic(env))
}

fn env_parse_panic<T: FromStr>(env: &'static str, val: String) -> impl Fn(T::Err) -> T {
    move |_| {
        panic!("can't parse {env} ({val})");
    }
}

fn env_get_num<T: FromStr>(env: &'static str, other: T) -> T {
    match std::env::var(env) {
        Ok(v) => v.parse::<T>().unwrap_or_else(env_parse_panic::<T>(env, v)),
        Err(_) => other,
    }
}

// fn get_dns_resolver() -> SocketAddr {
//     let env = std::env::var("DNS_RESOLVER");
// }

pub fn init_logger() {
    let env = Env::new().filter_or("LOG_LEVEL", "info");
    Logger::from_env(env).init();
}

async fn main_process() {}

fn main() {
    init_logger();

    let job_number = env_get_num("JOBS", 1);

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(job_number)
        .build()
        .expect("tokio runtime spawn");

    info!("Starting agent with {job_number} jobs...");
    runtime.block_on(main_process());
}
