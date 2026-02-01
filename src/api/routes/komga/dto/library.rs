//! Komga-compatible library DTOs
//!
//! These DTOs match the exact structure Komic expects from Komga's `/api/v1/libraries` endpoint.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::default_true;

/// Komga library DTO
///
/// Based on actual Komic traffic analysis - includes all fields observed in responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaLibraryDto {
    /// Library unique identifier (UUID as string)
    pub id: String,
    /// Library display name
    pub name: String,
    /// Root filesystem path
    pub root: String,
    /// Whether to analyze page dimensions
    #[serde(default)]
    pub analyze_dimensions: bool,
    /// Whether to convert archives to CBZ
    #[serde(default)]
    pub convert_to_cbz: bool,
    /// Whether to empty trash after scan
    #[serde(default)]
    pub empty_trash_after_scan: bool,
    /// Whether to hash files for deduplication
    #[serde(default)]
    pub hash_files: bool,
    /// Whether to hash files for KOReader sync
    #[serde(default)]
    pub hash_koreader: bool,
    /// Whether to hash pages
    #[serde(default)]
    pub hash_pages: bool,
    /// Whether to import barcode/ISBN
    #[serde(default)]
    pub import_barcode_isbn: bool,
    /// Whether to import book info from ComicInfo.xml
    #[serde(default)]
    pub import_comic_info_book: bool,
    /// Whether to import collection info from ComicInfo.xml
    #[serde(default)]
    pub import_comic_info_collection: bool,
    /// Whether to import read list from ComicInfo.xml
    #[serde(default)]
    pub import_comic_info_read_list: bool,
    /// Whether to import series info from ComicInfo.xml
    #[serde(default)]
    pub import_comic_info_series: bool,
    /// Whether to append volume to series name from ComicInfo
    #[serde(default)]
    pub import_comic_info_series_append_volume: bool,
    /// Whether to import EPUB book metadata
    #[serde(default)]
    pub import_epub_book: bool,
    /// Whether to import EPUB series metadata
    #[serde(default)]
    pub import_epub_series: bool,
    /// Whether to import local artwork
    #[serde(default)]
    pub import_local_artwork: bool,
    /// Whether to import Mylar series data
    #[serde(default)]
    pub import_mylar_series: bool,
    /// Directory path for oneshots (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oneshots_directory: Option<String>,
    /// Whether to repair file extensions
    #[serde(default)]
    pub repair_extensions: bool,
    /// Whether to scan CBZ/CBR files
    #[serde(default = "default_true")]
    pub scan_cbx: bool,
    /// Directory exclusion patterns
    #[serde(default)]
    pub scan_directory_exclusions: Vec<String>,
    /// Whether to scan EPUB files
    #[serde(default = "default_true")]
    pub scan_epub: bool,
    /// Whether to force modified time for scan
    #[serde(default)]
    pub scan_force_modified_time: bool,
    /// Scan interval (WEEKLY, DAILY, HOURLY, EVERY_6H, EVERY_12H, DISABLED)
    #[serde(default = "default_scan_interval")]
    pub scan_interval: String,
    /// Whether to scan on startup
    #[serde(default)]
    pub scan_on_startup: bool,
    /// Whether to scan PDF files
    #[serde(default = "default_true")]
    pub scan_pdf: bool,
    /// Series cover selection strategy (FIRST, FIRST_UNREAD_OR_FIRST, FIRST_UNREAD_OR_LAST, LAST)
    #[serde(default = "default_series_cover")]
    pub series_cover: String,
    /// Whether library is unavailable (path doesn't exist)
    #[serde(default)]
    pub unavailable: bool,
}

fn default_scan_interval() -> String {
    "WEEKLY".to_string()
}

fn default_series_cover() -> String {
    "FIRST".to_string()
}

impl Default for KomgaLibraryDto {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            root: String::new(),
            analyze_dimensions: false,
            convert_to_cbz: false,
            empty_trash_after_scan: false,
            hash_files: true, // Codex does hash files by default
            hash_koreader: false,
            hash_pages: false,
            import_barcode_isbn: false,
            import_comic_info_book: true,
            import_comic_info_collection: false,
            import_comic_info_read_list: false,
            import_comic_info_series: true,
            import_comic_info_series_append_volume: false,
            import_epub_book: true,
            import_epub_series: true,
            import_local_artwork: true,
            import_mylar_series: false,
            oneshots_directory: None,
            repair_extensions: false,
            scan_cbx: true,
            scan_directory_exclusions: Vec::new(),
            scan_epub: true,
            scan_force_modified_time: false,
            scan_interval: default_scan_interval(),
            scan_on_startup: false,
            scan_pdf: true,
            series_cover: default_series_cover(),
            unavailable: false,
        }
    }
}

