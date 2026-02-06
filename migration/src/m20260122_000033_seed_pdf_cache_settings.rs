use sea_orm::{ActiveModelTrait, Set, Statement, entity::prelude::*};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Define a minimal ActiveModel for settings to avoid circular dependencies
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

        // Check if the pdf_cache.cron_schedule setting already exists (idempotent)
        let exists_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'pdf_cache.cron_schedule'"
                    .to_owned(),
            ))
            .await?;

        let cron_exists = if let Some(row) = exists_result {
            let count: i64 = row.try_get("", "count")?;
            count > 0
        } else {
            false
        };

        // Seed pdf_cache.cron_schedule setting
        // Empty string = disabled (following the same pattern as deduplication.cron_schedule)
        if !cron_exists {
            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("pdf_cache.cron_schedule".to_string()),
                value: Set("".to_string()),
                value_type: Set("String".to_string()),
                category: Set("PDF".to_string()),
                description: Set(
                    "Cron schedule for automatic PDF cache cleanup. Leave empty to disable scheduled cleanup. Example: '0 4 * * 0' (every Sunday at 4 AM)."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("".to_string()),
                validation_rules: Set(None),
                min_value: Set(None),
                max_value: Set(None),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            };

            setting.insert(db).await?;
        }

        // Check if the pdf_cache.max_age_days setting already exists
        let exists_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'pdf_cache.max_age_days'"
                    .to_owned(),
            ))
            .await?;

        let max_age_exists = if let Some(row) = exists_result {
            let count: i64 = row.try_get("", "count")?;
            count > 0
        } else {
            false
        };

        // Seed pdf_cache.max_age_days setting
        if !max_age_exists {
            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("pdf_cache.max_age_days".to_string()),
                value: Set("30".to_string()),
                value_type: Set("Integer".to_string()),
                category: Set("PDF".to_string()),
                description: Set(
                    "Maximum age in days for cached PDF pages. Pages older than this will be deleted during cleanup. Set to 0 to keep pages indefinitely."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("30".to_string()),
                validation_rules: Set(None),
                min_value: Set(Some(0)),
                max_value: Set(Some(365)),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            };

            setting.insert(db).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE key IN ('pdf_cache.cron_schedule', 'pdf_cache.max_age_days')"
                .to_owned(),
        ))
        .await?;

        Ok(())
    }
}
