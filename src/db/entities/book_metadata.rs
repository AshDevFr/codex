//! `SeaORM` Entity for book_metadata table
//!
//! This table stores rich metadata for books (1:1 relationship with books).
//! Includes lock fields to prevent auto-refresh from overwriting user edits.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

// =============================================================================
// BookType Enum
// =============================================================================

/// Book type classification for content categorization
///
/// This enum defines the allowed book type values for book metadata.
/// The database stores these as lowercase strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookType {
    /// Western comic book
    Comic,
    /// Japanese manga
    Manga,
    /// Full-length novel
    Novel,
    /// Short novel (typically 17,500-40,000 words)
    Novella,
    /// Collection of short stories or works by multiple authors
    Anthology,
    /// Art collection book
    Artbook,
    /// Standalone story (single issue)
    Oneshot,
    /// Collection of multiple volumes/issues in one book
    Omnibus,
    /// Long-form comic narrative (typically standalone)
    GraphicNovel,
    /// Periodical publication
    Magazine,
}

impl BookType {
    /// Get the string representation used in the database
    pub fn as_str(&self) -> &'static str {
        match self {
            BookType::Comic => "comic",
            BookType::Manga => "manga",
            BookType::Novel => "novel",
            BookType::Novella => "novella",
            BookType::Anthology => "anthology",
            BookType::Artbook => "artbook",
            BookType::Oneshot => "oneshot",
            BookType::Omnibus => "omnibus",
            BookType::GraphicNovel => "graphic_novel",
            BookType::Magazine => "magazine",
        }
    }

    /// All valid book type values
    #[allow(dead_code)]
    pub fn all() -> &'static [BookType] {
        &[
            BookType::Comic,
            BookType::Manga,
            BookType::Novel,
            BookType::Novella,
            BookType::Anthology,
            BookType::Artbook,
            BookType::Oneshot,
            BookType::Omnibus,
            BookType::GraphicNovel,
            BookType::Magazine,
        ]
    }
}

