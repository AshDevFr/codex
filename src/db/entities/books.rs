use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "books")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub series_id: Uuid,
    pub library_id: Uuid,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_hash: String,
    pub partial_hash: String,
    pub format: String,
    pub page_count: i32,
    pub deleted: bool,
    pub analyzed: bool,
    /// Legacy single error field (deprecated, use analysis_errors instead)
    pub analysis_error: Option<String>,
    /// JSON map of error types to error details
    /// Stored as TEXT containing JSON: {"error_type": {"message": "...", "details": {...}, "occurred_at": "..."}}
    pub analysis_errors: Option<String>,
    pub modified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub thumbnail_path: Option<String>,
    pub thumbnail_generated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::book_metadata::Entity")]
    BookMetadata,
    #[sea_orm(has_many = "super::pages::Entity")]
    Pages,
    #[sea_orm(has_many = "super::read_progress::Entity")]
    ReadProgress,
    #[sea_orm(
        belongs_to = "super::series::Entity",
        from = "Column::SeriesId",
        to = "super::series::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Series,
    #[sea_orm(
        belongs_to = "super::libraries::Entity",
        from = "Column::LibraryId",
        to = "super::libraries::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Libraries,
}

impl Related<super::book_metadata::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BookMetadata.def()
    }
}

impl Related<super::pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pages.def()
    }
}

impl Related<super::read_progress::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReadProgress.def()
    }
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Libraries.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
