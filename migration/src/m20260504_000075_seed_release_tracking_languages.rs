//! Seed the server-wide `release_tracking.default_languages` setting (Phase 6).
//!
//! Aggregation feeds (e.g. MangaUpdates RSS) emit candidates in many languages.
//! Plugins filter client-side using a per-series `series_tracking.languages`
//! list, falling back to this server-wide default when that's NULL. ISO 639-1
//! codes; the seed value is `["en"]` to match the user's primary expectation
//! and the language tag MangaUpdates uses for English scanlations.
//!
//! Stored as a JSON array string; type `"Array"` so SettingsRepository parses
//! it via `serde_json::from_str` directly.

use sea_orm::{ActiveModelTrait, Set, Statement, entity::prelude::*};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Minimal ActiveModel for settings to avoid circular dependencies (matches the
// pattern used by sibling seed migrations, e.g.
// `m20260111_000026_seed_metrics_settings`).
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

        // Idempotent seed.
        let exists = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                "SELECT COUNT(*) as count FROM settings WHERE key = 'release_tracking.default_languages'"
                    .to_owned(),
            ))
            .await?;
        if let Some(row) = exists {
            let count: i64 = row.try_get("", "count")?;
            if count > 0 {
                return Ok(());
            }
        }

        let setting = ActiveModel {
            id: Set(Uuid::new_v4()),
            key: Set("release_tracking.default_languages".to_string()),
            value: Set("[\"en\"]".to_string()),
            value_type: Set("Array".to_string()),
            category: Set("Release Tracking".to_string()),
            description: Set(
                "Server-wide default language list (ISO 639-1) for release-source plugins that aggregate scanlations across multiple languages (e.g. MangaUpdates). Per-series overrides on `series_tracking.languages` take precedence."
                    .to_string(),
            ),
            is_sensitive: Set(false),
            default_value: Set("[\"en\"]".to_string()),
            validation_rules: Set(None),
            min_value: Set(None),
            max_value: Set(None),
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
            "DELETE FROM settings WHERE key = 'release_tracking.default_languages'".to_owned(),
        ))
        .await?;
        Ok(())
    }
}
