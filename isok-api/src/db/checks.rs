use crate::db::DbHandler;
use crate::errors::Result;
use isok_data::models::{ApiCheck, Check, CheckKind, CheckZone};
use sqlx::postgres::types::PgInterval;
use sqlx::types::Json;
use sqlx::{Postgres, Transaction};
use std::ops::Add;
use std::time::Duration;
use uuid::Uuid;

#[derive(sqlx::Type)]
#[sqlx(type_name = "checks_zone_kind", rename_all = "lowercase")]
enum CheckZoneKind {
    All,
    Region,
    Zone,
}

struct ChecksZone {
    check: Uuid,
    kind: CheckZoneKind,
    region: Option<Uuid>,
    zone: Option<Uuid>,
}

impl From<ChecksZone> for CheckZone {
    fn from(val: ChecksZone) -> Self {
        match val.kind {
            CheckZoneKind::All => CheckZone::All,
            CheckZoneKind::Region => CheckZone::Region(val.region.unwrap()),
            CheckZoneKind::Zone => CheckZone::Zone(val.zone.unwrap()),
        }
    }
}

impl DbHandler {
    async fn checks_get_check_zones(&self, id: Uuid) -> Result<Vec<ChecksZone>> {
        sqlx::query_as!(
            ChecksZone,
            r#"select "check", kind as "kind: CheckZoneKind", region, zone from checks_zones where "check" = $1"#, 
            id
        )
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn checks_get_by_ids(&self, ids: Vec<Uuid>) -> Result<Vec<ApiCheck>> {
        let recs = sqlx::query!(
            r#"select id, interval, name, tenant, kind as "kind: Json<CheckKind>" from checks where id in (select * from unnest($1::uuid[]))"#,
            &ids
        ).fetch_all(&self.pool).await?;

        let mut res = Vec::with_capacity(recs.len());
        for rec in recs {
            let interval: Duration = Duration::from_micros(rec.interval.microseconds as u64)
                .add(Duration::from_secs(rec.interval.days as u64 * 3600 * 24));
            res.push(ApiCheck {
                inner: Check {
                    id: rec.id,
                    interval,
                    kind: rec.kind.0,
                },
                name: rec.name,
                tenant: rec.tenant,
                zones: self
                    .checks_get_check_zones(rec.id)
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            });
        }

        Ok(res)
    }

    pub async fn checks_get_by_tenant(&self, tenant: Uuid) -> Result<Vec<ApiCheck>> {
        let recs = sqlx::query!(
            r#"select id, interval, name, tenant, kind as "kind: Json<CheckKind>" from checks where tenant = $1"#,
            tenant
        ).fetch_all(&self.pool).await?;

        let mut res = Vec::with_capacity(recs.len());
        for rec in recs {
            let interval: Duration = Duration::from_micros(rec.interval.microseconds as u64)
                .add(Duration::from_secs(rec.interval.days as u64 * 3600 * 24));
            res.push(ApiCheck {
                inner: Check {
                    id: rec.id,
                    interval,
                    kind: rec.kind.0,
                },
                name: rec.name,
                tenant: rec.tenant,
                zones: self
                    .checks_get_check_zones(rec.id)
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            });
        }

        Ok(res)
    }

    async fn checks_insert_checks_zones(
        &self,
        check: Uuid,
        cz: &Vec<CheckZone>,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<()> {
        for cz in cz {
            let cz = match cz {
                CheckZone::All => ChecksZone {
                    check,
                    kind: CheckZoneKind::All,
                    region: None,
                    zone: None,
                },
                CheckZone::Region(region) => ChecksZone {
                    check,
                    kind: CheckZoneKind::Region,
                    region: Some(*region),
                    zone: None,
                },
                CheckZone::Zone(zone) => ChecksZone {
                    check,
                    kind: CheckZoneKind::Zone,
                    region: None,
                    zone: Some(*zone),
                },
            };
            sqlx::query!(
                r#"insert into checks_zones ("check", kind, region, zone) values ($1, $2, $3, $4)"#,
                cz.check,
                cz.kind as CheckZoneKind,
                cz.region,
                cz.zone
            )
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    pub async fn checks_insert_check(&self, check: &ApiCheck) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let kind = sqlx::types::Json(check.inner.kind.clone());
        let itv: PgInterval = check.inner.interval.try_into().unwrap();

        sqlx::query!(
            "insert into checks (id, interval, name, tenant, kind) values ($1, $2, $3, $4, $5)",
            check.inner.id,
            itv,
            check.name,
            check.tenant,
            kind as Json<CheckKind>
        )
        .execute(&mut *tx)
        .await?;

        self.checks_insert_checks_zones(check.inner.id, &check.zones, &mut tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn checks_delete_by_id(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(r#"delete from checks_zones where "check" = $1"#, id)
            .execute(&mut *tx)
            .await?;

        sqlx::query!("delete from checks where id = $1", id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn checks_is_tenant(&self, check: Uuid, tenant: Uuid) -> Result<bool> {
        let res = sqlx::query!(
            r#"select 1 as a from checks where id = $1 and tenant = $2"#,
            check,
            tenant
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(res.is_some())
    }
}
