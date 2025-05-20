use crate::models::CheckName;
use chrono::{DateTime, Utc};
use http::Method;
use lazy_static::lazy_static;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::string::ToString;
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize, JsonSchema)]
pub struct Check {
    pub id: Uuid,
    pub interval: Duration,
    pub name: String,
    pub kind: CheckKind,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub enum CheckKind {
    Http(HttpCheck),
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HttpCheck {
    #[serde(with = "http_serde::method")]
    #[schemars(with = "String")]
    pub method: Method,
    #[serde(with = "http_serde::uri")]
    #[schemars(with = "String")]
    pub url: http::Uri,
    #[serde(with = "http_serde::header_map")]
    #[schemars(with = "String")]
    pub headers: http::HeaderMap,
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
    pub latency: u64,
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
    pub run_at: DateTime<Utc>,
    pub status: CheckStatus,
    pub metrics: CheckMetrics,
    pub error: Option<String>,
    pub details: CheckResultDetails,
}

#[derive(Deserialize, JsonSchema)]
pub struct ApiCheckInput {
    pub interval: Duration,
    #[schemars(with = "String")]
    pub name: CheckName,
    pub kind: CheckKind,
    pub zones: Vec<CheckZone>,
}

pub struct ApiCheck {
    pub inner: Check,
    pub tenant: Uuid,
    pub zones: Vec<Uuid>,
}

#[derive(Deserialize, JsonSchema)]
pub enum CheckZone {
    All,
    Region(Uuid),
    Zone(Uuid),
}

pub type ApiCheckMetrics = Vec<ApiCheckResult>;

pub type ApiChecksSummary = HashMap<Uuid, ApiCheckResult>;

#[derive(Serialize, JsonSchema)]
pub struct ApiCheckResult {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub status: CheckStatus,
    pub metrics: CheckMetrics,
    pub error: Option<String>,
    pub details: CheckResultDetails,
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
                    ]
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
                    value_placeholder: Some("Value".to_string())
                })
            }
        ],
    };
}

#[derive(Serialize, JsonSchema)]
pub struct CheckSchema {
    pub version: usize,
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
}
