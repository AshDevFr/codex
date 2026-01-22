//! Komga-compatible series DTOs
//!
//! These DTOs match the exact structure Komic expects from Komga's series endpoints.

use serde::{Deserialize, Serialize};

/// Komga web link DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KomgaWebLinkDto {
    /// Link label
    pub label: String,
    /// Link URL
    pub url: String,
}

/// Komga alternate title DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KomgaAlternateTitleDto {
    /// Title label (e.g., "Japanese", "Romaji")
    pub label: String,
    /// The alternate title text
    pub title: String,
}

/// Komga series metadata DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KomgaSeriesMetadataDto {
    /// Series status (ENDED, ONGOING, ABANDONED, HIATUS)
    pub status: String,
    /// Whether status is locked
    #[serde(default)]
    pub status_lock: bool,
    /// Series title
    pub title: String,
    /// Whether title is locked
    #[serde(default)]
    pub title_lock: bool,
    /// Sort title
    pub title_sort: String,
    /// Whether title_sort is locked
    #[serde(default)]
    pub title_sort_lock: bool,
    /// Series summary/description
    #[serde(default)]
    pub summary: String,
    /// Whether summary is locked
    #[serde(default)]
    pub summary_lock: bool,
    /// Reading direction (LEFT_TO_RIGHT, RIGHT_TO_LEFT, VERTICAL, WEBTOON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_direction: Option<String>,
    /// Whether reading_direction is locked
    #[serde(default)]
    pub reading_direction_lock: bool,
    /// Publisher name
    #[serde(default)]
    pub publisher: String,
    /// Whether publisher is locked
    #[serde(default)]
    pub publisher_lock: bool,
    /// Age rating
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<i32>,
    /// Whether age_rating is locked
    #[serde(default)]
    pub age_rating_lock: bool,
    /// Language code
    #[serde(default)]
    pub language: String,
    /// Whether language is locked
    #[serde(default)]
    pub language_lock: bool,
    /// Genres list
    #[serde(default)]
    pub genres: Vec<String>,
    /// Whether genres are locked
    #[serde(default)]
    pub genres_lock: bool,
    /// Tags list
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether tags are locked
    #[serde(default)]
    pub tags_lock: bool,
    /// Total book count (expected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_book_count: Option<i32>,
    /// Whether total_book_count is locked
    #[serde(default)]
    pub total_book_count_lock: bool,
    /// Sharing labels
    #[serde(default)]
    pub sharing_labels: Vec<String>,
    /// Whether sharing_labels are locked
    #[serde(default)]
    pub sharing_labels_lock: bool,
    /// External links
    #[serde(default)]
    pub links: Vec<KomgaWebLinkDto>,
    /// Whether links are locked
    #[serde(default)]
    pub links_lock: bool,
    /// Alternate titles
    #[serde(default)]
    pub alternate_titles: Vec<KomgaAlternateTitleDto>,
    /// Whether alternate_titles are locked
    #[serde(default)]
    pub alternate_titles_lock: bool,
    /// Metadata created timestamp (ISO 8601)
    pub created: String,
    /// Metadata last modified timestamp (ISO 8601)
    pub last_modified: String,
}

impl Default for KomgaSeriesMetadataDto {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            status: "ONGOING".to_string(),
            status_lock: false,
            title: String::new(),
            title_lock: false,
            title_sort: String::new(),
            title_sort_lock: false,
            summary: String::new(),
            summary_lock: false,
            reading_direction: None,
            reading_direction_lock: false,
            publisher: String::new(),
            publisher_lock: false,
            age_rating: None,
            age_rating_lock: false,
            language: String::new(),
            language_lock: false,
            genres: Vec::new(),
            genres_lock: false,
            tags: Vec::new(),
            tags_lock: false,
            total_book_count: None,
            total_book_count_lock: false,
            sharing_labels: Vec::new(),
            sharing_labels_lock: false,
            links: Vec::new(),
            links_lock: false,
            alternate_titles: Vec::new(),
            alternate_titles_lock: false,
            created: now.clone(),
            last_modified: now,
        }
    }
}

