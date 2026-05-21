//! Seed `duplicate_detection.trusted_external_id_sources` (empty list).
//!
//! The series duplicate detector groups by `(source, external_id)` on the
//! `series_external_ids` table. Some metadata sources (e.g. ANN via the
//! `api:animenewsnetwork` source) routinely write the same ID across distinct
//! series, producing false positives that span seven-plus unrelated entries.
//!
//! This setting whitelists which `source` values are trusted enough to
//! participate in the external-ID pass. Default `[]` disables the pass
//! entirely and leaves only the (library-scoped, lower-confidence) title pass
//! running. Operators opt in source-by-source via the admin settings API once
//! they trust the upstream data.
//!
//! Stored as a JSON array string; type `"Array"` so SettingsRepository parses
//! it via `serde_json::from_str` directly. Mirrors
//! `m20260504_000075_seed_release_tracking_languages` in shape.

use sea_orm::{ActiveModelTrait, Set, Statement, entity::prelude::*};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Minimal ActiveModel for settings to avoid circular dependencies (matches the
// pattern used by sibling seed migrations).
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

const SETTING_KEY: &str = "duplicate_detection.trusted_external_id_sources";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Idempotent seed.
        let exists = db
            .query_one(Statement::from_string(
                manager.get_database_backend(),
                format!(
                    "SELECT COUNT(*) as count FROM settings WHERE key = '{}'",
                    SETTING_KEY
                ),
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
            key: Set(SETTING_KEY.to_string()),
            value: Set("[]".to_string()),
            value_type: Set("Array".to_string()),
            category: Set("Duplicate Detection".to_string()),
            description: Set(
                "Whitelist of `series_external_ids.source` values trusted enough to be grouped by the duplicate detector's high-confidence external-ID pass (e.g. `plugin:mangabaka`, `api:anilist`). Empty disables the pass; the library-scoped title pass still runs. Opt in source-by-source once you trust the upstream data."
                    .to_string(),
            ),
            is_sensitive: Set(false),
            default_value: Set("[]".to_string()),
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
            format!("DELETE FROM settings WHERE key = '{}'", SETTING_KEY),
        ))
        .await?;
        Ok(())
    }
}
