use integration_tests::{AgentTestingRunner, NEXT_PORT, TRACING};
use isok_agent::jobs::http::HttpJob;
use isok_agent::jobs::https::HttpsJob;
use isok_agent::jobs::tcp::TcpJob;
use isok_agent::jobs::{Job, JobInnerConfig};
use isok_data::broker_rpc::check_result::Details;
use isok_data::broker_rpc::CheckBatchRequest;
use once_cell::sync::Lazy;
use pretty_assertions::assert_eq;
use rustls_pki_types::DnsName;
use std::time::Duration;

#[tokio::test]
async fn test_agent_feedback_ok_http() {
    Lazy::force(&TRACING);
    let http_config = JobInnerConfig::Http(HttpJob::new("https://google.com".to_string()));
    let check = Job::new(Duration::from_secs(10), http_config, "google".to_string());
    let expected_id = check.id().to_string();

    let socket_path = tempfile::NamedTempFile::new().unwrap().path().to_path_buf();
    let (mut rx, _) =
        AgentTestingRunner::create_path_socket_listener::<CheckBatchRequest>(socket_path.clone());

    let agent = AgentTestingRunner::new()
        .add_check(check.clone())
        .use_socket_sender(socket_path.clone())
        .run();

    let check_result = rx.recv().await.unwrap();
    agent.abort();
    let rcv_check = check_result
        .events
        .first()
        .expect("Expected to receive at least one check result");
    assert_eq!(expected_id, rcv_check.id_ulid);
    assert_eq!(
        rcv_check.status,
        isok_data::broker_rpc::CheckJobStatus::Reachable as i32
    );
}

#[tokio::test]
async fn test_agent_feedback_ok_tcp() {
    let port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u16;
    let (_, _) = AgentTestingRunner::create_tcp_socket_listener::<CheckBatchRequest>(port);
    let tcp_config = JobInnerConfig::Tcp(TcpJob::new(format!("127.0.0.1:{}", port)));
    let check = Job::new(Duration::from_secs(10), tcp_config, "tcp".to_string());

    let socket_path = tempfile::NamedTempFile::new().unwrap().path().to_path_buf();
    let (mut rx, _) =
        AgentTestingRunner::create_path_socket_listener::<CheckBatchRequest>(socket_path.clone());

    let agent = AgentTestingRunner::new()
        .add_check(check.clone())
        .use_socket_sender(socket_path.clone())
        .run();

    let check_result = rx.recv().await.unwrap();
    agent.abort();
    let rcv_check = check_result
        .events
        .first()
        .expect("Expected to receive at least one check result");
    assert_eq!(
        rcv_check.status,
        isok_data::broker_rpc::CheckJobStatus::Reachable as i32
    );
}

#[tokio::test]
async fn test_agent_feedback_ok_https() {
    let port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u16;
    let (_, _) = AgentTestingRunner::create_tcp_socket_listener::<CheckBatchRequest>(port);
    let job_config = JobInnerConfig::Https(HttpsJob::new(
        DnsName::try_from_str("clever-cloud.com").unwrap(),
    ));
    let check = Job::new(Duration::from_secs(10), job_config, "https".to_string());

    let socket_path = tempfile::NamedTempFile::new().unwrap().path().to_path_buf();
    let (mut rx, _) =
        AgentTestingRunner::create_path_socket_listener::<CheckBatchRequest>(socket_path.clone());

    let agent = AgentTestingRunner::new()
        .add_check(check)
        .use_socket_sender(socket_path)
        .run();

    let check_batch_request = rx.recv().await.unwrap();
    agent.abort();

    let check_result = check_batch_request
        .events
        .first()
        .expect("Expected to receive at least one check result");

    assert_eq!(
        check_result.status,
        isok_data::broker_rpc::CheckJobStatus::Reachable as i32
    );

    assert!(check_result.error.is_none());

    assert!(match check_result.details {
        Some(Details::DetailsHttps(details)) => details.is_valid,
        _ => false,
    })
}
