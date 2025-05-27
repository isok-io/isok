use std::{net::IpAddr, pin::Pin};

use tokio::{
    runtime,
    sync::mpsc::{UnboundedSender, unbounded_channel},
};
use tonic::client::GrpcService;
use tracing::error;

use isok_data::messages;

/// Get env var and parse it
pub fn env_get<T>(env: &'static str) -> Option<T>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    if let Ok(value) = std::env::var(env) {
        match value.parse::<T>() {
            Ok(parsed_value) => Some(parsed_value),
            Err(err) => {
                error!("Unable to parse environment variable {env} : {err}");
                std::process::exit(1);
            }
        }
    } else {
        None
    }
}

pub async fn offload_task() {}

pub struct GrpcService {
    pub tx: UnboundedSender<messages::CheckResult>,
}

impl tower::Service<tonic::Request<messages::CheckResult>> for GrpcService {
    type Response = tonic::Response<()>;

    type Error = core::convert::Infallible;

    type Future= Pin<Box<dyn Future<Output = Result<Self::Response,core::convert::Infallible>>>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn call(&mut self, req: tonic::Request<messages::CheckResult>) -> Self::Future {

         let tx = self.tx.clone();
        Box::pin(async move {
            tx.send(req.into_inner());
            Ok(tonic::Response::new(()))
        })

    }


    // type Response = ();

    // type Future =
    //     Pin<Box<dyn Future<Output = Result<tonic::Response<Self::Response>, tonic::Status>>>>;

    // fn call(&mut self, request: tonic::Request<messages::CheckResult>) -> Self::Future {
    //     let tx = self.tx.clone();
    //     Box::pin(async move {
    //         tx.send(request.into_inner());
    //         Ok(tonic::Response::new(()))
    //     })
    // }
}

impl tonic::server::NamedService for GrpcService {
    const NAME: &'static str = "/";
}

pub async fn main_process() {
    let address = env_get("ADDRESS").unwrap_or(IpAddr::from([0, 0, 0, 0]));
    let port = env_get("PORT").unwrap_or(8080u16);

    let (tx, rx) = unbounded_channel();

    let server = tonic::transport::Server::builder().add_service(GrpcService { tx});
        .serve(std::net::SocketAddr::new(address, port)) 

    tokio::select! {
        _ = server.await => {}
    }
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

    let worker_threads: Option<usize> = env_get("WORKER_THREADS");

    let mut runtime = runtime::Builder::new_multi_thread();
    if let Some(worker_threads) = worker_threads {
        runtime.worker_threads(worker_threads);
    }

    let runtime = match runtime.enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            error!("Unable to start tokio runtime : {err}");
            std::process::exit(1);
        }
    };

    runtime.block_on(main_process())
}
