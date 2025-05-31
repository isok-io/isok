use figment::error::Kind;
use figment::value::{Dict, Map, Tag, Value};
use figment::{Error, Metadata, Profile, Provider};
use std::collections::HashSet;

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
