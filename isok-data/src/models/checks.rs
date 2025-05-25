use crate::models::CheckName;
use crate::models::DurationSchema;
use crate::models::NameSchema;
use crate::models::duration_secs;
use chrono::{DateTime, Utc};
use http::Method;
use lazy_static::lazy_static;
use refined::Refinement;
use refined::boundable::unsigned::ClosedInterval;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::string::ToString;
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
pub struct Check {
    pub id: Uuid,
    #[serde(with = "duration_secs")]
    #[schemars(with = "DurationSchema<5, { 24 * 3600 }>")]
    pub interval: Duration,
    pub kind: CheckKind,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub enum CheckKind {
    Http(HttpCheck),
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct HttpCheck {
    #[serde(with = "http_serde::method")]
    #[schemars(with = "String")]
    pub method: Method,
    #[serde(with = "http_serde::uri")]
    #[schemars(with = "String")]
    pub url: http::Uri,
    #[serde(with = "http_serde::header_map")]
    #[schemars(with = "HashMap<String, String>")]
    pub headers: http::HeaderMap,
    pub body: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub enum CheckStatus {
    Unknown,
    Reachable,
    Unreachable,
    Timeout,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckMetrics {
    #[serde(with = "duration_secs")]
    #[schemars(with = "DurationSchema<5, { 24 * 3600 }>")]
    pub latency: Duration,
}

#[derive(Serialize, JsonSchema)]
pub struct HttpCheckResult {
    #[serde(with = "http_serde::status_code")]
    #[schemars(with = "u16")]
    pub status_code: http::StatusCode,
}

#[derive(Serialize, JsonSchema)]
pub enum CheckResultDetails {
    Http(HttpCheckResult),
}

pub struct CheckResult {
    pub id: Uuid,
    pub zone: Uuid,
    pub agent_id: String,
    pub run_at: DateTime<Utc>,
    pub status: CheckStatus,
    pub metrics: CheckMetrics,
    pub error: Option<String>,
    pub details: Option<CheckResultDetails>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ApiCheckInput {
    #[schemars(with = "DurationSchema<5, { 24 * 3600 }>")]
    pub interval: Refinement<u64, ClosedInterval<5, { 24 * 3600 }>>,
    #[schemars(with = "NameSchema<1>")]
    pub name: CheckName,
    pub kind: CheckKind,
    pub zones: Vec<CheckZone>,
}

#[derive(Serialize, JsonSchema)]
pub struct ApiCheck {
    #[serde(flatten)]
    pub inner: Check,
    pub name: String,
    pub tenant: Uuid,
    pub zones: Vec<CheckZone>,
}

impl ApiCheck {
    pub fn from_input(value: ApiCheckInput, id: Uuid, tenant: Uuid) -> Self {
        Self {
            inner: Check {
                id,
                interval: Duration::from_secs(*value.interval),
                kind: value.kind,
            },
            name: value.name.to_string(),
            tenant,
            zones: value.zones,
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub enum CheckZone {
    All,
    Region(Uuid),
    Zone(Uuid),
}

pub type ApiCheckMetrics = Vec<Option<ApiCheckResult>>;

pub type ApiChecksSummary = HashMap<Uuid, ApiCheckMetrics>;

#[derive(Serialize, JsonSchema)]
pub struct ApiCheckResult {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub status: ApiCheckStatus,
    pub metrics: CheckMetrics,
    pub error: Option<String>,
    pub details: CheckResultDetails,
}

#[derive(Serialize, JsonSchema)]
pub enum ApiCheckStatus {
    None,
    Reachable,
    Unreachable,
    ReachableUnreachable,
}

lazy_static! {
    pub static ref CHECK_SCHEMA_HTTP_V1: CheckSchema = CheckSchema {
        version: 1,
        kind: CheckSchemaCheckKind::Http,
        inputs: vec![CheckSchemaInput {
            title: "URL".to_string(),
            kind: CheckSchemaInputKind::Text(CheckSchemaTextInput {
                default_value: None,
                placeholder: Some("https://example.com".to_string()),
                variant: CheckSchemaTextInputVariant::Url,
            }),
        }],
        inputs_advanced: vec![
            CheckSchemaInput {
                title: "Method".to_string(),
                kind: CheckSchemaInputKind::Select(CheckSchemaSelectInput {
                    select_options: vec![
                        CheckSchemaSelectInputOption {
                            label: Method::GET.to_string(),
                            value: Method::GET.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::POST.to_string(),
                            value: Method::POST.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::PUT.to_string(),
                            value: Method::PUT.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::DELETE.to_string(),
                            value: Method::DELETE.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::PATCH.to_string(),
                            value: Method::PATCH.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::HEAD.to_string(),
                            value: Method::HEAD.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::OPTIONS.to_string(),
                            value: Method::OPTIONS.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::CONNECT.to_string(),
                            value: Method::CONNECT.to_string()
                        },
                        CheckSchemaSelectInputOption {
                            label: Method::TRACE.to_string(),
                            value: Method::TRACE.to_string()
                        }
                    ],
                    default_value: Some(CheckSchemaSelectInputOption {
                        label: Method::GET.to_string(),
                        value: Method::GET.to_string()
                    }),
                }),
            },
            CheckSchemaInput {
                title: "Body".to_string(),
                kind: CheckSchemaInputKind::Text(CheckSchemaTextInput {
                    default_value: None,
                    placeholder: Some("{}".to_string()),
                    variant: CheckSchemaTextInputVariant::Area,
                })
            },
            CheckSchemaInput {
                title: "Headers".to_string(),
                kind: CheckSchemaInputKind::KeyValue(CheckSchemaKeyValueInput {
                    key_placeholder: Some("Key".to_string()),
                    value_placeholder: Some("Value".to_string()),
                    default_value: Default::default(),
                })
            }
        ],
    };
}

#[derive(Serialize, JsonSchema)]
pub struct CheckSchema {
    pub version: usize,
    #[serde(rename = "type")]
    pub kind: CheckSchemaCheckKind,
    pub inputs: Vec<CheckSchemaInput>,
    pub inputs_advanced: Vec<CheckSchemaInput>,
}

#[derive(Serialize, JsonSchema)]
pub enum CheckSchemaCheckKind {
    Http,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckSchemaInput {
    pub title: String,
    pub kind: CheckSchemaInputKind,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type")]
pub enum CheckSchemaInputKind {
    Text(CheckSchemaTextInput),
    Select(CheckSchemaSelectInput),
    KeyValue(CheckSchemaKeyValueInput),
}

#[derive(Serialize, JsonSchema)]
pub struct CheckSchemaTextInput {
    pub default_value: Option<String>,
    pub placeholder: Option<String>,
    pub variant: CheckSchemaTextInputVariant,
}

#[derive(Serialize, JsonSchema)]
pub enum CheckSchemaTextInputVariant {
    Text,
    Url,
    Area,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckSchemaSelectInput {
    pub select_options: Vec<CheckSchemaSelectInputOption>,
    pub default_value: Option<CheckSchemaSelectInputOption>,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckSchemaSelectInputOption {
    pub label: String,
    pub value: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckSchemaKeyValueInput {
    pub key_placeholder: Option<String>,
    pub value_placeholder: Option<String>,
    pub default_value: HashMap<String, String>,
}
