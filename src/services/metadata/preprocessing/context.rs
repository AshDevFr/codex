//! Series context for condition evaluation.
//!
//! This module provides a `SeriesContext` structure that aggregates data
//! from various sources (series, metadata, external IDs, book count) to
//! provide a unified interface for condition evaluation.
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesContext {
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
            // Array fields (genres and tags)
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
    BookRepository, GenreRepository, SeriesExternalIdRepository, SeriesMetadataRepository,
    TagRepository,
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
    /// - Series metadata
    /// - Book count
    /// - Genres
    /// - Tags
    /// - External IDs
    /// - Custom metadata (from series_metadata.custom_metadata JSON field)
    pub async fn build(&self, db: &DatabaseConnection) -> Result<SeriesContext> {
        // Fetch all required data concurrently
        let (metadata_opt, book_count, genres, tags, external_ids) = tokio::try_join!(
            SeriesMetadataRepository::get_by_series_id(db, self.series_id),
            BookRepository::count_by_series(db, self.series_id),
            GenreRepository::get_genres_for_series(db, self.series_id),
            TagRepository::get_tags_for_series(db, self.series_id),
            SeriesExternalIdRepository::get_for_series(db, self.series_id),
        )?;

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
            }
        } else {
            // No metadata exists - still include genres/tags if they exist
            MetadataContext {
                genres: genres.iter().map(|g| g.name.clone()).collect(),
                tags: tags.iter().map(|t| t.name.clone()).collect(),
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
}
