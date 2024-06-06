use std::collections::HashMap;

use futures::future::BoxFuture;
use futures::FutureExt;
use log::error;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use isok_data::owner::{
    NormalOrganization, Organization, OrganizationInput, OrganizationType, OrganizationUserRole,
    User, UserInput, UserOrganization, UserRole,
};

use crate::api::auth::UserPassword;
use crate::api::errors::DbQueryError;

#[derive(Clone, Debug)]
pub struct DbHandler {
    pool: PgPool,
}

#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "organisation_type", rename_all = "lowercase")]
enum OrgType {
    User,
    Normal,
}

#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "organisation_user_role", rename_all = "lowercase")]
enum OrgUserRole {
    Owner,
    Member,
}

impl Into<OrganizationUserRole> for OrgUserRole {
    fn into(self) -> OrganizationUserRole {
        match self {
            OrgUserRole::Owner => OrganizationUserRole::Owner,
            OrgUserRole::Member => OrganizationUserRole::Member,
        }
    }
}

impl DbHandler {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(uri: String) -> Option<Self> {
        match PgPoolOptions::new().connect(&uri).await {
            Ok(p) => Some(Self::new(p)),
            Err(e) => {
                error!("Error while connecting to the db: {e}");
                None
            }
        }
    }

    pub async fn get_user_password(
        &self,
        email_address: String,
    ) -> Result<UserPassword, DbQueryError> {
        sqlx::query_as!(
            UserPassword,
            r#"
                SELECT user_id, password FROM users WHERE email_address = $1 AND deleted_at IS NULL
            "#,
            email_address
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbQueryError)
    }

    fn parse_organization(
        &self,
        organization_id: Uuid,
        org_type: OrgType,
        name: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
    ) -> BoxFuture<Result<Organization, DbQueryError>> {
        async move {
            Ok(Organization {
                organization_id,
                tags: sqlx::query!(
                r#"SELECT key, value FROM organizations_tags WHERE organization_id = $1"#,
                organization_id
            )
                    .fetch_all(&self.pool)
                    .await
                    .map_err(DbQueryError)
                    .map(|records| {
                        records
                            .into_iter()
                            .map(|record| (record.key, record.value))
                            .collect()
                    })?,
                organization_type: match org_type {
                    OrgType::User => OrganizationType::UserOrganization(UserOrganization),
                    OrgType::Normal => {
                        let rows = sqlx::query!(
                        r#"SELECT u.user_id, u.username, u.password, u.email_address, u.created_at, u.updated_at, u.deleted_at, o.organization_id, o.type as "type!: OrgType", o.name, o.created_at as org_created_at, o.updated_at as org_updated_at, o.deleted_at as org_deleted_at, uo.role as "role!: OrgUserRole" FROM users_organizations as uo JOIN users as u on uo.user_id = u.user_id JOIN organizations o on o.organization_id = u.self_organization WHERE uo.organization_id = $1 AND u.deleted_at IS NULL"#,
                        organization_id
                    )
                            .fetch_all(&self.pool)
                            .await
                            .map_err(DbQueryError)?;
                        let mut users = vec![];
                        for row in rows {
                            users.push(UserRole{role: row.role.into(), user: self.parse_user_with_organization(
                                row.user_id,
                                row.username,
                                row.password,
                                row.email_address,
                                sqlx::query!(
                                            r#"SELECT key, value FROM users_tags WHERE user_id = $1"#,
                                            row.user_id
                                        )
                                    .fetch_all(&self.pool)
                                    .await
                                    .map_err(DbQueryError)
                                    .map(|records| {
                                        records
                                            .into_iter()
                                            .map(|record| (record.key, record.value))
                                            .collect()
                                    })?,
                                row.created_at,
                                row.updated_at,
                                row.deleted_at,
                                row.organization_id,
                                row.r#type,
                                row.name,
                                row.org_created_at,
                                row.org_updated_at,
                                row.org_deleted_at,
                            )
                                .await?});
                        }
                        OrganizationType::NormalOrganization(NormalOrganization {
                            name: name.ok_or(DbQueryError(sqlx::Error::RowNotFound))?,
                            users,
                        })
                    }
                },
                created_at,
                updated_at,
                deleted_at,
            })
        }.boxed()
    }

