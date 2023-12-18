use std::sync::{Arc, Mutex};
use sqlx::{PgPool, Pool, Postgres};
use ping_data::check::Check;

pub mod api {
    pub async fn list_checks() {
        todo!()
    }

    pub async fn get_check() {
        todo!()
    }

    pub async fn create_check() {
        todo!()
    }

    pub async fn update_check() {
        todo!()
    }

    pub async fn delete_check() {
        todo!()
    }
}

struct CheckRepository {
    pool: PgPool
}

impl CheckRepository {
    pub fn new(pool: PgPool) -> Self {
        CheckRepository { pool }
    }

    pub async fn find_all(&self) -> Vec<Check> {
        todo!()
    }
}
