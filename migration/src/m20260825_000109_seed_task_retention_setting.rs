//! Seed the `task.completed_retention_seconds` runtime setting.
//!
//! The cleanup sweep deleted finished tasks after a hardcoded ten seconds while
//! the interval it ran on was already configurable. Ten seconds is shorter than
//! any client's poll, so failures, their `last_error`, and the scan rows that
//! `scan-status` reads were gone before anything could read them. The retention
//! belongs beside the interval as a setting.

use sea_orm::{ActiveModelTrait, Set, Statement, entity::prelude::*};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub category: String,
    pub description: String,
    pub is_sensitive: bool,
    pub default_value: String,
    pub validation_rules: Option<String>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: Option<Uuid>,
    pub version: i32,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let existing = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'task.completed_retention_seconds'"
                    .to_owned(),
            ))
            .await?;

        if let Some(row) = existing {
            let count: i64 = row.try_get("", "count")?;
            if count > 0 {
                return Ok(());
            }
        }

        let setting = ActiveModel {
            id: Set(Uuid::new_v4()),
            key: Set("task.completed_retention_seconds".to_string()),
            value: Set("3600".to_string()),
            value_type: Set("Integer".to_string()),
            category: Set("Task".to_string()),
            description: Set(
                "How long a finished task is kept before the cleanup sweep deletes it. \
                Long enough that a failure can still be found and retried; aggregate \
                history over days lives in the task metrics, not in this table."
                    .to_string(),
            ),
            is_sensitive: Set(false),
            default_value: Set("3600".to_string()),
            validation_rules: Set(None),
            // The floor restores the previous behaviour for anyone who wants it.
            // The ceiling is a week, where the day-scale purge endpoint starts
            // to mean something again.
            min_value: Set(Some(10)),
            max_value: Set(Some(604_800)),
            updated_at: Set(chrono::Utc::now()),
            updated_by: Set(None),
            version: Set(1),
            deleted_at: Set(None),
        };

        setting.insert(db).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE key = 'task.completed_retention_seconds'".to_owned(),
        ))
        .await?;
        Ok(())
    }
}
