use std::cell::OnceCell;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use time::OffsetDateTime;
use warp10::Warp10Serializable;

use crate::broker_rpc::check_result::Details;

tonic::include_proto!("isok.broker.rpc");

const GTS_DEFAULT_PREFIX: &str = "isok.check.";
const GTS_PREFIX: OnceCell<String> = OnceCell::new();

/// See [CheckJobStatus] for values
const GTS_STATUS: &str = "status";

const GTS_LATENCY: &str = "latency";

const GTS_HTTP_STATUS_CODE: &str = "http.status_code";

/// Represents the name of a Warp10 Series, it represents a combination
/// of a prefix defined by global config or equal to [GTS_DEFAULT_PREFIX]
/// and a suffix which is the name of the metric.
struct GtsClassName(String);

impl GtsClassName {
    fn with_prefix(suffix: &str) -> String {
        let cell = GTS_PREFIX;
        let gts = cell.get_or_init(|| GTS_DEFAULT_PREFIX.to_string());
        format!("{}.{}", gts, suffix)
    }

    fn new(suffix: &str) -> Self {
        Self(Self::with_prefix(suffix))
    }
}

impl From<&str> for GtsClassName {
    fn from(value: &str) -> Self {
        Self(Self::with_prefix(value))
    }
}

impl Display for GtsClassName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type BrokerGrpcClient = broker_client::BrokerClient<tonic::transport::Channel>;

impl CheckResult {
    pub fn warp10_serialize(&self, extra_labels: &HashMap<String, String>) -> String {
        self.build_warp10_metrics(extra_labels)
            .iter()
            .map(|m| m.warp10_serialize())
            .fold(String::new(), |acc, cur| {
                if acc.is_empty() {
                    cur
                } else {
                    (acc + "\n") + &cur
                }
            })
    }
    fn build_warp10_metrics(&self, extra_labels: &HashMap<String, String>) -> Vec<warp10::Data> {
        let mut labels = self.labels_from_tags();
        labels.push(warp10::Label::new("id", &self.id_ulid));

        if let Some(pretty_name) = &self.pretty_name {
            labels.push(warp10::Label::new("pretty_name", &pretty_name));
        }

        labels.extend(
            extra_labels
                .into_iter()
                .map(|(k, v)| warp10::Label::new(&k, &v)),
        );
        let time = match self.run_at {
            Some(run_at) => OffsetDateTime::from_unix_timestamp(run_at.seconds).unwrap(),
            None => OffsetDateTime::now_utc(),
        };
        let mut metrics = Vec::new();

        for (gts, value) in self.build_gts_pairs() {
            metrics.push(warp10::Data::new(
                time,
                None,
                gts.to_string(),
                labels.clone(),
                value,
            ));
        }

        if let Some(job_metrics) = &self.metrics {
            for (gts, value) in job_metrics.build_gts_pairs() {
                metrics.push(warp10::Data::new(
                    time,
                    None,
                    gts.to_string(),
                    labels.clone(),
                    value,
                ));
            }
        }

        metrics
    }

    fn labels_from_tags(&self) -> Vec<warp10::Label> {
        let mut labels = Vec::new();
        if let Some(tags) = &self.tags {
            labels.push(warp10::Label::new("zone", &tags.zone));
            labels.push(warp10::Label::new("region", &tags.region));
            labels.push(warp10::Label::new("agent_id", &tags.agent_id));
            return labels;
        }

        labels
    }

    fn build_gts_pairs(&self) -> Vec<(GtsClassName, warp10::Value)> {
        let mut pairs = Vec::new();

        pairs.push((
            GtsClassName::new(GTS_STATUS),
            warp10::Value::from(self.status as i64),
        ));

        if let Some(details) = &self.details {
            pairs.extend(details.build_gts_pairs());
        }
        pairs
    }
}

impl Details {
    fn build_gts_pairs(&self) -> Vec<(GtsClassName, warp10::Value)> {
        match self {
            Details::DetailsHttp(http) => {
                let gts = GtsClassName::new(GTS_HTTP_STATUS_CODE);
                vec![(gts, warp10::Value::from(http.status_code as i64))]
            }
            _ => Vec::new(),
        }
    }
}

