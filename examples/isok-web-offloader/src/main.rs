use std::collections::HashMap;

use isok_agent::config::{Config as AgentConfig, ResultSenderAdapter};
use isok_agent::jobs::JobInnerConfig;
use isok_broker::config::{Config as BrokerConfig, Transport};
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::kafka::Kafka;
use tokio::task::JoinHandle;
use tracing::info;

use crate::api::run_offloader_api;

mod api;

const BROKER_PORT: u16 = 8001;
const OFFLOADER_PORT: u16 = 8080;

async fn run_kafka_container() -> ContainerAsync<Kafka> {
    Kafka::default()
        .start()
        .await
        .expect("Failed to start kafka container")
}

async fn run_broker(kafka_boostrap_servers: String) -> JoinHandle<Result<(), String>> {
    let broker_config = BrokerConfig {
        transport: Transport::Kafka(isok_broker::config::KafkaConfig {
            topic: "isok.agent.results".to_string(),
            properties: HashMap::from([("bootstrap.servers".to_string(), kafka_boostrap_servers)]),
        }),
        api: isok_broker::config::ApiConfig {
            listen_address: std::net::SocketAddr::new(
                std::net::Ipv4Addr::LOCALHOST.into(),
                BROKER_PORT,
            ),
        },
    };
    tokio::spawn(async move {
        isok_broker::run(broker_config)
            .await
            .map_err(|e| format!("Failed to run broker: {:?}", e))
    })
}

async fn run_agent(broker_endpoint: String) -> JoinHandle<Result<(), String>> {
    let job_inner_config_alpha =
        isok_agent::jobs::http::HttpJob::new("https://google.com".to_string());
    let job_inner_config_beta =
        isok_agent::jobs::http::HttpJob::new("https://github.com".to_string());
    let job_inner_config_gamma =
        isok_agent::jobs::http::HttpJob::new("https://zorglubqsd.com".to_string());

    let agent_config = AgentConfig {
        result_sender_adapter: ResultSenderAdapter::Broker(isok_agent::config::BrokerConfig {
            main_broker: broker_endpoint,
            fallback_brokers: vec![],
            agent_id: Some("isok-agent".to_string()),
            zone: "dev".to_string(),
            region: "localhost".to_string(),
            batch: 100,
            batch_interval: 10,
        }),
        check_config_adapter: isok_agent::config::ConfigCheckAdapter::Static(
            isok_agent::config::StaticConfigAdapter {
                checks: vec![
                    isok_agent::jobs::Job::new(
                        std::time::Duration::from_secs(10),
                        JobInnerConfig::Http(job_inner_config_alpha),
                        "google".to_string(),
                    ),
                    isok_agent::jobs::Job::new(
                        std::time::Duration::from_secs(10),
                        JobInnerConfig::Http(job_inner_config_beta),
                        "github".to_string(),
                    ),
                    isok_agent::jobs::Job::new(
                        std::time::Duration::from_secs(10),
                        JobInnerConfig::Http(job_inner_config_gamma),
                        "zorglub".to_string(),
                    ),
                ],
            },
        ),
    };
    tokio::spawn(async move {
        isok_agent::run(agent_config)
            .await
            .map_err(|e| format!("Failed to run agent: {:?}", e))
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter("isok_web_offloader=info")
        // .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Spawning the kafka container");
    let kafka = run_kafka_container().await;
    let kafka_ip = kafka.get_host().await.expect("Failed to get kafka ip");
    let kafka_port = kafka
        .get_host_port_ipv4(9093)
        .await
        .expect("Failed to get kafka port");
    let kafka_boostrap_servers = format!("{}:{}", kafka_ip, kafka_port.to_string());

    info!("Spawning the broker");
    let _ = run_broker(kafka_boostrap_servers.clone()).await;

    info!("Spawning the agent");
    let _ = run_agent(format!("http://localhost:{}", BROKER_PORT)).await;

    info!("Spawning the offloader API with its dashboard");
    let _ = run_offloader_api(kafka_boostrap_servers).await;

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    info!(
        "You can now access the dashboard at http://localhost:{}",
        OFFLOADER_PORT
    );
    info!("You'll be able to see in live results from the agents.");
    info!("Use CTRL+C to stop the offloader");
    let _ = tokio::signal::ctrl_c().await;
    kafka.stop().await.expect("Failed to stop kafka container");

    Ok(())
}
