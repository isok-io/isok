use redis::{AsyncCommands, Client, RedisError};
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;
use log::error;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

/// Redis client, wrapper for Redis operations (without magic pool)
pub struct RedisClient {
    client: Client,
}

impl RedisClient {
    pub fn new(url: &str) -> Self {
        Self {
            client: Client::open(url).expect("Failed to connect to Redis"),
        }
    }

    pub async fn send(&self, context: &RedisContext) -> Option<RedisResult> {
        let before = SystemTime::now();
        let success = self.client.get_async_connection().await
            .and_then(|mut con| async {
                con.set(context.key(), &context.value()).await
            })
            .is_ok();

        let elapsed = before.elapsed().ok()?;

        Some(RedisResult {
            datetime: OffsetDateTime::now_utc(),
            process_time: elapsed,
            success,
        })
    }
}

impl Default for RedisClient {
    fn default() -> Self {
        Self::new("redis://127.0.0.1/")
    }
}

impl Clone for RedisClient {
    fn clone(&self) -> Self {
        Self::new(self.client.get_connection_info())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisContext {
    key: String,
    value: String,
}

impl RedisContext {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Result of a Redis operation
pub struct RedisResult {
    pub datetime: OffsetDateTime,
    pub process_time: Duration,
    pub success: bool,
}

//data transformations for storing data in Redis
pub trait RedisData {
    fn data(&self, uuid: Uuid) -> Vec<(OffsetDateTime, String, Uuid, i64)>;
}

impl RedisData for RedisResult {
    fn data(&self, uuid: Uuid) -> Vec<(OffsetDateTime, String, Uuid, i64)> {
        vec![
            (self.datetime, "redis_process_time".to_string(), uuid, self.process_time.as_millis() as i64),
            (self.datetime, "redis_operation_success".to_string(), uuid, self.success as i64),
        ]
    }
}