    async fn parse_user_with_organization(
        &self,
        user_id: Uuid,
        username: String,
        password: String,
        email_address: String,
        tags: HashMap<String, Option<String>>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        organization_id: Uuid,
        org_type: OrgType,
        name: Option<String>,
        org_created_at: DateTime<Utc>,
        org_updated_at: DateTime<Utc>,
        org_deleted_at: Option<DateTime<Utc>>,
    ) -> Result<User, DbQueryError> {
        Ok(User {
            user_id,
            username,
            password,
            email_address,
            self_organization: self
                .parse_organization(
                    organization_id,
                    org_type,
                    name,
                    org_created_at,
                    org_updated_at,
                    org_deleted_at,
                )
                .await?,
            tags,
            created_at,
            updated_at,
            deleted_at,
        })
    }

    pub async fn get_users(&self) -> Result<Vec<User>, DbQueryError> {
        let rows = sqlx::query!(
            r#"
                SELECT u.user_id, u.username, u.password, u.email_address, u.created_at, u.updated_at, u.deleted_at, o.organization_id, o.type as "type!: OrgType", o.name, o.created_at as org_created_at, o.updated_at as org_updated_at, o.deleted_at as org_deleted_at FROM users as u JOIN organizations o on o.organization_id = u.self_organization WHERE u.deleted_at IS NULL
            "#
        )
            .fetch_all(&self.pool)
            .await.map_err(DbQueryError)?;

        let mut users = vec![];

        for row in rows {
            users.push(
                self.parse_user_with_organization(
                    row.user_id,
                    row.username,
                    row.password,
                    row.email_address,
                    sqlx::query!(
                        r#"SELECT key, value FROM users_tags WHERE user_id = $1"#,
                        row.user_id
                    )
                    .fetch_all(&self.pool)
                    .await
                    .map_err(DbQueryError)
                    .map(|records| {
                        records
                            .into_iter()
                            .map(|record| (record.key, record.value))
                            .collect()
                    })?,
                    row.created_at,
                    row.updated_at,
                    row.deleted_at,
                    row.organization_id,
                    row.r#type,
                    row.name,
                    row.org_created_at,
                    row.org_updated_at,
                    row.org_deleted_at,
                )
                .await?,
            )
        }

        Ok(users)
    }

    pub async fn get_user(&self, user_id: Uuid) -> Result<User, DbQueryError> {
        let row = sqlx::query!(
            r#"
                SELECT u.user_id, u.username, u.password, u.email_address, u.created_at, u.updated_at, u.deleted_at, o.organization_id, o.type as "type!: OrgType", o.name, o.created_at as org_created_at, o.updated_at as org_updated_at, o.deleted_at as org_deleted_at FROM users as u JOIN organizations o on o.organization_id = u.self_organization WHERE u.user_id = $1 AND u.deleted_at IS NULL
            "#, user_id
        )
            .fetch_one(&self.pool)
            .await.map_err(DbQueryError)?;

        self.parse_user_with_organization(
            row.user_id,
            row.username,
            row.password,
            row.email_address,
            sqlx::query!(
                r#"SELECT key, value FROM users_tags WHERE user_id = $1"#,
                row.user_id
            )
            .fetch_all(&self.pool)
            .await
            .map_err(DbQueryError)
            .map(|records| {
                records
                    .into_iter()
                    .map(|record| (record.key, record.value))
                    .collect()
            })?,
            row.created_at,
            row.updated_at,
            row.deleted_at,
            row.organization_id,
            row.r#type,
            row.name,
            row.org_created_at,
            row.org_updated_at,
            row.org_deleted_at,
        )
        .await
    }

    pub async fn insert_user(&self, user: UserInput) -> Result<(), DbQueryError> {
        let mut transaction = self.pool.begin().await.map_err(DbQueryError)?;
        let now = Utc::now();

        let organization_id = sqlx::query!(
            r#"
INSERT INTO organizations (type, created_at, updated_at)
VALUES ('user', $1, $2) RETURNING organization_id
        "#,
            now,
            now
        )
        .fetch_one(transaction.as_mut())
        .await
        .map(|row| row.organization_id)
        .map_err(DbQueryError)?;

        sqlx::query!(
            r#"
INSERT INTO users (username, password, email_address, self_organization, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6)
        "#,
            user.username,
            user.password,
            user.email_address,
            organization_id,
            now,
            now
        )
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(DbQueryError)?;

        transaction.commit().await.map_err(DbQueryError)
    }

