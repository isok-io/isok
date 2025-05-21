use crate::Result;
use crate::db::DbHandler;
use isok_data::models::{Organisation, Tags};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

impl DbHandler {
    async fn orgs_get_tags_by_id(&self, id: Uuid) -> Result<Tags> {
        let tags = sqlx::query!(
            "SELECT key, value FROM organisations_tags WHERE organisation = $1",
            id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|e| (e.key, e.value))
        .collect();

        Ok(tags)
    }

    pub async fn orgs_get_by_id(&self, id: Uuid) -> Result<Option<Organisation>> {
        let org = sqlx::query!("select id, name from organisations where id = $1", id)
            .fetch_optional(&self.pool)
            .await?;

        match org {
            None => Ok(None),
            Some(rec) => {
                let members = sqlx::query!(
                    r#"select "user" from organisations_members where organisation = $1"#,
                    rec.id
                )
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|rec| rec.user)
                .collect::<Vec<_>>();
                Ok(Some(Organisation {
                    id,
                    name: rec.name,
                    members,
                    tags: self.orgs_get_tags_by_id(id).await?,
                }))
            }
        }
    }

    async fn orgs_get_members(&self, id: Uuid) -> Result<Vec<Uuid>> {
        let res = sqlx::query!(
            r#"
            select "user"
            from organisations_members
            where organisation = $1"#,
            id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|rec| rec.user);

        Ok(res.collect())
    }

    pub async fn orgs_get_user_orgs(&self, user_id: Uuid) -> Result<Vec<Organisation>> {
        let recs = sqlx::query!(
            r#"
            select o.id, o.name
            from organisations o
                join organisations_members om on o.id = om.organisation
            where "user" = $1"#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut res = Vec::with_capacity(recs.len());
        for rec in recs {
            res.push(Organisation {
                id: rec.id,
                name: rec.name,
                members: self.orgs_get_members(rec.id).await?,
                tags: self.orgs_get_tags_by_id(rec.id).await?,
            })
        }

        Ok(res)
    }

    pub async fn orgs_insert_org(&self, org: Organisation) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        self.tenants_insert_tenant(org.id, &mut tx).await?;

        let rec = sqlx::query!(
            "insert into organisations (id, name) values ($1, $2) returning id",
            org.id,
            org.name
        )
        .fetch_one(&mut *tx)
        .await?;

        for member in org.members {
            self.orgs_add_member_tx(org.id, member, &mut tx).await?;
        }

        tx.commit().await?;
        Ok(rec.id)
    }

    async fn orgs_add_member_tx(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<()> {
        sqlx::query!(
            "insert into organisations_members values ($1, $2)",
            org_id,
            user_id
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    pub async fn orgs_add_member(&self, org_id: Uuid, user_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        self.orgs_add_member_tx(org_id, user_id, &mut tx).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn orgs_remove_member(&self, org_id: Uuid, user_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"delete from organisations_members where organisation = $1 and "user" = $2"#,
            org_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn orgs_update_name(&self, id: Uuid, name: &str) -> Result<()> {
        sqlx::query!("update organisations set name = $1 where id = $2", name, id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn orgs_delete(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "delete from organisations_members where organisation = $1",
            id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!("delete from organisations where id = $1", id)
            .execute(&mut *tx)
            .await?;

        self.tenants_delete_tenant(id, &mut tx).await?;

        tx.commit().await?;
        Ok(())
    }
}
