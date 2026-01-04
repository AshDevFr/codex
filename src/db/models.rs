use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::parsers::metadata::{FileFormat, ImageFormat};

// ============================================================================
// Library Models
// ============================================================================

/// Scanning strategy type for library organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "snake_case")]
pub enum ScanningStrategy {
    /// Komga-compatible: Direct child folders = series
    KomgaCompatible,
    /// Volume-Chapter: Parent folder = series, child folders = volumes
    VolumeChapter,
    /// Flat: All files at root, series from filename/metadata
    Flat,
    /// Publisher Hierarchy: Skip first N levels then apply Komga rules
    PublisherHierarchy,
    /// Custom: User-defined regex patterns
    Custom,
}

impl ScanningStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KomgaCompatible => "komga_compatible",
            Self::VolumeChapter => "volume_chapter",
            Self::Flat => "flat",
            Self::PublisherHierarchy => "publisher_hierarchy",
            Self::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "komga_compatible" => Some(Self::KomgaCompatible),
            "volume_chapter" => Some(Self::VolumeChapter),
            "flat" => Some(Self::Flat),
            "publisher_hierarchy" => Some(Self::PublisherHierarchy),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Library record - top-level container for content
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Library {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub scanning_strategy: String, // Store as string in DB
    pub scanning_config: Option<String>, // JSON string
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_scanned_at: Option<DateTime<Utc>>,
}

impl Library {
    pub fn new(name: String, path: String, strategy: ScanningStrategy) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            scanning_strategy: strategy.as_str().to_string(),
            scanning_config: None,
            created_at: now,
            updated_at: now,
            last_scanned_at: None,
        }
    }

    pub fn get_scanning_strategy(&self) -> ScanningStrategy {
        ScanningStrategy::from_str(&self.scanning_strategy)
            .unwrap_or(ScanningStrategy::KomgaCompatible)
    }
}

// ============================================================================
// Series Models
// ============================================================================

