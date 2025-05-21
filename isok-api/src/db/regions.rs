use crate::Result;
use crate::db::DbHandler;
use isok_data::models::{Region, Tags, Zone};
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
}
