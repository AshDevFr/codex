//! `library_jobs` table: per-library scheduled jobs.
//!
//! Generic across job types via the `r#type` discriminator. The `config`
//! column carries a JSON payload whose shape depends on `r#type`.
//! Phase 9 introduces the `metadata_refresh` type; future work can add
//! `scan`, `cleanup`, etc. without schema changes.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "library_jobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub library_id: Uuid,
    /// Discriminator for `config`. e.g. `"metadata_refresh"`.
    #[sea_orm(column_name = "type")]
    pub r#type: String,
    pub name: String,
    pub enabled: bool,
    pub cron_schedule: String,
    pub timezone: Option<String>,
    /// Type-specific payload as JSON-encoded text.
    #[sea_orm(column_type = "Text")]
    pub config: String,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<String>,
    pub last_run_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::libraries::Entity",
        from = "Column::LibraryId",
        to = "super::libraries::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Library,
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Library.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
