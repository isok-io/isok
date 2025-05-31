use crate::messages;
use crate::messages::check_result_details::Details;
use crate::models;
use chrono::DateTime;
use http::StatusCode;
use prost_types::{Duration, Timestamp};
use std::str::FromStr;
use uuid::Uuid;

impl Into<messages::CheckMetrics> for models::CheckMetrics {
    fn into(self) -> messages::CheckMetrics {
        messages::CheckMetrics {
            latency: Some(Duration::try_from(self.latency).unwrap()),
        }
    }
}

impl Into<messages::HttpCheckResult> for models::HttpCheckResult {
    fn into(self) -> messages::HttpCheckResult {
        messages::HttpCheckResult {
            status_code: self.status_code.as_u16() as u32,
        }
    }
}

impl Into<messages::check_result_details::Details> for models::CheckResultDetails {
    fn into(self) -> messages::check_result_details::Details {
        match self {
            models::CheckResultDetails::Http(http_check_result) => {
                messages::check_result_details::Details::Http(http_check_result.into())
            }
        }
    }
}

impl Into<messages::CheckResultDetails> for models::CheckResultDetails {
    fn into(self) -> messages::CheckResultDetails {
        messages::CheckResultDetails {
            details: Some(self.into()),
        }
    }
}

impl Into<messages::CheckResult> for models::CheckResult {
    fn into(self) -> messages::CheckResult {
        messages::CheckResult {
            id: self.id.to_string(),
            agent_id: self.agent_id,
            zone: self.zone.to_string(),
            run_at: Some(Timestamp {
                seconds: self.run_at.timestamp(),
                nanos: 0,
            }),
            status: self.status as i32,
            metrics: Some(self.metrics.into()),
            error: self.error,
            details: self.details.map(Into::into),
        }
    }
}

#[derive(Debug)]
pub enum TryFromError {
    MissingField(&'static str),
    InvalidUuid(uuid::Error),
    InvalidTimestamp,
    InvalidStatusCode(http::status::InvalidStatusCode),
}

impl From<TryFromError> for prost::DecodeError {
    fn from(value: TryFromError) -> Self {
        Self::new(format!("{value:?}"))
    }
}

impl From<uuid::Error> for TryFromError {
    fn from(value: uuid::Error) -> Self {
        Self::InvalidUuid(value)
    }
}

impl From<http::status::InvalidStatusCode> for TryFromError {
    fn from(value: http::status::InvalidStatusCode) -> Self {
        Self::InvalidStatusCode(value)
    }
}

impl From<messages::CheckStatus> for models::CheckStatus {
    fn from(value: messages::CheckStatus) -> Self {
        match value {
            messages::CheckStatus::Unknown => Self::Unknown,
            messages::CheckStatus::Reachable => Self::Reachable,
            messages::CheckStatus::Unreachable => Self::Unreachable,
            messages::CheckStatus::Timeout => Self::Timeout,
        }
    }
}

impl TryFrom<messages::CheckMetrics> for models::CheckMetrics {
    type Error = TryFromError;

    fn try_from(value: messages::CheckMetrics) -> Result<Self, Self::Error> {
        let latency = value
            .latency
            .ok_or(TryFromError::MissingField("CheckMetrics::latency"))?;

        let res = Self {
            latency: std::time::Duration::new(latency.seconds as u64, latency.nanos as u32),
        };

        Ok(res)
    }
}

impl TryFrom<messages::CheckResultDetails> for models::CheckResultDetails {
    type Error = TryFromError;

    fn try_from(value: messages::CheckResultDetails) -> Result<Self, Self::Error> {
        let res = match value
            .details
            .ok_or(TryFromError::MissingField("CheckResultDetails::details"))?
        {
            Details::Http(http) => Self::Http(models::HttpCheckResult {
                status_code: StatusCode::from_u16(http.status_code as u16)?,
            }),
        };

        Ok(res)
    }
}

impl TryFrom<messages::CheckResult> for models::CheckResult {
    type Error = TryFromError;

    fn try_from(value: messages::CheckResult) -> Result<Self, Self::Error> {
        let run_at = value
            .run_at
            .ok_or(TryFromError::MissingField("CheckResult::run_at"))?;
        let message = Self {
            id: Uuid::from_str(&value.id)?,
            zone: Uuid::from_str(&value.zone)?,
            agent_id: value.agent_id.clone(),
            run_at: DateTime::from_timestamp(run_at.seconds, run_at.nanos as u32)
                .ok_or(TryFromError::InvalidTimestamp)?
                .to_utc(),
            status: value.status().into(),
            metrics: value
                .metrics
                .ok_or(TryFromError::MissingField("CheckResult::metrics"))?
                .try_into()?,
            error: value.error,
            details: if let Some(details) = value.details {
                Some(details.try_into()?)
            } else {
                None
            },
        };

        Ok(message)
    }
}
