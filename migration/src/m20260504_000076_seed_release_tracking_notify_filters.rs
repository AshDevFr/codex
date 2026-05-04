//! Seed the server-wide `release_tracking.notify_languages` and
//! `release_tracking.notify_plugins` settings (Phase 8 follow-up).
//!
//! These two arrays filter the in-app `release_announced` notification stream
//! (toasts + Releases nav badge):
//!
//! - `notify_languages` (ISO 639-1, default `[]`): when non-empty, only
//!   announcements whose `language` is in this list bump the badge / surface
//!   a toast. Empty = "let everything through."
//!
//! - `notify_plugins` (plugin IDs, default `[]`): when non-empty, only
//!   announcements emitted by a plugin in this list bump the badge / surface
//!   a toast. Empty = "all installed release-source plugins are allowed."
//!
//! These filters are server-wide because all admins of a Codex instance share
//! the same notification stream. Per-series mute lives on
//! `user_preferences.release_tracking.muted_series_ids` (per-user) — the
//! distinction is that muting individual series is a personal-pref override
//! over what would otherwise be a shared global notification, while the
//! language / plugin allowlists shape the global stream itself.
//!
//! Defaults are empty arrays (no filtering) so a fresh install behaves like
//! the old in-memory store: every announcement notifies. Admins can tighten
//! later via the `/settings/release-tracking` page.

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

const KEYS: &[(&str, &str)] = &[
    (
        "release_tracking.notify_languages",
        "Server-wide allowlist of ISO 639-1 language codes for release-tracking notifications. When non-empty, only announcements whose language is in this list bump the Releases badge and surface a toast. Empty array = let everything through.",
    ),
    (
        "release_tracking.notify_plugins",
        "Server-wide allowlist of release-source plugin IDs whose announcements should bump the Releases badge and surface a toast. Empty array = all installed release-source plugins are allowed.",
    ),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        for (key, description) in KEYS {
            // Idempotent seed. Static string concat is safe; `key` is a
            // compile-time constant from the KEYS table.
            let exists = db
                .query_one(Statement::from_string(
                    manager.get_database_backend(),
                    format!(
                        "SELECT COUNT(*) as count FROM settings WHERE key = '{}'",
                        key
                    ),
                ))
                .await?;
            if let Some(row) = exists {
                let count: i64 = row.try_get("", "count")?;
                if count > 0 {
                    continue;
                }
            }

            let setting = ActiveModel {
                id: Set(Uuid::new_v4()),
                key: Set((*key).to_string()),
                value: Set("[]".to_string()),
                value_type: Set("Array".to_string()),
                category: Set("Release Tracking".to_string()),
                description: Set((*description).to_string()),
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
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (key, _) in KEYS {
            db.execute(Statement::from_string(
                manager.get_database_backend(),
                format!("DELETE FROM settings WHERE key = '{}'", key),
            ))
            .await?;
        }
        Ok(())
    }
}
