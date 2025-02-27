use isok_agent::config::{Config as AgentConfig, ConfigCheckAdapter, ResultSenderAdapter};
use isok_agent::jobs::Job;
use isok_broker::config::{
    ApiConfig, Config as BrokerConfig, Error as BrokerError, KafkaConfig, Transport,
};
use isok_broker::run;
use once_cell::sync::Lazy;
use prost::Message;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::mocking::MockCluster;
use rdkafka::producer::DefaultProducerContext;
use rdkafka::{ClientConfig, ClientContext};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use ulid::Ulid;

pub static NEXT_PORT: AtomicUsize = AtomicUsize::new(24000);

pub static TRACING: Lazy<()> = Lazy::new(|| {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            "isok_agent=debug,isok_broker=debug,isok_data=debug,rdkafka=debug,librdkafka=debug",
        )
        .init()
});

pub struct BrokerTestingRunner<'c, T: ClientContext> {
    pub config: BrokerConfig,
    mock_cluster: MockCluster<'c, T>,
    broker_handle: Option<JoinHandle<Result<(), BrokerError>>>,
    pub listening_port: u16,
}

impl BrokerTestingRunner<'static, DefaultProducerContext> {
    pub fn new() -> Self {
        let mock_cluster = MockCluster::new(1).expect("Failed to create mock cluster");
        mock_cluster
            .create_topic("test", 1, 1)
            .expect("Failed to create topic");
        let kafka_config = KafkaConfig {
            topic: Self::generate_topic_name(),
            properties: HashMap::from([
                (
                    "bootstrap.servers".to_string(),
                    mock_cluster.bootstrap_servers(),
                ),
                // We set batch to 1, in order to send the message immediately
                // in case of tests
                ("batch.num.messages".to_string(), "1".to_string()),
            ]),
        };
        let port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u16;
        let config = BrokerConfig {
            transport: Transport::Kafka(kafka_config),
            api: ApiConfig {
                listen_address: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
            },
        };
        Self {
            config,
            mock_cluster,
            broker_handle: None,
            listening_port: port,
        }
    }

    fn generate_topic_name() -> String {
        format!("test-{}", Ulid::new().to_string())
    }

    pub fn start_broker(mut self) -> Self {
        let config = self.config.clone();
        self.broker_handle = Some(tokio::spawn(async move { run(config).await }));
        self
    }

    pub fn get_topic_consumer(&self) -> StreamConsumer {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", self.mock_cluster.bootstrap_servers())
            .set("group.id", "test_kafka_message_integrity")
            .set("fetch.wait.max.ms", "1000")
            .create()
            .expect("Consumer creation failed");

        let topic = match self.config.transport {
            Transport::Kafka(ref kafka_config) => &kafka_config.topic,
            _ => panic!("Not supported"),
        };

        consumer
            .subscribe(&[topic])
            .expect("Can't subscribe to specified topics");
        consumer
    }
}

pub struct AgentTestingRunner {
    config: AgentConfig,
}

impl Default for AgentTestingRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentTestingRunner {
    pub fn new() -> Self {
        Self {
            config: AgentConfig::default(),
        }
    }

    pub fn add_check(mut self, check: Job) -> Self {
        if let ConfigCheckAdapter::Static(adapter) = &mut self.config.check_config_adapter {
            adapter.checks.push(check);
        }
        self
    }

    pub fn use_socket_sender(mut self, path: PathBuf) -> Self {
        self.config.result_sender_adapter =
            ResultSenderAdapter::Socket(isok_agent::config::SocketConfig { path });
        self
    }

    pub fn create_path_socket_listener<T: Message + Default + 'static>(
        path: PathBuf,
    ) -> (UnboundedReceiver<T>, JoinHandle<()>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            let listener = UnixListener::bind(&path).unwrap();
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read message length
            let mut len_bytes = [0u8; 8];
            stream.read_exact(&mut len_bytes).await.unwrap();
            let len = u64::from_be_bytes(len_bytes) as usize;

            // Read message bytes
            let mut buffer = vec![0u8; len];
            stream.read_exact(&mut buffer).await.unwrap();

            // Decode the message
            let message = T::decode(&buffer[..]).unwrap();
            tx.send(message).unwrap();
        });
        (rx, handle)
    }

    pub fn create_tcp_socket_listener<T: Message + Default + 'static>(
        port: u16,
    ) -> (UnboundedReceiver<T>, JoinHandle<()>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .unwrap();
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).await.unwrap();
                let message = T::decode(&buffer[..]).unwrap();
                tx.send(message).unwrap();
            }
        });
        (rx, handle)
    }

    pub async fn start_threads(self) {
        tracing::info!("TESTS - Starting agent");
        isok_agent::run(self.config)
            .await
            .expect("Unable to run agent");
        tracing::info!("TESTS - Agent stopped");
    }

    pub fn run(self) -> JoinHandle<()> {
        tokio::spawn(self.start_threads())
    }
}
