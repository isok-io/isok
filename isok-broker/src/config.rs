use figment::providers::{Format, Toml};
use figment::{Error, Figment};
use isok_data::config::EnvAdapter;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub kafka: KafkaConfig,
    pub api: ApiConfig,
}

#[derive(Deserialize, Debug)]
pub struct KafkaConfig {
    pub topic: String,
    pub properties: HashMap<String, String>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct ApiConfig {
    pub listen_address: SocketAddr,
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
                properties: HashMap::from([(
                    "bootstrap.servers".to_string(),
                    "localhost:9092".to_string(),
                )]),
            },
            api: ApiConfig {
                listen_address: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9000),
            },
        }
    }
}
