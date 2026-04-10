//! Book export data collector
//!
//! Collects book data for export by batching queries across multiple
//! repositories. Only queries data for fields the user selected.
//! Enforces content access control via `ContentFilter`.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use crate::api::extractors::content_filter::ContentFilter;
use crate::db::entities::{book_metadata, books, read_progress};
use crate::db::repositories::{
    GenreRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, TagRepository,
};

// =============================================================================
// BookExportField enum
// =============================================================================

/// All available fields for book export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookExportField {
    // Always included (anchor field)
    BookName,
    // Optional identity
    BookId,
    SeriesId,
    LibraryId,
    SeriesName,
    LibraryName,
    // File info
    FileName,
    FilePath,
    FileSize,
    BookFormat,
    PageCount,
    // Series position
    Number,
    // Timestamps
    CreatedAt,
    UpdatedAt,
    // Metadata
    Title,
    Summary,
    Publisher,
    Year,
    Language,
    Authors,
    Genres,
    Tags,
    // Progress (user-specific)
    Progress,
    CurrentPage,
    Completed,
    CompletedAt,
}

impl BookExportField {
    /// All fields in display order.
    pub const ALL: &'static [BookExportField] = &[
        BookExportField::BookName,
        BookExportField::BookId,
        BookExportField::SeriesId,
        BookExportField::LibraryId,
        BookExportField::SeriesName,
        BookExportField::LibraryName,
        BookExportField::FileName,
        BookExportField::FilePath,
        BookExportField::FileSize,
        BookExportField::BookFormat,
        BookExportField::PageCount,
        BookExportField::Number,
        BookExportField::CreatedAt,
        BookExportField::UpdatedAt,
        BookExportField::Title,
        BookExportField::Summary,
        BookExportField::Publisher,
        BookExportField::Year,
        BookExportField::Language,
        BookExportField::Authors,
        BookExportField::Genres,
        BookExportField::Tags,
        BookExportField::Progress,
        BookExportField::CurrentPage,
        BookExportField::Completed,
        BookExportField::CompletedAt,
    ];

    /// Anchor fields that are always included regardless of user selection.
    pub const ANCHORS: &'static [BookExportField] = &[BookExportField::BookName];

    /// LLM-friendly field preset for quick selection.
    pub const LLM_SELECT: &'static [BookExportField] = &[
        BookExportField::Title,
        BookExportField::Summary,
        BookExportField::Year,
        BookExportField::Authors,
        BookExportField::Genres,
        BookExportField::SeriesName,
        BookExportField::Number,
        BookExportField::Progress,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            BookExportField::BookName => "book_name",
            BookExportField::BookId => "book_id",
            BookExportField::SeriesId => "series_id",
            BookExportField::LibraryId => "library_id",
            BookExportField::SeriesName => "series_name",
            BookExportField::LibraryName => "library_name",
            BookExportField::FileName => "file_name",
            BookExportField::FilePath => "file_path",
            BookExportField::FileSize => "file_size",
            BookExportField::BookFormat => "book_format",
            BookExportField::PageCount => "page_count",
            BookExportField::Number => "number",
            BookExportField::CreatedAt => "created_at",
            BookExportField::UpdatedAt => "updated_at",
            BookExportField::Title => "title",
            BookExportField::Summary => "summary",
            BookExportField::Publisher => "publisher",
            BookExportField::Year => "year",
            BookExportField::Language => "language",
            BookExportField::Authors => "authors",
            BookExportField::Genres => "genres",
            BookExportField::Tags => "tags",
            BookExportField::Progress => "progress",
            BookExportField::CurrentPage => "current_page",
            BookExportField::Completed => "completed",
            BookExportField::CompletedAt => "completed_at",
        }
    }

    pub fn parse(s: &str) -> Option<BookExportField> {
        match s {
            "book_name" => Some(BookExportField::BookName),
            "book_id" => Some(BookExportField::BookId),
            "series_id" => Some(BookExportField::SeriesId),
            "library_id" => Some(BookExportField::LibraryId),
            "series_name" => Some(BookExportField::SeriesName),
            "library_name" => Some(BookExportField::LibraryName),
            "file_name" => Some(BookExportField::FileName),
            "file_path" => Some(BookExportField::FilePath),
            "file_size" => Some(BookExportField::FileSize),
            "book_format" => Some(BookExportField::BookFormat),
            "page_count" => Some(BookExportField::PageCount),
            "number" => Some(BookExportField::Number),
            "created_at" => Some(BookExportField::CreatedAt),
            "updated_at" => Some(BookExportField::UpdatedAt),
            "title" => Some(BookExportField::Title),
            "summary" => Some(BookExportField::Summary),
            "publisher" => Some(BookExportField::Publisher),
            "year" => Some(BookExportField::Year),
            "language" => Some(BookExportField::Language),
            "authors" => Some(BookExportField::Authors),
            "genres" => Some(BookExportField::Genres),
            "tags" => Some(BookExportField::Tags),
            "progress" => Some(BookExportField::Progress),
            "current_page" => Some(BookExportField::CurrentPage),
            "completed" => Some(BookExportField::Completed),
            "completed_at" => Some(BookExportField::CompletedAt),
            _ => None,
        }
    }

    /// Human-readable label for display.
    pub fn label(&self) -> &'static str {
        match self {
            BookExportField::BookName => "Book Name",
            BookExportField::BookId => "Book ID",
            BookExportField::SeriesId => "Series ID",
            BookExportField::LibraryId => "Library ID",
            BookExportField::SeriesName => "Series Name",
            BookExportField::LibraryName => "Library Name",
            BookExportField::FileName => "File Name",
            BookExportField::FilePath => "File Path",
            BookExportField::FileSize => "File Size",
            BookExportField::BookFormat => "Format",
            BookExportField::PageCount => "Page Count",
            BookExportField::Number => "Number",
            BookExportField::CreatedAt => "Created At",
            BookExportField::UpdatedAt => "Updated At",
            BookExportField::Title => "Title",
            BookExportField::Summary => "Summary",
            BookExportField::Publisher => "Publisher",
            BookExportField::Year => "Year",
            BookExportField::Language => "Language",
            BookExportField::Authors => "Authors",
            BookExportField::Genres => "Genres",
            BookExportField::Tags => "Tags",
            BookExportField::Progress => "Progress",
            BookExportField::CurrentPage => "Current Page",
            BookExportField::Completed => "Completed",
            BookExportField::CompletedAt => "Completed At",
        }
    }

    /// Whether this field is an anchor (always included).
    pub fn is_anchor(&self) -> bool {
        BookExportField::ANCHORS.contains(self)
    }

    /// Whether this field is user-specific.
    pub fn is_user_specific(&self) -> bool {
        matches!(
            self,
            BookExportField::Progress
                | BookExportField::CurrentPage
                | BookExportField::Completed
                | BookExportField::CompletedAt
        )
    }

    /// Whether this field contains multiple values.
    pub fn is_multi_value(&self) -> bool {
        matches!(
            self,
            BookExportField::Authors | BookExportField::Genres | BookExportField::Tags
        )
    }
}

