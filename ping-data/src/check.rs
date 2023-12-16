use std::collections::HashMap;
use std::fmt::{Display, Formatter};
pub use std::net::IpAddr;
use std::time::Duration;

use chrono::NaiveDateTime;
pub use http::uri::Authority;
pub use http::uri::InvalidUri;
pub use http::Uri as HttpUri;
pub use serde::de::Visitor;
use serde::de::{Error, StdError};
pub use serde::{Deserialize, Serialize};
pub use serde::{Deserializer, Serializer};
use uuid::Uuid;

#[derive(Debug, Clone, Eq)]
pub struct Domain {
    inner: Authority,
}

impl PartialEq for Domain {
    fn eq(&self, other: &Self) -> bool {
        self.inner.host() == other.inner.host()
    }
}

impl Domain {
    fn new(domain: Authority) -> Self {
        Domain { inner: domain }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.inner.as_str()
    }
}

#[derive(Debug, Clone)]
pub enum LinkParseError {
    InvalidUriChar,
    InvalidScheme,
    InvalidAuthority,
    InvalidPort,
    InvalidFormat,
    SchemeMissing,
    AuthorityMissing,
    PathAndQueryMissing,
    TooLong,
    Empty,
    SchemeTooLong,
    Unknown,
}

impl StdError for LinkParseError {}

impl Display for LinkParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LinkParseError::InvalidUriChar => "invalid uri character",
                LinkParseError::InvalidScheme => "invalid scheme",
                LinkParseError::InvalidAuthority => "invalid authority",
                LinkParseError::InvalidPort => "invalid port",
                LinkParseError::InvalidFormat => "invalid format",
                LinkParseError::SchemeMissing => "scheme missing",
                LinkParseError::AuthorityMissing => "authority missing",
                LinkParseError::PathAndQueryMissing => "path missing",
                LinkParseError::TooLong => "uri too long",
                LinkParseError::Empty => "empty string",
                LinkParseError::SchemeTooLong => "scheme too long",
                LinkParseError::Unknown => "unknown",
            }
        )
    }
}

impl From<&str> for LinkParseError {
    fn from(value: &str) -> Self {
        match value {
            "invalid uri character" => LinkParseError::InvalidUriChar,
            "invalid scheme" => LinkParseError::InvalidScheme,
            "invalid authority" => LinkParseError::InvalidAuthority,
            "invalid port" => LinkParseError::InvalidPort,
            "invalid format" => LinkParseError::InvalidFormat,
            "scheme missing" => LinkParseError::SchemeMissing,
            "authority missing" => LinkParseError::AuthorityMissing,
            "path missing" => LinkParseError::PathAndQueryMissing,
            "uri too long" => LinkParseError::TooLong,
            "empty string" => LinkParseError::Empty,
            "scheme too long" => LinkParseError::SchemeTooLong,
            _ => LinkParseError::Unknown,
        }
    }
}

impl From<InvalidUri> for LinkParseError {
    fn from(value: InvalidUri) -> Self {
        Self::from(value.to_string().as_str())
    }
}

impl Error for LinkParseError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::from(msg.to_string().as_str())
    }
}

impl TryFrom<&str> for Domain {
    type Error = LinkParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Authority::try_from(value)
            .map(|a| Domain { inner: a })
            .map_err(|err| LinkParseError::from(err))
    }
}

impl Serialize for Domain {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.inner.as_str())
    }
}

struct DomainVisitor;

impl<'de> Visitor<'de> for DomainVisitor {
    type Value = Domain;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "Expecting valid domain name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Domain::try_from(v).map_err(|e| E::custom(e.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Domain::try_from(v.as_str()).map_err(|err| E::custom(err.to_string()))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Domain::try_from(String::from_utf8_lossy(v).to_string().as_str())
            .map_err(|err| E::custom(err.to_string()))
    }
}

impl<'de> Deserialize<'de> for Domain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DomainVisitor)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Host {
    IpAddr(IpAddr),
    Domain(Domain),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DnsCheck {
    domain: Domain,
    dns_server: Host,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IcmpCheck {
    host: Host,
}

#[derive(Debug, Clone)]
pub struct Uri {
    inner: HttpUri,
}

impl TryFrom<&str> for Uri {
    type Error = LinkParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        HttpUri::try_from(value)
            .map(|a| Uri { inner: a })
            .map_err(|err| LinkParseError::from(err))
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
        Uri::try_from(v).map_err(|e| E::custom(e.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Uri::try_from(v.as_str()).map_err(|e| E::custom(e.to_string()))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Uri::try_from(String::from_utf8_lossy(v).to_string().as_str())
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

impl Serialize for Uri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.inner.to_string().as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpCheck {
    uri: Uri,
    headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TcpCheck {
    host: Host,
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckKind {
    Dns(DnsCheck),
    Icmp(IcmpCheck),
    Http(HttpCheck),
    Tcp(TcpCheck),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    id: Uuid,
    kind: CheckKind,
    max_latency: Duration,
    interval: Duration,
    region: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInput {
    kind: CheckKind,
    max_latency: Duration,
    interval: Duration,
    region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutput {
    id: Uuid,
    kind: CheckKind,
    max_latency: Duration,
    interval: Duration,
    region: String,
}