/// Series record - collection of related books
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Series {
    pub id: Uuid,
    pub library_id: Uuid,
    pub name: String,
    pub normalized_name: String, // Lowercase, no special chars for searching
    pub sort_name: Option<String>, // Custom sort name
    pub summary: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<i32>,
    pub book_count: i32,
    // Rating fields
    pub user_rating: Option<f32>, // User's personal rating (0.0-10.0)
    pub external_rating: Option<f32>, // Rating from external source (0.0-10.0)
    pub external_rating_count: Option<i32>, // Number of ratings from external source
    pub external_rating_source: Option<String>, // Source name (e.g., "anilist", "mangaupdates")
    // Custom metadata
    pub custom_metadata: Option<String>, // JSON string for user-defined metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Series {
    pub fn new(library_id: Uuid, name: String) -> Self {
        let now = Utc::now();
        let normalized_name = Self::normalize_name(&name);

        Self {
            id: Uuid::new_v4(),
            library_id,
            name: name.clone(),
            normalized_name,
            sort_name: None,
            summary: None,
            publisher: None,
            year: None,
            book_count: 0,
            user_rating: None,
            external_rating: None,
            external_rating_count: None,
            external_rating_source: None,
            custom_metadata: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Normalize name for searching (lowercase, alphanumeric only)
    fn normalize_name(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Set user rating (0.0-10.0)
    pub fn set_user_rating(&mut self, rating: f32) -> Result<(), String> {
        if !(0.0..=10.0).contains(&rating) {
            return Err("Rating must be between 0.0 and 10.0".to_string());
        }
        self.user_rating = Some(rating);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set external rating from integration
    pub fn set_external_rating(&mut self, rating: f32, count: Option<i32>, source: String) -> Result<(), String> {
        if !(0.0..=10.0).contains(&rating) {
            return Err("Rating must be between 0.0 and 10.0".to_string());
        }
        self.external_rating = Some(rating);
        self.external_rating_count = count;
        self.external_rating_source = Some(source);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Get or parse custom metadata as JSON
    pub fn get_custom_metadata_json(&self) -> serde_json::Value {
        self.custom_metadata
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}))
    }

    /// Set custom metadata from JSON value
    pub fn set_custom_metadata(&mut self, metadata: serde_json::Value) {
        self.custom_metadata = Some(metadata.to_string());
        self.updated_at = Utc::now();
    }
}

// ============================================================================
// Book Models
// ============================================================================

/// Book record - individual file in the library
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Book {
    pub id: Uuid,
    pub series_id: Uuid,
    pub title: Option<String>,
    pub number: Option<f32>, // Can be fractional (e.g., 1.5)
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_hash: String, // SHA-256
    pub format: String, // Store FileFormat as string
    pub page_count: i32,
    pub modified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Book {
    pub fn new(series_id: Uuid, file_path: String, file_name: String) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            series_id,
            title: None,
            number: None,
            file_path,
            file_name,
            file_size: 0,
            file_hash: String::new(),
            format: String::new(),
            page_count: 0,
            modified_at: now,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn get_format(&self) -> Option<FileFormat> {
        match self.format.to_lowercase().as_str() {
            "cbz" => Some(FileFormat::CBZ),
            "cbr" => Some(FileFormat::CBR),
            "epub" => Some(FileFormat::EPUB),
            "pdf" => Some(FileFormat::PDF),
            _ => None,
        }
    }

    pub fn set_format(&mut self, format: FileFormat) {
        self.format = match format {
            FileFormat::CBZ => "cbz",
            FileFormat::CBR => "cbr",
            FileFormat::EPUB => "epub",
            FileFormat::PDF => "pdf",
        }.to_string();
    }
}

// ============================================================================
// Book Metadata Models
// ============================================================================

/// Extended metadata for a book (from ComicInfo.xml, EPUB metadata, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookMetadataRecord {
    pub id: Uuid,
    pub book_id: Uuid,
    // Content metadata
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
    // Date information
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    // Series information
    pub volume: Option<i32>,
    pub count: Option<i32>, // Total count in series
    // Identifiers
    pub isbns: Option<String>, // JSON array as string
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BookMetadataRecord {
    pub fn new(book_id: Uuid) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            book_id,
            summary: None,
            writer: None,
            penciller: None,
            inker: None,
            colorist: None,
            letterer: None,
            cover_artist: None,
            editor: None,
            publisher: None,
            imprint: None,
            genre: None,
            web: None,
            language_iso: None,
            format_detail: None,
            black_and_white: None,
            manga: None,
            year: None,
            month: None,
            day: None,
            volume: None,
            count: None,
            isbns: None,
            created_at: now,
            updated_at: now,
        }
    }
}

// ============================================================================
// Page Models
// ============================================================================

/// Page record - individual page within a book
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Page {
    pub id: Uuid,
    pub book_id: Uuid,
    pub page_number: i32, // 1-indexed
    pub file_name: String,
    pub format: String, // ImageFormat as string
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub created_at: DateTime<Utc>,
}

impl Page {
    pub fn new(book_id: Uuid, page_number: i32, file_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            book_id,
            page_number,
            file_name,
            format: String::new(),
            width: 0,
            height: 0,
            file_size: 0,
            created_at: Utc::now(),
        }
    }

    pub fn get_image_format(&self) -> Option<ImageFormat> {
        match self.format.to_lowercase().as_str() {
            "jpeg" => Some(ImageFormat::JPEG),
            "png" => Some(ImageFormat::PNG),
            "webp" => Some(ImageFormat::WEBP),
            "gif" => Some(ImageFormat::GIF),
            "avif" => Some(ImageFormat::AVIF),
            "bmp" => Some(ImageFormat::BMP),
            _ => None,
        }
    }

    pub fn set_image_format(&mut self, format: ImageFormat) {
        self.format = match format {
            ImageFormat::JPEG => "jpeg",
            ImageFormat::PNG => "png",
            ImageFormat::WEBP => "webp",
            ImageFormat::GIF => "gif",
            ImageFormat::AVIF => "avif",
            ImageFormat::BMP => "bmp",
        }.to_string();
    }
}

// ============================================================================
// User Models
// ============================================================================

/// User record - for authentication and authorization
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl User {
    pub fn new(username: String, email: String, password_hash: String) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            username,
            email,
            password_hash,
            is_admin: false,
            created_at: now,
            updated_at: now,
            last_login_at: None,
        }
    }
}

// ============================================================================
// Read Progress Models
// ============================================================================

