//! Series export data collector
//!
//! Collects series data for export by batching queries across multiple
//! repositories. Only queries data for fields the user selected.
//! Enforces content access control via `ContentFilter`.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use crate::api::extractors::content_filter::ContentFilter;
use crate::db::entities::series;
use crate::db::repositories::{
    AlternateTitleRepository, BookRepository, ExternalRatingRepository, GenreRepository,
    LibraryRepository, SeriesMetadataRepository, SeriesRepository, TagRepository,
    UserSeriesRatingRepository,
};

// =============================================================================
// ExportField enum
// =============================================================================

/// All available fields for series export.
/// The string representation is stable and used in API requests, DB storage,
/// and as CSV/JSON keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportField {
    // Always included (anchor fields)
    SeriesId,
    SeriesName,
    LibraryId,
    // Optional identity / metadata
    LibraryName,
    Path,
    CreatedAt,
    UpdatedAt,
    Title,
    Summary,
    Publisher,
    Status,
    Year,
    Language,
    Authors,
    Genres,
    Tags,
    AlternateTitles,
    // Counts
    ExpectedBookCount,
    ActualBookCount,
    UnreadBookCount,
    // Ratings
    UserRating,
    UserNotes,
    CommunityAvgRating,
    ExternalRatings,
}

impl ExportField {
    /// All fields in display order.
    pub const ALL: &'static [ExportField] = &[
        ExportField::SeriesId,
        ExportField::SeriesName,
        ExportField::LibraryId,
        ExportField::LibraryName,
        ExportField::Path,
        ExportField::CreatedAt,
        ExportField::UpdatedAt,
        ExportField::Title,
        ExportField::Summary,
        ExportField::Publisher,
        ExportField::Status,
        ExportField::Year,
        ExportField::Language,
        ExportField::Authors,
        ExportField::Genres,
        ExportField::Tags,
        ExportField::AlternateTitles,
        ExportField::ExpectedBookCount,
        ExportField::ActualBookCount,
        ExportField::UnreadBookCount,
        ExportField::UserRating,
        ExportField::UserNotes,
        ExportField::CommunityAvgRating,
        ExportField::ExternalRatings,
    ];

    /// Anchor fields that are always included regardless of user selection.
    pub const ANCHORS: &'static [ExportField] = &[
        ExportField::SeriesId,
        ExportField::SeriesName,
        ExportField::LibraryId,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ExportField::SeriesId => "series_id",
            ExportField::SeriesName => "series_name",
            ExportField::LibraryId => "library_id",
            ExportField::LibraryName => "library_name",
            ExportField::Path => "path",
            ExportField::CreatedAt => "created_at",
            ExportField::UpdatedAt => "updated_at",
            ExportField::Title => "title",
            ExportField::Summary => "summary",
            ExportField::Publisher => "publisher",
            ExportField::Status => "status",
            ExportField::Year => "year",
            ExportField::Language => "language",
            ExportField::Authors => "authors",
            ExportField::Genres => "genres",
            ExportField::Tags => "tags",
            ExportField::AlternateTitles => "alternate_titles",
            ExportField::ExpectedBookCount => "expected_book_count",
            ExportField::ActualBookCount => "actual_book_count",
            ExportField::UnreadBookCount => "unread_book_count",
            ExportField::UserRating => "user_rating",
            ExportField::UserNotes => "user_notes",
            ExportField::CommunityAvgRating => "community_avg_rating",
            ExportField::ExternalRatings => "external_ratings",
        }
    }

    pub fn parse(s: &str) -> Option<ExportField> {
        match s {
            "series_id" => Some(ExportField::SeriesId),
            "series_name" => Some(ExportField::SeriesName),
            "library_id" => Some(ExportField::LibraryId),
            "library_name" => Some(ExportField::LibraryName),
            "path" => Some(ExportField::Path),
            "created_at" => Some(ExportField::CreatedAt),
            "updated_at" => Some(ExportField::UpdatedAt),
            "title" => Some(ExportField::Title),
            "summary" => Some(ExportField::Summary),
            "publisher" => Some(ExportField::Publisher),
            "status" => Some(ExportField::Status),
            "year" => Some(ExportField::Year),
            "language" => Some(ExportField::Language),
            "authors" => Some(ExportField::Authors),
            "genres" => Some(ExportField::Genres),
            "tags" => Some(ExportField::Tags),
            "alternate_titles" => Some(ExportField::AlternateTitles),
            "expected_book_count" => Some(ExportField::ExpectedBookCount),
            "actual_book_count" => Some(ExportField::ActualBookCount),
            "unread_book_count" => Some(ExportField::UnreadBookCount),
            "user_rating" => Some(ExportField::UserRating),
            "user_notes" => Some(ExportField::UserNotes),
            "community_avg_rating" => Some(ExportField::CommunityAvgRating),
            "external_ratings" => Some(ExportField::ExternalRatings),
            _ => None,
        }
    }

    /// Whether this field is user-specific (changes per user).
    pub fn is_user_specific(&self) -> bool {
        matches!(
            self,
            ExportField::UserRating | ExportField::UserNotes | ExportField::UnreadBookCount
        )
    }

    /// Whether this field contains multiple values (joined with `;` in CSV).
    pub fn is_multi_value(&self) -> bool {
        matches!(
            self,
            ExportField::Authors
                | ExportField::Genres
                | ExportField::Tags
                | ExportField::AlternateTitles
                | ExportField::ExternalRatings
        )
    }
}

