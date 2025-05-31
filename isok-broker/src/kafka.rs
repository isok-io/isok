use crate::Result;
use crate::config::KafkaConfig;
use crate::errors::Error;
use isok_data::messages::{CheckResult, Message};
use rdkafka::ClientConfig;
use rdkafka::config::FromClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::time::Duration;

const RECORD_PRODUCE_TIMEOUT: Duration = Duration::from_secs(3);

pub struct Kafka {
    producer: FutureProducer,
    topic: String,
}

impl Kafka {
    pub fn new(config: KafkaConfig) -> Result<Self> {
        let topic = config.topic.clone();

        let mut producer = ClientConfig::new();

        for (key, value) in config.properties {
            producer.set(key, value);
        }

        let producer = FutureProducer::from_config(&producer)?;
        Ok(Kafka { producer, topic })
    }

    pub async fn process_result(&self, result: &CheckResult) -> Result<()> {
        let mut buffer = Vec::new();
        result
            .encode(&mut buffer)
            .map_err(|error| Error::Encode(error.to_string()))?;
        let record = FutureRecord::to(&self.topic).payload(&buffer).key("");

        self.producer
            .send(record, RECORD_PRODUCE_TIMEOUT)
            .await
            .map_err(|e| e.0)?;
        Ok(())
    }
}
