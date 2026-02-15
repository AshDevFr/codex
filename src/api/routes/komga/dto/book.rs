//! Komga-compatible book DTOs
//!
//! These DTOs match the exact structure Komic expects from Komga's book endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::series::KomgaAuthorDto;

/// Komga media DTO
///
/// Information about the book's media/file.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaMediaDto {
    /// Media status (READY, UNKNOWN, ERROR, UNSUPPORTED, OUTDATED)
    pub status: String,
    /// MIME type (e.g., "application/zip", "application/epub+zip", "application/pdf")
    pub media_type: String,
    /// Media profile (DIVINA for comics/manga, PDF for PDFs)
    pub media_profile: String,
    /// Number of pages
    pub pages_count: i32,
    /// Comment/notes about media analysis
    #[serde(default)]
    pub comment: String,
    /// Whether EPUB is DIVINA-compatible
    #[serde(default)]
    pub epub_divina_compatible: bool,
    /// Whether EPUB is a KePub file
    #[serde(default)]
    pub epub_is_kepub: bool,
}

impl Default for KomgaMediaDto {
    fn default() -> Self {
        Self {
            status: "READY".to_string(),
            media_type: "application/zip".to_string(),
            media_profile: "DIVINA".to_string(),
            pages_count: 0,
            comment: String::new(),
            epub_divina_compatible: false,
            epub_is_kepub: false,
        }
    }
}

impl KomgaMediaDto {
    /// Create from Codex book data
    pub fn from_codex(file_format: &str, page_count: i32, analysis_error: Option<&str>) -> Self {
        let media_type = match file_format.to_lowercase().as_str() {
            "cbz" | "zip" => "application/zip".to_string(),
            "cbr" | "rar" => "application/x-rar-compressed".to_string(),
            "epub" => "application/epub+zip".to_string(),
            "pdf" => "application/pdf".to_string(),
            _ => "application/octet-stream".to_string(),
        };

        let media_profile = match file_format.to_lowercase().as_str() {
            "pdf" => "PDF".to_string(),
            "epub" => "EPUB".to_string(),
            _ => "DIVINA".to_string(),
        };

        let status = if analysis_error.is_some() {
            "ERROR".to_string()
        } else {
            "READY".to_string()
        };

        Self {
            status,
            media_type,
            media_profile,
            pages_count: page_count,
            comment: analysis_error.unwrap_or_default().to_string(),
            epub_divina_compatible: false,
            epub_is_kepub: false,
        }
    }
}

/// Komga book metadata DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaBookMetadataDto {
    /// Book title
    pub title: String,
    /// Whether title is locked
    #[serde(default)]
    pub title_lock: bool,
    /// Book summary
    #[serde(default)]
    pub summary: String,
    /// Whether summary is locked
    #[serde(default)]
    pub summary_lock: bool,
    /// Book number (display string)
    pub number: String,
    /// Whether number is locked
    #[serde(default)]
    pub number_lock: bool,
    /// Number for sorting (float for chapter ordering)
    pub number_sort: f64,
    /// Whether number_sort is locked
    #[serde(default)]
    pub number_sort_lock: bool,
    /// Release date (YYYY-MM-DD or full ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    /// Whether release_date is locked
    #[serde(default)]
    pub release_date_lock: bool,
    /// Authors list
    #[serde(default)]
    pub authors: Vec<KomgaAuthorDto>,
    /// Whether authors are locked
    #[serde(default)]
    pub authors_lock: bool,
    /// Tags list
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether tags are locked
    #[serde(default)]
    pub tags_lock: bool,
    /// ISBN
    #[serde(default)]
    pub isbn: String,
    /// Whether ISBN is locked
    #[serde(default)]
    pub isbn_lock: bool,
    /// Links
    #[serde(default)]
    pub links: Vec<KomgaBookLinkDto>,
    /// Whether links are locked
    #[serde(default)]
    pub links_lock: bool,
    /// Created timestamp (ISO 8601)
    pub created: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified: String,
}

impl Default for KomgaBookMetadataDto {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            title: String::new(),
            title_lock: false,
            summary: String::new(),
            summary_lock: false,
            number: String::new(),
            number_lock: false,
            number_sort: 0.0,
            number_sort_lock: false,
            release_date: None,
            release_date_lock: false,
            authors: Vec::new(),
            authors_lock: false,
            tags: Vec::new(),
            tags_lock: false,
            isbn: String::new(),
            isbn_lock: false,
            links: Vec::new(),
            links_lock: false,
            created: now.clone(),
            last_modified: now,
        }
    }
}

