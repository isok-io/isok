use crate::Result;
use crate::db::DbHandler;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

impl DbHandler {
    pub async fn tenants_insert_tenant(
        &self,
        id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<()> {
        sqlx::query!(r#"insert into tenants values ($1)"#, id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    pub async fn tenants_delete_tenant(
        &self,
        id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<()> {
        sqlx::query!("delete from tenants where id = $1", id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }
}
