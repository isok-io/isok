use crate::Result;
use crate::api::MetricsFilter;
use crate::config::Warp10Config;
use crate::errors::Error;
use axum::http::StatusCode;
use isok_data::models::{
    ApiCheckMetrics, ApiCheckMetricsResult, ApiCheckResult, ApiCheckStatus, CheckKind,
    CheckResultDetails, CheckStatus, HttpCheckResult,
};
use reqwest::header::HeaderValue;
use reqwest::{Client, Response};
use serde::Deserialize;
use sqlx::types::chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::string::ToString;
use std::time::Duration;
use tracing::error;
use uuid::Uuid;

const GTS_DEFAULT_PREFIX: &str = "isok.check";

const QUANTITATIVE_METRICS_MC2: &str = r#"{
    'token' "$TOKEN"
    'class' "$CLASS"
    'labels' { 'id' '$CHECK_ID' }
    'start' "$START"
    'end' "$END"
} FETCH 'data' STORE
[ $data bucketizer.mean 0 $BUCKET s 0 ] BUCKETIZE 'bucketized' STORE
[ $bucketized [ 'id' ] reducer.mean.exclude-nulls ] REDUCE"#;

const QUALITATIVE_METRICS_MC2: &str = r#"{
    'token' "$TOKEN"
    'class' "$CLASS"
    'labels' { 'id' '$CHECK_ID' }
    'start' "$START"
    'end' "$END"
} FETCH 'data' STORE
[ $data 75.0 bucketizer.percentile 0 $BUCKET s 0 ] BUCKETIZE"#;

const TEXT_METRICS_MC2: &str = r#"{
    'token' "$TOKEN"
    'class' "$CLASS"
    'labels' { 'id' '$CHECK_ID' }
    'start' "$START"
    'end' "$END"
} FETCH 'data' STORE
[ $data bucketizer.last 0 $BUCKET s 0 ] BUCKETIZE"#;

pub enum Gts {
    Status,
    Latency,
    Error,
    HttpStatusCode,
}

impl Gts {
    fn to_class(&self) -> String {
        let class = match self {
            Gts::Status => "status",
            Gts::Latency => "latency",
            Gts::Error => "error",
            Gts::HttpStatusCode => "http.status_code",
        };

        format!("{GTS_DEFAULT_PREFIX}.{class}")
    }
}

#[derive(Deserialize, Debug)]
pub struct Warp10Response<Value> {
    #[serde(rename = "c")]
    pub class: String,
    #[serde(rename = "l")]
    pub labels: HashMap<String, String>,
    #[serde(rename = "v")]
    pub values: Vec<(u64, Value)>,
}

