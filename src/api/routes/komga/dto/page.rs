//! Komga-compatible page DTOs
//!
//! These DTOs match the exact structure Komic expects from Komga's page endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::book::format_file_size;

/// Komga page DTO
///
/// Represents a single page within a book.
/// Based on actual Komic traffic analysis for GET /api/v1/books/{id}/pages
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaPageDto {
    /// Original filename within archive
    pub file_name: String,
    /// MIME type (e.g., "image/png", "image/jpeg", "image/webp")
    pub media_type: String,
    /// Page number (1-indexed)
    pub number: i32,
    /// Image width in pixels
    pub width: i32,
    /// Image height in pixels
    pub height: i32,
    /// Page file size in bytes
    pub size_bytes: i64,
    /// Human-readable file size (e.g., "2.5 MiB")
    pub size: String,
}

impl Default for KomgaPageDto {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            media_type: "image/jpeg".to_string(),
            number: 1,
            width: 0,
            height: 0,
            size_bytes: 0,
            size: "0 B".to_string(),
        }
    }
}

impl KomgaPageDto {
    /// Create a new page DTO
    #[allow(dead_code)]
    pub fn new(
        file_name: String,
        media_type: String,
        number: i32,
        width: i32,
        height: i32,
        size_bytes: i64,
    ) -> Self {
        Self {
            file_name,
            media_type,
            number,
            width,
            height,
            size_bytes,
            size: format_file_size(size_bytes),
        }
    }

    /// Create from Codex page info
    pub fn from_codex(
        file_name: &str,
        number: i32,
        width: Option<i32>,
        height: Option<i32>,
        size_bytes: Option<i64>,
        media_type: Option<&str>,
    ) -> Self {
        let media_type = media_type
            .map(|s| s.to_string())
            .unwrap_or_else(|| guess_media_type(file_name));

        Self {
            file_name: file_name.to_string(),
            media_type,
            number,
            width: width.unwrap_or(0),
            height: height.unwrap_or(0),
            size_bytes: size_bytes.unwrap_or(0),
            size: format_file_size(size_bytes.unwrap_or(0)),
        }
    }
}

/// Guess MIME type from filename extension
fn guess_media_type(filename: &str) -> String {
    let lower = filename.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.ends_with(".png") {
        "image/png".to_string()
    } else if lower.ends_with(".gif") {
        "image/gif".to_string()
    } else if lower.ends_with(".webp") {
        "image/webp".to_string()
    } else if lower.ends_with(".avif") {
        "image/avif".to_string()
    } else if lower.ends_with(".bmp") {
        "image/bmp".to_string()
    } else if lower.ends_with(".tiff") || lower.ends_with(".tif") {
        "image/tiff".to_string()
    } else {
        "image/jpeg".to_string() // Default to JPEG
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_page_dto_serialization() {
        let page = KomgaPageDto {
            file_name: "page001.jpg".to_string(),
            media_type: "image/jpeg".to_string(),
            number: 1,
            width: 1920,
            height: 2560,
            size_bytes: 2621440,
            size: "2.5 MiB".to_string(),
        };

        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"fileName\":\"page001.jpg\""));
        assert!(json.contains("\"mediaType\":\"image/jpeg\""));
        assert!(json.contains("\"number\":1"));
        assert!(json.contains("\"width\":1920"));
        assert!(json.contains("\"height\":2560"));
        assert!(json.contains("\"sizeBytes\":2621440"));
        assert!(json.contains("\"size\":\"2.5 MiB\""));
    }

    #[test]
    fn test_page_dto_camel_case() {
        let page = KomgaPageDto::default();
        let json = serde_json::to_string(&page).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"fileName\""));
        assert!(json.contains("\"mediaType\""));
        assert!(json.contains("\"sizeBytes\""));
    }

    #[test]
    fn test_page_dto_new() {
        let page = KomgaPageDto::new(
            "chapter1/001.png".to_string(),
            "image/png".to_string(),
            1,
            1200,
            1800,
            1048576,
        );

        assert_eq!(page.file_name, "chapter1/001.png");
        assert_eq!(page.media_type, "image/png");
        assert_eq!(page.number, 1);
        assert_eq!(page.width, 1200);
        assert_eq!(page.height, 1800);
        assert_eq!(page.size_bytes, 1048576);
        assert_eq!(page.size, "1.0 MiB");
    }

    #[test]
    fn test_page_dto_from_codex() {
        let page =
            KomgaPageDto::from_codex("image.jpg", 5, Some(800), Some(1200), Some(512000), None);

        assert_eq!(page.number, 5);
        assert_eq!(page.width, 800);
        assert_eq!(page.height, 1200);
        assert_eq!(page.media_type, "image/jpeg");
        assert_eq!(page.size_bytes, 512000);
    }

    #[test]
    fn test_page_dto_from_codex_with_media_type() {
        let page = KomgaPageDto::from_codex(
            "image.webp",
            1,
            Some(1000),
            Some(1500),
            Some(100000),
            Some("image/webp"),
        );

        assert_eq!(page.media_type, "image/webp");
    }

    #[test]
    fn test_page_dto_from_codex_defaults() {
        let page = KomgaPageDto::from_codex("unknown.img", 1, None, None, None, None);

        assert_eq!(page.width, 0);
        assert_eq!(page.height, 0);
        assert_eq!(page.size_bytes, 0);
        assert_eq!(page.size, "0 B");
        assert_eq!(page.media_type, "image/jpeg"); // Default
    }

    #[test]
    fn test_guess_media_type() {
        assert_eq!(guess_media_type("image.jpg"), "image/jpeg");
        assert_eq!(guess_media_type("image.JPEG"), "image/jpeg");
        assert_eq!(guess_media_type("image.png"), "image/png");
        assert_eq!(guess_media_type("image.PNG"), "image/png");
        assert_eq!(guess_media_type("image.gif"), "image/gif");
        assert_eq!(guess_media_type("image.webp"), "image/webp");
        assert_eq!(guess_media_type("image.avif"), "image/avif");
        assert_eq!(guess_media_type("image.bmp"), "image/bmp");
        assert_eq!(guess_media_type("image.tiff"), "image/tiff");
        assert_eq!(guess_media_type("image.tif"), "image/tiff");
        assert_eq!(guess_media_type("unknown.xyz"), "image/jpeg"); // Default
    }

    #[test]
    fn test_page_dto_deserialization() {
        let json = r#"{
            "fileName": "test.jpg",
            "mediaType": "image/jpeg",
            "number": 42,
            "width": 1000,
            "height": 1500,
            "sizeBytes": 500000,
            "size": "488.3 KiB"
        }"#;

        let page: KomgaPageDto = serde_json::from_str(json).unwrap();
        assert_eq!(page.file_name, "test.jpg");
        assert_eq!(page.number, 42);
        assert_eq!(page.width, 1000);
        assert_eq!(page.height, 1500);
    }
}