impl fmt::Display for BookExportField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// BookExportRow
// =============================================================================

/// A single row of exported book data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookExportRow {
    // Anchor (always present)
    pub book_name: String,
    // Optional identity fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_name: Option<String>,
    // File info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,
    // Series position
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    // Timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    // Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
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
    // Progress
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

impl BookExportRow {
    fn from_book(b: &books::Model, meta: Option<&book_metadata::Model>) -> Self {
        // Book name: prefer metadata title, fall back to file_name
        let name = meta
            .and_then(|m| m.title.clone())
            .unwrap_or_else(|| b.file_name.clone());

        Self {
            book_name: name,
            book_id: None,
            series_id: None,
            library_id: None,
            series_name: None,
            library_name: None,
            file_name: None,
            file_path: None,
            file_size: None,
            book_format: None,
            page_count: None,
            number: None,
            created_at: None,
            updated_at: None,
            title: None,
            summary: None,
            publisher: None,
            year: None,
            language: None,
            authors: None,
            genres: None,
            tags: None,
            progress: None,
            current_page: None,
            completed: None,
            completed_at: None,
        }
    }

    /// Get the anchor name for this row (used by markdown writer heading).
    pub fn name(&self) -> &str {
        &self.book_name
    }

    /// Get the string value for a given book export field.
    pub fn get_field_value(&self, field: &BookExportField) -> String {
        match field {
            BookExportField::BookName => self.book_name.clone(),
            BookExportField::BookId => self.book_id.clone().unwrap_or_default(),
            BookExportField::SeriesId => self.series_id.clone().unwrap_or_default(),
            BookExportField::LibraryId => self.library_id.clone().unwrap_or_default(),
            BookExportField::SeriesName => self.series_name.clone().unwrap_or_default(),
            BookExportField::LibraryName => self.library_name.clone().unwrap_or_default(),
            BookExportField::FileName => self.file_name.clone().unwrap_or_default(),
            BookExportField::FilePath => self.file_path.clone().unwrap_or_default(),
            BookExportField::FileSize => self.file_size.map(|s| s.to_string()).unwrap_or_default(),
            BookExportField::BookFormat => self.book_format.clone().unwrap_or_default(),
            BookExportField::PageCount => {
                self.page_count.map(|c| c.to_string()).unwrap_or_default()
            }
            BookExportField::Number => self.number.clone().unwrap_or_default(),
            BookExportField::CreatedAt => self.created_at.clone().unwrap_or_default(),
            BookExportField::UpdatedAt => self.updated_at.clone().unwrap_or_default(),
            BookExportField::Title => self.title.clone().unwrap_or_default(),
            BookExportField::Summary => self.summary.clone().unwrap_or_default(),
            BookExportField::Publisher => self.publisher.clone().unwrap_or_default(),
            BookExportField::Year => self.year.map(|y| y.to_string()).unwrap_or_default(),
            BookExportField::Language => self.language.clone().unwrap_or_default(),
            BookExportField::Authors => self.authors.clone().unwrap_or_default(),
            BookExportField::Genres => self.genres.clone().unwrap_or_default(),
            BookExportField::Tags => self.tags.clone().unwrap_or_default(),
            BookExportField::Progress => {
                self.progress.map(|p| format!("{p:.1}")).unwrap_or_default()
            }
            BookExportField::CurrentPage => {
                self.current_page.map(|p| p.to_string()).unwrap_or_default()
            }
            BookExportField::Completed => self
                .completed
                .map(|c| if c { "true" } else { "false" }.to_string())
                .unwrap_or_default(),
            BookExportField::CompletedAt => self.completed_at.clone().unwrap_or_default(),
        }
    }
}

