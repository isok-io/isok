pub mod stdout;
pub mod warp10;

use async_trait::async_trait;
use isok_data::models::CheckResult;
use std::collections::HashMap;

#[async_trait]
pub trait Exporter {
    async fn send_result(&self, result: CheckResult, partition: i32, offset: i64);

    async fn get_commited(&self) -> HashMap<i32, i64>;
}
