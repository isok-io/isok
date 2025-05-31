use crate::db::DbHandler;
use crate::errors::Result;
use isok_data::models::{Agent, Tags};
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

impl DbHandler {
    async fn agents_get_tags_by_id(&self, id: &str) -> Result<Tags> {
        let tags = sqlx::query!("SELECT key, value FROM agents_tags WHERE agent = $1", id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();

        Ok(tags)
    }

    pub async fn agents_get_all(
        &self,
        where_healthchecked_at_lte: Option<DateTime<Utc>>,
    ) -> Result<Vec<Agent>> {
        let agent_records = sqlx::query!(
            "select id, zone, endpoint, token, added_at, healthchecked_at from agents where healthchecked_at <= $1",
            where_healthchecked_at_lte.unwrap_or(DateTime::<Utc>::MAX_UTC)
        )
            .fetch_all(&self.pool)
            .await?;

        let mut res = Vec::with_capacity(agent_records.len());
        for rec in agent_records {
            let tags = self.agents_get_tags_by_id(&rec.id).await?;
            res.push(Agent {
                id: rec.id,
                zone: rec.zone,
                endpoint: rec.endpoint,
                token: rec.token,
                added_at: rec.added_at,
                healthchecked_at: rec.healthchecked_at,
                tags,
            });
        }

        Ok(res)
    }

    pub async fn agents_get_by_id(&self, id: &str) -> Result<Option<Agent>> {
        let rec = sqlx::query!("select id, zone, endpoint, token, added_at, healthchecked_at from agents where id = $1", id)
            .fetch_optional(&self.pool)
            .await?;

        let res = match rec {
            Some(rec) => {
                let tags = self.agents_get_tags_by_id(&rec.id).await?;

                Some(Agent {
                    id: rec.id,
                    zone: rec.zone,
                    endpoint: rec.endpoint,
                    token: rec.token,
                    added_at: rec.added_at,
                    healthchecked_at: rec.healthchecked_at,
                    tags,
                })
            }
            None => None,
        };

        Ok(res)
    }

    pub async fn agents_get_by_check(&self, check: Uuid) -> Result<Vec<Agent>> {
        let agent_records = sqlx::query!(
            r#"select id, zone, endpoint, token, added_at, healthchecked_at from agents a
               join agents_checks ac on a.id = ac.agent where ac."check" = $1"#,
            check
        )
        .fetch_all(&self.pool)
        .await?;

        let mut res = Vec::with_capacity(agent_records.len());
        for rec in agent_records {
            let tags = self.agents_get_tags_by_id(&rec.id).await?;
            res.push(Agent {
                id: rec.id,
                zone: rec.zone,
                endpoint: rec.endpoint,
                token: rec.token,
                added_at: rec.added_at,
                healthchecked_at: rec.healthchecked_at,
                tags,
            });
        }

        Ok(res)
    }

    pub async fn agents_insert(&self, agent: &Agent) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "insert into agents (id, zone, endpoint, token, added_at, healthchecked_at) values ($1, $2, $3, $4, $5, $6)",
            agent.id,
            agent.zone,
            agent.endpoint,
            agent.token,
            agent.added_at,
            agent.healthchecked_at
        )
            .execute(&mut *tx)
            .await?;

        for (key, value) in &agent.tags {
            sqlx::query!(
                "insert into agents_tags (agent, key, value) values ($1, $2, $3)",
                &agent.id,
                key,
                value.as_ref()
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    pub async fn agents_delete(&self, id: String) -> Result<Vec<Uuid>> {
        let mut tx = self.pool.begin().await?;

        let checks = sqlx::query!(
            r#"delete from agents_checks where agent = $1 returning "check""#,
            &id
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|rec| rec.check)
        .collect();

        sqlx::query!("delete from agents_tags where agent = $1", &id)
            .execute(&mut *tx)
            .await?;

        sqlx::query!("delete from agents where id = $1", id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(checks)
    }

    pub async fn agents_update_healthchecked_at(
        &self,
        id: &str,
        healthchecked_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query!(
            "update agents set healthchecked_at = $1 where id = $2",
            healthchecked_at,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn agents_get_checks(&self, agent: &str) -> Result<Vec<Uuid>> {
        let res = sqlx::query!(
            r#"select "check" from agents_checks where agent = $1"#,
            agent
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|rec| rec.check)
        .collect();

        Ok(res)
    }

    pub async fn agents_add_checks(
        &self,
        agent: &str,
        checks: Vec<Uuid>,
    ) -> Result<Transaction<Postgres>> {
        let mut tx = self.pool.begin().await?;

        for check in checks {
            sqlx::query!(r#"insert into agents_checks values ($1, $2)"#, agent, check)
                .execute(&mut *tx)
                .await?;
        }

        Ok(tx)
    }

    pub async fn agents_delete_checks(&self, agent: &str, checks: &Vec<Uuid>) -> Result<()> {
        sqlx::query!(r#"delete from agents_checks where agent = $1 and "check" in (select * from unnest($2::uuid[]))"#, agent, checks)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn agents_get_incomplete_checks(&self) -> Result<Vec<Uuid>> {
        let res = sqlx::query!(r#"
            with wregions as (select cz."check" as id, z.id as zone
                        from checks_zones cz
                                 join zones z on cz.region = z.region),
            wzones as (select cz."check" as id, cz.zone as zone from checks_zones cz where kind = 'zone'),
            wall as (select cz."check" as id, z.id as zone
                      from checks_zones cz
                               join zones z on true
                      where cz.kind = 'all'),
            wcurrent as (select ac."check" as id, a.zone as zone
                        from agents_checks ac
                                 join agents a on ac.agent = a.id)
            (select id, zone from wregions union select id, zone from wzones union select id, zone from wall)
            except select id, zone from wcurrent"#)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|rec| rec.id)
            .collect();

        Ok(res)
    }

    pub async fn agents_get_incomplete_checks_by_zone(&self, zone: Uuid) -> Result<Vec<Uuid>> {
        let res = sqlx::query!(r#"
            with wregions as (select cz."check" as id, z.id as zone
                        from checks_zones cz
                                 join zones z on cz.region = z.region),
            wzones as (select cz."check" as id, cz.zone as zone from checks_zones cz where kind = 'zone'),
            wall as (select cz."check" as id, z.id as zone
                      from checks_zones cz
                               join zones z on true
                      where cz.kind = 'all'),
            wcurrent as (select ac."check" as id, a.zone as zone
                        from agents_checks ac
                                 join agents a on ac.agent = a.id)
            (select id, zone from wregions union select id, zone from wzones union select id, zone from wall)
            except select id, zone from wcurrent
            where zone = $1"#, zone)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|rec| rec.id)
            .collect();

        Ok(res)
    }
}