// =============================================================================
// Helpers
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

const DEFAULT_BATCH_SIZE: usize = 200;

/// Resolve which book IDs the user can see in the given libraries.
pub async fn resolve_book_ids(
    db: &DatabaseConnection,
    user_id: Uuid,
    library_ids: &[Uuid],
) -> Result<Vec<Uuid>> {
    use crate::db::entities::books::Entity as Books;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let content_filter = ContentFilter::for_user(db, user_id).await?;

    let mut all_book_ids = Vec::new();
    for &lib_id in library_ids {
        let books = Books::find()
            .filter(books::Column::LibraryId.eq(lib_id))
            .filter(books::Column::Deleted.eq(false))
            .all(db)
            .await?;

        // Get series IDs for content filter
        let series_ids: Vec<Uuid> = books.iter().map(|b| b.series_id).collect();
        let visible_series = content_filter.filter_series_ids(series_ids);

        for book in &books {
            if visible_series.contains(&book.series_id) {
                all_book_ids.push(book.id);
            }
        }
    }

    Ok(all_book_ids)
}

/// Collect book data in batches and call `on_row` for each row.
pub async fn collect_batched(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_ids: &[Uuid],
    fields: &[BookExportField],
    mut on_row: impl FnMut(BookExportRow),
) -> Result<usize> {
    if book_ids.is_empty() {
        return Ok(0);
    }

    let field_set: std::collections::HashSet<BookExportField> = fields.iter().copied().collect();
    let has = |f: BookExportField| field_set.contains(&f);

    // Pre-load library names if needed
    let library_names: HashMap<Uuid, String> = if has(BookExportField::LibraryName) {
        let all_libs = LibraryRepository::list_all(db).await?;
        all_libs.into_iter().map(|l| (l.id, l.name)).collect()
    } else {
        HashMap::new()
    };

    // Pre-load series names if needed
    let needs_series_name = has(BookExportField::SeriesName);

    let mut total_rows = 0;

    for chunk in book_ids.chunks(DEFAULT_BATCH_SIZE) {
        // Load books for this chunk
        let books_map = load_book_chunk(db, chunk).await?;

        // Load metadata (always needed for book name anchor)
        let metadata_map = load_metadata_chunk(db, chunk).await?;

        // Load series names if needed
        let series_names: HashMap<Uuid, String> = if needs_series_name {
            let series_ids: Vec<Uuid> = books_map.values().map(|b| b.series_id).collect();
            let series_list = SeriesRepository::find_by_ids(db, &series_ids).await?;
            series_list.into_iter().map(|s| (s.id, s.name)).collect()
        } else {
            HashMap::new()
        };

        // Load genres if needed
        let genres_map = if has(BookExportField::Genres) {
            GenreRepository::get_genres_for_book_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        // Load tags if needed
        let tags_map = if has(BookExportField::Tags) {
            TagRepository::get_tags_for_book_ids(db, chunk).await?
        } else {
            HashMap::new()
        };

        // Load reading progress if needed
        let needs_progress = has(BookExportField::Progress)
            || has(BookExportField::CurrentPage)
            || has(BookExportField::Completed)
            || has(BookExportField::CompletedAt);
        let progress_map = if needs_progress {
            ReadProgressRepository::get_by_user_books(db, user_id, chunk).await?
        } else {
            HashMap::new()
        };

        // Assemble rows
        for &bid in chunk {
            let Some(book) = books_map.get(&bid) else {
                continue;
            };
            let meta = metadata_map.get(&bid);
            let mut row = BookExportRow::from_book(book, meta);

            // Optional ID fields
            if has(BookExportField::BookId) {
                row.book_id = Some(book.id.to_string());
            }
            if has(BookExportField::SeriesId) {
                row.series_id = Some(book.series_id.to_string());
            }
            if has(BookExportField::LibraryId) {
                row.library_id = Some(book.library_id.to_string());
            }
            if has(BookExportField::SeriesName) {
                row.series_name = series_names.get(&book.series_id).cloned();
            }
            if has(BookExportField::LibraryName) {
                row.library_name = library_names.get(&book.library_id).cloned();
            }

            // File info
            if has(BookExportField::FileName) {
                row.file_name = Some(book.file_name.clone());
            }
            if has(BookExportField::FilePath) {
                row.file_path = Some(book.file_path.clone());
            }
            if has(BookExportField::FileSize) {
                row.file_size = Some(book.file_size);
            }
            if has(BookExportField::BookFormat) {
                row.book_format = Some(book.format.clone());
            }
            if has(BookExportField::PageCount) {
                row.page_count = Some(book.page_count);
            }

            // Timestamps
            if has(BookExportField::CreatedAt) {
                row.created_at = Some(book.created_at.to_rfc3339());
            }
            if has(BookExportField::UpdatedAt) {
                row.updated_at = Some(book.updated_at.to_rfc3339());
            }

            // Metadata from book_metadata
            if let Some(m) = meta {
                if has(BookExportField::Title) {
                    row.title = m.title.clone();
                }
                if has(BookExportField::Summary) {
                    row.summary = m.summary.clone();
                }
                if has(BookExportField::Publisher) {
                    row.publisher = m.publisher.clone();
                }
                if has(BookExportField::Year) {
                    row.year = m.year;
                }
                if has(BookExportField::Language) {
                    row.language = m.language_iso.clone();
                }
                if has(BookExportField::Authors) {
                    row.authors = format_authors(&m.authors_json);
                }
                if has(BookExportField::Number) {
                    row.number = m.number.map(|n| n.to_string());
                }
            }

            // Genres
            if has(BookExportField::Genres)
                && let Some(genres) = genres_map.get(&bid)
            {
                let names: Vec<&str> = genres.iter().map(|g| g.name.as_str()).collect();
                if !names.is_empty() {
                    row.genres = Some(names.join("; "));
                }
            }

            // Tags
            if has(BookExportField::Tags)
                && let Some(tags) = tags_map.get(&bid)
            {
                let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
                if !names.is_empty() {
                    row.tags = Some(names.join("; "));
                }
            }

            // Reading progress
            if let Some(progress) = progress_map.get(&bid) {
                if has(BookExportField::CurrentPage) {
                    row.current_page = Some(progress.current_page);
                }
                if has(BookExportField::Completed) {
                    row.completed = Some(progress.completed);
                }
                if has(BookExportField::CompletedAt) {
                    row.completed_at = progress.completed_at.map(|t| t.to_rfc3339());
                }
                if has(BookExportField::Progress) {
                    row.progress = compute_book_progress(book, progress);
                }
            } else if has(BookExportField::Progress) {
                // No progress record = 0% progress
                row.progress = Some(0.0);
            }

            on_row(row);
            total_rows += 1;
        }
    }

    Ok(total_rows)
}

/// Compute progress percentage for a single book.
fn compute_book_progress(book: &books::Model, progress: &read_progress::Model) -> Option<f64> {
    if progress.completed {
        return Some(100.0);
    }
    // For EPUB: use stored progress_percentage if available
    if let Some(pct) = progress.progress_percentage {
        return Some(pct * 100.0);
    }
    // For other formats: current_page / page_count
    if book.page_count > 0 {
        Some(progress.current_page as f64 / book.page_count as f64 * 100.0)
    } else {
        Some(0.0)
    }
}

/// Load book models for a chunk of IDs.
async fn load_book_chunk(
    db: &DatabaseConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, books::Model>> {
    use crate::db::entities::books::Entity as Books;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let results = Books::find()
        .filter(books::Column::Id.is_in(ids.to_vec()))
        .all(db)
        .await?;

    Ok(results.into_iter().map(|b| (b.id, b)).collect())
}

/// Load book metadata for a chunk of book IDs.
async fn load_metadata_chunk(
    db: &DatabaseConnection,
    book_ids: &[Uuid],
) -> Result<HashMap<Uuid, book_metadata::Model>> {
    use crate::db::entities::book_metadata::Entity as BookMetadata;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let results = BookMetadata::find()
        .filter(book_metadata::Column::BookId.is_in(book_ids.to_vec()))
        .all(db)
        .await?;

    Ok(results.into_iter().map(|m| (m.book_id, m)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_book_export_field_roundtrip() {
        for field in BookExportField::ALL {
            let s = field.as_str();
            let parsed = BookExportField::parse(s);
            assert_eq!(parsed, Some(*field), "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_book_export_field_parse_unknown() {
        assert_eq!(BookExportField::parse("nonexistent"), None);
    }

    #[test]
    fn test_book_anchors_are_subset_of_all() {
        for anchor in BookExportField::ANCHORS {
            assert!(
                BookExportField::ALL.contains(anchor),
                "Anchor {:?} not in ALL",
                anchor
            );
        }
    }

    #[test]
    fn test_book_llm_select_subset_of_all() {
        for field in BookExportField::LLM_SELECT {
            assert!(
                BookExportField::ALL.contains(field),
                "LLM_SELECT field {:?} not in ALL",
                field
            );
        }
    }

    #[test]
    fn test_book_user_specific_fields() {
        assert!(BookExportField::Progress.is_user_specific());
        assert!(BookExportField::CurrentPage.is_user_specific());
        assert!(BookExportField::Completed.is_user_specific());
        assert!(BookExportField::CompletedAt.is_user_specific());
        assert!(!BookExportField::Title.is_user_specific());
    }

    #[test]
    fn test_book_multi_value_fields() {
        assert!(BookExportField::Authors.is_multi_value());
        assert!(BookExportField::Genres.is_multi_value());
        assert!(BookExportField::Tags.is_multi_value());
        assert!(!BookExportField::Title.is_multi_value());
    }

    #[test]
    fn test_book_anchor_fields() {
        assert!(BookExportField::BookName.is_anchor());
        assert!(!BookExportField::BookId.is_anchor());
    }

    #[test]
    fn test_book_field_labels() {
        assert_eq!(BookExportField::BookName.label(), "Book Name");
        assert_eq!(BookExportField::Progress.label(), "Progress");
        assert_eq!(BookExportField::CompletedAt.label(), "Completed At");
    }
}