impl KomgaLibraryDto {
    /// Create a new KomgaLibraryDto from Codex library data
    pub fn from_codex(
        id: uuid::Uuid,
        name: &str,
        path: &str,
        is_active: bool,
        excluded_patterns: Option<&str>,
    ) -> Self {
        let scan_directory_exclusions = excluded_patterns
            .map(|p| p.lines().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Self {
            id: id.to_string(),
            name: name.to_string(),
            root: path.to_string(),
            unavailable: !is_active,
            scan_directory_exclusions,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_library_dto_serialization() {
        let library = KomgaLibraryDto {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "Comics".to_string(),
            root: "/media/comics".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&library).unwrap();
        assert!(json.contains("\"id\":\"550e8400-e29b-41d4-a716-446655440000\""));
        assert!(json.contains("\"name\":\"Comics\""));
        assert!(json.contains("\"root\":\"/media/comics\""));
        assert!(json.contains("\"scanCbx\":true"));
        assert!(json.contains("\"scanEpub\":true"));
        assert!(json.contains("\"scanPdf\":true"));
    }

    #[test]
    fn test_library_dto_camel_case() {
        let library = KomgaLibraryDto::default();
        let json = serde_json::to_string(&library).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"analyzeDimensions\""));
        assert!(json.contains("\"convertToCbz\""));
        assert!(json.contains("\"emptyTrashAfterScan\""));
        assert!(json.contains("\"hashFiles\""));
        assert!(json.contains("\"hashKoreader\""));
        assert!(json.contains("\"hashPages\""));
        assert!(json.contains("\"importBarcodeIsbn\""));
        assert!(json.contains("\"importComicInfoBook\""));
        assert!(json.contains("\"scanDirectoryExclusions\""));
        assert!(json.contains("\"seriesCover\""));
    }

    #[test]
    fn test_library_dto_from_codex() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let library = KomgaLibraryDto::from_codex(
            id,
            "My Comics",
            "/home/user/comics",
            true,
            Some(".DS_Store\nThumbs.db"),
        );

        assert_eq!(library.id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(library.name, "My Comics");
        assert_eq!(library.root, "/home/user/comics");
        assert!(!library.unavailable);
        assert_eq!(
            library.scan_directory_exclusions,
            vec![".DS_Store", "Thumbs.db"]
        );
    }

    #[test]
    fn test_library_dto_unavailable_when_inactive() {
        let id = uuid::Uuid::new_v4();
        let library = KomgaLibraryDto::from_codex(id, "Inactive", "/path", false, None);

        assert!(library.unavailable);
    }

    #[test]
    fn test_library_dto_deserialization() {
        let json = r#"{
            "id": "test-id",
            "name": "Test Library",
            "root": "/test/path",
            "analyzeDimensions": false,
            "convertToCbz": false,
            "emptyTrashAfterScan": false,
            "hashFiles": true,
            "hashKoreader": false,
            "hashPages": false,
            "importBarcodeIsbn": false,
            "importComicInfoBook": true,
            "importComicInfoCollection": false,
            "importComicInfoReadList": false,
            "importComicInfoSeries": true,
            "importComicInfoSeriesAppendVolume": false,
            "importEpubBook": true,
            "importEpubSeries": true,
            "importLocalArtwork": true,
            "importMylarSeries": false,
            "repairExtensions": false,
            "scanCbx": true,
            "scanDirectoryExclusions": [],
            "scanEpub": true,
            "scanForceModifiedTime": false,
            "scanInterval": "WEEKLY",
            "scanOnStartup": false,
            "scanPdf": true,
            "seriesCover": "FIRST",
            "unavailable": false
        }"#;

        let library: KomgaLibraryDto = serde_json::from_str(json).unwrap();
        assert_eq!(library.id, "test-id");
        assert_eq!(library.name, "Test Library");
        assert_eq!(library.root, "/test/path");
        assert!(library.scan_cbx);
    }
}
