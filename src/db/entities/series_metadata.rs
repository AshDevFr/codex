//! `SeaORM` Entity for series_metadata table
//!
//! This table stores rich descriptive metadata for series (1:1 relationship with series).
//! Includes lock fields to prevent auto-refresh from overwriting user edits.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "series_metadata")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub series_id: Uuid,
    pub title: String,
    pub title_sort: Option<String>,
    pub summary: Option<String>,
    pub publisher: Option<String>,
    pub imprint: Option<String>,
    pub status: Option<String>, // ongoing, ended, hiatus, abandoned, unknown
    pub age_rating: Option<i32>,
    pub language: Option<String>,          // BCP47: "en", "ja", "ko"
    pub reading_direction: Option<String>, // ltr, rtl, ttb
    pub year: Option<i32>,
    pub total_book_count: Option<i32>, // Expected total (for ongoing series)
    // Lock fields
    pub total_book_count_lock: bool,
    pub title_lock: bool,
    pub title_sort_lock: bool,
    pub summary_lock: bool,
    pub publisher_lock: bool,
    pub imprint_lock: bool,
    pub status_lock: bool,
    pub age_rating_lock: bool,
    pub language_lock: bool,
    pub reading_direction_lock: bool,
    pub year_lock: bool,
    pub genres_lock: bool,
    pub tags_lock: bool,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::series::Entity",
        from = "Column::SeriesId",
        to = "super::series::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Series,
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