impl fmt::Display for BookType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for BookType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "comic" => Ok(BookType::Comic),
            "manga" => Ok(BookType::Manga),
            "novel" => Ok(BookType::Novel),
            "novella" => Ok(BookType::Novella),
            "anthology" => Ok(BookType::Anthology),
            "artbook" | "art_book" => Ok(BookType::Artbook),
            "oneshot" | "one_shot" => Ok(BookType::Oneshot),
            "omnibus" => Ok(BookType::Omnibus),
            "graphic_novel" | "graphicnovel" => Ok(BookType::GraphicNovel),
            "magazine" => Ok(BookType::Magazine),
            _ => Err(format!("Invalid book type: {}", s)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "book_metadata")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub book_id: Uuid,
    // Display fields (moved from books table)
    pub title: Option<String>,
    pub title_sort: Option<String>,
    pub number: Option<Decimal>,
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
    // New book metadata fields (Phase 1)
    /// Book type classification (comic, manga, novel, etc.)
    pub book_type: Option<String>,
    /// Book subtitle
    pub subtitle: Option<String>,
    /// Structured author information as JSON array
    /// Format: [{"name": "...", "role": "author|co-author|editor|...", "sort_name": "..."}]
    pub authors_json: Option<String>,
    /// Translator name
    pub translator: Option<String>,
    /// Edition information (e.g., "First Edition", "Revised Edition")
    pub edition: Option<String>,
    /// Original title (for translated works)
    pub original_title: Option<String>,
    /// Original publication year (for re-releases or translations)
    pub original_year: Option<i32>,
    /// Position in a series (e.g., 1.0, 2.5 for .5 volumes)
    pub series_position: Option<Decimal>,
    /// Total number of books in the series
    pub series_total: Option<i32>,
    /// Subject/topic tags as JSON array or comma-separated string
    pub subjects: Option<String>,
    /// Awards as JSON array
    /// Format: [{"name": "...", "year": 2020, "category": "...", "won": true|false}]
    pub awards_json: Option<String>,
    /// JSON escape hatch for user-defined fields
    pub custom_metadata: Option<String>,
    // Lock fields - prevent auto-refresh from overwriting user edits
    pub title_lock: bool,
    pub title_sort_lock: bool,
    pub number_lock: bool,
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
    // New lock fields for Phase 1 fields
    pub book_type_lock: bool,
    pub subtitle_lock: bool,
    pub authors_json_lock: bool,
    pub translator_lock: bool,
    pub edition_lock: bool,
    pub original_title_lock: bool,
    pub original_year_lock: bool,
    pub series_position_lock: bool,
    pub series_total_lock: bool,
    pub subjects_lock: bool,
    pub awards_json_lock: bool,
    pub custom_metadata_lock: bool,
    /// Lock cover to prevent auto-updates (mirrors series_metadata.cover_lock)
    pub cover_lock: bool,
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_book_type_as_str() {
        assert_eq!(BookType::Comic.as_str(), "comic");
        assert_eq!(BookType::Manga.as_str(), "manga");
        assert_eq!(BookType::Novel.as_str(), "novel");
        assert_eq!(BookType::Novella.as_str(), "novella");
        assert_eq!(BookType::Anthology.as_str(), "anthology");
        assert_eq!(BookType::Artbook.as_str(), "artbook");
        assert_eq!(BookType::Oneshot.as_str(), "oneshot");
        assert_eq!(BookType::Omnibus.as_str(), "omnibus");
        assert_eq!(BookType::GraphicNovel.as_str(), "graphic_novel");
        assert_eq!(BookType::Magazine.as_str(), "magazine");
    }

    #[test]
    fn test_book_type_from_str() {
        assert_eq!(BookType::from_str("comic"), Ok(BookType::Comic));
        assert_eq!(BookType::from_str("MANGA"), Ok(BookType::Manga));
        assert_eq!(BookType::from_str("Novel"), Ok(BookType::Novel));
        assert_eq!(BookType::from_str("novella"), Ok(BookType::Novella));
        assert_eq!(BookType::from_str("anthology"), Ok(BookType::Anthology));
        assert_eq!(BookType::from_str("artbook"), Ok(BookType::Artbook));
        assert_eq!(BookType::from_str("art_book"), Ok(BookType::Artbook));
        assert_eq!(BookType::from_str("oneshot"), Ok(BookType::Oneshot));
        assert_eq!(BookType::from_str("one_shot"), Ok(BookType::Oneshot));
        assert_eq!(BookType::from_str("omnibus"), Ok(BookType::Omnibus));
        assert_eq!(
            BookType::from_str("graphic_novel"),
            Ok(BookType::GraphicNovel)
        );
        assert_eq!(
            BookType::from_str("graphicnovel"),
            Ok(BookType::GraphicNovel)
        );
        assert_eq!(BookType::from_str("magazine"), Ok(BookType::Magazine));
    }

    #[test]
    fn test_book_type_from_str_invalid() {
        assert!(BookType::from_str("invalid").is_err());
        assert!(BookType::from_str("").is_err());
        assert!(BookType::from_str("book").is_err());
    }

    #[test]
    fn test_book_type_display() {
        assert_eq!(format!("{}", BookType::Comic), "comic");
        assert_eq!(format!("{}", BookType::GraphicNovel), "graphic_novel");
    }

    #[test]
    fn test_book_type_all() {
        let all = BookType::all();
        assert_eq!(all.len(), 10);
        assert!(all.contains(&BookType::Comic));
        assert!(all.contains(&BookType::Magazine));
    }

    #[test]
    fn test_book_type_serialize() {
        let comic = BookType::Comic;
        let serialized = serde_json::to_string(&comic).unwrap();
        assert_eq!(serialized, "\"comic\"");

        let graphic_novel = BookType::GraphicNovel;
        let serialized = serde_json::to_string(&graphic_novel).unwrap();
        assert_eq!(serialized, "\"graphic_novel\"");
    }

    #[test]
    fn test_book_type_deserialize() {
        let comic: BookType = serde_json::from_str("\"comic\"").unwrap();
        assert_eq!(comic, BookType::Comic);

        let graphic_novel: BookType = serde_json::from_str("\"graphic_novel\"").unwrap();
        assert_eq!(graphic_novel, BookType::GraphicNovel);
    }
}
