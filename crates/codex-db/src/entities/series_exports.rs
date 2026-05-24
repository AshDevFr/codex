//! `SeaORM` Entity for series_exports table
//!
//! Tracks user-initiated series data exports (JSON/CSV).
//! Each row represents one export job with its status, configuration,
//! and output file metadata.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "series_exports")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    /// "json", "csv", or "md"
    pub format: String,
    /// "pending", "running", "completed", "failed", or "cancelled"
    pub status: String,
    /// "series", "books", or "both"
    pub export_type: String,
    /// JSON array of library UUIDs selected for this export
    pub library_ids: serde_json::Value,
    /// JSON array of series field key strings selected for this export
    pub fields: serde_json::Value,
    /// JSON array of book field key strings (for "books" or "both" export types)
    pub book_fields: Option<serde_json::Value>,
    /// Relative path to the generated file (set on completion)
    pub file_path: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub row_count: Option<i32>,
    /// Error message if status is "failed"
    pub error: Option<String>,
    /// Reference to the background task executing this export
    pub task_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
