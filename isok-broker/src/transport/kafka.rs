use std::time::Duration;

use isok_data::broker_rpc::CheckResult;
use prost::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;

use crate::config::KafkaConfig;
use crate::transport::{ResultTransport, TransportError};

static RECORD_PRODUCE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct KafkaMessageBroker {
    producer: FutureProducer,
    topic: String,
}

impl ResultTransport for KafkaMessageBroker {
    async fn process_result(&self, result: &CheckResult) -> Result<(), TransportError> {
        let mut buffer = Vec::new();
        result.encode(&mut buffer).unwrap();
        let record = FutureRecord::to(&self.topic)
            .payload(&buffer)
            .key(&result.id_ulid);

        self.producer
            .send(record, RECORD_PRODUCE_TIMEOUT)
            .await
            .map_err(|e| TransportError::BatchFatalError(format!("{:?}", e)))?;
        Ok(())
    }

    async fn health_check(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

impl KafkaMessageBroker {
    pub fn try_new(config: KafkaConfig) -> Result<Self, TransportError> {
        let topic = config.topic.clone();
        let producer = FutureProducer::try_from(config)?;
        Ok(KafkaMessageBroker { producer, topic })
    }
}

impl TryFrom<KafkaConfig> for FutureProducer {
    type Error = TransportError;

    fn try_from(value: KafkaConfig) -> Result<Self, Self::Error> {
        ClientConfig::from_iter(value.properties)
            .create()
            .map_err(TransportError::UnableToCreateProducer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KafkaConfig;
    use isok_data::broker_rpc::CheckJobStatus;
    use prost::Message;
    use rdkafka::consumer::{Consumer, StreamConsumer};
    use rdkafka::mocking::MockCluster;
    use rdkafka::Message as KafkaMessage;
    use std::collections::HashMap;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_kafka_message_integrity() {
        let topic = "test2";
        let mock_cluster = MockCluster::new(3).unwrap();

        mock_cluster
            .create_topic(topic, 32, 3)
            .expect("Failed to create topic");

        let config = KafkaConfig {
            topic: topic.to_string(),
            properties: HashMap::from([(
                "bootstrap.servers".to_string(),
                mock_cluster.bootstrap_servers(),
            )]),
        };

        let kafka =
            KafkaMessageBroker::try_new(config).expect("Failed to create Kafka message broker");
        let batch = vec![CheckResult {
            id_ulid: "test".to_string(),
            pretty_name: None,
            run_at: None,
            status: CheckJobStatus::Reachable.into(),
            metrics: Default::default(),
            tags: None,
            details: Default::default(),
            error: None,
        }];

        let batch_thread = batch.clone();
        tokio::spawn(async move {
            // @AlexandreBrg: There is an issue with the mocked cluster,
            // if send <100k messages, the consumer will not receive any message.
            // I personally think it's linked to queue buffering properties, tried
            // multiple things to fix it, but nothing worked.
            // Related issue: https://github.com/fede1024/rust-rdkafka/issues/629
            let mut i: usize = 0;
            loop {
                kafka
                    .process_batch(&batch_thread)
                    .await
                    .expect("Failed to process batch");
                i += 1;
                if i > 100000 {
                    break;
                }
            }
        });

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", mock_cluster.bootstrap_servers())
            .set("group.id", "test_kafka_message_integrity")
            .create()
            .expect("Consumer creation failed");

        consumer
            .subscribe(&[topic])
            .expect("Can't subscribe to specified topics");

        let msg = consumer.recv().await.expect("Expected message");
        let payload = msg.payload().expect("Expected payload");

        let message = CheckResult::decode(payload).expect("Expected decode to succeed");
        assert_eq!(message, batch[0]);
    }
}
