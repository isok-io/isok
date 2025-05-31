use figment::providers::{Format, Toml};
use figment::{Error, Figment};
use http_serde::http::Uri;
use isok_data::config::EnvAdapter;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub kafka: KafkaConfig,
    pub exporter: Exporter,
}

#[derive(Deserialize, Debug)]
pub struct KafkaConfig {
    pub topic: String,
    pub properties: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Exporter {
    #[serde(rename = "warp10")]
    Warp10(Warp10Config),
    #[serde(rename = "stdout")]
    Stdout,
}

#[derive(Deserialize, Debug)]
pub struct Warp10Config {
    /// Valid Warp10 token that is used to write results as metrics
    pub write_token: String,
    /// Warp10 service to which the broker will send metrics, in the form of `http://<host>:<port>`
    #[serde(with = "http_serde::uri")]
    pub endpoint: Uri,
    /// Maximum number of check results that can be buffered before sending them to Warp10,
    /// if batch_interval is not reached
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Maximum interval to which we send a batch, if batch_size is not reached, in milliseconds
    #[serde(default = "default_batch_interval")]
    pub batch_interval: u64,
}

fn default_batch_size() -> usize {
    100
}

fn default_batch_interval() -> u64 {
    10000
}

impl Config {
    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self, Error> {
        Figment::new()
            .merge(EnvAdapter::wrap(Toml::file(path.into())))
            .extract()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            kafka: KafkaConfig {
                topic: "isok.agent.results".to_string(),
                properties: HashMap::from([
                    (
                        "bootstrap.servers".to_string(),
                        "localhost:9092".to_string(),
                    ),
                    ("group.id".to_string(), "isok.offloader".to_string()),
                    ("enable.auto.commit".to_string(), "true".to_string()),
                    ("auto.commit.interval.ms".to_string(), "5000".to_string()),
                    ("enable.auto.offset.store".to_string(), "false".to_string()),
                    ("enable.partition.eof".to_string(), "false".to_string()),
                ]),
            },
            exporter: Exporter::Stdout,
        }
    }
}