/// Komga book link DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaBookLinkDto {
    /// Link label
    pub label: String,
    /// Link URL
    pub url: String,
}

/// Komga read progress DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaReadProgressDto {
    /// Current page number (1-indexed)
    pub page: i32,
    /// Whether the book is completed
    pub completed: bool,
    /// When the book was last read (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_date: Option<String>,
    /// Created timestamp (ISO 8601)
    pub created: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified: String,
    /// Device ID that last updated progress
    #[serde(default)]
    pub device_id: String,
    /// Device name that last updated progress
    #[serde(default)]
    pub device_name: String,
}

impl Default for KomgaReadProgressDto {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            page: 1,
            completed: false,
            read_date: None,
            created: now.clone(),
            last_modified: now,
            device_id: String::new(),
            device_name: String::new(),
        }
    }
}

/// Komga book DTO
///
/// Based on actual Komic traffic analysis. This is the main book representation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaBookDto {
    /// Book unique identifier (UUID as string)
    pub id: String,
    /// Series ID
    pub series_id: String,
    /// Series title (required by Komic for display)
    pub series_title: String,
    /// Library ID
    pub library_id: String,
    /// Book filename/name
    pub name: String,
    /// File URL/path
    pub url: String,
    /// Book number in series
    pub number: i32,
    /// Created timestamp (ISO 8601)
    pub created: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified: String,
    /// File last modified timestamp (ISO 8601)
    pub file_last_modified: String,
    /// File size in bytes
    pub size_bytes: i64,
    /// Human-readable file size (e.g., "869.9 MiB")
    pub size: String,
    /// Media information
    pub media: KomgaMediaDto,
    /// Book metadata
    pub metadata: KomgaBookMetadataDto,
    /// User's read progress (null if not started)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_progress: Option<KomgaReadProgressDto>,
    /// Whether book is deleted (soft delete)
    #[serde(default)]
    pub deleted: bool,
    /// File hash
    #[serde(default)]
    pub file_hash: String,
    /// Whether this is a oneshot
    #[serde(default)]
    pub oneshot: bool,
}

impl Default for KomgaBookDto {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: String::new(),
            series_id: String::new(),
            series_title: String::new(),
            library_id: String::new(),
            name: String::new(),
            url: String::new(),
            number: 0,
            created: now.clone(),
            last_modified: now.clone(),
            file_last_modified: now,
            size_bytes: 0,
            size: "0 B".to_string(),
            media: KomgaMediaDto::default(),
            metadata: KomgaBookMetadataDto::default(),
            read_progress: None,
            deleted: false,
            file_hash: String::new(),
            oneshot: false,
        }
    }
}

impl KomgaBookDto {
    /// Create a KomgaBookDto from Codex book data
    pub fn from_codex(
        book: &crate::db::entities::books::Model,
        series_title: &str,
        number: i32,
        read_progress: Option<&crate::db::entities::read_progress::Model>,
    ) -> Self {
        Self::from_codex_with_metadata(book, series_title, number, read_progress, None)
    }

    /// Create a KomgaBookDto from Codex book data with optional book metadata
    pub fn from_codex_with_metadata(
        book: &crate::db::entities::books::Model,
        series_title: &str,
        number: i32,
        read_progress: Option<&crate::db::entities::read_progress::Model>,
        book_metadata: Option<&crate::db::entities::book_metadata::Model>,
    ) -> Self {
        let media = KomgaMediaDto::from_codex(
            &book.format,
            book.page_count,
            book.analysis_error.as_deref(),
        );

        let metadata = build_book_metadata(book, number, book_metadata);

        let progress = read_progress.map(|p| KomgaReadProgressDto {
            page: p.current_page,
            completed: p.completed,
            read_date: Some(p.updated_at.to_rfc3339()),
            created: p.started_at.to_rfc3339(),
            last_modified: p.updated_at.to_rfc3339(),
            device_id: String::new(),
            device_name: String::new(),
        });

        Self {
            id: book.id.to_string(),
            series_id: book.series_id.to_string(),
            series_title: series_title.to_string(),
            library_id: book.library_id.to_string(),
            name: book.file_name.clone(),
            url: book.file_path.clone(),
            number,
            created: book.created_at.to_rfc3339(),
            last_modified: book.updated_at.to_rfc3339(),
            file_last_modified: book.modified_at.to_rfc3339(),
            size_bytes: book.file_size,
            size: format_file_size(book.file_size),
            media,
            metadata,
            read_progress: progress,
            deleted: book.deleted,
            file_hash: book.file_hash.clone(),
            oneshot: false,
        }
    }
}

