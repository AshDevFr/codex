use sea_orm::{ActiveModelTrait, Set, Statement, entity::prelude::*};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Minimal ActiveModel for settings to avoid circular dependencies
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

/// All export setting keys seeded by this migration
const EXPORT_SETTING_KEYS: &[&str] = &[
    "exports.retention_days",
    "exports.max_per_user",
    "exports.max_concurrent_per_user",
    "exports.storage_cap_bytes",
    "exports.cleanup_cron",
    "exports.dir",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // Helper to check if a setting already exists
        async fn setting_exists(
            db: &SchemaManagerConnection<'_>,
            backend: sea_orm::DatabaseBackend,
            key: &str,
        ) -> Result<bool, DbErr> {
            let result = db
                .query_one(Statement::from_string(
                    backend,
                    format!(
                        "SELECT COUNT(*) as count FROM settings WHERE key = '{}'",
                        key
                    ),
                ))
                .await?;
            match result {
                Some(row) => {
                    let count: i64 = row.try_get("", "count")?;
                    Ok(count > 0)
                }
                None => Ok(false),
            }
        }

        // exports.retention_days - how long completed exports are kept
        if !setting_exists(db, backend, "exports.retention_days").await? {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("exports.retention_days".to_string()),
                value: Set("7".to_string()),
                value_type: Set("Integer".to_string()),
                category: Set("Exports".to_string()),
                description: Set(
                    "Number of days to keep completed series exports before automatic deletion."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("7".to_string()),
                validation_rules: Set(None),
                min_value: Set(Some(1)),
                max_value: Set(Some(90)),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            }
            .insert(db)
            .await?;
        }

        // exports.max_per_user - max completed exports kept per user
        if !setting_exists(db, backend, "exports.max_per_user").await? {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("exports.max_per_user".to_string()),
                value: Set("10".to_string()),
                value_type: Set("Integer".to_string()),
                category: Set("Exports".to_string()),
                description: Set(
                    "Maximum number of completed exports kept per user. When exceeded, the oldest export is automatically deleted."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("10".to_string()),
                validation_rules: Set(None),
                min_value: Set(Some(1)),
                max_value: Set(Some(100)),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            }
            .insert(db)
            .await?;
        }

        // exports.max_concurrent_per_user - max non-terminal exports per user
        if !setting_exists(db, backend, "exports.max_concurrent_per_user").await? {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("exports.max_concurrent_per_user".to_string()),
                value: Set("3".to_string()),
                value_type: Set("Integer".to_string()),
                category: Set("Exports".to_string()),
                description: Set(
                    "Maximum number of pending or running exports allowed per user at the same time."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("3".to_string()),
                validation_rules: Set(None),
                min_value: Set(Some(1)),
                max_value: Set(Some(10)),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            }
            .insert(db)
            .await?;
        }

        // exports.storage_cap_bytes - global storage cap for all export files
        if !setting_exists(db, backend, "exports.storage_cap_bytes").await? {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("exports.storage_cap_bytes".to_string()),
                value: Set("2147483648".to_string()), // 2 GiB
                value_type: Set("Integer".to_string()),
                category: Set("Exports".to_string()),
                description: Set(
                    "Maximum total disk space (in bytes) for all export files across all users. When exceeded, the oldest exports are deleted during cleanup. Default: 2 GiB (2147483648)."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("2147483648".to_string()),
                validation_rules: Set(None),
                min_value: Set(Some(104857600)), // 100 MiB minimum
                max_value: Set(None),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            }
            .insert(db)
            .await?;
        }

        // exports.cleanup_cron - cron schedule for cleanup task
        if !setting_exists(db, backend, "exports.cleanup_cron").await? {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("exports.cleanup_cron".to_string()),
                value: Set("0 30 * * * *".to_string()),
                value_type: Set("String".to_string()),
                category: Set("Exports".to_string()),
                description: Set(
                    "Cron schedule for automatic export cleanup (expired files, stale temp files, storage cap enforcement). Uses 6-part cron format (sec min hour day month weekday). Leave empty to disable."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("0 30 * * * *".to_string()),
                validation_rules: Set(None),
                min_value: Set(None),
                max_value: Set(None),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            }
            .insert(db)
            .await?;
        }

        // exports.dir - directory for export files
        if !setting_exists(db, backend, "exports.dir").await? {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("exports.dir".to_string()),
                value: Set("data/exports".to_string()),
                value_type: Set("String".to_string()),
                category: Set("Exports".to_string()),
                description: Set(
                    "Directory path for storing export files. Can be absolute or relative to the working directory."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("data/exports".to_string()),
                validation_rules: Set(None),
                min_value: Set(None),
                max_value: Set(None),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            }
            .insert(db)
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let keys = EXPORT_SETTING_KEYS
            .iter()
            .map(|k| format!("'{k}'"))
            .collect::<Vec<_>>()
            .join(", ");

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            format!("DELETE FROM settings WHERE key IN ({keys})"),
        ))
        .await?;

        Ok(())
    }
}
