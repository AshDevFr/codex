//! Series and book context for condition evaluation and template rendering.
//!
//! This module provides `SeriesContext` and `BookContext` structures that aggregate data
//! from various sources (series/book, metadata, external IDs, book count) to
//! provide a unified interface for condition evaluation and template rendering.
//!
//! ## Example
//!
//! ```ignore
//! use codex::services::metadata::preprocessing::context::SeriesContext;
//!
//! let context = SeriesContext::new(series_id)
//!     .with_metadata(metadata)
//!     .with_external_ids(external_ids)
//!     .with_book_count(5)
//!     .with_custom_metadata(custom_json);
//!
//! // Context can then be used for condition evaluation
//! ```

use sea_orm::prelude::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// Series Context
// =============================================================================

/// Aggregated context for evaluating conditions against a series.
///
/// This structure holds all the data needed to evaluate conditions,
/// including series metadata, external IDs, book count, and custom metadata.
///
/// ## JSON Output
///
/// When serialized to JSON, this structure uses camelCase field names:
///
/// ```json
/// {
///   "seriesId": "550e8400-e29b-41d4-a716-446655440000",
///   "bookCount": 5,
///   "metadata": {
///     "title": "One Piece",
///     "titleSort": "One Piece",
///     "genres": ["Action", "Adventure"],
///     "tags": ["pirates", "treasure"]
///   },
///   "externalIds": {
///     "plugin:mangabaka": { "id": "12345", "url": "...", "hash": "..." }
///   },
///   "customMetadata": { "myField": "preserved as-is" }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesContext {
    /// Type discriminator — always "series" for SeriesContext
    #[serde(rename = "type")]
    pub context_type: String,

    /// Series ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<Uuid>,

    /// Number of books in the series
    pub book_count: i64,

    /// Metadata fields
    #[serde(default)]
    pub metadata: MetadataContext,

    /// External IDs mapped by source name
    #[serde(default)]
    pub external_ids: HashMap<String, ExternalIdContext>,

    /// Custom metadata fields (preserved as-is, no case transformation)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_metadata: Option<Value>,
}

impl Default for SeriesContext {
    fn default() -> Self {
        Self {
            context_type: "series".to_string(),
            series_id: None,
            book_count: 0,
            metadata: MetadataContext::default(),
            external_ids: HashMap::new(),
            custom_metadata: None,
        }
    }
}

/// Series metadata context for condition evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataContext {
    pub title: Option<String>,
    pub title_sort: Option<String>,
    pub summary: Option<String>,
    pub publisher: Option<String>,
    pub imprint: Option<String>,
    pub status: Option<String>,
    pub age_rating: Option<i32>,
    pub language: Option<String>,
    pub reading_direction: Option<String>,
    pub year: Option<i32>,
    pub total_book_count: Option<i32>,

    /// Genre names for this series
    #[serde(default)]
    pub genres: Vec<String>,

    /// Tag names for this series
    #[serde(default)]
    pub tags: Vec<String>,

    /// Alternate titles (e.g., Japanese, Romaji, English)
    #[serde(default)]
    pub alternate_titles: Vec<AlternateTitleContext>,

    /// Structured author information
    #[serde(default)]
    pub authors: Vec<AuthorContext>,

    /// External ratings from various sources
    #[serde(default)]
    pub external_ratings: Vec<ExternalRatingContext>,

    /// External links to other sites
    #[serde(default)]
    pub external_links: Vec<ExternalLinkContext>,

    // Lock fields
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
    pub total_book_count_lock: bool,
    pub genres_lock: bool,
    pub tags_lock: bool,
    pub custom_metadata_lock: bool,
    pub cover_lock: bool,
    pub authors_json_lock: bool,
}

/// External ID context for a single source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalIdContext {
    /// External ID value
    pub id: String,
    /// External URL (optional)
    pub url: Option<String>,
    /// Metadata hash (optional)
    pub hash: Option<String>,
}

/// Alternate title context (e.g., Japanese title, Romaji, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitleContext {
    /// Label for this alternate title (e.g., "Japanese", "Romaji")
    pub label: String,
    /// The alternate title text
    pub title: String,
}

/// Structured author information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorContext {
    /// Author name
    pub name: String,
    /// Role (e.g., "author", "artist", "editor")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Sort name (e.g., "Lastname, Firstname")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
}

/// External rating from a source (e.g., MyAnimeList, AniList).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRatingContext {
    /// Source name (e.g., "myanimelist", "anilist")
    pub source: String,
    /// Rating value (normalized to 0-100)
    pub rating: f64,
    /// Number of votes (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub votes: Option<i32>,
}

/// External link to another site.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLinkContext {
    /// Source name (e.g., "mangadex", "myanimelist")
    pub source: String,
    /// URL to the external resource
    pub url: String,
    /// External ID on the source (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

// =============================================================================
// Builder Methods
// =============================================================================

