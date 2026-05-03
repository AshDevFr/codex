use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "libraries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub path: String,
    /// Series detection strategy (series_volume, series_volume_chapter, flat, etc.)
    pub series_strategy: String,
    /// Strategy-specific configuration (JSON)
    #[sea_orm(column_type = "Json")]
    pub series_config: Option<serde_json::Value>,
    /// Book naming strategy (filename, metadata_first, smart, series_name)
    pub book_strategy: String,
    /// Book strategy-specific configuration (JSON)
    #[sea_orm(column_type = "Json")]
    pub book_config: Option<serde_json::Value>,
    /// Book number strategy (file_order, metadata, filename, smart)
    pub number_strategy: String,
    /// Number strategy-specific configuration (JSON)
    #[sea_orm(column_type = "Json")]
    pub number_config: Option<serde_json::Value>,
    /// Legacy: stores cron/scan settings (kept for backward compatibility)
    pub scanning_config: Option<String>,
    pub default_reading_direction: String,
    pub allowed_formats: Option<String>,
    pub excluded_patterns: Option<String>,
    /// Title preprocessing rules as JSON array of regex rules
    /// Applied during scan to clean series titles before search
    #[sea_orm(column_type = "Text")]
    pub title_preprocessing_rules: Option<String>,
    /// Auto-match conditions as JSON object
    /// Controls when auto-matching runs for this library
    #[sea_orm(column_type = "Text")]
    pub auto_match_conditions: Option<String>,
    /// Scheduled metadata-refresh configuration as JSON object
    /// Controls per-library cron, field groups, providers, and safety toggles
    /// for the scheduled metadata refresh feature. NULL = feature off (defaults
    /// applied when read).
    #[sea_orm(column_type = "Text")]
    pub metadata_refresh_config: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_scanned_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::series::Entity")]
    Series,
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
