use crate::Result;
use crate::db::DbHandler;
use isok_data::models::{CheckZone, Region, Tags, Zone};
use uuid::Uuid;

impl DbHandler {
    async fn regions_get_tags_by_id(&self, id: Uuid) -> Result<Tags> {
        let tags = sqlx::query!(
            r#"SELECT key, value FROM regions_tags WHERE region = $1"#,
            id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|e| (e.key, e.value))
        .collect();

        Ok(tags)
    }

    async fn zones_get_tags_by_id(&self, id: Uuid) -> Result<Tags> {
        let tags = sqlx::query!(r#"SELECT key, value FROM zones_tags WHERE zone = $1"#, id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();

        Ok(tags)
    }

    pub async fn regions_get_all(&self) -> Result<Vec<Region>> {
        let rec = sqlx::query!("select id, name from regions")
            .fetch_all(&self.pool)
            .await?;

        let mut regions = Vec::with_capacity(rec.len());

        for r in rec {
            regions.push(Region {
                id: r.id,
                name: r.name,
                zones: self.zones_get_by_region(r.id).await?,
                tags: self.regions_get_tags_by_id(r.id).await?,
            })
        }

        Ok(regions)
    }

    pub async fn zones_get_by_region(&self, region: Uuid) -> Result<Vec<Zone>> {
        let rec = sqlx::query!("select id, name from zones where region = $1", region)
            .fetch_all(&self.pool)
            .await?;

        let mut zones = Vec::with_capacity(rec.len());

        for r in rec {
            zones.push(Zone {
                id: r.id,
                name: r.name,
                tags: self.zones_get_tags_by_id(r.id).await?,
            })
        }

        Ok(zones)
    }

    pub async fn zones_get_by_id(&self, zone: Uuid) -> Result<Option<Zone>> {
        let rec = sqlx::query!("select id, name from zones where id = $1", zone)
            .fetch_optional(&self.pool)
            .await?;

        let res = if let Some(rec) = rec {
            Some(Zone {
                id: rec.id,
                name: rec.name,
                tags: self.zones_get_tags_by_id(rec.id).await?,
            })
        } else {
            None
        };

        Ok(res)
    }

    pub async fn zones_get_all(&self) -> Result<Vec<Zone>> {
        let rec = sqlx::query!("select id, name from zones")
            .fetch_all(&self.pool)
            .await?;

        let mut zones = Vec::with_capacity(rec.len());

        for r in rec {
            zones.push(Zone {
                id: r.id,
                name: r.name,
                tags: self.zones_get_tags_by_id(r.id).await?,
            })
        }

        Ok(zones)
    }

    pub async fn zones_resolve_check_zone(&self, check_zone: &CheckZone) -> Result<Vec<Zone>> {
        match check_zone {
            CheckZone::All => self.zones_get_all().await,
            CheckZone::Region(region) => self.zones_get_by_region(*region).await,
            CheckZone::Zone(zone) => self
                .zones_get_by_id(*zone)
                .await
                .map(|zone| zone.map(|zone| vec![zone]).unwrap_or_default()),
        }
    }
}