impl fmt::Display for ExportField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// SeriesExportRow
// =============================================================================

/// A single row of exported series data.
/// Uses Option for all non-anchor fields so unselected fields are null/absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesExportRow {
    // Anchors (always present)
    pub series_id: String,
    pub series_name: String,
    pub library_id: String,
    // Optional fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genres: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternate_titles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_book_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_book_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread_book_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_rating: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_avg_rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ratings: Option<String>,
}

impl SeriesExportRow {
    fn from_series(s: &series::Model) -> Self {
        Self {
            series_id: s.id.to_string(),
            series_name: s.name.clone(),
            library_id: s.library_id.to_string(),
            library_name: None,
            path: None,
            created_at: None,
            updated_at: None,
            title: None,
            summary: None,
            publisher: None,
            status: None,
            year: None,
            language: None,
            authors: None,
            genres: None,
            tags: None,
            alternate_titles: None,
            expected_book_count: None,
            actual_book_count: None,
            unread_book_count: None,
            user_rating: None,
            user_notes: None,
            community_avg_rating: None,
            external_ratings: None,
        }
    }

    /// Get the string value for a given export field.
    /// Used by the CSV writer to emit columns in a consistent order.
    pub fn get_field_value(&self, field: &ExportField) -> String {
        match field {
            ExportField::SeriesId => self.series_id.clone(),
            ExportField::SeriesName => self.series_name.clone(),
            ExportField::LibraryId => self.library_id.clone(),
            ExportField::LibraryName => self.library_name.clone().unwrap_or_default(),
            ExportField::Path => self.path.clone().unwrap_or_default(),
            ExportField::CreatedAt => self.created_at.clone().unwrap_or_default(),
            ExportField::UpdatedAt => self.updated_at.clone().unwrap_or_default(),
            ExportField::Title => self.title.clone().unwrap_or_default(),
            ExportField::Summary => self.summary.clone().unwrap_or_default(),
            ExportField::Publisher => self.publisher.clone().unwrap_or_default(),
            ExportField::Status => self.status.clone().unwrap_or_default(),
            ExportField::Year => self.year.map(|y| y.to_string()).unwrap_or_default(),
            ExportField::Language => self.language.clone().unwrap_or_default(),
            ExportField::Authors => self.authors.clone().unwrap_or_default(),
            ExportField::Genres => self.genres.clone().unwrap_or_default(),
            ExportField::Tags => self.tags.clone().unwrap_or_default(),
            ExportField::AlternateTitles => self.alternate_titles.clone().unwrap_or_default(),
            ExportField::ExpectedBookCount => self
                .expected_book_count
                .map(|c| c.to_string())
                .unwrap_or_default(),
            ExportField::ActualBookCount => self
                .actual_book_count
                .map(|c| c.to_string())
                .unwrap_or_default(),
            ExportField::UnreadBookCount => self
                .unread_book_count
                .map(|c| c.to_string())
                .unwrap_or_default(),
            ExportField::UserRating => self.user_rating.map(|r| r.to_string()).unwrap_or_default(),
            ExportField::UserNotes => self.user_notes.clone().unwrap_or_default(),
            ExportField::CommunityAvgRating => self
                .community_avg_rating
                .map(|r| format!("{r:.2}"))
                .unwrap_or_default(),
            ExportField::ExternalRatings => self.external_ratings.clone().unwrap_or_default(),
        }
    }
}