/// Read progress tracking for users
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReadProgress {
    pub id: Uuid,
    pub user_id: Uuid,
    pub book_id: Uuid,
    pub current_page: i32,
    pub completed: bool,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl ReadProgress {
    pub fn new(user_id: Uuid, book_id: Uuid) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            user_id,
            book_id,
            current_page: 1,
            completed: false,
            started_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}

// ============================================================================
// Metadata Source Models
// ============================================================================

/// External metadata source tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MetadataSource {
    pub id: Uuid,
    pub series_id: Uuid,
    pub source_name: String, // e.g., "mangabaka", "anilist"
    pub external_id: String,
    pub external_url: Option<String>,
    pub confidence: f32, // 0.0 to 1.0
    pub metadata_json: String, // Full metadata as JSON
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MetadataSource {
    pub fn new(series_id: Uuid, source_name: String, external_id: String) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            series_id,
            source_name,
            external_id,
            external_url: None,
            confidence: 0.0,
            metadata_json: String::from("{}"),
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanning_strategy_as_str() {
        assert_eq!(ScanningStrategy::KomgaCompatible.as_str(), "komga_compatible");
        assert_eq!(ScanningStrategy::VolumeChapter.as_str(), "volume_chapter");
        assert_eq!(ScanningStrategy::Flat.as_str(), "flat");
        assert_eq!(ScanningStrategy::PublisherHierarchy.as_str(), "publisher_hierarchy");
        assert_eq!(ScanningStrategy::Custom.as_str(), "custom");
    }

    #[test]
    fn test_scanning_strategy_from_str() {
        assert_eq!(
            ScanningStrategy::from_str("komga_compatible"),
            Some(ScanningStrategy::KomgaCompatible)
        );
        assert_eq!(
            ScanningStrategy::from_str("volume_chapter"),
            Some(ScanningStrategy::VolumeChapter)
        );
        assert_eq!(ScanningStrategy::from_str("invalid"), None);
    }

    #[test]
    fn test_library_new() {
        let lib = Library::new(
            "Test Library".to_string(),
            "/path/to/library".to_string(),
            ScanningStrategy::KomgaCompatible,
        );

        assert_eq!(lib.name, "Test Library");
        assert_eq!(lib.path, "/path/to/library");
        assert_eq!(lib.scanning_strategy, "komga_compatible");
        assert!(lib.scanning_config.is_none());
        assert!(lib.last_scanned_at.is_none());
    }

    #[test]
    fn test_series_normalize_name() {
        let series = Series::new(Uuid::new_v4(), "One-Piece (Vol. 1)".to_string());
        // Hyphens are removed, special chars filtered, multiple spaces collapsed
        assert_eq!(series.normalized_name, "onepiece vol 1");

        // Test with various inputs
        let series2 = Series::new(Uuid::new_v4(), "The Amazing Spider-Man!".to_string());
        assert_eq!(series2.normalized_name, "the amazing spiderman");

        let series3 = Series::new(Uuid::new_v4(), "Batman:  The Dark   Knight".to_string());
        assert_eq!(series3.normalized_name, "batman the dark knight");
    }

    #[test]
    fn test_series_user_rating() {
        let mut series = Series::new(Uuid::new_v4(), "Test Series".to_string());

        // Valid rating
        assert!(series.set_user_rating(7.5).is_ok());
        assert_eq!(series.user_rating, Some(7.5));

        // Edge cases
        assert!(series.set_user_rating(0.0).is_ok());
        assert!(series.set_user_rating(10.0).is_ok());

        // Invalid ratings
        assert!(series.set_user_rating(-1.0).is_err());
        assert!(series.set_user_rating(11.0).is_err());
    }

    #[test]
    fn test_series_external_rating() {
        let mut series = Series::new(Uuid::new_v4(), "Test Series".to_string());

        // Valid rating with source
        assert!(series.set_external_rating(8.5, Some(1234), "anilist".to_string()).is_ok());
        assert_eq!(series.external_rating, Some(8.5));
        assert_eq!(series.external_rating_count, Some(1234));
        assert_eq!(series.external_rating_source, Some("anilist".to_string()));

        // Invalid rating
        assert!(series.set_external_rating(15.0, None, "test".to_string()).is_err());
    }

    #[test]
    fn test_series_custom_metadata() {
        let mut series = Series::new(Uuid::new_v4(), "Test Series".to_string());

        // Set custom metadata
        let metadata = serde_json::json!({
            "custom_field": "value",
            "tags": ["action", "adventure"],
            "status": "ongoing"
        });
        series.set_custom_metadata(metadata.clone());

        // Get custom metadata
        let retrieved = series.get_custom_metadata_json();
        assert_eq!(retrieved["custom_field"], "value");
        assert_eq!(retrieved["tags"][0], "action");
        assert_eq!(retrieved["status"], "ongoing");

        // Empty metadata returns empty object
        let empty_series = Series::new(Uuid::new_v4(), "Empty".to_string());
        let empty_meta = empty_series.get_custom_metadata_json();
        assert_eq!(empty_meta, serde_json::json!({}));
    }

    #[test]
    fn test_book_format_conversion() {
        let mut book = Book::new(
            Uuid::new_v4(),
            "/path/to/book.cbz".to_string(),
            "book.cbz".to_string(),
        );

        book.set_format(FileFormat::CBZ);
        assert_eq!(book.format, "cbz");
        assert_eq!(book.get_format(), Some(FileFormat::CBZ));
    }

    #[test]
    fn test_page_image_format_conversion() {
        let mut page = Page::new(
            Uuid::new_v4(),
            1,
            "page001.jpg".to_string(),
        );

        page.set_image_format(ImageFormat::JPEG);
        assert_eq!(page.format, "jpeg");
        assert_eq!(page.get_image_format(), Some(ImageFormat::JPEG));
    }

    #[test]
    fn test_user_new() {
        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hashed_password".to_string(),
        );

        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert!(!user.is_admin);
        assert!(user.last_login_at.is_none());
    }

    #[test]
    fn test_read_progress_new() {
        let progress = ReadProgress::new(Uuid::new_v4(), Uuid::new_v4());

        assert_eq!(progress.current_page, 1);
        assert!(!progress.completed);
        assert!(progress.completed_at.is_none());
    }
}

