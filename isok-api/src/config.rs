use biscuit_auth::{KeyPair, PrivateKey};
use figment::error::Kind;
use figment::providers::{Format, Toml};
use figment::value::{Dict, Map, Tag, Value};
use figment::{Error, Figment, Metadata, Profile, Provider};
use isok_data::models::RefinementOps;
use isok_data::models::U32InRange;
use serde::Deserialize;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api: ApiConfig,
}

#[derive(Deserialize, Debug)]
pub struct DatabaseConfig {
    pub database_url: String,
}

#[derive(Deserialize, Debug)]
pub struct ApiConfig {
    #[serde(default = "default_api_addresses")]
    pub addresses: Vec<SocketAddr>,
    pub argon2_params: Argon2Params,
    #[serde(with = "private_key")]
    pub private_key: PrivateKey,
}

fn default_api_addresses() -> Vec<SocketAddr> {
    vec!["127.0.0.1:8080".parse().unwrap()]
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
                argon2_params: Default::default(),
                private_key: KeyPair::new().private(),
            },
        }
    }
}

pub struct EnvAdapter {
    provider: Box<dyn Provider>,
    suffix: String,
}

impl EnvAdapter {
    pub fn wrap<T: Provider + 'static>(provider: T) -> Self {
        Self {
            provider: Box::new(provider),
            suffix: "_env".to_string(),
        }
    }

    pub fn with_suffix(self, suffix: &str) -> Self {
        Self {
            suffix: suffix.to_string(),
            ..self
        }
    }

    fn process_string(
        key: String,
        value: String,
        tag: Tag,
        suffix: &str,
        all_keys: &HashSet<String>,
    ) -> Result<Option<(String, Value)>, Error> {
        if let Some(stripped_key) = key.as_str().strip_suffix(suffix) {
            if all_keys.contains(stripped_key) {
                return Ok(None);
            }
            let contents = std::env::var(&value).map_err(|e| {
                Kind::Message(format!(
                    "Could not read env var `{value}` from config value `{key}`: {e:#}"
                ))
            })?;
            return Ok(Some((
                stripped_key.to_string(),
                Value::String(tag, contents),
            )));
        }
        Ok(Some((key, Value::String(tag, value))))
    }

    fn process_dict(provider_dict: Dict, suffix: &str) -> Result<Dict, Error> {
        let keys = provider_dict
            .keys()
            .map(String::clone)
            .collect::<HashSet<_>>();
        let process_key_value = |(key, value): (String, Value)| {
            Ok(match value {
                Value::String(tag, v) => Self::process_string(key, v, tag, suffix, &keys)?,
                Value::Dict(tag, d) => {
                    Some((key, Value::Dict(tag, Self::process_dict(d, suffix)?)))
                }
                v => Some((key, v)),
            })
        };
        provider_dict
            .into_iter()
            .filter_map(|kv| process_key_value(kv).transpose())
            .collect()
    }
}

impl Provider for EnvAdapter {
    fn metadata(&self) -> Metadata {
        self.provider.metadata()
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        self.provider
            .data()?
            .into_iter()
            .map(|(profile, dict)| Ok((profile, Self::process_dict(dict, &self.suffix)?)))
            .collect()
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