// =============================================================================
// Helpers for formatting multi-value fields
// =============================================================================

/// Format authors_json string into "name (role); name (role); ..." format.
fn format_authors(authors_json: &Option<String>) -> Option<String> {
    let json_str = authors_json.as_deref()?;
    let authors: Vec<serde_json::Value> = serde_json::from_str(json_str).ok()?;
    if authors.is_empty() {
        return None;
    }
    let parts: Vec<String> = authors
        .iter()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?;
            let role = a.get("role").and_then(|r| r.as_str());
            match role {
                Some(r) if !r.is_empty() => Some(format!("{name} ({r})")),
                _ => Some(name.to_string()),
            }
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

// =============================================================================
// Collector
// =============================================================================

/// Default batch size for chunked queries.
const DEFAULT_BATCH_SIZE: usize = 200;

/// Resolve which series IDs the user can see in the given libraries.
pub async fn resolve_series_ids(
    db: &DatabaseConnection,
    user_id: Uuid,
    library_ids: &[Uuid],
) -> Result<Vec<Uuid>> {
    let content_filter = ContentFilter::for_user(db, user_id).await?;

    let mut all_ids = Vec::new();
    for &lib_id in library_ids {
        let series_list = SeriesRepository::list_by_library(db, lib_id).await?;
        let ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();
        all_ids.extend(ids);
    }

    // Apply content filter (sharing-tag based access control)
    let visible_ids = content_filter.filter_series_ids(all_ids);
    Ok(visible_ids)
}

/// Collect series data in batches and call `on_row` for each row.
/// Returns the total number of rows emitted.
///
/// Only queries data for the fields in `fields`. Anchor fields are always
/// populated regardless of the `fields` set.
pub async fn collect_batched(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_ids: &[Uuid],
    fields: &[ExportField],
    mut on_row: impl FnMut(SeriesExportRow),
) -> Result<usize> {
    if series_ids.is_empty() {
        return Ok(0);
    }

    let field_set: std::collections::HashSet<ExportField> = fields.iter().copied().collect();
    let has = |f: ExportField| field_set.contains(&f);

    // Pre-load library names if needed (small number of libraries)
    let library_names: HashMap<Uuid, String> = if has(ExportField::LibraryName) {
        let all_libs = LibraryRepository::list_all(db).await?;
        all_libs.into_iter().map(|l| (l.id, l.name)).collect()
    } else {
        HashMap::new()
    };

    let mut total_rows = 0;

    for chunk in series_ids.chunks(DEFAULT_BATCH_SIZE) {
        // Load series models for this chunk
        let series_map = load_series_chunk(db, chunk).await?;

        // Load metadata if any metadata fields are selected
        let needs_metadata = has(ExportField::Title)
            || has(ExportField::Summary)
            || has(ExportField::Publisher)
            || has(ExportField::Status)
            || has(ExportField::Year)
            || has(ExportField::Language)
            || has(ExportField::Authors)
            || has(ExportField::ExpectedBookCount);

        let metadata_map = if needs_metadata {
            SeriesMetadataRepository::get_by_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        // Load optional join-table data only when selected
        let genres_map = if has(ExportField::Genres) {
            GenreRepository::get_genres_for_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        let tags_map = if has(ExportField::Tags) {
            TagRepository::get_tags_for_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        let alt_titles_map = if has(ExportField::AlternateTitles) {
            AlternateTitleRepository::get_for_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        let book_counts = if has(ExportField::ActualBookCount) {
            SeriesRepository::get_book_counts_for_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        let unread_counts = if has(ExportField::UnreadBookCount) {
            BookRepository::count_unread_in_series_ids(db, chunk, user_id).await?
        } else {
            HashMap::new()
        };

        let user_ratings = if has(ExportField::UserRating) || has(ExportField::UserNotes) {
            UserSeriesRatingRepository::get_for_user_and_series_ids(db, user_id, chunk).await?
        } else {
            HashMap::new()
        };

        let community_avgs = if has(ExportField::CommunityAvgRating) {
            UserSeriesRatingRepository::calculate_averages_for_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        let external_ratings_map = if has(ExportField::ExternalRatings) {
            ExternalRatingRepository::get_for_series_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        // Assemble rows
        for &sid in chunk {
            let Some(s) = series_map.get(&sid) else {
                continue;
            };
            let mut row = SeriesExportRow::from_series(s);

            // Library name
            if has(ExportField::LibraryName) {
                row.library_name = library_names.get(&s.library_id).cloned();
            }

            // Series-level fields
            if has(ExportField::Path) {
                row.path = Some(s.path.clone());
            }
            if has(ExportField::CreatedAt) {
                row.created_at = Some(s.created_at.to_rfc3339());
            }
            if has(ExportField::UpdatedAt) {
                row.updated_at = Some(s.updated_at.to_rfc3339());
            }

            // Metadata fields
            if let Some(meta) = metadata_map.get(&sid) {
                if has(ExportField::Title) {
                    row.title = Some(meta.title.clone());
                }
                if has(ExportField::Summary) {
                    row.summary = meta.summary.clone();
                }
                if has(ExportField::Publisher) {
                    row.publisher = meta.publisher.clone();
                }
                if has(ExportField::Status) {
                    row.status = meta.status.clone();
                }
                if has(ExportField::Year) {
                    row.year = meta.year;
                }
                if has(ExportField::Language) {
                    row.language = meta.language.clone();
                }
                if has(ExportField::Authors) {
                    row.authors = format_authors(&meta.authors_json);
                }
                if has(ExportField::ExpectedBookCount) {
                    row.expected_book_count = meta.total_book_count;
                }
            }

            // Multi-value join fields
            if has(ExportField::Genres)
                && let Some(genres) = genres_map.get(&sid)
            {
                let names: Vec<&str> = genres.iter().map(|g| g.name.as_str()).collect();
                if !names.is_empty() {
                    row.genres = Some(names.join("; "));
                }
            }
            if has(ExportField::Tags)
                && let Some(tags) = tags_map.get(&sid)
            {
                let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
                if !names.is_empty() {
                    row.tags = Some(names.join("; "));
                }
            }
            if has(ExportField::AlternateTitles)
                && let Some(alts) = alt_titles_map.get(&sid)
            {
                let titles: Vec<String> = alts
                    .iter()
                    .map(|a| {
                        if a.label.is_empty() {
                            a.title.clone()
                        } else {
                            format!("{}: {}", a.label, a.title)
                        }
                    })
                    .collect();
                if !titles.is_empty() {
                    row.alternate_titles = Some(titles.join("; "));
                }
            }

            // Counts
            if has(ExportField::ActualBookCount) {
                row.actual_book_count = book_counts.get(&sid).copied();
            }
            if has(ExportField::UnreadBookCount) {
                row.unread_book_count = unread_counts.get(&sid).copied();
            }

            // User-specific ratings
            if let Some(rating) = user_ratings.get(&sid) {
                if has(ExportField::UserRating) {
                    row.user_rating = Some(rating.rating);
                }
                if has(ExportField::UserNotes) {
                    row.user_notes = rating.notes.clone();
                }
            }

            // Community average
            if has(ExportField::CommunityAvgRating) {
                row.community_avg_rating = community_avgs.get(&sid).copied();
            }

            // External ratings
            if has(ExportField::ExternalRatings)
                && let Some(ext_ratings) = external_ratings_map.get(&sid)
            {
                let parts: Vec<String> = ext_ratings
                    .iter()
                    .map(|r| {
                        let votes = r
                            .vote_count
                            .map(|v| format!(" ({v} votes)"))
                            .unwrap_or_default();
                        format!("{}={}{}", r.source_name, r.rating, votes)
                    })
                    .collect();
                if !parts.is_empty() {
                    row.external_ratings = Some(parts.join("; "));
                }
            }

            on_row(row);
            total_rows += 1;
        }
    }

    Ok(total_rows)
}

/// Load series models for a chunk of IDs into a HashMap.
async fn load_series_chunk(
    db: &DatabaseConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, series::Model>> {
    use crate::db::entities::series::Entity as Series;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let results = Series::find()
        .filter(crate::db::entities::series::Column::Id.is_in(ids.to_vec()))
        .all(db)
        .await?;

    Ok(results.into_iter().map(|s| (s.id, s)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_field_roundtrip() {
        for field in ExportField::ALL {
            let s = field.as_str();
            let parsed = ExportField::parse(s);
            assert_eq!(parsed, Some(*field), "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_export_field_parse_unknown() {
        assert_eq!(ExportField::parse("nonexistent"), None);
    }

    #[test]
    fn test_anchors_are_subset_of_all() {
        for anchor in ExportField::ANCHORS {
            assert!(
                ExportField::ALL.contains(anchor),
                "Anchor {:?} not in ALL",
                anchor
            );
        }
    }

    #[test]
    fn test_format_authors_none() {
        assert_eq!(format_authors(&None), None);
    }

    #[test]
    fn test_format_authors_empty() {
        assert_eq!(format_authors(&Some("[]".to_string())), None);
    }

    #[test]
    fn test_format_authors_with_roles() {
        let json = r#"[{"name":"John Doe","role":"author"},{"name":"Jane Smith","role":"editor"}]"#;
        let result = format_authors(&Some(json.to_string()));
        assert_eq!(
            result,
            Some("John Doe (author); Jane Smith (editor)".to_string())
        );
    }

    #[test]
    fn test_format_authors_without_roles() {
        let json = r#"[{"name":"John Doe"},{"name":"Jane Smith","role":""}]"#;
        let result = format_authors(&Some(json.to_string()));
        assert_eq!(result, Some("John Doe; Jane Smith".to_string()));
    }

    #[test]
    fn test_user_specific_fields() {
        assert!(ExportField::UserRating.is_user_specific());
        assert!(ExportField::UserNotes.is_user_specific());
        assert!(ExportField::UnreadBookCount.is_user_specific());
        assert!(!ExportField::Title.is_user_specific());
        assert!(!ExportField::CommunityAvgRating.is_user_specific());
    }

    #[test]
    fn test_multi_value_fields() {
        assert!(ExportField::Authors.is_multi_value());
        assert!(ExportField::Genres.is_multi_value());
        assert!(ExportField::Tags.is_multi_value());
        assert!(ExportField::AlternateTitles.is_multi_value());
        assert!(ExportField::ExternalRatings.is_multi_value());
        assert!(!ExportField::Title.is_multi_value());
        assert!(!ExportField::UserRating.is_multi_value());
    }
}
