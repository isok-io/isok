use log::error;
use redis::{AsyncCommands, Client};
use ping_data::check::RedisCheck;
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;
use uuid::Uuid;
use crate::warp10::{Data, Value, warp10_data};

/// Redis client, [`Client`] wrapper for storage in a structure 
pub struct RedisClient {
    client: Client,
}

impl RedisClient {
    pub fn new(url: &str) -> Self {
        let client = Client::open(url).expect("Failed to connect to Redis");
        Self { client }
    }

    pub async fn send_redis(&self, context: &RedisContext) -> Option<RedisResult> {
        let before = SystemTime::now();
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                let success = conn.set::<&str, &str, ()>(context.key(), context.value()).await.is_ok();
                let elapsed = before.elapsed().expect("Time should not go backwards");

                Some(RedisResult {
                    datetime: OffsetDateTime::now_utc(),
                    process_time: elapsed,
                    success,
                })
            },
            Err(e) => {
                error!("Failed to connect to Redis: {:?}", e);
                None
            }
        }
    }
}

impl Default for RedisClient {
    fn default() -> Self {
        Self::new("redis://127.0.0.1/")
    }
}

impl Clone for RedisClient {
    fn clone(&self) -> Self {
        Self::new("redis://127.0.0.1/")
    }
}

/// Context for a Redis operation
#[derive(Debug)]
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

impl Clone for RedisContext {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl From<RedisCheck> for RedisContext {
    fn from(value: RedisCheck) -> Self {
        let value_string = value.expected_response.unwrap_or_else(|| "".to_string());
        Self::new(value.command, value_string)
    }
}


pub struct RedisResult {
    pub datetime: OffsetDateTime,
    pub process_time: Duration,
    pub success: bool,
}

impl RedisResult {
    pub fn data(&self, uuid: Uuid) -> Vec<Data> {
        vec![
            warp10_data(
                self.datetime,
                "redis_operation_time",
                uuid,
                Value::Long(self.process_time.as_millis() as i64),
            ),
            warp10_data(
                self.datetime,
                "redis_operation_success",
                uuid,
                Value::Boolean(self.success),
            ),
        ]
    }
}
