use crate::Result;
use crate::config::KafkaConfig;
use crate::exporter::Exporter;
use isok_data::messages::{CheckResult, Message};
use rdkafka::config::RDKafkaLogLevel;
use rdkafka::consumer::{Consumer, ConsumerContext, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::{ClientConfig, ClientContext, Message as KafkaMessage, TopicPartitionList};
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio::task::JoinSet;
use tracing::{debug, error, info, trace, warn};

struct LoggingConsumerContext;

impl ClientContext for LoggingConsumerContext {
    fn log(&self, level: RDKafkaLogLevel, fac: &str, log_message: &str) {
        match level {
            RDKafkaLogLevel::Emerg
            | RDKafkaLogLevel::Alert
            | RDKafkaLogLevel::Critical
            | RDKafkaLogLevel::Error => error!(target: "librdkafka", "{fac} {log_message}"),
            RDKafkaLogLevel::Warning => warn!(target: "librdkafka", "{fac} {log_message}"),
            RDKafkaLogLevel::Notice | RDKafkaLogLevel::Info => {
                info!(target: "librdkafka", "{fac} {log_message}")
            }
            RDKafkaLogLevel::Debug => debug!(target: "librdkafka", "{fac} {log_message}"),
        }
    }
}

impl ConsumerContext for LoggingConsumerContext {
    fn commit_callback(&self, result: KafkaResult<()>, offsets: &TopicPartitionList) {
        match result {
            Ok(_) => {
                debug!("Offsets committed successfully");
                trace!(?offsets)
            }
            Err(e) => warn!("Error while committing offsets: {}", e),
        };
    }
}

type LoggingConsumer = StreamConsumer<LoggingConsumerContext>;

pub struct Kafka {
    consumer: LoggingConsumer,
    topic: String,
}

impl Kafka {
    pub async fn new(
        config: KafkaConfig,
        exporter: Box<dyn Exporter + Send>,
        js: &mut JoinSet<Result<()>>,
        shutdown_rx: Receiver<()>,
    ) -> Result<Arc<Self>> {
        let ctx = LoggingConsumerContext;
        let mut consumer = ClientConfig::new();

        for (key, value) in config.properties {
            consumer.set(key, value);
        }

        let consumer: LoggingConsumer = consumer
            .set_log_level(RDKafkaLogLevel::Debug)
            .create_with_context(ctx)?;
        consumer.subscribe(&[&config.topic])?;

        let s = Arc::new(Self {
            consumer,
            topic: config.topic,
        });

        js.spawn(s.clone().worker(exporter, shutdown_rx));

        Ok(s)
    }

    async fn worker(
        self: Arc<Self>,
        exporter: Box<dyn Exporter + Send>,
        mut shutdown_rx: Receiver<()>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                Ok(message) = self.consumer.recv() => {
                    if let Some(payload) = message.payload() {
                        let check_result: isok_data::models::CheckResult = match CheckResult::decode(payload)
                            .and_then(|message| message.try_into().map_err(Into::into))
                        {
                            Ok(check_result) => check_result,
                            Err(error) => {
                                error!(?error, "Failed to decode message");
                                continue;
                            }
                        };
                        exporter.send_result(check_result, message.partition(), message.offset()).await;
                    } else {
                        warn!("Received empty message");
                    }
                }
                idx = exporter.get_commited() => {
                    for (partition, offset) in idx {
                        self.consumer.store_offset(&self.topic, partition, offset)?;
                    }

                }
                _ = shutdown_rx.changed() => {
                    info!("Shutting down");
                    self.consumer.unsubscribe();
                    self.consumer.unassign()?;
                    break;
                }
            }
        }

        Ok(())
    }
}
