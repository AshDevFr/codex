//! `SeaORM` Entity for series_metadata table
//!
//! This table stores rich descriptive metadata for series (1:1 relationship with series).
//! Includes lock fields to prevent auto-refresh from overwriting user edits.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

// =============================================================================
// Series Status Enum
// =============================================================================

/// Series publication status - canonical values stored in database
///
/// This enum defines the allowed status values for series metadata.
/// The database stores these as lowercase strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeriesStatus {
    /// Series is currently being published
    Ongoing,
    /// Series has finished publication
    Ended,
    /// Series is on hiatus
    Hiatus,
    /// Series was abandoned/cancelled
    Abandoned,
    /// Publication status is unknown
    #[default]
    Unknown,
}

impl SeriesStatus {
    /// Get the string representation used in the database
    pub fn as_str(&self) -> &'static str {
        match self {
            SeriesStatus::Ongoing => "ongoing",
            SeriesStatus::Ended => "ended",
            SeriesStatus::Hiatus => "hiatus",
            SeriesStatus::Abandoned => "abandoned",
            SeriesStatus::Unknown => "unknown",
        }
    }

    /// All valid status values
    #[allow(dead_code)]
    pub fn all() -> &'static [SeriesStatus] {
        &[
            SeriesStatus::Ongoing,
            SeriesStatus::Ended,
            SeriesStatus::Hiatus,
            SeriesStatus::Abandoned,
            SeriesStatus::Unknown,
        ]
    }
}

impl fmt::Display for SeriesStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for SeriesStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ongoing" => Ok(SeriesStatus::Ongoing),
            "ended" | "completed" => Ok(SeriesStatus::Ended), // Accept "completed" as alias
            "hiatus" => Ok(SeriesStatus::Hiatus),
            "abandoned" | "cancelled" => Ok(SeriesStatus::Abandoned), // Accept "cancelled" as alias
            "unknown" => Ok(SeriesStatus::Unknown),
            _ => Err(format!("Invalid series status: {}", s)),
        }
    }
}

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
    pub custom_metadata: Option<String>, // JSON escape hatch for user-defined fields
    /// Structured author information as JSON array
    /// Format: [{"name": "...", "role": "author|co-author|editor|...", "sort_name": "..."}]
    pub authors_json: Option<String>,
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
    pub custom_metadata_lock: bool,
    pub authors_json_lock: bool,
    pub cover_lock: bool,
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
