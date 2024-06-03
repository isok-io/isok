use log::error;
use ping_data::check::HttpCheck;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Client, Method, Request, Url, StatusCode};
use std::collections::HashMap;
use std::time::Duration;
use nom::AsBytes;
use time::OffsetDateTime;
use uuid::Uuid;
use ping_data::pulsar_messages::{CheckMessage, CheckResult};
use serde::{Deserialize, Serialize};
use ping_data::check_kinds::http::HttpFields;

/// Http client, [`Client`] wrapper for storage in a [MagicPool](crate::magic_pool::MagicPool)
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Send async an http [`Request`]
    async fn send(&self, req: Request) -> Option<HttpResult> {
        let before = std::time::SystemTime::now();
        let res = self.client.execute(req).await.ok()?;
        let elapsed = before.elapsed().ok()?;

        Some(HttpResult {
            datetime: OffsetDateTime::now_utc(),
            request_time: elapsed,
            status: res.status().as_u16(),
        })
    }

    pub async fn run(&self, req: Request) -> HttpResult {
        self.send(req).await.unwrap_or_else(
            || HttpResult {
                datetime: OffsetDateTime::now_utc(),
                request_time: Duration::from_millis(i64::MAX as u64),
                status: 500,
            }
        )
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        Self::new()
    }
}

/// Context of an http [Job](crate::job::Job), [`Request`] wrapper
#[derive(Debug)]
pub struct HttpContext {
    req: Request,
}

impl HttpContext {
    fn insert_header(header_map: &mut HeaderMap, key: String, value: String) -> Option<()> {
        let key = HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| error!("Can't parse http header key {key} : {e}"))
            .ok()?;
        let value = HeaderValue::from_bytes(value.as_bytes())
            .map_err(|e| error!("Can't parse http header value {value} : {e}"))
            .ok()?;

        header_map.insert(key, value)?;

        Some(())
    }

    pub fn new(url: &str, headers: HashMap<String, String>) -> Self {
        let mut req = Request::new(Method::GET, Url::parse(url).unwrap());
        let header_map = req.headers_mut();
        headers.into_iter().for_each(|(k, v)| {
            Self::insert_header(header_map, k, v);
        });

        Self { req }
    }

    pub fn url(&self) -> String {
        self.req.url().to_string()
    }
}

impl Clone for HttpContext {
    fn clone(&self) -> Self {
        let mut req = Request::new(Method::GET, self.req.url().clone());
        let header_map = req.headers_mut();
        *header_map = self.req.headers().clone();

        Self { req }
    }
}

impl From<HttpCheck> for HttpContext {
    fn from(value: HttpCheck) -> Self {
        Self::new(&value.uri.to_string(), value.headers)
    }
}

impl Into<Request> for HttpContext {
    fn into(self) -> Request {
        self.req
    }
}

/// Result of an http request ready to be send to warp10
pub struct HttpResult {
    pub datetime: OffsetDateTime,
    pub request_time: Duration,
    pub status: u16,
}

impl Into<CheckResult<HttpFields>> for HttpResult {
    fn into(self) -> CheckResult<HttpFields> {
        CheckResult {
            timestamp: self.datetime,
            latency: self.request_time,
            fields: HttpFields::new(self.status),
        }
    }
}