    pub async fn update_user_username(
        &self,
        user_id: Uuid,
        username: String,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET username = $1, updated_at = $2 WHERE user_id = $3 AND deleted_at IS NULL
        "#,
            username,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn update_user_email(
        &self,
        user_id: Uuid,
        email_address: String,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET email_address = $1, updated_at = $2 WHERE user_id = $3 AND deleted_at IS NULL
        "#,
            email_address,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn update_user_password(
        &self,
        user_id: Uuid,
        password: String,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET password = $1, updated_at = $2 WHERE user_id = $3
        "#,
            password,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn delete_user(&self, user_id: Uuid) -> Result<(), DbQueryError> {
        let now = Utc::now();

        sqlx::query!(
            r#"
UPDATE users SET deleted_at = $1 WHERE user_id = $2
        "#,
            now,
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(DbQueryError)?;

        sqlx::query!(
            r#"
UPDATE organizations SET deleted_at = $1 WHERE organization_id = (SELECT self_organization FROM users WHERE user_id = $2)
        "#,
            now,
            user_id
        )
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(DbQueryError)
    }

    pub async fn is_user_in_organization(
        &self,
        user_id: &Uuid,
        organization_id: &Uuid,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
SELECT 1 as x FROM users_organizations as uo JOIN organizations o on o.organization_id = uo.organization_id WHERE user_id = $1 AND uo.organization_id = $2 AND o.deleted_at IS NULL
        "#,
            user_id,
            organization_id
        )
        .fetch_one(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn get_user_organizations(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Organization>, DbQueryError> {
        let rows = sqlx::query!(
            r#"
                SELECT o.organization_id, o.type as "type!: OrgType", o.name, o.created_at, o.updated_at, o.deleted_at FROM users_organizations as uo JOIN organizations o on o.organization_id = uo.organization_id WHERE uo.user_id = $1 AND o.deleted_at IS NULL
            "#,
            user_id
        )
            .fetch_all(&self.pool)
            .await.map_err(DbQueryError)?;

        let mut organizations = vec![];

        for row in rows {
            organizations.push(
                self.parse_organization(
                    row.organization_id,
                    row.r#type,
                    row.name,
                    row.created_at,
                    row.updated_at,
                    row.deleted_at,
                )
                .await?,
            )
        }

        Ok(organizations)
    }

    pub async fn get_user_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> Result<Organization, DbQueryError> {
        let row = sqlx::query!(
            r#"
                SELECT o.organization_id, o.type as "type!: OrgType", o.name, o.created_at, o.updated_at, o.deleted_at FROM users_organizations as uo JOIN organizations o on o.organization_id = uo.organization_id WHERE uo.user_id = $1 AND uo.organization_id = $2 AND o.deleted_at IS NULL
            "#,
            user_id,
            organization_id
        )
            .fetch_one(&self.pool)
            .await.map_err(DbQueryError)?;

        self.parse_organization(
            row.organization_id,
            row.r#type,
            row.name,
            row.created_at,
            row.updated_at,
            row.deleted_at,
        )
        .await
    }

    pub async fn insert_organization(
        &self,
        user_id: Uuid,
        organization: OrganizationInput,
    ) -> Result<(), DbQueryError> {
        let mut transaction = self.pool.begin().await.map_err(DbQueryError)?;
        let now = Utc::now();

        let organization_id = sqlx::query!(
            r#"
INSERT INTO organizations (type, name, created_at, updated_at)
VALUES ('normal', $1, $2, $3) RETURNING organization_id
        "#,
            organization.name,
            now,
            now
        )
        .fetch_one(transaction.as_mut())
        .await
        .map(|row| row.organization_id)
        .map_err(DbQueryError)?;

        sqlx::query!(
            r#"
INSERT INTO users_organizations (user_id, organization_id, role)
VALUES ($1, $2, 'owner'::organisation_user_role)
        "#,
            user_id,
            organization_id
        )
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(DbQueryError)?;

        transaction.commit().await.map_err(DbQueryError)
    }

    pub async fn delete_organization(&self, organization_id: Uuid) -> Result<(), DbQueryError> {
        let now = Utc::now();

        sqlx::query!(
            r#"UPDATE organizations SET deleted_at = $1 WHERE organization_id = $2"#,
            now,
            organization_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }
}