/// Komga books metadata aggregation DTO
///
/// Aggregated metadata from all books in the series.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KomgaBooksMetadataAggregationDto {
    /// Authors from all books
    #[serde(default)]
    pub authors: Vec<KomgaAuthorDto>,
    /// Tags from all books
    #[serde(default)]
    pub tags: Vec<String>,
    /// Release date range (earliest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    /// Summary (from first book or series)
    #[serde(default)]
    pub summary: String,
    /// Summary number (if multiple summaries)
    #[serde(default)]
    pub summary_number: String,
    /// Created timestamp (ISO 8601)
    pub created: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified: String,
}

/// Komga author DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KomgaAuthorDto {
    /// Author name
    pub name: String,
    /// Author role (WRITER, PENCILLER, INKER, COLORIST, LETTERER, COVER, EDITOR)
    pub role: String,
}

/// Komga series DTO
///
/// Based on actual Komic traffic analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KomgaSeriesDto {
    /// Series unique identifier (UUID as string)
    pub id: String,
    /// Library ID
    pub library_id: String,
    /// Series name
    pub name: String,
    /// File URL/path
    pub url: String,
    /// Created timestamp (ISO 8601)
    pub created: String,
    /// Last modified timestamp (ISO 8601)
    pub last_modified: String,
    /// File last modified timestamp (ISO 8601)
    pub file_last_modified: String,
    /// Total books count
    pub books_count: i32,
    /// Read books count
    pub books_read_count: i32,
    /// Unread books count
    pub books_unread_count: i32,
    /// In-progress books count
    pub books_in_progress_count: i32,
    /// Series metadata
    pub metadata: KomgaSeriesMetadataDto,
    /// Aggregated books metadata
    pub books_metadata: KomgaBooksMetadataAggregationDto,
    /// Whether series is deleted (soft delete)
    #[serde(default)]
    pub deleted: bool,
    /// Whether this is a oneshot (single book)
    #[serde(default)]
    pub oneshot: bool,
}

impl Default for KomgaSeriesDto {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: String::new(),
            library_id: String::new(),
            name: String::new(),
            url: String::new(),
            created: now.clone(),
            last_modified: now.clone(),
            file_last_modified: now,
            books_count: 0,
            books_read_count: 0,
            books_unread_count: 0,
            books_in_progress_count: 0,
            metadata: KomgaSeriesMetadataDto::default(),
            books_metadata: KomgaBooksMetadataAggregationDto::default(),
            deleted: false,
            oneshot: false,
        }
    }
}

/// Convert Codex reading direction to Komga format
pub fn codex_to_komga_reading_direction(direction: Option<&str>) -> Option<String> {
    direction.map(|d| match d.to_lowercase().as_str() {
        "ltr" => "LEFT_TO_RIGHT".to_string(),
        "rtl" => "RIGHT_TO_LEFT".to_string(),
        "ttb" => "VERTICAL".to_string(),
        "webtoon" => "WEBTOON".to_string(),
        _ => "LEFT_TO_RIGHT".to_string(),
    })
}

