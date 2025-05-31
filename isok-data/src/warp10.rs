use crate::models::{CheckMetrics, CheckResult, CheckResultDetails};
use std::cell::OnceCell;
use std::fmt::{Display, Formatter};
use time::OffsetDateTime;
use warp10::Warp10Serializable;

const GTS_DEFAULT_PREFIX: &str = "isok.check";
const GTS_PREFIX: OnceCell<String> = OnceCell::new();

/// See [CheckJobStatus] for values
const GTS_STATUS: &str = "status";

const GTS_LATENCY: &str = "latency";

const GTS_ERROR: &str = "error";

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

impl CheckResult {
    pub fn warp10_serialize(&self) -> String {
        self.build_warp10_metrics()
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
    fn build_warp10_metrics(&self) -> Vec<warp10::Data> {
        let labels = vec![
            warp10::Label::new("id", &self.id.as_simple().to_string()),
            warp10::Label::new("zone", &self.zone.as_simple().to_string()),
        ];

        let time = OffsetDateTime::from_unix_timestamp_nanos(
            self.run_at.timestamp_millis() as i128 * 1000 * 1000,
        )
        .unwrap();
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

        for (gts, value) in self.metrics.build_gts_pairs() {
            metrics.push(warp10::Data::new(
                time,
                None,
                gts.to_string(),
                labels.clone(),
                value,
            ));
        }

        metrics
    }

    fn build_gts_pairs(&self) -> Vec<(GtsClassName, warp10::Value)> {
        let mut pairs = Vec::with_capacity(2);

        pairs.push((
            GtsClassName::new(GTS_STATUS),
            warp10::Value::from(self.status as i64),
        ));

        if let Some(details) = &self.details {
            pairs.extend(details.build_gts_pairs());
        }

        if let Some(error) = &self.error {
            pairs.push((
                GtsClassName::new(GTS_ERROR),
                warp10::Value::from(error.clone()),
            ));
        }
        pairs
    }
}

impl CheckResultDetails {
    fn build_gts_pairs(&self) -> Vec<(GtsClassName, warp10::Value)> {
        match self {
            CheckResultDetails::Http(http) => {
                let gts = GtsClassName::new(GTS_HTTP_STATUS_CODE);
                vec![(gts, warp10::Value::from(http.status_code.as_u16() as i64))]
            }
        }
    }
}

impl CheckMetrics {
    fn build_gts_pairs(&self) -> Vec<(GtsClassName, warp10::Value)> {
        let pairs = vec![(
            GtsClassName::new(GTS_LATENCY),
            warp10::Value::from(self.latency.as_millis() as i64),
        )];

        pairs
    }
}