impl SeriesContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new context with a series ID.
    pub fn with_series_id(series_id: Uuid) -> Self {
        Self {
            series_id: Some(series_id),
            ..Self::default()
        }
    }

    /// Set the series ID.
    pub fn series_id(mut self, series_id: Uuid) -> Self {
        self.series_id = Some(series_id);
        self
    }

    /// Set the book count.
    pub fn book_count(mut self, count: i64) -> Self {
        self.book_count = count;
        self
    }

    /// Set the metadata context.
    pub fn metadata(mut self, metadata: MetadataContext) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add an external ID.
    pub fn external_id(mut self, source: impl Into<String>, id: impl Into<String>) -> Self {
        self.external_ids.insert(
            source.into(),
            ExternalIdContext {
                id: id.into(),
                url: None,
                hash: None,
            },
        );
        self
    }

    /// Add an external ID with full details.
    pub fn external_id_full(
        mut self,
        source: impl Into<String>,
        context: ExternalIdContext,
    ) -> Self {
        self.external_ids.insert(source.into(), context);
        self
    }

    /// Set the external IDs map.
    pub fn external_ids(mut self, ids: HashMap<String, ExternalIdContext>) -> Self {
        self.external_ids = ids;
        self
    }

    /// Set custom metadata.
    pub fn custom_metadata(mut self, custom_metadata: Value) -> Self {
        self.custom_metadata = Some(custom_metadata);
        self
    }

    // =========================================================================
    // Field Access Methods
    // =========================================================================

    /// Get a field value by path.
    ///
    /// Supports both camelCase and snake_case paths for backwards compatibility.
    ///
    /// Supported paths:
    /// - `bookCount` / `book_count` - Number of books
    /// - `externalIds.count` / `external_ids.count` - Number of external sources linked
    /// - `externalIds.<source>` / `external_ids.<source>` - External ID for a specific source
    /// - `metadata.<field>` - Metadata field (title, year, status, genres, tags, etc.)
    /// - `metadata.<field>Lock` / `metadata.<field>_lock` - Lock state for a metadata field
    /// - `customMetadata.<field>` / `custom_metadata.<field>` - Custom metadata field
    pub fn get_field(&self, path: &str) -> Option<FieldValue> {
        let parts: Vec<&str> = path.splitn(2, '.').collect();

        match parts[0] {
            // Type discriminator
            "type" => Some(FieldValue::String(self.context_type.clone())),
            // Support both camelCase and snake_case for top-level fields
            "bookCount" | "book_count" => Some(FieldValue::Number(self.book_count as f64)),
            "externalIds" | "external_ids" => self.get_external_id_field(parts.get(1).copied()),
            "metadata" => parts.get(1).and_then(|f| self.get_metadata_field(f)),
            "customMetadata" | "custom_metadata" => {
                parts.get(1).and_then(|f| self.get_custom_metadata_field(f))
            }
            _ => None,
        }
    }

    fn get_external_id_field(&self, subfield: Option<&str>) -> Option<FieldValue> {
        match subfield {
            None => None,
            Some("count") => Some(FieldValue::Number(self.external_ids.len() as f64)),
            Some(source) => self
                .external_ids
                .get(source)
                .map(|ctx| FieldValue::String(ctx.id.clone())),
        }
    }

    fn get_metadata_field(&self, field: &str) -> Option<FieldValue> {
        // Support both camelCase and snake_case field names
        match field {
            "title" => self.metadata.title.clone().map(FieldValue::String),
            "titleSort" | "title_sort" => self.metadata.title_sort.clone().map(FieldValue::String),
            "summary" => self.metadata.summary.clone().map(FieldValue::String),
            "publisher" => self.metadata.publisher.clone().map(FieldValue::String),
            "imprint" => self.metadata.imprint.clone().map(FieldValue::String),
            "status" => self.metadata.status.clone().map(FieldValue::String),
            "ageRating" | "age_rating" => self
                .metadata
                .age_rating
                .map(|v| FieldValue::Number(v as f64)),
            "language" => self.metadata.language.clone().map(FieldValue::String),
            "readingDirection" | "reading_direction" => self
                .metadata
                .reading_direction
                .clone()
                .map(FieldValue::String),
            "year" => self.metadata.year.map(|v| FieldValue::Number(v as f64)),
            "totalBookCount" | "total_book_count" => self
                .metadata
                .total_book_count
                .map(|v| FieldValue::Number(v as f64)),
            // Array fields
            "genres" => {
                let arr: Vec<Value> = self
                    .metadata
                    .genres
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "tags" => {
                let arr: Vec<Value> = self
                    .metadata
                    .tags
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "alternateTitles" | "alternate_titles" => {
                let arr: Vec<Value> = self
                    .metadata
                    .alternate_titles
                    .iter()
                    .map(|at| serde_json::to_value(at).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "authors" => {
                let arr: Vec<Value> = self
                    .metadata
                    .authors
                    .iter()
                    .map(|a| serde_json::to_value(a).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "externalRatings" | "external_ratings" => {
                let arr: Vec<Value> = self
                    .metadata
                    .external_ratings
                    .iter()
                    .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "externalLinks" | "external_links" => {
                let arr: Vec<Value> = self
                    .metadata
                    .external_links
                    .iter()
                    .map(|l| serde_json::to_value(l).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            // Lock fields (support both camelCase and snake_case)
            "titleLock" | "title_lock" => Some(FieldValue::Bool(self.metadata.title_lock)),
            "titleSortLock" | "title_sort_lock" => {
                Some(FieldValue::Bool(self.metadata.title_sort_lock))
            }
            "summaryLock" | "summary_lock" => Some(FieldValue::Bool(self.metadata.summary_lock)),
            "publisherLock" | "publisher_lock" => {
                Some(FieldValue::Bool(self.metadata.publisher_lock))
            }
            "imprintLock" | "imprint_lock" => Some(FieldValue::Bool(self.metadata.imprint_lock)),
            "statusLock" | "status_lock" => Some(FieldValue::Bool(self.metadata.status_lock)),
            "ageRatingLock" | "age_rating_lock" => {
                Some(FieldValue::Bool(self.metadata.age_rating_lock))
            }
            "languageLock" | "language_lock" => Some(FieldValue::Bool(self.metadata.language_lock)),
            "readingDirectionLock" | "reading_direction_lock" => {
                Some(FieldValue::Bool(self.metadata.reading_direction_lock))
            }
            "yearLock" | "year_lock" => Some(FieldValue::Bool(self.metadata.year_lock)),
            "totalBookCountLock" | "total_book_count_lock" => {
                Some(FieldValue::Bool(self.metadata.total_book_count_lock))
            }
            "genresLock" | "genres_lock" => Some(FieldValue::Bool(self.metadata.genres_lock)),
            "tagsLock" | "tags_lock" => Some(FieldValue::Bool(self.metadata.tags_lock)),
            "customMetadataLock" | "custom_metadata_lock" => {
                Some(FieldValue::Bool(self.metadata.custom_metadata_lock))
            }
            "coverLock" | "cover_lock" => Some(FieldValue::Bool(self.metadata.cover_lock)),
            "authorsJsonLock" | "authors_json_lock" => {
                Some(FieldValue::Bool(self.metadata.authors_json_lock))
            }
            _ => None,
        }
    }

    fn get_custom_metadata_field(&self, field: &str) -> Option<FieldValue> {
        self.custom_metadata.as_ref().and_then(|c| {
            // Traverse nested path (e.g., "author.name" -> ["author", "name"])
            let value = field.split('.').try_fold(c, |v, key| v.get(key))?;

            Some(match value {
                Value::String(s) => FieldValue::String(s.clone()),
                Value::Number(n) => FieldValue::Number(n.as_f64().unwrap_or(0.0)),
                Value::Bool(b) => FieldValue::Bool(*b),
                Value::Null => FieldValue::Null,
                Value::Array(a) => FieldValue::Array(a.clone()),
                Value::Object(_) => FieldValue::Json(value.clone()),
            })
        })
    }

    /// Check if an external ID exists for a specific source.
    pub fn has_external_id(&self, source: &str) -> bool {
        self.external_ids.contains_key(source)
    }

    /// Get the external ID for a specific source.
    pub fn get_external_id(&self, source: &str) -> Option<&str> {
        self.external_ids.get(source).map(|ctx| ctx.id.as_str())
    }

    /// Get the number of external sources linked.
    pub fn external_id_count(&self) -> usize {
        self.external_ids.len()
    }

    /// Set genres on the metadata context.
    pub fn genres(mut self, genres: Vec<String>) -> Self {
        self.metadata.genres = genres;
        self
    }

    /// Set tags on the metadata context.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.metadata.tags = tags;
        self
    }
}

// =============================================================================
// Series Context Builder (async from database)
// =============================================================================

use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::db::repositories::{
    AlternateTitleRepository, BookRepository, ExternalLinkRepository, ExternalRatingRepository,
    GenreRepository, SeriesExternalIdRepository, SeriesMetadataRepository, TagRepository,
};

/// Builder for creating a `SeriesContext` from database entities.
///
/// This provides an async interface to fetch all required data from the database
/// and construct a fully-populated `SeriesContext`.
///
/// ## Example
///
/// ```ignore
/// let context = SeriesContextBuilder::new(series_id)
///     .build(&db)
///     .await?;
/// ```
pub struct SeriesContextBuilder {
    series_id: Uuid,
}

impl SeriesContextBuilder {
    /// Create a new builder for the given series ID.
    pub fn new(series_id: Uuid) -> Self {
        Self { series_id }
    }

    /// Build the full `SeriesContext` by fetching all required data from the database.
    ///
    /// This fetches:
    /// - Series metadata (including authors_json)
    /// - Book count
    /// - Genres
    /// - Tags
    /// - External IDs
    /// - Alternate titles
    /// - External ratings
    /// - External links
    /// - Custom metadata (from series_metadata.custom_metadata JSON field)
    pub async fn build(&self, db: &DatabaseConnection) -> Result<SeriesContext> {
        // Fetch all required data concurrently
        let (
            metadata_opt,
            book_count,
            genres,
            tags,
            external_ids,
            alternate_titles,
            external_ratings,
            external_links,
        ) = tokio::try_join!(
            SeriesMetadataRepository::get_by_series_id(db, self.series_id),
            BookRepository::count_by_series(db, self.series_id),
            GenreRepository::get_genres_for_series(db, self.series_id),
            TagRepository::get_tags_for_series(db, self.series_id),
            SeriesExternalIdRepository::get_for_series(db, self.series_id),
            AlternateTitleRepository::get_for_series(db, self.series_id),
            ExternalRatingRepository::get_for_series(db, self.series_id),
            ExternalLinkRepository::get_for_series(db, self.series_id),
        )?;

        // Build alternate titles context
        let alternate_titles_ctx: Vec<AlternateTitleContext> = alternate_titles
            .into_iter()
            .map(|at| AlternateTitleContext {
                label: at.label,
                title: at.title,
            })
            .collect();

        // Build external ratings context
        let external_ratings_ctx: Vec<ExternalRatingContext> = external_ratings
            .into_iter()
            .map(|r| ExternalRatingContext {
                source: r.source_name,
                rating: Decimal::to_string(&r.rating).parse::<f64>().unwrap_or(0.0),
                votes: r.vote_count,
            })
            .collect();

        // Build external links context
        let external_links_ctx: Vec<ExternalLinkContext> = external_links
            .into_iter()
            .map(|l| ExternalLinkContext {
                source: l.source_name,
                url: l.url,
                external_id: l.external_id,
            })
            .collect();

        // Parse authors_json if present
        let authors_ctx: Vec<AuthorContext> = metadata_opt
            .as_ref()
            .and_then(|m| m.authors_json.as_ref())
            .and_then(|json_str| serde_json::from_str(json_str).ok())
            .unwrap_or_default();

        // Build metadata context
        let metadata_context = if let Some(ref m) = metadata_opt {
            MetadataContext {
                title: Some(m.title.clone()),
                title_sort: m.title_sort.clone(),
                summary: m.summary.clone(),
                publisher: m.publisher.clone(),
                imprint: m.imprint.clone(),
                status: m.status.clone(),
                age_rating: m.age_rating,
                language: m.language.clone(),
                reading_direction: m.reading_direction.clone(),
                year: m.year,
                total_book_count: m.total_book_count,
                genres: genres.iter().map(|g| g.name.clone()).collect(),
                tags: tags.iter().map(|t| t.name.clone()).collect(),
                alternate_titles: alternate_titles_ctx,
                authors: authors_ctx,
                external_ratings: external_ratings_ctx,
                external_links: external_links_ctx,
                title_lock: m.title_lock,
                title_sort_lock: m.title_sort_lock,
                summary_lock: m.summary_lock,
                publisher_lock: m.publisher_lock,
                imprint_lock: m.imprint_lock,
                status_lock: m.status_lock,
                age_rating_lock: m.age_rating_lock,
                language_lock: m.language_lock,
                reading_direction_lock: m.reading_direction_lock,
                year_lock: m.year_lock,
                total_book_count_lock: m.total_book_count_lock,
                genres_lock: m.genres_lock,
                tags_lock: m.tags_lock,
                custom_metadata_lock: m.custom_metadata_lock,
                cover_lock: m.cover_lock,
                authors_json_lock: m.authors_json_lock,
            }
        } else {
            // No metadata exists - still include genres/tags and related data
            MetadataContext {
                genres: genres.iter().map(|g| g.name.clone()).collect(),
                tags: tags.iter().map(|t| t.name.clone()).collect(),
                alternate_titles: alternate_titles_ctx,
                authors: authors_ctx,
                external_ratings: external_ratings_ctx,
                external_links: external_links_ctx,
                ..Default::default()
            }
        };

        // Build external IDs map
        let external_ids_map: HashMap<String, ExternalIdContext> = external_ids
            .into_iter()
            .map(|eid| {
                (
                    eid.source.clone(),
                    ExternalIdContext {
                        id: eid.external_id,
                        url: eid.external_url,
                        hash: eid.metadata_hash,
                    },
                )
            })
            .collect();

        // Parse custom_metadata JSON if present
        let custom_metadata = metadata_opt
            .as_ref()
            .and_then(|m| m.custom_metadata.as_ref())
            .and_then(|json_str| serde_json::from_str(json_str).ok());

        // Build final context
        let mut context = SeriesContext::with_series_id(self.series_id)
            .book_count(book_count as i64)
            .metadata(metadata_context)
            .external_ids(external_ids_map);

        if let Some(custom) = custom_metadata {
            context = context.custom_metadata(custom);
        }

        Ok(context)
    }
}

// =============================================================================
// Book Context
// =============================================================================

/// Aggregated context for evaluating conditions and rendering templates for a book.
///
/// Includes all book metadata, genres, tags, external IDs/links, custom metadata,
/// and the parent series context for cross-referencing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookContext {
    /// Type discriminator — always "book" for BookContext
    #[serde(rename = "type")]
    pub context_type: String,

    /// Book ID
    pub book_id: Uuid,

    /// Parent series ID
    pub series_id: Uuid,

    /// Library ID
    pub library_id: Uuid,

    /// File format (e.g., "cbz", "epub", "pdf")
    pub file_format: String,

    /// Number of pages
    pub page_count: i32,

    /// File size in bytes
    pub file_size: i64,

    /// Book metadata fields
    #[serde(default)]
    pub metadata: BookMetadataContext,

    /// External IDs mapped by source name
    #[serde(default)]
    pub external_ids: HashMap<String, ExternalIdContext>,

    /// Custom metadata fields (preserved as-is, no case transformation)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_metadata: Option<Value>,

    /// Parent series context for cross-referencing
    pub series: SeriesContext,
}

/// Book metadata context for template rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookMetadataContext {
    // Display fields
    pub title: Option<String>,
    pub title_sort: Option<String>,
    pub number: Option<f64>,
    pub subtitle: Option<String>,

    // Content fields
    pub summary: Option<String>,
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
    pub book_type: Option<String>,

    // Structured fields
    pub authors: Vec<AuthorContext>,
    pub translator: Option<String>,
    pub edition: Option<String>,
    pub original_title: Option<String>,
    pub original_year: Option<i32>,
    pub series_position: Option<f64>,
    pub series_total: Option<i32>,
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub awards: Vec<BookAwardContext>,

    /// Genre names for this book
    #[serde(default)]
    pub genres: Vec<String>,

    /// Tag names for this book
    #[serde(default)]
    pub tags: Vec<String>,

    /// External links for this book
    #[serde(default)]
    pub external_links: Vec<ExternalLinkContext>,

    // Lock fields
    pub title_lock: bool,
    pub title_sort_lock: bool,
    pub number_lock: bool,
    pub summary_lock: bool,
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
    pub cover_lock: bool,
}

/// Book award context for template rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookAwardContext {
    /// Award name
    pub name: String,
    /// Year awarded (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Award category (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Whether the book won (vs nominated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub won: Option<bool>,
}

impl BookContext {
    /// Get a field value by path.
    ///
    /// Supports both camelCase and snake_case paths for backwards compatibility.
    pub fn get_field(&self, path: &str) -> Option<FieldValue> {
        let parts: Vec<&str> = path.splitn(2, '.').collect();

        match parts[0] {
            "type" => Some(FieldValue::String(self.context_type.clone())),
            "bookId" | "book_id" => Some(FieldValue::String(self.book_id.to_string())),
            "seriesId" | "series_id" => Some(FieldValue::String(self.series_id.to_string())),
            "libraryId" | "library_id" => Some(FieldValue::String(self.library_id.to_string())),
            "fileFormat" | "file_format" => Some(FieldValue::String(self.file_format.clone())),
            "pageCount" | "page_count" => Some(FieldValue::Number(self.page_count as f64)),
            "fileSize" | "file_size" => Some(FieldValue::Number(self.file_size as f64)),
            "externalIds" | "external_ids" => self.get_external_id_field(parts.get(1).copied()),
            "metadata" => parts.get(1).and_then(|f| self.get_metadata_field(f)),
            "customMetadata" | "custom_metadata" => {
                parts.get(1).and_then(|f| self.get_custom_metadata_field(f))
            }
            "series" => parts.get(1).and_then(|f| self.series.get_field(f)),
            _ => None,
        }
    }

    fn get_external_id_field(&self, subfield: Option<&str>) -> Option<FieldValue> {
        match subfield {
            None => None,
            Some("count") => Some(FieldValue::Number(self.external_ids.len() as f64)),
            Some(source) => self
                .external_ids
                .get(source)
                .map(|ctx| FieldValue::String(ctx.id.clone())),
        }
    }

    fn get_metadata_field(&self, field: &str) -> Option<FieldValue> {
        match field {
            "title" => self.metadata.title.clone().map(FieldValue::String),
            "titleSort" | "title_sort" => self.metadata.title_sort.clone().map(FieldValue::String),
            "number" => self.metadata.number.map(FieldValue::Number),
            "subtitle" => self.metadata.subtitle.clone().map(FieldValue::String),
            "summary" => self.metadata.summary.clone().map(FieldValue::String),
            "publisher" => self.metadata.publisher.clone().map(FieldValue::String),
            "imprint" => self.metadata.imprint.clone().map(FieldValue::String),
            "genre" => self.metadata.genre.clone().map(FieldValue::String),
            "languageIso" | "language_iso" => {
                self.metadata.language_iso.clone().map(FieldValue::String)
            }
            "formatDetail" | "format_detail" => {
                self.metadata.format_detail.clone().map(FieldValue::String)
            }
            "blackAndWhite" | "black_and_white" => {
                self.metadata.black_and_white.map(FieldValue::Bool)
            }
            "manga" => self.metadata.manga.map(FieldValue::Bool),
            "year" => self.metadata.year.map(|v| FieldValue::Number(v as f64)),
            "month" => self.metadata.month.map(|v| FieldValue::Number(v as f64)),
            "day" => self.metadata.day.map(|v| FieldValue::Number(v as f64)),
            "volume" => self.metadata.volume.map(|v| FieldValue::Number(v as f64)),
            "count" => self.metadata.count.map(|v| FieldValue::Number(v as f64)),
            "isbns" => self.metadata.isbns.clone().map(FieldValue::String),
            "bookType" | "book_type" => self.metadata.book_type.clone().map(FieldValue::String),
            "translator" => self.metadata.translator.clone().map(FieldValue::String),
            "edition" => self.metadata.edition.clone().map(FieldValue::String),
            "originalTitle" | "original_title" => {
                self.metadata.original_title.clone().map(FieldValue::String)
            }
            "originalYear" | "original_year" => self
                .metadata
                .original_year
                .map(|v| FieldValue::Number(v as f64)),
            "seriesPosition" | "series_position" => {
                self.metadata.series_position.map(FieldValue::Number)
            }
            "seriesTotal" | "series_total" => self
                .metadata
                .series_total
                .map(|v| FieldValue::Number(v as f64)),
            "authors" => {
                let arr: Vec<Value> = self
                    .metadata
                    .authors
                    .iter()
                    .map(|a| serde_json::to_value(a).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "subjects" => {
                let arr: Vec<Value> = self
                    .metadata
                    .subjects
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "awards" => {
                let arr: Vec<Value> = self
                    .metadata
                    .awards
                    .iter()
                    .map(|a| serde_json::to_value(a).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "genres" => {
                let arr: Vec<Value> = self
                    .metadata
                    .genres
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "tags" => {
                let arr: Vec<Value> = self
                    .metadata
                    .tags
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            "externalLinks" | "external_links" => {
                let arr: Vec<Value> = self
                    .metadata
                    .external_links
                    .iter()
                    .map(|l| serde_json::to_value(l).unwrap_or(Value::Null))
                    .collect();
                Some(FieldValue::Array(arr))
            }
            _ => None,
        }
    }

    fn get_custom_metadata_field(&self, field: &str) -> Option<FieldValue> {
        self.custom_metadata.as_ref().and_then(|c| {
            let value = field.split('.').try_fold(c, |v, key| v.get(key))?;
            Some(match value {
                Value::String(s) => FieldValue::String(s.clone()),
                Value::Number(n) => FieldValue::Number(n.as_f64().unwrap_or(0.0)),
                Value::Bool(b) => FieldValue::Bool(*b),
                Value::Null => FieldValue::Null,
                Value::Array(a) => FieldValue::Array(a.clone()),
                Value::Object(_) => FieldValue::Json(value.clone()),
            })
        })
    }
}

// =============================================================================
// Book Context Builder (async from database)
// =============================================================================

use crate::db::repositories::{
    BookExternalIdRepository, BookExternalLinkRepository, BookMetadataRepository,
};

/// Builder for creating a `BookContext` from database entities.
pub struct BookContextBuilder {
    book_id: Uuid,
}

impl BookContextBuilder {
    /// Create a new builder for the given book ID.
    pub fn new(book_id: Uuid) -> Self {
        Self { book_id }
    }

    /// Build the full `BookContext` by fetching all required data from the database.
    pub async fn build(&self, db: &DatabaseConnection) -> Result<BookContext> {
        // Fetch book first to get series_id
        let book = BookRepository::get_by_id(db, self.book_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Book not found: {}", self.book_id))?;

        // Fetch all book data concurrently
        let (book_metadata_opt, book_genres, book_tags, book_external_ids, book_external_links) = tokio::try_join!(
            BookMetadataRepository::get_by_book_id(db, self.book_id),
            GenreRepository::get_genres_for_book(db, self.book_id),
            TagRepository::get_tags_for_book(db, self.book_id),
            BookExternalIdRepository::get_for_book(db, self.book_id),
            BookExternalLinkRepository::get_for_book(db, self.book_id),
        )?;

        // Build parent series context concurrently
        let series_context = SeriesContextBuilder::new(book.series_id).build(db).await?;

        // Parse book authors_json
        let book_authors: Vec<AuthorContext> = book_metadata_opt
            .as_ref()
            .and_then(|m| m.authors_json.as_ref())
            .and_then(|json_str| serde_json::from_str(json_str).ok())
            .unwrap_or_default();

        // Parse book subjects
        let subjects: Vec<String> = book_metadata_opt
            .as_ref()
            .and_then(|m| m.subjects.as_ref())
            .and_then(|s| {
                // Try parsing as JSON array first, then fall back to comma-separated
                serde_json::from_str::<Vec<String>>(s).ok().or_else(|| {
                    Some(
                        s.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    )
                })
            })
            .unwrap_or_default();

        // Parse book awards_json
        let awards: Vec<BookAwardContext> = book_metadata_opt
            .as_ref()
            .and_then(|m| m.awards_json.as_ref())
            .and_then(|json_str| serde_json::from_str(json_str).ok())
            .unwrap_or_default();

        // Build book external links context
        let book_external_links_ctx: Vec<ExternalLinkContext> = book_external_links
            .into_iter()
            .map(|l| ExternalLinkContext {
                source: l.source_name,
                url: l.url,
                external_id: l.external_id,
            })
            .collect();

        // Build book metadata context
        let book_metadata_context = if let Some(ref m) = book_metadata_opt {
            BookMetadataContext {
                title: m.title.clone(),
                title_sort: m.title_sort.clone(),
                number: m
                    .number
                    .and_then(|d| Decimal::to_string(&d).parse::<f64>().ok()),
                subtitle: m.subtitle.clone(),
                summary: m.summary.clone(),
                publisher: m.publisher.clone(),
                imprint: m.imprint.clone(),
                genre: m.genre.clone(),
                language_iso: m.language_iso.clone(),
                format_detail: m.format_detail.clone(),
                black_and_white: m.black_and_white,
                manga: m.manga,
                year: m.year,
                month: m.month,
                day: m.day,
                volume: m.volume,
                count: m.count,
                isbns: m.isbns.clone(),
                book_type: m.book_type.clone(),
                authors: book_authors,
                translator: m.translator.clone(),
                edition: m.edition.clone(),
                original_title: m.original_title.clone(),
                original_year: m.original_year,
                series_position: m
                    .series_position
                    .and_then(|d| Decimal::to_string(&d).parse::<f64>().ok()),
                series_total: m.series_total,
                subjects,
                awards,
                genres: book_genres.iter().map(|g| g.name.clone()).collect(),
                tags: book_tags.iter().map(|t| t.name.clone()).collect(),
                external_links: book_external_links_ctx,
                title_lock: m.title_lock,
                title_sort_lock: m.title_sort_lock,
                number_lock: m.number_lock,
                summary_lock: m.summary_lock,
                publisher_lock: m.publisher_lock,
                imprint_lock: m.imprint_lock,
                genre_lock: m.genre_lock,
                language_iso_lock: m.language_iso_lock,
                format_detail_lock: m.format_detail_lock,
                black_and_white_lock: m.black_and_white_lock,
                manga_lock: m.manga_lock,
                year_lock: m.year_lock,
                month_lock: m.month_lock,
                day_lock: m.day_lock,
                volume_lock: m.volume_lock,
                count_lock: m.count_lock,
                isbns_lock: m.isbns_lock,
                book_type_lock: m.book_type_lock,
                subtitle_lock: m.subtitle_lock,
                authors_json_lock: m.authors_json_lock,
                translator_lock: m.translator_lock,
                edition_lock: m.edition_lock,
                original_title_lock: m.original_title_lock,
                original_year_lock: m.original_year_lock,
                series_position_lock: m.series_position_lock,
                series_total_lock: m.series_total_lock,
                subjects_lock: m.subjects_lock,
                awards_json_lock: m.awards_json_lock,
                custom_metadata_lock: m.custom_metadata_lock,
                cover_lock: m.cover_lock,
            }
        } else {
            BookMetadataContext {
                genres: book_genres.iter().map(|g| g.name.clone()).collect(),
                tags: book_tags.iter().map(|t| t.name.clone()).collect(),
                external_links: book_external_links_ctx,
                ..Default::default()
            }
        };

        // Build book external IDs map
        let book_external_ids_map: HashMap<String, ExternalIdContext> = book_external_ids
            .into_iter()
            .map(|eid| {
                (
                    eid.source.clone(),
                    ExternalIdContext {
                        id: eid.external_id,
                        url: eid.external_url,
                        hash: eid.metadata_hash,
                    },
                )
            })
            .collect();

        // Parse custom_metadata JSON if present
        let custom_metadata = book_metadata_opt
            .as_ref()
            .and_then(|m| m.custom_metadata.as_ref())
            .and_then(|json_str| serde_json::from_str(json_str).ok());

        Ok(BookContext {
            context_type: "book".to_string(),
            book_id: book.id,
            series_id: book.series_id,
            library_id: book.library_id,
            file_format: book.format.clone(),
            page_count: book.page_count,
            file_size: book.file_size,
            metadata: book_metadata_context,
            external_ids: book_external_ids_map,
            custom_metadata,
            series: series_context,
        })
    }
}

// =============================================================================
// Field Value Type
// =============================================================================

/// A typed field value for condition evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
    Array(Vec<Value>),
    Json(Value),
}

impl FieldValue {
    /// Check if this value is null or empty.
    pub fn is_null_or_empty(&self) -> bool {
        match self {
            FieldValue::Null => true,
            FieldValue::String(s) => s.is_empty(),
            FieldValue::Array(a) => a.is_empty(),
            _ => false,
        }
    }

    /// Try to convert to a string.
    pub fn as_string(&self) -> Option<String> {
        match self {
            FieldValue::String(s) => Some(s.clone()),
            FieldValue::Number(n) => Some(n.to_string()),
            FieldValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Try to convert to a number.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            FieldValue::Number(n) => Some(*n),
            FieldValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Try to convert to a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FieldValue::Bool(b) => Some(*b),
            FieldValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            FieldValue::Number(n) => Some(*n != 0.0),
            _ => None,
        }
    }
}

// =============================================================================
// Conversion from JSON Value
// =============================================================================

impl From<Value> for FieldValue {
    fn from(value: Value) -> Self {
        match value {
            Value::String(s) => FieldValue::String(s),
            Value::Number(n) => FieldValue::Number(n.as_f64().unwrap_or(0.0)),
            Value::Bool(b) => FieldValue::Bool(b),
            Value::Null => FieldValue::Null,
            Value::Array(a) => FieldValue::Array(a),
            Value::Object(_) => FieldValue::Json(value),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_context_builder() {
        let series_id = Uuid::new_v4();
        let context = SeriesContext::with_series_id(series_id)
            .book_count(10)
            .external_id("plugin:mangabaka", "12345")
            .external_id("comicinfo", "67890");

        assert_eq!(context.series_id, Some(series_id));
        assert_eq!(context.book_count, 10);
        assert_eq!(context.external_ids.len(), 2);
        assert!(context.has_external_id("plugin:mangabaka"));
        assert_eq!(context.get_external_id("plugin:mangabaka"), Some("12345"));
    }

    #[test]
    fn test_get_field_book_count() {
        let context = SeriesContext::new().book_count(5);
        let value = context.get_field("book_count");
        assert_eq!(value, Some(FieldValue::Number(5.0)));
    }

    #[test]
    fn test_get_field_external_ids_count() {
        let context = SeriesContext::new()
            .external_id("plugin:mangabaka", "123")
            .external_id("comicinfo", "456");
        let value = context.get_field("external_ids.count");
        assert_eq!(value, Some(FieldValue::Number(2.0)));
    }

    #[test]
    fn test_get_field_external_id_specific() {
        let context = SeriesContext::new().external_id("plugin:mangabaka", "12345");

        let value = context.get_field("external_ids.plugin:mangabaka");
        assert_eq!(value, Some(FieldValue::String("12345".to_string())));

        let value = context.get_field("external_ids.nonexistent");
        assert_eq!(value, None);
    }

    #[test]
    fn test_get_field_metadata() {
        let metadata = MetadataContext {
            title: Some("One Piece".to_string()),
            year: Some(1999),
            status: Some("ongoing".to_string()),
            title_lock: true,
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        assert_eq!(
            context.get_field("metadata.title"),
            Some(FieldValue::String("One Piece".to_string()))
        );
        assert_eq!(
            context.get_field("metadata.year"),
            Some(FieldValue::Number(1999.0))
        );
        assert_eq!(
            context.get_field("metadata.status"),
            Some(FieldValue::String("ongoing".to_string()))
        );
        assert_eq!(
            context.get_field("metadata.title_lock"),
            Some(FieldValue::Bool(true))
        );
        assert_eq!(context.get_field("metadata.publisher"), None);
    }

    #[test]
    fn test_get_field_custom() {
        let custom_metadata = serde_json::json!({
            "myField": "myValue",
            "myNumber": 42,
            "myBool": true
        });
        let context = SeriesContext::new().custom_metadata(custom_metadata);

        assert_eq!(
            context.get_field("custom_metadata.myField"),
            Some(FieldValue::String("myValue".to_string()))
        );
        assert_eq!(
            context.get_field("custom_metadata.myNumber"),
            Some(FieldValue::Number(42.0))
        );
        assert_eq!(
            context.get_field("custom_metadata.myBool"),
            Some(FieldValue::Bool(true))
        );
        assert_eq!(context.get_field("custom_metadata.nonexistent"), None);
    }

    #[test]
    fn test_get_field_custom_nested() {
        let custom_metadata = serde_json::json!({
            "author": {
                "name": "Oda",
                "country": "Japan",
                "details": {
                    "active": true,
                    "years": 25
                }
            },
            "ratings": {
                "score": 9.5
            }
        });
        let context = SeriesContext::new().custom_metadata(custom_metadata);

        // Single level nesting
        assert_eq!(
            context.get_field("custom_metadata.author.name"),
            Some(FieldValue::String("Oda".to_string()))
        );
        assert_eq!(
            context.get_field("custom_metadata.author.country"),
            Some(FieldValue::String("Japan".to_string()))
        );
        assert_eq!(
            context.get_field("custom_metadata.ratings.score"),
            Some(FieldValue::Number(9.5))
        );

        // Double level nesting
        assert_eq!(
            context.get_field("custom_metadata.author.details.active"),
            Some(FieldValue::Bool(true))
        );
        assert_eq!(
            context.get_field("custom_metadata.author.details.years"),
            Some(FieldValue::Number(25.0))
        );

        // Getting nested object as Json
        let author_details = context.get_field("custom_metadata.author.details");
        assert!(matches!(author_details, Some(FieldValue::Json(_))));

        // Non-existent nested paths
        assert_eq!(
            context.get_field("custom_metadata.author.nonexistent"),
            None
        );
        assert_eq!(context.get_field("custom_metadata.nonexistent.field"), None);
    }

    #[test]
    fn test_field_value_is_null_or_empty() {
        assert!(FieldValue::Null.is_null_or_empty());
        assert!(FieldValue::String(String::new()).is_null_or_empty());
        assert!(FieldValue::Array(Vec::new()).is_null_or_empty());
        assert!(!FieldValue::String("test".to_string()).is_null_or_empty());
        assert!(!FieldValue::Number(0.0).is_null_or_empty());
        assert!(!FieldValue::Bool(false).is_null_or_empty());
    }

    #[test]
    fn test_field_value_as_string() {
        assert_eq!(
            FieldValue::String("test".to_string()).as_string(),
            Some("test".to_string())
        );
        assert_eq!(FieldValue::Number(42.0).as_string(), Some("42".to_string()));
        assert_eq!(FieldValue::Bool(true).as_string(), Some("true".to_string()));
        assert_eq!(FieldValue::Null.as_string(), None);
    }

    #[test]
    fn test_field_value_as_number() {
        assert_eq!(FieldValue::Number(42.0).as_number(), Some(42.0));
        assert_eq!(FieldValue::String("42".to_string()).as_number(), Some(42.0));
        assert_eq!(FieldValue::String("invalid".to_string()).as_number(), None);
        assert_eq!(FieldValue::Null.as_number(), None);
    }

    #[test]
    fn test_field_value_as_bool() {
        assert_eq!(FieldValue::Bool(true).as_bool(), Some(true));
        assert_eq!(FieldValue::Bool(false).as_bool(), Some(false));
        assert_eq!(FieldValue::String("true".to_string()).as_bool(), Some(true));
        assert_eq!(
            FieldValue::String("false".to_string()).as_bool(),
            Some(false)
        );
        assert_eq!(FieldValue::Number(1.0).as_bool(), Some(true));
        assert_eq!(FieldValue::Number(0.0).as_bool(), Some(false));
        assert_eq!(FieldValue::Null.as_bool(), None);
    }

    #[test]
    fn test_metadata_context_default() {
        let metadata = MetadataContext::default();
        assert!(metadata.title.is_none());
        assert!(!metadata.title_lock);
    }

    #[test]
    fn test_external_id_context() {
        let ctx = ExternalIdContext {
            id: "12345".to_string(),
            url: Some("https://example.com/12345".to_string()),
            hash: Some("abc123".to_string()),
        };

        assert_eq!(ctx.id, "12345");
        assert_eq!(ctx.url, Some("https://example.com/12345".to_string()));
        assert_eq!(ctx.hash, Some("abc123".to_string()));
    }

    // ==========================================================================
    // JSON Serialization Tests (camelCase)
    // ==========================================================================

    #[test]
    fn test_series_context_serializes_to_camel_case() {
        let series_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let context = SeriesContext::with_series_id(series_id)
            .book_count(5)
            .external_id("plugin:mangabaka", "12345");

        let json = serde_json::to_value(&context).unwrap();

        // Type discriminator should be "series"
        assert_eq!(json["type"], "series");

        // Top-level fields should be camelCase
        assert!(json.get("seriesId").is_some(), "seriesId should exist");
        assert!(json.get("bookCount").is_some(), "bookCount should exist");
        assert!(
            json.get("externalIds").is_some(),
            "externalIds should exist"
        );

        // Should NOT have snake_case versions
        assert!(
            json.get("series_id").is_none(),
            "series_id should not exist"
        );
        assert!(
            json.get("book_count").is_none(),
            "book_count should not exist"
        );
        assert!(
            json.get("external_ids").is_none(),
            "external_ids should not exist"
        );

        // Verify values
        assert_eq!(json["seriesId"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["bookCount"], 5);
    }

    #[test]
    fn test_metadata_context_serializes_to_camel_case() {
        let metadata = MetadataContext {
            title: Some("One Piece".to_string()),
            title_sort: Some("One Piece".to_string()),
            age_rating: Some(13),
            reading_direction: Some("rtl".to_string()),
            total_book_count: Some(100),
            genres: vec!["Action".to_string(), "Adventure".to_string()],
            tags: vec!["pirates".to_string(), "treasure".to_string()],
            title_lock: true,
            genres_lock: false,
            custom_metadata_lock: false,
            ..Default::default()
        };

        let json = serde_json::to_value(&metadata).unwrap();

        // Verify camelCase field names
        assert!(json.get("titleSort").is_some(), "titleSort should exist");
        assert!(json.get("ageRating").is_some(), "ageRating should exist");
        assert!(
            json.get("readingDirection").is_some(),
            "readingDirection should exist"
        );
        assert!(
            json.get("totalBookCount").is_some(),
            "totalBookCount should exist"
        );
        assert!(json.get("titleLock").is_some(), "titleLock should exist");
        assert!(json.get("genresLock").is_some(), "genresLock should exist");
        assert!(
            json.get("customMetadataLock").is_some(),
            "customMetadataLock should exist"
        );

        // Should NOT have snake_case versions
        assert!(
            json.get("title_sort").is_none(),
            "title_sort should not exist"
        );
        assert!(
            json.get("age_rating").is_none(),
            "age_rating should not exist"
        );
        assert!(
            json.get("reading_direction").is_none(),
            "reading_direction should not exist"
        );
        assert!(
            json.get("total_book_count").is_none(),
            "total_book_count should not exist"
        );
        assert!(
            json.get("title_lock").is_none(),
            "title_lock should not exist"
        );

        // Verify values
        assert_eq!(json["title"], "One Piece");
        assert_eq!(json["titleSort"], "One Piece");
        assert_eq!(json["ageRating"], 13);
        assert_eq!(json["readingDirection"], "rtl");
        assert_eq!(json["totalBookCount"], 100);
        assert_eq!(json["titleLock"], true);
        assert_eq!(json["genres"], serde_json::json!(["Action", "Adventure"]));
        assert_eq!(json["tags"], serde_json::json!(["pirates", "treasure"]));
    }

    #[test]
    fn test_full_context_json_structure() {
        let series_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let metadata = MetadataContext {
            title: Some("One Piece".to_string()),
            genres: vec!["Action".to_string(), "Adventure".to_string()],
            tags: vec!["pirates".to_string()],
            ..Default::default()
        };

        let custom = serde_json::json!({
            "myField": "preserved as-is",
            "some_snake_field": 123
        });

        let context = SeriesContext::with_series_id(series_id)
            .book_count(5)
            .metadata(metadata)
            .external_id_full(
                "plugin:mangabaka",
                ExternalIdContext {
                    id: "12345".to_string(),
                    url: Some("https://mangabaka.com/series/12345".to_string()),
                    hash: Some("abc123".to_string()),
                },
            )
            .custom_metadata(custom);

        let json = serde_json::to_value(&context).unwrap();

        // Verify structure matches expected format
        assert_eq!(json["seriesId"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["bookCount"], 5);
        assert_eq!(json["metadata"]["title"], "One Piece");
        assert_eq!(
            json["metadata"]["genres"],
            serde_json::json!(["Action", "Adventure"])
        );
        assert_eq!(json["metadata"]["tags"], serde_json::json!(["pirates"]));
        assert_eq!(json["externalIds"]["plugin:mangabaka"]["id"], "12345");
        assert_eq!(
            json["externalIds"]["plugin:mangabaka"]["url"],
            "https://mangabaka.com/series/12345"
        );

        // customMetadata should be preserved as-is (no case transformation)
        assert_eq!(json["customMetadata"]["myField"], "preserved as-is");
        assert_eq!(json["customMetadata"]["some_snake_field"], 123);
    }

    // ==========================================================================
    // Field Access Tests (dual snake_case/camelCase support)
    // ==========================================================================

    #[test]
    fn test_get_field_camel_case_paths() {
        let metadata = MetadataContext {
            title_sort: Some("One Piece".to_string()),
            age_rating: Some(13),
            reading_direction: Some("rtl".to_string()),
            total_book_count: Some(100),
            title_sort_lock: true,
            ..Default::default()
        };
        let context = SeriesContext::new().book_count(5).metadata(metadata);

        // camelCase paths should work
        assert_eq!(
            context.get_field("bookCount"),
            Some(FieldValue::Number(5.0))
        );
        assert_eq!(
            context.get_field("metadata.titleSort"),
            Some(FieldValue::String("One Piece".to_string()))
        );
        assert_eq!(
            context.get_field("metadata.ageRating"),
            Some(FieldValue::Number(13.0))
        );
        assert_eq!(
            context.get_field("metadata.readingDirection"),
            Some(FieldValue::String("rtl".to_string()))
        );
        assert_eq!(
            context.get_field("metadata.totalBookCount"),
            Some(FieldValue::Number(100.0))
        );
        assert_eq!(
            context.get_field("metadata.titleSortLock"),
            Some(FieldValue::Bool(true))
        );
    }

    #[test]
    fn test_get_field_snake_case_paths_still_work() {
        // Ensure backwards compatibility with snake_case paths
        let metadata = MetadataContext {
            title_sort: Some("One Piece".to_string()),
            age_rating: Some(13),
            ..Default::default()
        };
        let context = SeriesContext::new().book_count(5).metadata(metadata);

        // snake_case paths should still work
        assert_eq!(
            context.get_field("book_count"),
            Some(FieldValue::Number(5.0))
        );
        assert_eq!(
            context.get_field("metadata.title_sort"),
            Some(FieldValue::String("One Piece".to_string()))
        );
        assert_eq!(
            context.get_field("metadata.age_rating"),
            Some(FieldValue::Number(13.0))
        );
    }

    #[test]
    fn test_get_field_custom_metadata_camel_case() {
        let custom = serde_json::json!({
            "myField": "value1",
            "my_field": "value2"
        });
        let context = SeriesContext::new().custom_metadata(custom);

        // customMetadata paths work with both prefixes
        assert_eq!(
            context.get_field("customMetadata.myField"),
            Some(FieldValue::String("value1".to_string()))
        );
        assert_eq!(
            context.get_field("custom_metadata.myField"),
            Some(FieldValue::String("value1".to_string()))
        );

        // The actual field name in custom metadata is used as-is
        assert_eq!(
            context.get_field("customMetadata.my_field"),
            Some(FieldValue::String("value2".to_string()))
        );
    }

    #[test]
    fn test_get_field_external_ids_camel_case() {
        let context = SeriesContext::new().external_id("plugin:mangabaka", "12345");

        // Both camelCase and snake_case prefixes should work
        assert_eq!(
            context.get_field("externalIds.plugin:mangabaka"),
            Some(FieldValue::String("12345".to_string()))
        );
        assert_eq!(
            context.get_field("external_ids.plugin:mangabaka"),
            Some(FieldValue::String("12345".to_string()))
        );
        assert_eq!(
            context.get_field("externalIds.count"),
            Some(FieldValue::Number(1.0))
        );
        assert_eq!(
            context.get_field("external_ids.count"),
            Some(FieldValue::Number(1.0))
        );
    }

    // ==========================================================================
    // Genres and Tags Field Access Tests
    // ==========================================================================

    #[test]
    fn test_get_field_genres() {
        let metadata = MetadataContext {
            genres: vec![
                "Action".to_string(),
                "Adventure".to_string(),
                "Comedy".to_string(),
            ],
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        let value = context.get_field("metadata.genres");
        assert!(matches!(value, Some(FieldValue::Array(_))));

        if let Some(FieldValue::Array(arr)) = value {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], serde_json::json!("Action"));
            assert_eq!(arr[1], serde_json::json!("Adventure"));
            assert_eq!(arr[2], serde_json::json!("Comedy"));
        }
    }

    #[test]
    fn test_get_field_tags() {
        let metadata = MetadataContext {
            tags: vec!["pirates".to_string(), "treasure".to_string()],
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        let value = context.get_field("metadata.tags");
        assert!(matches!(value, Some(FieldValue::Array(_))));

        if let Some(FieldValue::Array(arr)) = value {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], serde_json::json!("pirates"));
            assert_eq!(arr[1], serde_json::json!("treasure"));
        }
    }

    #[test]
    fn test_get_field_empty_genres_and_tags() {
        let context = SeriesContext::new(); // Default has empty genres/tags

        let genres = context.get_field("metadata.genres");
        let tags = context.get_field("metadata.tags");

        assert!(matches!(genres, Some(FieldValue::Array(ref arr)) if arr.is_empty()));
        assert!(matches!(tags, Some(FieldValue::Array(ref arr)) if arr.is_empty()));
    }

    #[test]
    fn test_genres_and_tags_builder_methods() {
        let context = SeriesContext::new()
            .genres(vec!["Action".to_string(), "Drama".to_string()])
            .tags(vec!["romance".to_string()]);

        assert_eq!(context.metadata.genres, vec!["Action", "Drama"]);
        assert_eq!(context.metadata.tags, vec!["romance"]);
    }

    // ==========================================================================
    // Type Discriminator Tests
    // ==========================================================================

    #[test]
    fn test_series_context_type_discriminator() {
        let context = SeriesContext::new();
        assert_eq!(context.context_type, "series");
        assert_eq!(
            context.get_field("type"),
            Some(FieldValue::String("series".to_string()))
        );
    }

    #[test]
    fn test_series_context_type_serializes_as_type() {
        let context = SeriesContext::new();
        let json = serde_json::to_value(&context).unwrap();
        assert_eq!(json["type"], "series");
        // Should NOT have "contextType"
        assert!(json.get("contextType").is_none());
    }

    // ==========================================================================
    // Alternate Titles Tests
    // ==========================================================================

    #[test]
    fn test_alternate_title_context_serialization() {
        let at = AlternateTitleContext {
            label: "Japanese".to_string(),
            title: "ワンピース".to_string(),
        };
        let json = serde_json::to_value(&at).unwrap();
        assert_eq!(json["label"], "Japanese");
        assert_eq!(json["title"], "ワンピース");
    }

    #[test]
    fn test_get_field_alternate_titles() {
        let metadata = MetadataContext {
            alternate_titles: vec![
                AlternateTitleContext {
                    label: "Japanese".to_string(),
                    title: "ワンピース".to_string(),
                },
                AlternateTitleContext {
                    label: "Romaji".to_string(),
                    title: "Wan Pīsu".to_string(),
                },
            ],
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        let value = context.get_field("metadata.alternateTitles");
        assert!(matches!(value, Some(FieldValue::Array(_))));
        if let Some(FieldValue::Array(arr)) = value {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0]["label"], "Japanese");
            assert_eq!(arr[0]["title"], "ワンピース");
            assert_eq!(arr[1]["label"], "Romaji");
        }

        // snake_case should also work
        let value2 = context.get_field("metadata.alternate_titles");
        assert!(matches!(value2, Some(FieldValue::Array(_))));
    }

    // ==========================================================================
    // Author Context Tests
    // ==========================================================================

    #[test]
    fn test_author_context_serialization() {
        let author = AuthorContext {
            name: "Oda Eiichiro".to_string(),
            role: Some("author".to_string()),
            sort_name: Some("Oda, Eiichiro".to_string()),
        };
        let json = serde_json::to_value(&author).unwrap();
        assert_eq!(json["name"], "Oda Eiichiro");
        assert_eq!(json["role"], "author");
        assert_eq!(json["sortName"], "Oda, Eiichiro");
    }

    #[test]
    fn test_author_context_optional_fields_skipped() {
        let author = AuthorContext {
            name: "Unknown".to_string(),
            role: None,
            sort_name: None,
        };
        let json = serde_json::to_value(&author).unwrap();
        assert_eq!(json["name"], "Unknown");
        assert!(json.get("role").is_none());
        assert!(json.get("sortName").is_none());
    }

    #[test]
    fn test_get_field_authors() {
        let metadata = MetadataContext {
            authors: vec![
                AuthorContext {
                    name: "Oda Eiichiro".to_string(),
                    role: Some("author".to_string()),
                    sort_name: None,
                },
                AuthorContext {
                    name: "Artist Name".to_string(),
                    role: Some("artist".to_string()),
                    sort_name: None,
                },
            ],
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        let value = context.get_field("metadata.authors");
        assert!(matches!(value, Some(FieldValue::Array(_))));
        if let Some(FieldValue::Array(arr)) = value {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0]["name"], "Oda Eiichiro");
            assert_eq!(arr[0]["role"], "author");
        }
    }

    // ==========================================================================
    // External Rating Context Tests
    // ==========================================================================

    #[test]
    fn test_external_rating_context_serialization() {
        let rating = ExternalRatingContext {
            source: "myanimelist".to_string(),
            rating: 85.5,
            votes: Some(12345),
        };
        let json = serde_json::to_value(&rating).unwrap();
        assert_eq!(json["source"], "myanimelist");
        assert_eq!(json["rating"], 85.5);
        assert_eq!(json["votes"], 12345);
    }

    #[test]
    fn test_external_rating_context_no_votes() {
        let rating = ExternalRatingContext {
            source: "anilist".to_string(),
            rating: 90.0,
            votes: None,
        };
        let json = serde_json::to_value(&rating).unwrap();
        assert_eq!(json["source"], "anilist");
        assert!(json.get("votes").is_none());
    }

    #[test]
    fn test_get_field_external_ratings() {
        let metadata = MetadataContext {
            external_ratings: vec![
                ExternalRatingContext {
                    source: "myanimelist".to_string(),
                    rating: 85.5,
                    votes: Some(12345),
                },
                ExternalRatingContext {
                    source: "anilist".to_string(),
                    rating: 90.0,
                    votes: None,
                },
            ],
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        let value = context.get_field("metadata.externalRatings");
        assert!(matches!(value, Some(FieldValue::Array(_))));
        if let Some(FieldValue::Array(arr)) = value {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0]["source"], "myanimelist");
            assert_eq!(arr[0]["rating"], 85.5);
            assert_eq!(arr[0]["votes"], 12345);
        }

        // snake_case should also work
        let value2 = context.get_field("metadata.external_ratings");
        assert!(matches!(value2, Some(FieldValue::Array(_))));
    }

    // ==========================================================================
    // External Link Context Tests
    // ==========================================================================

    #[test]
    fn test_external_link_context_serialization() {
        let link = ExternalLinkContext {
            source: "mangadex".to_string(),
            url: "https://mangadex.org/title/123".to_string(),
            external_id: Some("123".to_string()),
        };
        let json = serde_json::to_value(&link).unwrap();
        assert_eq!(json["source"], "mangadex");
        assert_eq!(json["url"], "https://mangadex.org/title/123");
        assert_eq!(json["externalId"], "123");
    }

    #[test]
    fn test_external_link_context_no_external_id() {
        let link = ExternalLinkContext {
            source: "wiki".to_string(),
            url: "https://en.wikipedia.org/wiki/One_Piece".to_string(),
            external_id: None,
        };
        let json = serde_json::to_value(&link).unwrap();
        assert!(json.get("externalId").is_none());
    }

    #[test]
    fn test_get_field_external_links() {
        let metadata = MetadataContext {
            external_links: vec![ExternalLinkContext {
                source: "mangadex".to_string(),
                url: "https://mangadex.org/title/123".to_string(),
                external_id: Some("123".to_string()),
            }],
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        let value = context.get_field("metadata.externalLinks");
        assert!(matches!(value, Some(FieldValue::Array(_))));
        if let Some(FieldValue::Array(arr)) = value {
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["source"], "mangadex");
        }

        // snake_case should also work
        let value2 = context.get_field("metadata.external_links");
        assert!(matches!(value2, Some(FieldValue::Array(_))));
    }

    // ==========================================================================
    // New Lock Fields Tests
    // ==========================================================================

    #[test]
    fn test_get_field_new_lock_fields() {
        let metadata = MetadataContext {
            cover_lock: true,
            authors_json_lock: true,
            ..Default::default()
        };
        let context = SeriesContext::new().metadata(metadata);

        assert_eq!(
            context.get_field("metadata.coverLock"),
            Some(FieldValue::Bool(true))
        );
        assert_eq!(
            context.get_field("metadata.cover_lock"),
            Some(FieldValue::Bool(true))
        );
        assert_eq!(
            context.get_field("metadata.authorsJsonLock"),
            Some(FieldValue::Bool(true))
        );
        assert_eq!(
            context.get_field("metadata.authors_json_lock"),
            Some(FieldValue::Bool(true))
        );
    }

    // ==========================================================================
    // Full Context with New Fields
    // ==========================================================================

    #[test]
    fn test_full_context_with_new_fields_json_structure() {
        let series_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let metadata = MetadataContext {
            title: Some("One Piece".to_string()),
            genres: vec!["Action".to_string()],
            alternate_titles: vec![AlternateTitleContext {
                label: "Japanese".to_string(),
                title: "ワンピース".to_string(),
            }],
            authors: vec![AuthorContext {
                name: "Oda".to_string(),
                role: Some("author".to_string()),
                sort_name: None,
            }],
            external_ratings: vec![ExternalRatingContext {
                source: "mal".to_string(),
                rating: 90.0,
                votes: Some(100000),
            }],
            external_links: vec![ExternalLinkContext {
                source: "mangadex".to_string(),
                url: "https://mangadex.org/title/123".to_string(),
                external_id: Some("123".to_string()),
            }],
            ..Default::default()
        };

        let context = SeriesContext::with_series_id(series_id)
            .book_count(100)
            .metadata(metadata);

        let json = serde_json::to_value(&context).unwrap();

        // Type discriminator
        assert_eq!(json["type"], "series");

        // New metadata arrays
        assert_eq!(json["metadata"]["alternateTitles"][0]["label"], "Japanese");
        assert_eq!(
            json["metadata"]["alternateTitles"][0]["title"],
            "ワンピース"
        );
        assert_eq!(json["metadata"]["authors"][0]["name"], "Oda");
        assert_eq!(json["metadata"]["authors"][0]["role"], "author");
        assert_eq!(json["metadata"]["externalRatings"][0]["source"], "mal");
        assert_eq!(json["metadata"]["externalRatings"][0]["rating"], 90.0);
        assert_eq!(json["metadata"]["externalLinks"][0]["source"], "mangadex");
    }

    #[test]
    fn test_metadata_context_new_fields_serialize_camel_case() {
        let metadata = MetadataContext {
            alternate_titles: vec![AlternateTitleContext {
                label: "JP".to_string(),
                title: "テスト".to_string(),
            }],
            authors: vec![AuthorContext {
                name: "Test".to_string(),
                role: None,
                sort_name: Some("Test, Author".to_string()),
            }],
            external_ratings: vec![ExternalRatingContext {
                source: "test".to_string(),
                rating: 50.0,
                votes: None,
            }],
            external_links: vec![ExternalLinkContext {
                source: "test".to_string(),
                url: "https://example.com".to_string(),
                external_id: None,
            }],
            cover_lock: true,
            authors_json_lock: true,
            ..Default::default()
        };

        let json = serde_json::to_value(&metadata).unwrap();

        // Verify camelCase serialization of new fields
        assert!(
            json.get("alternateTitles").is_some(),
            "alternateTitles should exist"
        );
        assert!(
            json.get("externalRatings").is_some(),
            "externalRatings should exist"
        );
        assert!(
            json.get("externalLinks").is_some(),
            "externalLinks should exist"
        );
        assert!(json.get("coverLock").is_some(), "coverLock should exist");
        assert!(
            json.get("authorsJsonLock").is_some(),
            "authorsJsonLock should exist"
        );

        // Should NOT have snake_case
        assert!(json.get("alternate_titles").is_none());
        assert!(json.get("external_ratings").is_none());
        assert!(json.get("external_links").is_none());
        assert!(json.get("cover_lock").is_none());
        assert!(json.get("authors_json_lock").is_none());

        // Author sortName should be camelCase
        assert_eq!(json["authors"][0]["sortName"], "Test, Author");
        assert!(json["authors"][0].get("sort_name").is_none());
    }

    // ==========================================================================
    // BookContext Tests
    // ==========================================================================

    fn create_test_book_context() -> BookContext {
        let series_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let book_id = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap();
        let library_id = Uuid::parse_str("770e8400-e29b-41d4-a716-446655440002").unwrap();

        let series_context = SeriesContext::with_series_id(series_id)
            .book_count(10)
            .metadata(MetadataContext {
                title: Some("One Piece".to_string()),
                ..Default::default()
            });

        BookContext {
            context_type: "book".to_string(),
            book_id,
            series_id,
            library_id,
            file_format: "cbz".to_string(),
            page_count: 200,
            file_size: 50_000_000,
            metadata: BookMetadataContext {
                title: Some("Chapter 1".to_string()),
                number: Some(1.0),
                summary: Some("The beginning".to_string()),
                year: Some(1997),
                authors: vec![AuthorContext {
                    name: "Oda Eiichiro".to_string(),
                    role: Some("author".to_string()),
                    sort_name: None,
                }],
                genres: vec!["Action".to_string()],
                tags: vec!["pirates".to_string()],
                subjects: vec!["Adventure".to_string()],
                awards: vec![BookAwardContext {
                    name: "Best Manga".to_string(),
                    year: Some(1998),
                    category: Some("Shonen".to_string()),
                    won: Some(true),
                }],
                ..Default::default()
            },
            external_ids: {
                let mut map = HashMap::new();
                map.insert(
                    "plugin:test".to_string(),
                    ExternalIdContext {
                        id: "book-123".to_string(),
                        url: None,
                        hash: None,
                    },
                );
                map
            },
            custom_metadata: Some(serde_json::json!({"readStatus": "completed"})),
            series: series_context,
        }
    }

    #[test]
    fn test_book_context_type_discriminator() {
        let ctx = create_test_book_context();
        assert_eq!(ctx.context_type, "book");
        assert_eq!(
            ctx.get_field("type"),
            Some(FieldValue::String("book".to_string()))
        );
    }

    #[test]
    fn test_book_context_serializes_with_type() {
        let ctx = create_test_book_context();
        let json = serde_json::to_value(&ctx).unwrap();

        assert_eq!(json["type"], "book");
        assert!(json.get("contextType").is_none());
        assert_eq!(json["fileFormat"], "cbz");
        assert_eq!(json["pageCount"], 200);
        assert_eq!(json["fileSize"], 50_000_000);
    }

    #[test]
    fn test_book_context_get_field_top_level() {
        let ctx = create_test_book_context();

        assert_eq!(
            ctx.get_field("fileFormat"),
            Some(FieldValue::String("cbz".to_string()))
        );
        assert_eq!(
            ctx.get_field("file_format"),
            Some(FieldValue::String("cbz".to_string()))
        );
        assert_eq!(ctx.get_field("pageCount"), Some(FieldValue::Number(200.0)));
        assert_eq!(
            ctx.get_field("fileSize"),
            Some(FieldValue::Number(50_000_000.0))
        );
    }

    #[test]
    fn test_book_context_get_field_metadata() {
        let ctx = create_test_book_context();

        assert_eq!(
            ctx.get_field("metadata.title"),
            Some(FieldValue::String("Chapter 1".to_string()))
        );
        assert_eq!(
            ctx.get_field("metadata.number"),
            Some(FieldValue::Number(1.0))
        );
        assert_eq!(
            ctx.get_field("metadata.year"),
            Some(FieldValue::Number(1997.0))
        );
        assert_eq!(
            ctx.get_field("metadata.summary"),
            Some(FieldValue::String("The beginning".to_string()))
        );
    }

    #[test]
    fn test_book_context_get_field_arrays() {
        let ctx = create_test_book_context();

        let authors = ctx.get_field("metadata.authors");
        assert!(matches!(authors, Some(FieldValue::Array(_))));
        if let Some(FieldValue::Array(arr)) = authors {
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["name"], "Oda Eiichiro");
        }

        let genres = ctx.get_field("metadata.genres");
        assert!(matches!(genres, Some(FieldValue::Array(ref arr)) if arr.len() == 1));

        let subjects = ctx.get_field("metadata.subjects");
        assert!(matches!(subjects, Some(FieldValue::Array(ref arr)) if arr.len() == 1));

        let awards = ctx.get_field("metadata.awards");
        assert!(matches!(awards, Some(FieldValue::Array(_))));
        if let Some(FieldValue::Array(arr)) = awards {
            assert_eq!(arr[0]["name"], "Best Manga");
            assert_eq!(arr[0]["year"], 1998);
        }
    }

    #[test]
    fn test_book_context_get_field_external_ids() {
        let ctx = create_test_book_context();

        assert_eq!(
            ctx.get_field("externalIds.plugin:test"),
            Some(FieldValue::String("book-123".to_string()))
        );
        assert_eq!(
            ctx.get_field("externalIds.count"),
            Some(FieldValue::Number(1.0))
        );
    }

    #[test]
    fn test_book_context_get_field_custom_metadata() {
        let ctx = create_test_book_context();

        assert_eq!(
            ctx.get_field("customMetadata.readStatus"),
            Some(FieldValue::String("completed".to_string()))
        );
        assert_eq!(
            ctx.get_field("custom_metadata.readStatus"),
            Some(FieldValue::String("completed".to_string()))
        );
    }

    #[test]
    fn test_book_context_get_field_series_crossref() {
        let ctx = create_test_book_context();

        // Access parent series context via series. prefix
        assert_eq!(
            ctx.get_field("series.metadata.title"),
            Some(FieldValue::String("One Piece".to_string()))
        );
        assert_eq!(
            ctx.get_field("series.bookCount"),
            Some(FieldValue::Number(10.0))
        );
        assert_eq!(
            ctx.get_field("series.type"),
            Some(FieldValue::String("series".to_string()))
        );
    }

    #[test]
    fn test_book_context_full_json_structure() {
        let ctx = create_test_book_context();
        let json = serde_json::to_value(&ctx).unwrap();

        // Type discriminator
        assert_eq!(json["type"], "book");

        // Top-level fields in camelCase
        assert!(json.get("bookId").is_some());
        assert!(json.get("seriesId").is_some());
        assert!(json.get("libraryId").is_some());
        assert_eq!(json["fileFormat"], "cbz");
        assert_eq!(json["pageCount"], 200);

        // Metadata
        assert_eq!(json["metadata"]["title"], "Chapter 1");
        assert_eq!(json["metadata"]["number"], 1.0);
        assert_eq!(json["metadata"]["genres"], serde_json::json!(["Action"]));
        assert_eq!(json["metadata"]["authors"][0]["name"], "Oda Eiichiro");

        // Nested series context
        assert_eq!(json["series"]["type"], "series");
        assert_eq!(json["series"]["metadata"]["title"], "One Piece");
        assert_eq!(json["series"]["bookCount"], 10);

        // Custom metadata preserved
        assert_eq!(json["customMetadata"]["readStatus"], "completed");
    }

    #[test]
    fn test_book_award_context_serialization() {
        let award = BookAwardContext {
            name: "Hugo Award".to_string(),
            year: Some(2020),
            category: Some("Best Novel".to_string()),
            won: Some(true),
        };
        let json = serde_json::to_value(&award).unwrap();
        assert_eq!(json["name"], "Hugo Award");
        assert_eq!(json["year"], 2020);
        assert_eq!(json["category"], "Best Novel");
        assert_eq!(json["won"], true);
    }

    #[test]
    fn test_book_award_context_optional_fields_skipped() {
        let award = BookAwardContext {
            name: "Award".to_string(),
            year: None,
            category: None,
            won: None,
        };
        let json = serde_json::to_value(&award).unwrap();
        assert_eq!(json["name"], "Award");
        assert!(json.get("year").is_none());
        assert!(json.get("category").is_none());
        assert!(json.get("won").is_none());
    }

    #[test]
    fn test_book_metadata_context_camel_case_serialization() {
        let meta = BookMetadataContext {
            title: Some("Test Book".to_string()),
            language_iso: Some("en".to_string()),
            format_detail: Some("Digital".to_string()),
            black_and_white: Some(false),
            book_type: Some("comic".to_string()),
            original_title: Some("Original".to_string()),
            original_year: Some(2000),
            series_position: Some(1.5),
            series_total: Some(10),
            ..Default::default()
        };

        let json = serde_json::to_value(&meta).unwrap();

        // camelCase verification
        assert!(json.get("languageIso").is_some());
        assert!(json.get("formatDetail").is_some());
        assert!(json.get("blackAndWhite").is_some());
        assert!(json.get("bookType").is_some());
        assert!(json.get("originalTitle").is_some());
        assert!(json.get("originalYear").is_some());
        assert!(json.get("seriesPosition").is_some());
        assert!(json.get("seriesTotal").is_some());

        // Should NOT have snake_case
        assert!(json.get("language_iso").is_none());
        assert!(json.get("format_detail").is_none());
        assert!(json.get("black_and_white").is_none());
        assert!(json.get("book_type").is_none());
        assert!(json.get("original_title").is_none());
    }
}
