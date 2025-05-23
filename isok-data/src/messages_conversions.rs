use crate::messages;
use crate::models;

use prost_types::{Duration, Timestamp};

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
