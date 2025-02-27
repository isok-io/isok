use integration_tests::{BrokerTestingRunner, TRACING};
use isok_data::broker_rpc::broker_client::BrokerClient;
use isok_data::broker_rpc::{CheckBatchRequest, CheckResult};
use once_cell::sync::Lazy;
use prost::Message;
use rdkafka::Message as KafkaMessage;

#[tokio::test]
async fn test_kafka_message_integrity() {
    Lazy::force(&TRACING);
    let broker = BrokerTestingRunner::new().start_broker();

    let mut client = BrokerClient::connect(format!("http://localhost:{}", broker.listening_port))
        .await
        .expect("Failed to connect to broker");
    // @AlexandreBrg: Whenever using the mock cluster, and the batch size is at most 1,
    // the producer will only be produced (or consumer consume?) after a few seconds,
    // with at least 2 messages. Since MockCluster is an tech preview stuff, and librdkafka
    // is not yet stable on this, I assume it could be a bug, or misunderstanding of the
    // properties.
    tokio::spawn(async move {
        loop {
            let batch = CheckBatchRequest {
                tags: Some(isok_data::broker_rpc::Tags {
                    agent_id: "test".to_string(),
                    zone: "dev".to_string(),
                    region: "localhost".to_string(),
                }),
                events: vec![CheckResult {
                    id_ulid: "test".to_string(),
                    pretty_name: None,
                    run_at: None,
                    status: isok_data::broker_rpc::CheckJobStatus::Reachable as i32,
                    metrics: Default::default(),
                    tags: None,
                    details: Default::default(),
                    error: None,
                }],
                created_at: None,
            };
            client
                .batch_send(batch)
                .await
                .expect("Failed to send batch");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let consumer = broker.get_topic_consumer();
    let msg = consumer.recv().await.expect("Expected message");

    let result = CheckResult::decode(msg.payload().unwrap()).expect("Expected to decode message");
    assert_eq!(
        result.status,
        isok_data::broker_rpc::CheckJobStatus::Reachable as i32
    );
    assert_eq!(result.id_ulid, "test");
}