/// Convert Codex series status to Komga format
pub fn codex_to_komga_status(status: Option<&str>) -> String {
    match status.map(|s| s.to_lowercase()).as_deref() {
        Some("ended") | Some("complete") | Some("completed") => "ENDED".to_string(),
        Some("ongoing") | Some("publishing") => "ONGOING".to_string(),
        Some("hiatus") => "HIATUS".to_string(),
        Some("abandoned") | Some("cancelled") | Some("canceled") => "ABANDONED".to_string(),
        _ => "ONGOING".to_string(), // Default to ongoing for unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_series_dto_serialization() {
        let series = KomgaSeriesDto {
            id: "test-series-id".to_string(),
            library_id: "test-library-id".to_string(),
            name: "Test Series".to_string(),
            url: "/media/comics/Test Series".to_string(),
            books_count: 10,
            books_read_count: 5,
            books_unread_count: 5,
            books_in_progress_count: 0,
            ..Default::default()
        };

        let json = serde_json::to_string(&series).unwrap();
        assert!(json.contains("\"id\":\"test-series-id\""));
        assert!(json.contains("\"libraryId\":\"test-library-id\""));
        assert!(json.contains("\"name\":\"Test Series\""));
        assert!(json.contains("\"booksCount\":10"));
        assert!(json.contains("\"booksReadCount\":5"));
        assert!(json.contains("\"booksUnreadCount\":5"));
    }

    #[test]
    fn test_series_metadata_camel_case() {
        let metadata = KomgaSeriesMetadataDto::default();
        let json = serde_json::to_string(&metadata).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"statusLock\""));
        assert!(json.contains("\"titleSort\""));
        assert!(json.contains("\"titleSortLock\""));
        assert!(json.contains("\"summaryLock\""));
        assert!(json.contains("\"readingDirection\"") || !json.contains("\"reading_direction\""));
        assert!(json.contains("\"publisherLock\""));
        assert!(json.contains("\"ageRating\"") || !json.contains("\"age_rating\""));
        assert!(json.contains("\"genresLock\""));
        assert!(json.contains("\"totalBookCount\"") || !json.contains("\"total_book_count\""));
        assert!(json.contains("\"sharingLabels\""));
        assert!(json.contains("\"alternateTitles\""));
        assert!(json.contains("\"lastModified\""));
    }

    #[test]
    fn test_reading_direction_conversion() {
        assert_eq!(
            codex_to_komga_reading_direction(Some("ltr")),
            Some("LEFT_TO_RIGHT".to_string())
        );
        assert_eq!(
            codex_to_komga_reading_direction(Some("rtl")),
            Some("RIGHT_TO_LEFT".to_string())
        );
        assert_eq!(
            codex_to_komga_reading_direction(Some("ttb")),
            Some("VERTICAL".to_string())
        );
        assert_eq!(
            codex_to_komga_reading_direction(Some("webtoon")),
            Some("WEBTOON".to_string())
        );
        assert_eq!(codex_to_komga_reading_direction(None), None);
    }

    #[test]
    fn test_status_conversion() {
        assert_eq!(codex_to_komga_status(Some("ended")), "ENDED");
        assert_eq!(codex_to_komga_status(Some("complete")), "ENDED");
        assert_eq!(codex_to_komga_status(Some("ongoing")), "ONGOING");
        assert_eq!(codex_to_komga_status(Some("hiatus")), "HIATUS");
        assert_eq!(codex_to_komga_status(Some("abandoned")), "ABANDONED");
        assert_eq!(codex_to_komga_status(Some("cancelled")), "ABANDONED");
        assert_eq!(codex_to_komga_status(None), "ONGOING");
        assert_eq!(codex_to_komga_status(Some("unknown")), "ONGOING");
    }

    #[test]
    fn test_author_dto() {
        let author = KomgaAuthorDto {
            name: "Frank Miller".to_string(),
            role: "WRITER".to_string(),
        };

        let json = serde_json::to_string(&author).unwrap();
        assert!(json.contains("\"name\":\"Frank Miller\""));
        assert!(json.contains("\"role\":\"WRITER\""));
    }

    #[test]
    fn test_web_link_dto() {
        let link = KomgaWebLinkDto {
            label: "MyAnimeList".to_string(),
            url: "https://myanimelist.net/manga/123".to_string(),
        };

        let json = serde_json::to_string(&link).unwrap();
        assert!(json.contains("\"label\":\"MyAnimeList\""));
        assert!(json.contains("\"url\":\"https://myanimelist.net/manga/123\""));
    }

    #[test]
    fn test_alternate_title_dto() {
        let title = KomgaAlternateTitleDto {
            label: "Japanese".to_string(),
            title: "進撃の巨人".to_string(),
        };

        let json = serde_json::to_string(&title).unwrap();
        assert!(json.contains("\"label\":\"Japanese\""));
        assert!(json.contains("\"title\":\"進撃の巨人\""));
    }

    #[test]
    fn test_books_metadata_aggregation() {
        let aggregation = KomgaBooksMetadataAggregationDto {
            authors: vec![
                KomgaAuthorDto {
                    name: "Writer A".to_string(),
                    role: "WRITER".to_string(),
                },
                KomgaAuthorDto {
                    name: "Artist B".to_string(),
                    role: "PENCILLER".to_string(),
                },
            ],
            tags: vec!["Action".to_string(), "Adventure".to_string()],
            release_date: Some("2020-01-15".to_string()),
            summary: "A great series".to_string(),
            summary_number: "1".to_string(),
            created: "2024-01-01T00:00:00Z".to_string(),
            last_modified: "2024-01-15T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&aggregation).unwrap();
        assert!(json.contains("\"authors\""));
        assert!(json.contains("\"tags\""));
        assert!(json.contains("\"releaseDate\""));
        assert!(json.contains("\"summaryNumber\""));
    }
}