impl CheckJobMetrics {
    fn build_gts_pairs(&self) -> Vec<(GtsClassName, warp10::Value)> {
        let mut pairs = Vec::new();
        if let Some(latency) = &self.latency {
            pairs.push((
                GtsClassName::new(GTS_LATENCY),
                warp10::Value::from(*latency as i64),
            ));
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JobId;
    use pretty_assertions::assert_eq;
    use prost_types::Timestamp;

    #[test]
    fn test_gts_class_name() {
        let gts_class_name = GtsClassName::new("prefix");

        assert_eq!(
            gts_class_name.to_string(),
            format!("{}.prefix", GTS_DEFAULT_PREFIX)
        );
    }

    #[test]
    fn test_build_warp10_metrics() {
        let result = CheckResult {
            id_ulid: "01ARZ3NDEKTSV4RRWETS2PGZ5M".to_string(),
            run_at: Some(Timestamp::date(2025, 1, 1).unwrap()),
            status: CheckJobStatus::Reachable.into(),
            metrics: Default::default(),
            tags: None,
            details: Default::default(),
            pretty_name: Some("test".to_string()),
        };

        let metrics = result.warp10_serialize(&HashMap::new());
        assert_eq!(metrics, "1735689600000000// isok%2Echeck%2E%2Estatus{id=01ARZ3NDEKTSV4RRWETS2PGZ5M,pretty%5Fname=test} 1");
    }

    #[test]
    fn test_build_warp10_metrics_with_labels() {
        let result = CheckResult {
            id_ulid: "01ARZ3NDEKTSV4RRWETS2PGZ5M".to_string(),
            run_at: Some(Timestamp::date(2025, 1, 1).unwrap()),
            status: CheckJobStatus::Reachable.into(),
            metrics: Default::default(),
            tags: Some(Tags {
                zone: "dev".to_string(),
                region: "localhost".to_string(),
                agent_id: "test".to_string(),
            }),
            details: Default::default(),
            pretty_name: Some("test".to_string()),
        };

        let metrics = result.warp10_serialize(&HashMap::new());
        assert_eq!(metrics, "1735689600000000// isok%2Echeck%2E%2Estatus{zone=dev,region=localhost,agent%5Fid=test,id=01ARZ3NDEKTSV4RRWETS2PGZ5M,pretty%5Fname=test} 1");
    }

    #[test]
    fn test_build_warp10_metrics_http_job() {
        let result = CheckResult {
            id_ulid: "some_id".to_string(),
            run_at: Some(Timestamp::date(2025, 1, 1).unwrap()),
            status: CheckJobStatus::Reachable.into(),
            metrics: Default::default(),
            tags: None,
            details: Some(Details::DetailsHttp(JobDetailsHttp { status_code: 200 })),
            pretty_name: Some("test".to_string()),
        };

        let metrics = result.warp10_serialize(&HashMap::new());
        // split by newline to avoid windows line endings
        let metrics = metrics.split("\n").collect::<Vec<_>>();
        assert_eq!(metrics.len(), 2);
        assert_eq!(
            metrics[0],
            "1735689600000000// isok%2Echeck%2E%2Estatus{id=some%5Fid,pretty%5Fname=test} 1"
        );
        assert_eq!(
            metrics[1],
            "1735689600000000// isok%2Echeck%2E%2Ehttp%2Estatus%5Fcode{id=some%5Fid,pretty%5Fname=test} 200"
        );
    }

    #[test]
    fn test_warp10_serialize_correct_time() {
        let result = CheckResult {
            id_ulid: JobId::generate().to_string(),
            pretty_name: Some("test".to_string()),
            run_at: Some(Timestamp::date(2025, 1, 1).unwrap()),
            status: CheckJobStatus::Reachable.into(),
            metrics: Default::default(),
            tags: None,
            details: None,
        };

        let metrics = result.warp10_serialize(&HashMap::new());
        let time = metrics.split("\n").collect::<Vec<_>>()[0]
            .split("//")
            .collect::<Vec<_>>()[0];
        assert_eq!(time, "1735689600000000");
    }
}
