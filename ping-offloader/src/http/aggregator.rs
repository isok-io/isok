use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use time::{OffsetDateTime};
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;
use ping_data::check_kinds::http::HttpFields;
use ping_data::pulsar_messages::{CheckData, CheckResult};

pub struct Aggregator {
    http_receiver: Receiver<CheckData<HttpFields>>,
    buffer: HashMap<Uuid, HashMap<String, CheckData<HttpFields>>>,
}

impl Aggregator {
    pub async fn run(&mut self) {
        while let Some(check_data) = self.http_receiver.recv().await.ok() {
            if let Some(check_buffer) = self.buffer.get_mut(&check_data.check_id) {
                match check_buffer.get(&check_data.agent_id) {
                    Some(_) => {
                        let mut check_result: CheckResult<HttpFields> = CheckResult {
                            fields: HttpFields {
                                status_code: 0
                            },
                            latency: Duration::from_secs(0),
                            timestamp: OffsetDateTime::now_utc(),
                        };
                        for data in check_buffer.values() {
                            check_result.fields.status_code = check_result.fields.status_code.max(data.fields.status_code);
                            check_result.latency = check_result.latency.max(data.latency);
                            check_result.timestamp = data.timestamp;
                        }
                        //todo: send data to something like pulsar
                    }
                    None => {
                        check_buffer.insert(check_data.agent_id, check_data.clone());
                    }
                }
            } else {
                let mut hm = HashMap::new();
                hm.insert(check_data.agent_id, check_data.clone());
                self.buffer.insert(check_data.check_id, hm);
            }
        }
    }
}
