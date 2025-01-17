use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Instant;

use async_trait::async_trait;
use isok_data::broker_rpc::CheckJobStatus;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use crate::batch_sender::JobResult;
use crate::jobs::{Execute, JobError};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct TcpJob {
    endpoint: String,
    secured: bool,
}

impl TcpJob {
    pub fn new(endpoint: String) -> Self {
        TcpJob {
            endpoint,
            secured: false,
        }
    }
}

#[async_trait]
impl Execute for TcpJob {
    async fn execute(&self, msg: &mut JobResult) -> Result<(), JobError> {
        let addr = SocketAddr::from_str(&self.endpoint);
        if let Ok(addr) = addr {
            let start_time = Instant::now();
            match TcpStream::connect(addr).await {
                Ok(_) => {
                    let latency = start_time.elapsed();
                    msg.set_status(CheckJobStatus::Reachable);
                    msg.set_latency(latency);
                }
                Err(_) => {
                    msg.set_status(CheckJobStatus::Unreachable);
                }
            }
        } else {
            msg.set_status(CheckJobStatus::Unreachable);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::batch_sender::JobResult;
    use crate::jobs::tcp::TcpJob;
    use crate::jobs::Execute;
    use isok_data::broker_rpc::CheckJobStatus;
    use isok_data::JobId;

    #[tokio::test]
    async fn test_tcp_job() {
        let tcp = TcpJob {
            endpoint: "toto".to_string(),
            secured: false,
        };

        let tcp2 = TcpJob {
            endpoint: "tata".to_string(),
            secured: false,
        };

        let jobs = vec![tcp, tcp2];
        let a = serde_yaml::to_string(&jobs);
        println!("{:#}", a.unwrap());
    }

    #[tokio::test]
    async fn test_tcp_job_invalid_endpoint() {
        let tcp = TcpJob {
            endpoint: "toto".to_string(),
            secured: false,
        };
        let mut job_result = JobResult::new(JobId::generate(), "toto".to_string());
        tcp.execute(&mut job_result)
            .await
            .expect("Expected execution to succeed");
        assert_eq!(job_result.status, CheckJobStatus::Unreachable);
    }

    #[tokio::test]
    async fn test_tcp_job_valid_endpoint_online() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Unable to bind to port");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (_, _) = listener
                .accept()
                .await
                .expect("Unable to accept connection");
        });
        let tcp = TcpJob {
            endpoint: "127.0.0.1".to_string() + ":" + &port.to_string(),
            secured: false,
        };
        let mut job_result = JobResult::new(JobId::generate(), "toto".to_string());
        tcp.execute(&mut job_result)
            .await
            .expect("Expected execution to succeed");
        assert_eq!(job_result.status, CheckJobStatus::Reachable);
    }

    #[tokio::test]
    async fn test_tcp_job_valid_endpoint_offline() {
        let tcp = TcpJob {
            endpoint: "127.0.0.1:65534".to_string(),
            secured: false,
        };
        let mut job_result = JobResult::new(JobId::generate(), "toto".to_string());
        tcp.execute(&mut job_result)
            .await
            .expect("Expected execution to succeed");
        assert_eq!(job_result.status, CheckJobStatus::Unreachable);
    }
}
