use sea_orm::{entity::prelude::*, ActiveModelTrait, Set, Statement};
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

        // Check if settings already exist (idempotent)
        let count_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings".to_owned(),
            ))
            .await?;

        if let Some(row) = count_result {
            let count: i64 = row.try_get("", "count")?;
            if count > 0 {
                // Settings already seeded, skip
                return Ok(());
            }
        }

        // Seed default settings (only runtime-configurable operational settings)
        let settings = vec![
            // Scanner settings (2 runtime-configurable settings)
            (
                "scanner.scan_timeout_minutes",
                "120",
                "Integer",
                "Scanner",
                "Maximum time (in minutes) for a single scan before timeout",
                false,
                "120",
                Some(10),
                Some(1440),
            ),
            (
                "scanner.retry_failed_files",
                "false",
                "Boolean",
                "Scanner",
                "Automatically retry files that failed to scan",
                false,
                "false",
                None,
                None,
            ),
            // Application settings (1 setting)
            (
                "application.name",
                "Codex",
                "String",
                "Application",
                "Application display name (for branding/white-labeling)",
                false,
                "Codex",
                None,
                None,
            ),
            // Task worker settings (2 settings - currently hardcoded)
            (
                "task.poll_interval_seconds",
                "5",
                "Integer",
                "Task",
                "Interval (in seconds) for polling task queue",
                false,
                "5",
                Some(1),
                Some(60),
            ),
            (
                "task.cleanup_interval_seconds",
                "30",
                "Integer",
                "Task",
                "Interval (in seconds) for cleaning up completed tasks",
                false,
                "30",
                Some(10),
                Some(300),
            ),
            (
                "task.prioritize_scans_over_analysis",
                "true",
                "Boolean",
                "Task",
                "Prioritize scan tasks over analysis tasks when picking from the queue",
                false,
                "true",
                None,
                None,
            ),
            // Authentication settings (1 setting)
            (
                "auth.registration_enabled",
                "false",
                "Boolean",
                "Authentication",
                "Allow new users to register accounts (disabled by default for security)",
                false,
                "false",
                None,
                None,
            ),
            // Deduplication settings (2 settings)
            (
                "deduplication.cron_schedule",
                "",
                "String",
                "Deduplication",
                "Cron schedule for automatic duplicate detection (e.g., '0 2 * * *' for daily at 2am). Leave empty to disable automatic scanning.",
                false,
                "",
                None,
                None,
            ),
            (
                "deduplication.enabled",
                "true",
                "Boolean",
                "Deduplication",
                "Enable automatic duplicate detection scanning",
                false,
                "true",
                None,
                None,
            ),
        ];

        for (
            key,
            value,
            value_type,
            category,
            description,
            is_sensitive,
            default_value,
            min_val,
            max_val,
        ) in settings
        {
            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set(key.to_string()),
                value: Set(value.to_string()),
                value_type: Set(value_type.to_string()),
                category: Set(category.to_string()),
                description: Set(description.to_string()),
                is_sensitive: Set(is_sensitive),
                default_value: Set(default_value.to_string()),
                validation_rules: Set(None),
                min_value: Set(min_val),
                max_value: Set(max_val),
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
        // Delete seeded settings (cleanup for rollback)
        let db = manager.get_connection();

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE updated_by IS NULL".to_owned(),
        ))
        .await?;

        Ok(())
    }
}
