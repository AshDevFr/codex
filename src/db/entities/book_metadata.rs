//! `SeaORM` Entity for book_metadata table
//!
//! This table stores rich metadata for books (1:1 relationship with books).
//! Includes lock fields to prevent auto-refresh from overwriting user edits.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "book_metadata")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub book_id: Uuid,
    // Content fields
    pub summary: Option<String>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub inker: Option<String>,
    pub colorist: Option<String>,
    pub letterer: Option<String>,
    pub cover_artist: Option<String>,
    pub editor: Option<String>,
    pub publisher: Option<String>,
    pub imprint: Option<String>,
    pub genre: Option<String>,
    pub web: Option<String>,
    pub language_iso: Option<String>,
    pub format_detail: Option<String>,
    pub black_and_white: Option<bool>,
    pub manga: Option<bool>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub volume: Option<i32>,
    pub count: Option<i32>,
    pub isbns: Option<String>,
    // Lock fields - prevent auto-refresh from overwriting user edits
    pub summary_lock: bool,
    pub writer_lock: bool,
    pub penciller_lock: bool,
    pub inker_lock: bool,
    pub colorist_lock: bool,
    pub letterer_lock: bool,
    pub cover_artist_lock: bool,
    pub editor_lock: bool,
    pub publisher_lock: bool,
    pub imprint_lock: bool,
    pub genre_lock: bool,
    pub web_lock: bool,
    pub language_iso_lock: bool,
    pub format_detail_lock: bool,
    pub black_and_white_lock: bool,
    pub manga_lock: bool,
    pub year_lock: bool,
    pub month_lock: bool,
    pub day_lock: bool,
    pub volume_lock: bool,
    pub count_lock: bool,
    pub isbns_lock: bool,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::books::Entity",
        from = "Column::BookId",
        to = "super::books::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Books,
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Books.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
