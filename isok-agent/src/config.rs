use std::path::{Path, PathBuf};

use figment::providers::{Format, Yaml};
use figment::Figment;
use serde::Deserialize;

use crate::errors::{Error, Result};
use crate::jobs::Job;
use crate::registry::JobRegistry;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Config {
    pub check_config_adapter: ConfigCheckAdapter,
    pub result_sender_adapter: ResultSenderAdapter,
}

impl GetJobsRegistry for Config {
    fn get_jobs_registry(&self) -> Result<JobRegistry> {
        self.check_config_adapter.get_jobs_registry()
    }
}

impl Config {
    pub fn from_config_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(Error::InvalidConfigPath);
        }

        Ok(Figment::new().merge(Yaml::file(path)).extract()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            check_config_adapter: ConfigCheckAdapter::Static(StaticConfigAdapter {
                checks: vec![],
            }),
            result_sender_adapter: ResultSenderAdapter::Stdout,
        }
    }
}

pub trait GetJobsRegistry {
    fn get_jobs_registry(&self) -> Result<JobRegistry>;
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ResultSenderAdapter {
    #[serde(rename = "stdout")]
    Stdout,
    #[serde(rename = "broker")]
    Broker(BrokerConfig),
    #[serde(rename = "socket")]
    Socket(SocketConfig),
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct SocketConfig {
    /// Path to a unix socket that
    pub path: PathBuf,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct BrokerConfig {
    pub main_broker: String,
    pub fallback_brokers: Vec<String>,
    pub agent_id: Option<String>,
    pub zone: String,
    pub region: String,
    pub batch: u64,
    pub batch_interval: u64,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "name")]
pub enum ConfigCheckAdapter {
    #[serde(rename = "static")]
    Static(StaticConfigAdapter),
    #[serde(rename = "file")]
    File(FileConfigCheckAdapter),
}

impl GetJobsRegistry for ConfigCheckAdapter {
    fn get_jobs_registry(&self) -> Result<JobRegistry> {
        match self {
            ConfigCheckAdapter::File(adapter) => adapter.get_jobs_registry(),
            ConfigCheckAdapter::Static(adapter) => adapter.get_jobs_registry(),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct FileConfigCheckAdapter {
    path: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct StaticConfigAdapter {
    pub checks: Vec<Job>,
}

impl GetJobsRegistry for FileConfigCheckAdapter {
    fn get_jobs_registry(&self) -> Result<JobRegistry> {
        JobRegistry::from_configuration_file(PathBuf::from(&self.path))
    }
}

impl GetJobsRegistry for StaticConfigAdapter {
    fn get_jobs_registry(&self) -> Result<JobRegistry> {
        JobRegistry::from_static_config(self.checks.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_example_config() {
        let _ = Config::from_config_file(
            env!("CARGO_MANIFEST_DIR").to_string() + "/assets/config/agent.example.yaml",
        )
            .expect("Unable to load default config");
    }
}
