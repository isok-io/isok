use axum::http::Uri;
use biscuit_auth::{KeyPair, PrivateKey};
use figment::providers::{Format, Toml};
use figment::{Error, Figment};
use isok_data::config::EnvAdapter;
use isok_data::models::RefinementOps;
use isok_data::models::U32InRange;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api: ApiConfig,
    pub agents_handler: AgentsHandlerConfig,
    pub warp10: Warp10Config,
}

#[derive(Deserialize, Debug)]
pub struct DatabaseConfig {
    pub database_url: String,
}

#[derive(Deserialize, Debug)]
pub struct ApiConfig {
    #[serde(default = "default_api_addresses")]
    pub addresses: Vec<SocketAddr>,
    pub cors_origins: Vec<String>,
    pub agent_token: String,
    pub argon2_params: Argon2Params,
    #[serde(with = "private_key")]
    pub private_key: PrivateKey,
}

fn default_api_addresses() -> Vec<SocketAddr> {
    vec!["127.0.0.1:8080".parse().unwrap()]
}

#[derive(Deserialize, Debug)]
pub struct AgentsHandlerConfig {
    #[serde(with = "duration_secs")]
    pub healthcheck_itv: Duration,
    pub max_retries: u8,
}

type MCost =
    U32InRange<{ argon2::Params::MIN_M_COST as usize }, { argon2::Params::MAX_M_COST as usize }>;
type TCost =
    U32InRange<{ argon2::Params::MIN_T_COST as usize }, { argon2::Params::MAX_T_COST as usize }>;
type PCost =
    U32InRange<{ argon2::Params::MIN_P_COST as usize }, { argon2::Params::MAX_P_COST as usize }>;

#[derive(Deserialize, Debug)]
pub struct Argon2Params {
    #[serde(default = "default_m_cost")]
    pub m_cost: MCost,
    #[serde(default = "default_t_cost")]
    pub t_cost: TCost,
    #[serde(default = "default_p_cost")]
    pub p_cost: PCost,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            m_cost: default_m_cost(),
            t_cost: default_t_cost(),
            p_cost: default_p_cost(),
        }
    }
}

fn default_m_cost() -> MCost {
    MCost::refine(argon2::Params::DEFAULT_M_COST).expect("DEFAULT_M_COST not in range")
}

fn default_t_cost() -> TCost {
    TCost::refine(argon2::Params::DEFAULT_T_COST).expect("DEFAULT_T_COST not in range")
}

fn default_p_cost() -> PCost {
    PCost::refine(argon2::Params::DEFAULT_P_COST).expect("DEFAULT_P_COST not in range")
}

#[derive(Deserialize, Debug)]
pub struct Warp10Config {
    /// Valid Warp10 token that is used to read results
    pub read_token: String,
    /// Valid Warp10 token that is used to delete results
    pub write_token: String,
    /// Warp10 service to which the broker will read metrics, in the form of `http://<host>:<port>`
    #[serde(with = "http_serde::uri")]
    pub endpoint: Uri,
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
            database: DatabaseConfig {
                database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            },
            api: ApiConfig {
                addresses: default_api_addresses(),
                cors_origins: vec![],
                agent_token: "token".to_string(),
                argon2_params: Default::default(),
                private_key: KeyPair::new().private(),
            },
            agents_handler: AgentsHandlerConfig {
                healthcheck_itv: Duration::from_secs(30),
                max_retries: 3,
            },
            warp10: Warp10Config {
                read_token: "".to_string(),
                write_token: "".to_string(),
                endpoint: Default::default(),
            },
        }
    }
}

mod private_key {
    use biscuit_auth::PrivateKey;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PrivateKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        PrivateKey::from_bytes_hex(&s).map_err(serde::de::Error::custom)
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer};
    use std::time::Duration;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(s))
    }
}
