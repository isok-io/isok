use crate::Result;
use crate::db::DbHandler;
use isok_data::models::{Tags, User};
use uuid::Uuid;

impl DbHandler {
    async fn users_get_tags_by_id(&self, id: Uuid) -> Result<Tags> {
        let tags = sqlx::query!(r#"SELECT key, value FROM users_tags WHERE "user" = $1"#, id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();

        Ok(tags)
    }

    pub async fn users_get_by_email(&self, email: &str) -> Result<Option<User>> {
        let rec = sqlx::query!(
            "select id, email, password from users where email = $1",
            email
        )
        .fetch_optional(&self.pool)
        .await?;

        let res = match rec {
            Some(rec) => Some(User {
                id: rec.id,
                email: rec.email,
                password: rec.password,
                tags: self.users_get_tags_by_id(rec.id).await?,
            }),
            None => None,
        };

        Ok(res)
    }

    pub async fn users_get_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let rec = sqlx::query!("select id, email, password from users where id = $1", id)
            .fetch_optional(&self.pool)
            .await?;

        let res = match rec {
            Some(rec) => Some(User {
                id: rec.id,
                email: rec.email,
                password: rec.password,
                tags: self.users_get_tags_by_id(rec.id).await?,
            }),
            None => None,
        };

        Ok(res)
    }

    pub async fn users_update_password(&self, user_id: Uuid, password: &str) -> Result<()> {
        sqlx::query!(
            "update users set password = $1 where id = $2",
            password,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn users_insert_user(&self, user: User) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        self.tenants_insert_tenant(user.id, &mut tx).await?;

        let rec = sqlx::query!(
            "insert into users (id, email, password) values ($1, $2, $3) returning id",
            user.id,
            user.email,
            user.password
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(rec.id)
    }

    pub async fn users_patch_user(
        &self,
        id: Uuid,
        email: Option<String>,
        password: Option<String>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        if let Some(email) = email {
            sqlx::query!("update users set email = $1 where id = $2", email, id)
                .execute(&mut *tx)
                .await?;
        }

        if let Some(password) = password {
            sqlx::query!("update users set password = $1 where id = $2", password, id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await.map_err(Into::into)
    }

    pub async fn users_delete(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!("delete from users where id = $1", id)
            .execute(&mut *tx)
            .await?;

        self.tenants_delete_tenant(id, &mut tx).await?;

        tx.commit().await.map_err(Into::into)
    }
}
