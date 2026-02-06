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

        // Check if the plugins.auto_match_confidence_threshold setting already exists (idempotent)
        let exists_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'plugins.auto_match_confidence_threshold'"
                    .to_owned(),
            ))
            .await?;

        let threshold_exists = if let Some(row) = exists_result {
            let count: i64 = row.try_get("", "count")?;
            count > 0
        } else {
            false
        };

        // Seed plugins.auto_match_confidence_threshold setting
        if !threshold_exists {
            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("plugins.auto_match_confidence_threshold".to_string()),
                value: Set("0.8".to_string()),
                value_type: Set("Float".to_string()),
                category: Set("Plugins".to_string()),
                description: Set(
                    "Minimum relevance score (0.0-1.0) required for plugin auto-match to proceed. If the best search result has a relevance score below this threshold, the auto-match will be skipped. Set to 0 to always match regardless of score. If a plugin does not return relevance scores, auto-match proceeds anyway."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("0.8".to_string()),
                validation_rules: Set(None),
                min_value: Set(None), // Float validation doesn't use min/max (they are i64)
                max_value: Set(None),
                updated_at: Set(chrono::Utc::now()),
                updated_by: Set(None),
                version: Set(1),
                deleted_at: Set(None),
            };

            setting.insert(db).await?;
        }

        // Check if the plugins.post_scan_auto_match_enabled setting already exists
        let exists_result = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'plugins.post_scan_auto_match_enabled'"
                    .to_owned(),
            ))
            .await?;

        let auto_match_exists = if let Some(row) = exists_result {
            let count: i64 = row.try_get("", "count")?;
            count > 0
        } else {
            false
        };

        // Seed plugins.post_scan_auto_match_enabled setting
        if !auto_match_exists {
            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set("plugins.post_scan_auto_match_enabled".to_string()),
                value: Set("false".to_string()),
                value_type: Set("Boolean".to_string()),
                category: Set("Plugins".to_string()),
                description: Set(
                    "Enable automatic metadata matching after library scans. When enabled, after a series is analyzed during a library scan, plugins with the 'library:scan' scope will automatically attempt to match and apply metadata. WARNING: This can trigger many API calls on large libraries. Disabled by default for safety."
                        .to_string(),
                ),
                is_sensitive: Set(false),
                default_value: Set("false".to_string()),
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

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM settings WHERE key IN ('plugins.auto_match_confidence_threshold', 'plugins.post_scan_auto_match_enabled')"
                .to_owned(),
        ))
        .await?;

        Ok(())
    }
}
