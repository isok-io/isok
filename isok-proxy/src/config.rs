pub use std::collections::HashMap;
pub use std::fmt::Display;
pub use std::fmt::Formatter;
pub use std::io::Read;
pub use std::path::Path;
pub use std::str::FromStr;

pub use http::Uri as HttpUri;
pub use log::error;
pub use serde::de::{Error, Visitor};
pub use serde::Deserialize;
pub use serde::Deserializer;

pub use isok_data::check::LinkParseError;

#[cfg(not(feature = "env_config"))]
pub use crate::Cli;

#[derive(Debug, Clone)]
pub struct Uri {
    inner: HttpUri,
}

impl Display for Uri {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl FromStr for Uri {
    type Err = LinkParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        HttpUri::try_from(value)
            .map(|a| Uri { inner: a })
            .map_err(LinkParseError::from)
    }
}

impl From<Uri> for HttpUri {
    fn from(val: Uri) -> Self {
        val.inner
    }
}

struct UriVisitor;

impl<'de> Visitor<'de> for UriVisitor {
    type Value = Uri;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "Expecting valid uri")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Uri::from_str(v).map_err(|e| E::custom(e.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Uri::from_str(v.as_str()).map_err(|err| E::custom(err.to_string()))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Uri::from_str(String::from_utf8_lossy(v).to_string().as_str())
            .map_err(|err| E::custom(err.to_string()))
    }
}

impl<'de> Deserialize<'de> for Uri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UriVisitor)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiConfig {
    pub uri: Uri,
    pub tokens: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IncompleteConfig {
    pub address: Option<String>,
    pub port: Option<u16>,
    pub apis: HashMap<String, ApiConfig>,
    pub db: Option<String>,
    pub token: Option<String>,
    pub hash_m_cost: Option<u32>,
    pub hash_t_cost: Option<u32>,
    pub hash_p_cost: Option<u32>,
    pub hash_len: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub address: String,
    pub port: u16,
    pub apis: HashMap<String, ApiConfig>,
    pub db: String,
    pub token: String,
    pub hash_m_cost: u32,
    pub hash_t_cost: u32,
    pub hash_p_cost: u32,
    pub hash_len: Option<usize>,
}

impl IncompleteConfig {
    #[cfg(not(feature = "env_config"))]
    pub fn from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<Result<Self, toml::de::Error>, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let mut buf_reader = std::io::BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents)?;
        Ok(toml::from_str(&contents))
    }

    pub fn from_env() -> Self {
        let env = |env: &str| std::env::var(env).ok();
        let u32_parse = |val: String, e_msg: &str| {
            u32::from_str(val.as_str())
                .map_err(|e| {
                    error!("{e_msg}: {e}");
                    std::process::exit(1)
                })
                .unwrap()
        };

        Self {
            address: env("ADDRESS"),
            port: env("PORT").map(|p| {
                u16::from_str(p.as_str())
                    .map_err(|e| {
                        error!("Failed to parse port: {e}");
                        std::process::exit(1)
                    })
                    .unwrap()
            }),
            apis: env("APIS")
                .map(|s| {
                    serde_json::from_str::<HashMap<String, ApiConfig>>(s.as_str())
                        .map_err(|e| {
                            error!("Failed to parse apis config: {e}");
                            std::process::exit(1)
                        })
                        .unwrap()
                })
                .unwrap_or(HashMap::new()),
            db: env("DATABASE_URL"),
            token: env("TOKEN"),
            hash_m_cost: env("HASH_M_COST").map(|p| u32_parse(p, "Failed to parse m cost")),
            hash_t_cost: env("HASH_T_COST").map(|p| u32_parse(p, "Failed to parse t cost")),
            hash_p_cost: env("HASH_P_COST").map(|p| u32_parse(p, "Failed to parse p cost")),
            hash_len: env("HASH_LEN").map(|p| u32_parse(p, "Failed to parse hash length") as usize),
        }
    }

    #[cfg(not(feature = "env_config"))]
    pub fn from_cli(cli: Cli) -> Self {
        Self {
            address: cli.address,
            port: cli.port,
            apis: cli
                .api
                .map(|vec| {
                    vec.iter()
                        .map(|s| {
                            serde_json::from_str::<ApiConfig>(s.as_str())
                                .map_err(|e| {
                                    error!("Failed to parse apis config: {e}");
                                    std::process::exit(1)
                                })
                                .unwrap()
                        })
                        .map(|api| (api.uri.to_string(), api))
                        .collect::<HashMap<String, ApiConfig>>()
                })
                .unwrap_or_default(),
            db: cli.db,
            token: cli.token,
            hash_m_cost: cli.hash_m_cost,
            hash_t_cost: cli.hash_t_cost,
            hash_p_cost: cli.hash_p_cost,
            hash_len: cli.hash_len,
        }
    }

    pub fn merge(self, conf: Self) -> Self {
        Self {
            address: conf.address.or(self.address),
            port: conf.port.or(self.port),
            apis: Self::merge_apis_config(self.apis, conf.apis),
            db: conf.db.or(self.db),
            token: conf.token.or(self.token),
            hash_m_cost: conf.hash_m_cost.or(self.hash_m_cost),
            hash_t_cost: conf.hash_t_cost.or(self.hash_t_cost),
            hash_p_cost: conf.hash_p_cost.or(self.hash_p_cost),
            hash_len: conf.hash_len.or(self.hash_len),
        }
    }

    fn merge_apis_config(
        a: HashMap<String, ApiConfig>,
        b: HashMap<String, ApiConfig>,
    ) -> HashMap<String, ApiConfig> {
        let mut res = a.clone();
        for (key, value) in b {
            res.insert(key, value);
        }
        res
    }

    pub fn to_config(self) -> Config {
        Config {
            address: self.address.unwrap_or("127.0.0.1".to_string()),
            port: self.port.unwrap_or(8080),
            apis: if self.apis.is_empty() {
                error!("Need at least one api configured");
                std::process::exit(1)
            } else {
                self.apis
            },
            db: self.db.unwrap_or_else(|| {
                error!("Missing db config");
                std::process::exit(1)
            }),
            token: self.token.unwrap_or_else(|| {
                error!("Missing auth token config");
                std::process::exit(1)
            }),
            hash_m_cost: self.hash_m_cost.unwrap_or(argon2::Params::DEFAULT_M_COST),
            hash_t_cost: self.hash_t_cost.unwrap_or(argon2::Params::DEFAULT_T_COST),
            hash_p_cost: self.hash_p_cost.unwrap_or(argon2::Params::DEFAULT_P_COST),
            hash_len: self.hash_len,
        }
    }
}
