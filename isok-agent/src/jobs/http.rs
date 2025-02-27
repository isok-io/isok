use std::collections::HashMap;
use std::str::FromStr;
use std::time::Instant;

use isok_data::broker_rpc::check_result::Details;
use isok_data::broker_rpc::{CheckJobStatus, JobDetailsHttp};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::batch_sender::JobResult;
use crate::jobs::{Execute, JobError};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct HttpJob {
    endpoint: String,
    headers: HashMap<String, String>,
    #[serde(
        deserialize_with = "deserialize_reqwest_method",
        serialize_with = "serialize_reqwest_method",
        default = "default_method"
    )]
    method: reqwest::Method,
}

fn default_method() -> reqwest::Method {
    reqwest::Method::GET
}

fn deserialize_reqwest_method<'de, D>(deserializer: D) -> Result<reqwest::Method, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    reqwest::Method::from_str(&buf).map_err(serde::de::Error::custom)
}

fn serialize_reqwest_method<S>(method: &reqwest::Method, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(method.as_str())
}

pub struct HttpJobResult {
    pub status_code: StatusCode,
}

impl HttpJob {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            headers: HashMap::from([("Content-Type".to_string(), "application/json".to_string())]),
            method: default_method(),
        }
    }
}

impl Execute for HttpJob {
    async fn execute(&self, msg: &mut JobResult) -> Result<(), JobError> {
        let mut headers_map = HeaderMap::new();
        for (key, value) in self.headers.iter() {
            let header_name = HeaderName::from_str(key).map_err(|_| {
                JobError::InvalidJobConfig(format!("Header name {} is invalid", key))
            })?;
            let header_value = HeaderValue::from_str(value).map_err(|_| {
                JobError::InvalidJobConfig(format!("Header value {} is invalid", key))
            })?;
            headers_map.insert(header_name, header_value);
        }

        let client = reqwest::Client::builder()
            .default_headers(headers_map)
            .build()?;

        let start_time = Instant::now();
        match client
            .request(self.method.clone(), &self.endpoint)
            .send()
            .await
        {
            Ok(response) => {
                let latency = start_time.elapsed();
                let _ = response.status();
                msg.set_status(CheckJobStatus::Reachable);
                msg.set_latency(latency);
                msg.set_details(Some(Details::DetailsHttp(JobDetailsHttp {
                    status_code: response.status().as_u16() as u32,
                })));
            }
            Err(_) => {
                msg.set_status(CheckJobStatus::Unreachable);
            }
        }
        Ok(())
    }
}