impl<Value: Clone> Warp10Response<Value> {
    pub fn into_time_slot(self, filter: &MetricsFilter) -> Warp10ResponseTimeSlot<Value> {
        let bucket = Warp10::compute_bucket(filter);
        let bucket = Duration::from_secs_f32(bucket);

        let mut slots = (0..filter.points)
            .map(|point| point as f32)
            .map(|point| filter.start + bucket.mul_f32(point))
            .map(|start| {
                (
                    TimeSlot {
                        start,
                        end: start + bucket,
                    },
                    None,
                )
            })
            .collect::<Vec<_>>();

        for value in self.values {
            let dt = DateTime::from_timestamp_micros(value.0 as i64).unwrap_or_default();

            slots
                .iter_mut()
                .filter(|s| s.0.datetime_in(&dt))
                .for_each(|s| s.1 = Some(value.1.clone()))
        }

        Warp10ResponseTimeSlot {
            class: self.class,
            labels: self.labels,
            values: slots,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct TimeSlot {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeSlot {
    pub fn datetime_in(&self, dt: &DateTime<Utc>) -> bool {
        &self.start < dt && &self.end >= dt
    }
}

impl PartialOrd<Self> for TimeSlot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeSlot {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl Hash for TimeSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i64((self.start.timestamp_millis() + self.end.timestamp_millis()) / 2)
    }
}

#[derive(Debug)]
pub struct Warp10ResponseTimeSlot<Value> {
    pub class: String,
    pub labels: HashMap<String, String>,
    pub values: Vec<(TimeSlot, Option<Value>)>,
}

pub struct Warp10 {
    read_token: String,
    write_token: String,
    endpoint: String,
    client: Client,
}

impl Warp10 {
    pub fn new(config: Warp10Config) -> Self {
        Self {
            read_token: config.read_token,
            write_token: config.write_token,
            endpoint: config.endpoint.to_string(),
            client: Client::new(),
        }
    }

    fn compute_bucket(filter: &MetricsFilter) -> f32 {
        let bucket = (filter.end - filter.start) / filter.points as i32;

        bucket.as_seconds_f32()
    }

    pub async fn get_i64_metrics_by_check_id(
        &self,
        gts: &Gts,
        id: Uuid,
        filter: &MetricsFilter,
    ) -> Result<Vec<Warp10Response<i64>>> {
        let res = self
            .get_metrics_by_check_id(gts, id, filter)
            .await?
            .json::<Vec<Vec<Warp10Response<i64>>>>()
            .await?
            .pop()
            .ok_or(Error::Warp10("Empty result".to_string()))?;

        Ok(res)
    }

    pub async fn get_f32_metrics_by_check_id(
        &self,
        gts: &Gts,
        id: Uuid,
        filter: &MetricsFilter,
    ) -> Result<Vec<Warp10Response<f32>>> {
        let res = self
            .get_metrics_by_check_id(gts, id, filter)
            .await?
            .json::<Vec<Vec<Warp10Response<f32>>>>()
            .await?
            .pop()
            .ok_or(Error::Warp10("Empty result".to_string()))?;

        Ok(res)
    }

    pub async fn get_u16_metrics_by_check_id(
        &self,
        gts: &Gts,
        id: Uuid,
        filter: &MetricsFilter,
    ) -> Result<Vec<Warp10Response<u16>>> {
        let res = self
            .get_metrics_by_check_id(gts, id, filter)
            .await?
            .json::<Vec<Vec<Warp10Response<u16>>>>()
            .await?
            .pop()
            .ok_or(Error::Warp10("Empty result".to_string()))?;

        Ok(res)
    }

    pub async fn get_string_metrics_by_check_id(
        &self,
        gts: &Gts,
        id: Uuid,
        filter: &MetricsFilter,
    ) -> Result<Vec<Warp10Response<String>>> {
        let res = self
            .get_metrics_by_check_id(gts, id, filter)
            .await?
            .json::<Vec<Vec<Warp10Response<String>>>>()
            .await?
            .pop()
            .ok_or(Error::Warp10("Empty result".to_string()))?;

        Ok(res)
    }

    async fn get_metrics_by_check_id(
        &self,
        gts: &Gts,
        id: Uuid,
        filter: &MetricsFilter,
    ) -> Result<Response> {
        let query = match gts {
            Gts::Status | Gts::HttpStatusCode => QUALITATIVE_METRICS_MC2,
            Gts::Latency => QUANTITATIVE_METRICS_MC2,
            Gts::Error => TEXT_METRICS_MC2,
        }
        .replacen("$TOKEN", &self.read_token, 1)
        .replacen("$CLASS", &gts.to_class(), 1)
        .replacen("$CHECK_ID", &id.as_simple().to_string(), 1)
        .replacen("$START", &filter.start.to_rfc3339(), 1)
        .replacen("$END", &filter.end.to_rfc3339(), 1)
        .replacen("$BUCKET", &Self::compute_bucket(filter).to_string(), 1);

        let res = self
            .client
            .post(format!("{}api/v0/exec", &self.endpoint))
            .body(format!("<% %>  EVAL\n{query}"))
            .send()
            .await?;

        if res.status().is_success() {
            Ok(res)
        } else {
            error!(path = format!("{}api/v0/exec", &self.endpoint), status = ?res.status());
            Err(Error::Warp10(
                res.headers()
                    .get("X-Warp10-Error")
                    .unwrap_or(&HeaderValue::from_static(""))
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            ))
        }
    }

    pub async fn get_check_results(
        &self,
        id: Uuid,
        kind: &CheckKind,
        filter: &MetricsFilter,
    ) -> Result<ApiCheckMetrics> {
        let mut status = self
            .get_i64_metrics_by_check_id(&Gts::Status, id, filter)
            .await?
            .into_iter()
            .flat_map(|l| l.into_time_slot(filter).values)
            .fold(HashMap::new(), |mut acc, e| {
                let status: ApiCheckStatus = CheckStatus::try_from(e.1.unwrap_or(0))
                    .unwrap_or(CheckStatus::Unknown)
                    .into();
                acc.entry(e.0)
                    .and_modify(|v| match (v, status.clone()) {
                        (v @ ApiCheckStatus::None, o) => *v = o,
                        (_, ApiCheckStatus::None) => (),
                        (ApiCheckStatus::Unreachable, ApiCheckStatus::Unreachable) => (),
                        (ApiCheckStatus::Reachable, ApiCheckStatus::Reachable) => (),
                        (v, _) => *v = ApiCheckStatus::ReachableUnreachable,
                    })
                    .or_insert(status);
                acc
            })
            .into_iter()
            .collect::<Vec<_>>();

        status.sort_by(|(key, _), (key2, _)| key.cmp(key2));

        let latencies = self
            .get_f32_metrics_by_check_id(&Gts::Latency, id, filter)
            .await?
            .into_iter()
            .map(|l| l.into_time_slot(filter))
            .last()
            .map(|v| v.values)
            .unwrap_or_else(|| {
                Warp10Response {
                    class: "".to_string(),
                    labels: Default::default(),
                    values: vec![],
                }
                .into_time_slot(filter)
                .values
            });

        let mut errors = self
            .get_string_metrics_by_check_id(&Gts::Error, id, filter)
            .await?
            .into_iter()
            .flat_map(|l| l.into_time_slot(filter).values)
            .fold(HashMap::new(), |mut acc, e| {
                acc.entry(e.0)
                    .and_modify(|v| {
                        if let (v @ None, Some(o)) = (v, e.1.clone()) {
                            *v = Some(o)
                        }
                    })
                    .or_insert(e.1);
                acc
            })
            .into_iter()
            .collect::<Vec<_>>();
        if errors.is_empty() {
            errors.append(
                &mut Warp10Response {
                    class: "".to_string(),
                    labels: Default::default(),
                    values: vec![],
                }
                .into_time_slot(filter)
                .values,
            )
        }
        errors.sort_by(|(key, _), (key2, _)| key.cmp(key2));

        match kind {
            CheckKind::Http(_) => {
                let mut status_code = self
                    .get_u16_metrics_by_check_id(&Gts::HttpStatusCode, id, filter)
                    .await?
                    .into_iter()
                    .flat_map(|l| l.into_time_slot(filter).values)
                    .fold(HashMap::new(), |mut acc, e| {
                        acc.entry(e.0)
                            .and_modify(|v| {
                                if let (v @ None, Some(o)) = (v, e.1) {
                                    *v = Some(o)
                                }
                            })
                            .or_insert(e.1);
                        acc
                    })
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            key,
                            value.and_then(|value| StatusCode::from_u16(value).ok()),
                        )
                    })
                    .collect::<Vec<_>>();
                status_code.sort_by(|(key, _), (key2, _)| key.cmp(key2));

                let res = status
                    .into_iter()
                    .zip(latencies.into_iter())
                    .zip(errors.into_iter())
                    .zip(status_code.into_iter())
                    .map(|((((ts, status), latencies), errors), status_code)| {
                        if status_code.1.is_none() {
                            None
                        } else {
                            Some(ApiCheckResult {
                                start: ts.start,
                                end: ts.end,
                                status,
                                metrics: ApiCheckMetricsResult {
                                    latency: latencies.1.unwrap_or_default(),
                                },
                                error: errors.1,
                                details: CheckResultDetails::Http(HttpCheckResult {
                                    status_code: status_code.1.unwrap_or_default(),
                                }),
                            })
                        }
                    })
                    .collect::<Vec<_>>();

                Ok(res)
            }
        }
    }

    pub async fn delete_results(&self, id: Uuid) -> Result<()> {
        let res = self
            .client
            .get(format!(
                "{}api/v0/delete?deleteall&selector=~{GTS_DEFAULT_PREFIX}.*{{id={}}}",
                &self.endpoint,
                id.as_simple().to_string()
            ))
            .header("X-Warp10-Token", &self.write_token)
            .send()
            .await?;

        if res.status().is_success() {
            Ok(())
        } else {
            error!(path = format!("{}api/v0/delete", &self.endpoint), status = ?res.status());
            Err(Error::Warp10(
                res.headers()
                    .get("X-Warp10-Error")
                    .unwrap_or(&HeaderValue::from_static(""))
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            ))
        }
    }
}