/// Build KomgaBookMetadataDto from book and optional book_metadata
fn build_book_metadata(
    book: &crate::db::entities::books::Model,
    number: i32,
    book_metadata: Option<&crate::db::entities::book_metadata::Model>,
) -> KomgaBookMetadataDto {
    let Some(meta) = book_metadata else {
        return KomgaBookMetadataDto {
            title: book.file_name.clone(),
            number: number.to_string(),
            number_sort: number as f64,
            created: book.created_at.to_rfc3339(),
            last_modified: book.updated_at.to_rfc3339(),
            ..Default::default()
        };
    };

    // Collect authors from authors_json field
    let authors = meta
        .authors_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<Vec<serde_json::Value>>(json).ok())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let name = entry.get("name")?.as_str()?.trim().to_string();
                    let role = entry
                        .get("role")
                        .and_then(|r| r.as_str())
                        .unwrap_or("writer")
                        .to_string();
                    if name.is_empty() {
                        None
                    } else {
                        Some(KomgaAuthorDto { name, role })
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Build release date from year/month/day
    let release_date = match (meta.year, meta.month, meta.day) {
        (Some(y), Some(m), Some(d)) => Some(format!("{:04}-{:02}-{:02}", y, m, d)),
        (Some(y), Some(m), None) => Some(format!("{:04}-{:02}-01", y, m)),
        (Some(y), None, None) => Some(format!("{:04}-01-01", y)),
        _ => None,
    };

    // Collect tags from genre field (comma-separated)
    let tags: Vec<String> = meta
        .genre
        .as_deref()
        .map(|g| {
            g.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let title = meta.title.clone().unwrap_or_else(|| book.file_name.clone());

    let number_sort = meta
        .number
        .map(|n| n.to_string().parse::<f64>().unwrap_or(number as f64))
        .unwrap_or(number as f64);

    KomgaBookMetadataDto {
        title,
        title_lock: meta.title_lock,
        summary: meta.summary.clone().unwrap_or_default(),
        summary_lock: meta.summary_lock,
        number: meta
            .number
            .map(|n| n.to_string())
            .unwrap_or_else(|| number.to_string()),
        number_lock: meta.number_lock,
        number_sort,
        number_sort_lock: meta.number_lock,
        release_date,
        release_date_lock: meta.year_lock,
        authors,
        authors_lock: meta.authors_json_lock,
        tags,
        tags_lock: meta.genre_lock,
        isbn: meta.isbns.clone().unwrap_or_default(),
        isbn_lock: meta.isbns_lock,
        links: Vec::new(),
        links_lock: false,
        created: meta.created_at.to_rfc3339(),
        last_modified: meta.updated_at.to_rfc3339(),
    }
}

/// Format bytes into human-readable size string
pub fn format_file_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Request DTO for updating read progress
///
/// Observed from actual Komic traffic: `{ "completed": false, "page": 151 }`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaReadProgressUpdateDto {
    /// Current page number (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i32>,
    /// Whether book is completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    /// Device ID (optional, may be used by some clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Device name (optional, may be used by some clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// Request DTO for searching/filtering books (POST /api/v1/books/list)
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaBooksSearchRequestDto {
    /// Library IDs to filter by
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_id: Option<Vec<String>>,
    /// Series IDs to filter by
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<Vec<String>>,
    /// Search term
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
    /// Read status filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_status: Option<Vec<String>>,
    /// Media status filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_status: Option<Vec<String>>,
    /// Tags filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<Vec<String>>,
    /// Authors filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Vec<String>>,
    /// Deleted filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    /// Condition object for complex queries (used by Komic for readStatus filtering)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<serde_json::Value>,
    /// Full text search query
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text_search: Option<String>,
}

/// Extract readStatus value from Komga condition object
///
/// Komic sends conditions like:
/// ```json
/// {
///   "condition": {
///     "allOf": [
///       { "readStatus": { "operator": "is", "value": "IN_PROGRESS" } }
///     ]
///   }
/// }
/// ```
pub fn extract_read_status_from_condition(condition: &serde_json::Value) -> Option<&str> {
    // Check allOf array
    if let Some(all_of) = condition.get("allOf").and_then(|v| v.as_array()) {
        for item in all_of {
            // Check for direct readStatus
            if let Some(value) = item
                .get("readStatus")
                .and_then(|rs| rs.get("value"))
                .and_then(|v| v.as_str())
            {
                return Some(value);
            }
            // Check for anyOf containing readStatus (nested condition)
            if let Some(any_of) = item.get("anyOf").and_then(|v| v.as_array()) {
                for inner_item in any_of {
                    if let Some(value) = inner_item
                        .get("readStatus")
                        .and_then(|rs| rs.get("value"))
                        .and_then(|v| v.as_str())
                    {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

/// Extract seriesId value from Komga condition object
///
/// Komic sends conditions like:
/// ```json
/// {
///   "condition": {
///     "allOf": [
///       { "seriesId": { "operator": "is", "value": "54018da2-5b41-4fa7-8376-ba1bbe8eb7a9" } }
///     ]
///   }
/// }
/// ```
pub fn extract_series_id_from_condition(condition: &serde_json::Value) -> Option<&str> {
    // Check allOf array
    if let Some(all_of) = condition.get("allOf").and_then(|v| v.as_array()) {
        for item in all_of {
            // Check for direct seriesId
            if let Some(value) = item
                .get("seriesId")
                .and_then(|rs| rs.get("value"))
                .and_then(|v| v.as_str())
            {
                return Some(value);
            }
            // Check for anyOf containing seriesId (nested condition)
            if let Some(any_of) = item.get("anyOf").and_then(|v| v.as_array()) {
                for inner_item in any_of {
                    if let Some(value) = inner_item
                        .get("seriesId")
                        .and_then(|rs| rs.get("value"))
                        .and_then(|v| v.as_str())
                    {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

/// Release date condition extracted from Komga condition object
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDateCondition {
    /// The operator: "after" or "before"
    pub operator: String,
    /// The ISO 8601 datetime string
    pub date_time: String,
}

/// Extract releaseDate condition from Komga condition object
///
/// Komic sends conditions like:
/// ```json
/// {
///   "condition": {
///     "allOf": [
///       { "releaseDate": { "dateTime": "2026-01-02T21:36:17Z", "operator": "after" } }
///     ]
///   }
/// }
/// ```
pub fn extract_release_date_from_condition(
    condition: &serde_json::Value,
) -> Option<ReleaseDateCondition> {
    // Check allOf array
    if let Some(all_of) = condition.get("allOf").and_then(|v| v.as_array()) {
        for item in all_of {
            // Check for direct releaseDate
            if let Some(rd) = item.get("releaseDate")
                && let (Some(operator), Some(date_time)) = (
                    rd.get("operator").and_then(|v| v.as_str()),
                    rd.get("dateTime").and_then(|v| v.as_str()),
                )
            {
                return Some(ReleaseDateCondition {
                    operator: operator.to_string(),
                    date_time: date_time.to_string(),
                });
            }
            // Check for anyOf containing releaseDate (nested condition)
            if let Some(any_of) = item.get("anyOf").and_then(|v| v.as_array()) {
                for inner_item in any_of {
                    if let Some(rd) = inner_item.get("releaseDate")
                        && let (Some(operator), Some(date_time)) = (
                            rd.get("operator").and_then(|v| v.as_str()),
                            rd.get("dateTime").and_then(|v| v.as_str()),
                        )
                    {
                        return Some(ReleaseDateCondition {
                            operator: operator.to_string(),
                            date_time: date_time.to_string(),
                        });
                    }
                }
            }
        }
    }
    None
}

/// Extract libraryId value from Komga condition object
///
/// Komic sends conditions like:
/// ```json
/// {
///   "condition": {
///     "allOf": [
///       { "libraryId": { "operator": "is", "value": "283d008a-3e47-4a2a-9b29-8af595120577" } }
///     ]
///   }
/// }
/// ```
pub fn extract_library_id_from_condition(condition: &serde_json::Value) -> Option<&str> {
    // Check allOf array
    if let Some(all_of) = condition.get("allOf").and_then(|v| v.as_array()) {
        for item in all_of {
            // Check for direct libraryId
            if let Some(value) = item
                .get("libraryId")
                .and_then(|rs| rs.get("value"))
                .and_then(|v| v.as_str())
            {
                return Some(value);
            }
            // Check for anyOf containing libraryId (nested condition)
            if let Some(any_of) = item.get("anyOf").and_then(|v| v.as_array()) {
                for inner_item in any_of {
                    if let Some(value) = inner_item
                        .get("libraryId")
                        .and_then(|rs| rs.get("value"))
                        .and_then(|v| v.as_str())
                    {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_book_dto_serialization() {
        let book = KomgaBookDto {
            id: "test-book-id".to_string(),
            series_id: "test-series-id".to_string(),
            series_title: "Test Series".to_string(),
            library_id: "test-library-id".to_string(),
            name: "Chapter 1.cbz".to_string(),
            url: "/media/comics/Test/Chapter 1.cbz".to_string(),
            number: 1,
            size_bytes: 52428800,
            size: "50.0 MiB".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&book).unwrap();
        assert!(json.contains("\"id\":\"test-book-id\""));
        assert!(json.contains("\"seriesId\":\"test-series-id\""));
        assert!(json.contains("\"seriesTitle\":\"Test Series\""));
        assert!(json.contains("\"libraryId\":\"test-library-id\""));
        assert!(json.contains("\"name\":\"Chapter 1.cbz\""));
        assert!(json.contains("\"sizeBytes\":52428800"));
    }

    #[test]
    fn test_book_dto_camel_case() {
        let book = KomgaBookDto::default();
        let json = serde_json::to_string(&book).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"seriesId\""));
        assert!(json.contains("\"seriesTitle\""));
        assert!(json.contains("\"libraryId\""));
        assert!(json.contains("\"fileLastModified\""));
        assert!(json.contains("\"sizeBytes\""));
        assert!(json.contains("\"lastModified\""));
        assert!(json.contains("\"fileHash\""));
    }

    #[test]
    fn test_media_dto_from_codex() {
        let media = KomgaMediaDto::from_codex("cbz", 100, None);
        assert_eq!(media.status, "READY");
        assert_eq!(media.media_type, "application/zip");
        assert_eq!(media.media_profile, "DIVINA");
        assert_eq!(media.pages_count, 100);

        let media_epub = KomgaMediaDto::from_codex("epub", 200, None);
        assert_eq!(media_epub.media_type, "application/epub+zip");
        assert_eq!(media_epub.media_profile, "EPUB");

        let media_pdf = KomgaMediaDto::from_codex("pdf", 50, None);
        assert_eq!(media_pdf.media_type, "application/pdf");
        assert_eq!(media_pdf.media_profile, "PDF");

        let media_error = KomgaMediaDto::from_codex("cbz", 0, Some("Parse error"));
        assert_eq!(media_error.status, "ERROR");
        assert_eq!(media_error.comment, "Parse error");
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1024), "1.0 KiB");
        assert_eq!(format_file_size(1536), "1.5 KiB");
        assert_eq!(format_file_size(1048576), "1.0 MiB");
        assert_eq!(format_file_size(52428800), "50.0 MiB");
        assert_eq!(format_file_size(912261120), "870.0 MiB");
        assert_eq!(format_file_size(1073741824), "1.0 GiB");
    }

    #[test]
    fn test_read_progress_dto() {
        let progress = KomgaReadProgressDto {
            page: 42,
            completed: false,
            read_date: Some("2024-01-15T10:30:00Z".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"page\":42"));
        assert!(json.contains("\"completed\":false"));
        assert!(json.contains("\"readDate\":\"2024-01-15T10:30:00Z\""));
        assert!(json.contains("\"deviceId\""));
        assert!(json.contains("\"deviceName\""));
    }

    #[test]
    fn test_read_progress_update_dto() {
        let update = KomgaReadProgressUpdateDto {
            page: Some(151),
            completed: Some(false),
            device_id: None,
            device_name: None,
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"page\":151"));
        assert!(json.contains("\"completed\":false"));
        // device_id and device_name should be skipped when None
        assert!(!json.contains("deviceId"));
        assert!(!json.contains("deviceName"));
    }

    #[test]
    fn test_read_progress_update_deserialization() {
        // Test actual Komic request format
        let json = r#"{"completed":false,"page":151}"#;
        let update: KomgaReadProgressUpdateDto = serde_json::from_str(json).unwrap();
        assert_eq!(update.page, Some(151));
        assert_eq!(update.completed, Some(false));
    }

    #[test]
    fn test_book_metadata_dto() {
        let metadata = KomgaBookMetadataDto {
            title: "Chapter 1: The Beginning".to_string(),
            summary: "The story begins...".to_string(),
            number: "1".to_string(),
            number_sort: 1.0,
            release_date: Some("2024-01-15".to_string()),
            authors: vec![KomgaAuthorDto {
                name: "Test Author".to_string(),
                role: "WRITER".to_string(),
            }],
            ..Default::default()
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"title\":\"Chapter 1: The Beginning\""));
        assert!(json.contains("\"numberSort\":1.0"));
        assert!(json.contains("\"releaseDate\":\"2024-01-15\""));
    }

    #[test]
    fn test_books_search_request() {
        let request = KomgaBooksSearchRequestDto {
            library_id: Some(vec!["lib1".to_string()]),
            series_id: Some(vec!["series1".to_string(), "series2".to_string()]),
            search_term: Some("batman".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"libraryId\""));
        assert!(json.contains("\"seriesId\""));
        assert!(json.contains("\"searchTerm\""));
    }

    #[test]
    fn test_book_without_read_progress() {
        let book = KomgaBookDto {
            id: "test".to_string(),
            read_progress: None,
            ..Default::default()
        };

        let json = serde_json::to_string(&book).unwrap();
        // readProgress should be skipped when None
        assert!(!json.contains("readProgress"));
    }

    #[test]
    fn test_book_with_read_progress() {
        let book = KomgaBookDto {
            id: "test".to_string(),
            read_progress: Some(KomgaReadProgressDto {
                page: 50,
                completed: false,
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&book).unwrap();
        assert!(json.contains("\"readProgress\""));
        assert!(json.contains("\"page\":50"));
    }

    #[test]
    fn test_extract_read_status_in_progress() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"readStatus":{"operator":"is","value":"IN_PROGRESS"}}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_read_status_from_condition(&condition),
            Some("IN_PROGRESS")
        );
    }

    #[test]
    fn test_extract_read_status_read() {
        let condition: serde_json::Value =
            serde_json::from_str(r#"{"allOf":[{"readStatus":{"operator":"is","value":"READ"}}]}"#)
                .unwrap();
        assert_eq!(extract_read_status_from_condition(&condition), Some("READ"));
    }

    #[test]
    fn test_extract_read_status_unread() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"readStatus":{"operator":"is","value":"UNREAD"}}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_read_status_from_condition(&condition),
            Some("UNREAD")
        );
    }

    #[test]
    fn test_extract_read_status_empty_condition() {
        let condition: serde_json::Value = serde_json::from_str(r#"{"allOf":[]}"#).unwrap();
        assert_eq!(extract_read_status_from_condition(&condition), None);
    }

    #[test]
    fn test_extract_read_status_no_read_status() {
        let condition: serde_json::Value =
            serde_json::from_str(r#"{"allOf":[{"someOther":"value"}]}"#).unwrap();
        assert_eq!(extract_read_status_from_condition(&condition), None);
    }

    #[test]
    fn test_extract_read_status_nested_any_of() {
        // This is the actual format Komic sends for readStatus filter
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"anyOf":[{"readStatus":{"operator":"is","value":"IN_PROGRESS"}}]}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_read_status_from_condition(&condition),
            Some("IN_PROGRESS")
        );
    }

    #[test]
    fn test_extract_series_id_from_condition() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"seriesId":{"operator":"is","value":"54018da2-5b41-4fa7-8376-ba1bbe8eb7a9"}}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_series_id_from_condition(&condition),
            Some("54018da2-5b41-4fa7-8376-ba1bbe8eb7a9")
        );
    }

    #[test]
    fn test_extract_series_id_empty_condition() {
        let condition: serde_json::Value = serde_json::from_str(r#"{"allOf":[]}"#).unwrap();
        assert_eq!(extract_series_id_from_condition(&condition), None);
    }

    #[test]
    fn test_extract_series_id_no_series_id() {
        let condition: serde_json::Value =
            serde_json::from_str(r#"{"allOf":[{"readStatus":{"operator":"is","value":"READ"}}]}"#)
                .unwrap();
        assert_eq!(extract_series_id_from_condition(&condition), None);
    }

    #[test]
    fn test_extract_library_id_from_condition() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"libraryId":{"operator":"is","value":"283d008a-3e47-4a2a-9b29-8af595120577"}}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_library_id_from_condition(&condition),
            Some("283d008a-3e47-4a2a-9b29-8af595120577")
        );
    }

    #[test]
    fn test_extract_library_id_empty_condition() {
        let condition: serde_json::Value = serde_json::from_str(r#"{"allOf":[]}"#).unwrap();
        assert_eq!(extract_library_id_from_condition(&condition), None);
    }

    #[test]
    fn test_extract_library_id_no_library_id() {
        let condition: serde_json::Value =
            serde_json::from_str(r#"{"allOf":[{"readStatus":{"operator":"is","value":"READ"}}]}"#)
                .unwrap();
        assert_eq!(extract_library_id_from_condition(&condition), None);
    }

    #[test]
    fn test_extract_library_id_with_multiple_conditions() {
        // Test the actual format Komic sends
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"libraryId":{"operator":"is","value":"283d008a-3e47-4a2a-9b29-8af595120577"}},{"releaseDate":{"dateTime":"2026-01-02T21:36:17Z","operator":"after"}}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_library_id_from_condition(&condition),
            Some("283d008a-3e47-4a2a-9b29-8af595120577")
        );
    }

    #[test]
    fn test_extract_release_date_after() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"releaseDate":{"dateTime":"2026-01-02T21:36:17Z","operator":"after"}}]}"#,
        )
        .unwrap();
        let result = extract_release_date_from_condition(&condition).unwrap();
        assert_eq!(result.operator, "after");
        assert_eq!(result.date_time, "2026-01-02T21:36:17Z");
    }

    #[test]
    fn test_extract_release_date_before() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"releaseDate":{"dateTime":"2025-06-15T00:00:00Z","operator":"before"}}]}"#,
        )
        .unwrap();
        let result = extract_release_date_from_condition(&condition).unwrap();
        assert_eq!(result.operator, "before");
        assert_eq!(result.date_time, "2025-06-15T00:00:00Z");
    }

    #[test]
    fn test_extract_release_date_with_other_conditions() {
        // Test extraction when releaseDate is alongside other conditions (actual Komic format)
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"libraryId":{"operator":"is","value":"283d008a-3e47-4a2a-9b29-8af595120577"}},{"releaseDate":{"dateTime":"2026-01-02T21:36:17Z","operator":"after"}}]}"#,
        )
        .unwrap();
        let result = extract_release_date_from_condition(&condition).unwrap();
        assert_eq!(result.operator, "after");
        assert_eq!(result.date_time, "2026-01-02T21:36:17Z");
    }

    #[test]
    fn test_extract_release_date_nested_any_of() {
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"anyOf":[{"releaseDate":{"dateTime":"2026-01-02T21:36:17Z","operator":"after"}}]}]}"#,
        )
        .unwrap();
        let result = extract_release_date_from_condition(&condition).unwrap();
        assert_eq!(result.operator, "after");
        assert_eq!(result.date_time, "2026-01-02T21:36:17Z");
    }

    #[test]
    fn test_extract_release_date_empty_condition() {
        let condition: serde_json::Value = serde_json::from_str(r#"{"allOf":[]}"#).unwrap();
        assert!(extract_release_date_from_condition(&condition).is_none());
    }

    #[test]
    fn test_extract_release_date_no_release_date() {
        let condition: serde_json::Value =
            serde_json::from_str(r#"{"allOf":[{"readStatus":{"operator":"is","value":"READ"}}]}"#)
                .unwrap();
        assert!(extract_release_date_from_condition(&condition).is_none());
    }

    #[test]
    fn test_extract_release_date_missing_fields() {
        // Missing dateTime
        let condition: serde_json::Value =
            serde_json::from_str(r#"{"allOf":[{"releaseDate":{"operator":"after"}}]}"#).unwrap();
        assert!(extract_release_date_from_condition(&condition).is_none());

        // Missing operator
        let condition: serde_json::Value = serde_json::from_str(
            r#"{"allOf":[{"releaseDate":{"dateTime":"2026-01-02T21:36:17Z"}}]}"#,
        )
        .unwrap();
        assert!(extract_release_date_from_condition(&condition).is_none());
    }
}
