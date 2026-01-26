//! Metadata types for parsed book content
//!
//! TODO: Remove allow(dead_code) once all metadata features are fully integrated

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;

/// File format type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
pub enum FileFormat {
    CBZ,
    CBR,
    EPUB,
    PDF,
}

/// Result of detecting file format from bytes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileFormatDetection {
    /// A supported file format was detected
    Supported(FileFormat),
    /// An unsupported file format was detected (includes MIME type for logging)
    Unsupported(String),
    /// Could not determine the format from the bytes
    Unknown,
}

impl FileFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "cbz" => Some(FileFormat::CBZ),
            "cbr" => Some(FileFormat::CBR),
            "epub" => Some(FileFormat::EPUB),
            "pdf" => Some(FileFormat::PDF),
            _ => None,
        }
    }

    /// Detect file format from a file path by reading its magic bytes
    ///
    /// This reads the first few bytes of the file to detect the format.
    /// Falls back to extension-based detection if magic bytes are inconclusive.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path = path.as_ref();

        // Try to read magic bytes first
        if let Ok(data) = std::fs::read(path) {
            if let FileFormatDetection::Supported(format) = Self::detect_from_bytes(&data) {
                return Some(format);
            }
        }

        // Fall back to extension
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }

    /// Detect file format from raw bytes using magic byte detection
    ///
    /// Uses the `infer` crate to detect the format from file signatures.
    /// Note: CBZ and EPUB are both ZIP-based formats and require additional
    /// heuristics or extension hints to distinguish.
    pub fn detect_from_bytes(data: &[u8]) -> FileFormatDetection {
        match infer::get(data) {
            Some(kind) => match kind.mime_type() {
                "application/pdf" => FileFormatDetection::Supported(FileFormat::PDF),
                "application/x-rar-compressed" | "application/vnd.rar" => {
                    FileFormatDetection::Supported(FileFormat::CBR)
                }
                "application/zip" => {
                    // ZIP-based formats need additional checks
                    // EPUB has specific structure, CBZ is generic ZIP with images
                    if Self::is_epub_zip(data) {
                        FileFormatDetection::Supported(FileFormat::EPUB)
                    } else {
                        // Default to CBZ for other ZIP files
                        FileFormatDetection::Supported(FileFormat::CBZ)
                    }
                }
                "application/epub+zip" => FileFormatDetection::Supported(FileFormat::EPUB),
                // Any other format is unsupported
                mime => {
                    tracing::debug!(
                        detected_mime = %mime,
                        "Unsupported file format detected"
                    );
                    FileFormatDetection::Unsupported(mime.to_string())
                }
            },
            None => FileFormatDetection::Unknown,
        }
    }

    /// Check if a ZIP file is an EPUB by looking for the mimetype file
    ///
    /// EPUB files must have a "mimetype" file as the first entry containing
    /// "application/epub+zip".
    fn is_epub_zip(data: &[u8]) -> bool {
        // EPUB spec requires mimetype file to be uncompressed and first in archive
        // The mimetype content starts at byte 38 in a valid EPUB
        // We look for "mimetypeapplication/epub+zip" pattern

        // Quick check: look for "mimetype" followed by "application/epub+zip"
        // This is a heuristic that works for most EPUBs without full ZIP parsing
        if data.len() < 60 {
            return false;
        }

        // Search for the pattern in the first 100 bytes
        let search_range = std::cmp::min(data.len(), 100);
        let search_data = &data[..search_range];

        // Look for "mimetype" marker
        if let Some(pos) = search_data.windows(8).position(|w| w == b"mimetype") {
            // Check if "application/epub+zip" follows within reasonable distance
            let start = pos + 8;
            let end = std::cmp::min(data.len(), start + 50);
            if end > start {
                let content_area = &data[start..end];
                return content_area
                    .windows(20)
                    .any(|w| w == b"application/epub+zip");
            }
        }

        false
    }
}

/// Reading direction for books and series
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum ReadingDirection {
    /// Left to right (Western comics, most books)
    #[default]
    LeftToRight,
    /// Right to left (Manga, some Asian comics)
    RightToLeft,
    /// Top to bottom (Webtoons, vertical scrolling)
    TopToBottom,
}

impl ReadingDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReadingDirection::LeftToRight => "LEFT_TO_RIGHT",
            ReadingDirection::RightToLeft => "RIGHT_TO_LEFT",
            ReadingDirection::TopToBottom => "TOP_TO_BOTTOM",
        }
    }
}

impl FromStr for ReadingDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LEFT_TO_RIGHT" | "LTR" => Ok(ReadingDirection::LeftToRight),
            "RIGHT_TO_LEFT" | "RTL" => Ok(ReadingDirection::RightToLeft),
            "TOP_TO_BOTTOM" | "TTB" | "VERTICAL" => Ok(ReadingDirection::TopToBottom),
            _ => Err(format!("Unknown reading direction: {}", s)),
        }
    }
}

/// Image format type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
pub enum ImageFormat {
    JPEG,
    PNG,
    WEBP,
    GIF,
    AVIF,
    BMP,
    /// SVG images - note that dimensions cannot be easily determined without rendering
    SVG,
    /// JPEG XL images - decoded using jxl-oxide
    JXL,
}

/// Page information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// Page number (1-indexed)
    pub page_number: usize,
    /// File name within archive
    pub file_name: String,
    /// Image format
    pub format: ImageFormat,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// File size in bytes
    pub file_size: u64,
}

/// ComicInfo.xml metadata (subset of fields)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,
    pub count: Option<i32>,
    pub volume: Option<i32>,
    pub summary: Option<String>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
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
    pub page_count: Option<i32>,
    pub language_iso: Option<String>,
    pub format: Option<String>,
    pub black_and_white: Option<String>,
    pub manga: Option<String>,
}

/// Complete book metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMetadata {
    /// File path
    pub file_path: String,
    /// File format
    pub format: FileFormat,
    /// File size in bytes
    pub file_size: u64,
    /// SHA-256 hash
    pub file_hash: String,
    /// Last modification time
    pub modified_at: DateTime<Utc>,
    /// Total page count
    pub page_count: usize,
    /// Page information
    pub pages: Vec<PageInfo>,
    /// ComicInfo.xml metadata (if available)
    pub comic_info: Option<ComicInfo>,
    /// Detected ISBNs/barcodes
    pub isbns: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_format_from_extension_cbz() {
        assert_eq!(FileFormat::from_extension("cbz"), Some(FileFormat::CBZ));
        assert_eq!(FileFormat::from_extension("CBZ"), Some(FileFormat::CBZ));
        assert_eq!(FileFormat::from_extension("CbZ"), Some(FileFormat::CBZ));
    }

    #[test]
    fn test_file_format_from_extension_cbr() {
        assert_eq!(FileFormat::from_extension("cbr"), Some(FileFormat::CBR));
        assert_eq!(FileFormat::from_extension("CBR"), Some(FileFormat::CBR));
    }

    #[test]
    fn test_file_format_from_extension_epub() {
        assert_eq!(FileFormat::from_extension("epub"), Some(FileFormat::EPUB));
        assert_eq!(FileFormat::from_extension("EPUB"), Some(FileFormat::EPUB));
    }

    #[test]
    fn test_file_format_from_extension_pdf() {
        assert_eq!(FileFormat::from_extension("pdf"), Some(FileFormat::PDF));
        assert_eq!(FileFormat::from_extension("PDF"), Some(FileFormat::PDF));
    }

    #[test]
    fn test_file_format_from_extension_invalid() {
        assert_eq!(FileFormat::from_extension("txt"), None);
        assert_eq!(FileFormat::from_extension("zip"), None);
        assert_eq!(FileFormat::from_extension(""), None);
        assert_eq!(FileFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_file_format_equality() {
        assert_eq!(FileFormat::CBZ, FileFormat::CBZ);
        assert_ne!(FileFormat::CBZ, FileFormat::CBR);
        assert_ne!(FileFormat::EPUB, FileFormat::PDF);
    }

    #[test]
    fn test_file_format_serialization() {
        let format = FileFormat::CBZ;
        let json = serde_json::to_string(&format).unwrap();
        assert_eq!(json, r#""cbz""#);

        let format = FileFormat::PDF;
        let json = serde_json::to_string(&format).unwrap();
        assert_eq!(json, r#""pdf""#);
    }

    #[test]
    fn test_file_format_deserialization() {
        let format: FileFormat = serde_json::from_str(r#""cbz""#).unwrap();
        assert_eq!(format, FileFormat::CBZ);

        let format: FileFormat = serde_json::from_str(r#""epub""#).unwrap();
        assert_eq!(format, FileFormat::EPUB);
    }

    #[test]
    fn test_image_format_equality() {
        assert_eq!(ImageFormat::JPEG, ImageFormat::JPEG);
        assert_ne!(ImageFormat::PNG, ImageFormat::JPEG);
    }

    #[test]
    fn test_comic_info_default() {
        let info = ComicInfo::default();
        assert!(info.title.is_none());
        assert!(info.series.is_none());
        assert!(info.publisher.is_none());
    }

    #[test]
    fn test_page_info_creation() {
        let page = PageInfo {
            page_number: 1,
            file_name: "page001.jpg".to_string(),
            format: ImageFormat::JPEG,
            width: 1920,
            height: 1080,
            file_size: 512000,
        };

        assert_eq!(page.page_number, 1);
        assert_eq!(page.file_name, "page001.jpg");
        assert_eq!(page.format, ImageFormat::JPEG);
        assert_eq!(page.width, 1920);
        assert_eq!(page.height, 1080);
        assert_eq!(page.file_size, 512000);
    }

    #[test]
    fn test_reading_direction_default() {
        let direction = ReadingDirection::default();
        assert_eq!(direction, ReadingDirection::LeftToRight);
    }

    #[test]
    fn test_reading_direction_from_str() {
        // Test standard formats
        assert_eq!(
            "LEFT_TO_RIGHT".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::LeftToRight
        );
        assert_eq!(
            "RIGHT_TO_LEFT".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::RightToLeft
        );
        assert_eq!(
            "TOP_TO_BOTTOM".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::TopToBottom
        );

        // Test short forms
        assert_eq!(
            "LTR".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::LeftToRight
        );
        assert_eq!(
            "RTL".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::RightToLeft
        );
        assert_eq!(
            "TTB".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::TopToBottom
        );

        // Test aliases
        assert_eq!(
            "VERTICAL".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::TopToBottom
        );

        // Test case insensitivity
        assert_eq!(
            "left_to_right".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::LeftToRight
        );
        assert_eq!(
            "ltr".parse::<ReadingDirection>().unwrap(),
            ReadingDirection::LeftToRight
        );

        // Test invalid input
        assert!("invalid".parse::<ReadingDirection>().is_err());
        assert!("".parse::<ReadingDirection>().is_err());
    }

    #[test]
    fn test_reading_direction_as_str() {
        assert_eq!(ReadingDirection::LeftToRight.as_str(), "LEFT_TO_RIGHT");
        assert_eq!(ReadingDirection::RightToLeft.as_str(), "RIGHT_TO_LEFT");
        assert_eq!(ReadingDirection::TopToBottom.as_str(), "TOP_TO_BOTTOM");
    }

    #[test]
    fn test_reading_direction_serialization() {
        let direction = ReadingDirection::LeftToRight;
        let json = serde_json::to_string(&direction).unwrap();
        assert_eq!(json, r#""LEFT_TO_RIGHT""#);

        let direction = ReadingDirection::RightToLeft;
        let json = serde_json::to_string(&direction).unwrap();
        assert_eq!(json, r#""RIGHT_TO_LEFT""#);

        let direction = ReadingDirection::TopToBottom;
        let json = serde_json::to_string(&direction).unwrap();
        assert_eq!(json, r#""TOP_TO_BOTTOM""#);
    }

    #[test]
    fn test_reading_direction_deserialization() {
        let direction: ReadingDirection = serde_json::from_str(r#""LEFT_TO_RIGHT""#).unwrap();
        assert_eq!(direction, ReadingDirection::LeftToRight);

        let direction: ReadingDirection = serde_json::from_str(r#""RIGHT_TO_LEFT""#).unwrap();
        assert_eq!(direction, ReadingDirection::RightToLeft);

        let direction: ReadingDirection = serde_json::from_str(r#""TOP_TO_BOTTOM""#).unwrap();
        assert_eq!(direction, ReadingDirection::TopToBottom);
    }

    #[test]
    fn test_reading_direction_equality() {
        assert_eq!(ReadingDirection::LeftToRight, ReadingDirection::LeftToRight);
        assert_ne!(ReadingDirection::LeftToRight, ReadingDirection::RightToLeft);
        assert_ne!(ReadingDirection::RightToLeft, ReadingDirection::TopToBottom);
    }

    mod detect_from_bytes {
        use super::*;

        #[test]
        fn test_pdf_magic_bytes() {
            // PDF starts with %PDF-
            let pdf_data = b"%PDF-1.4\n...";
            assert_eq!(
                FileFormat::detect_from_bytes(pdf_data),
                FileFormatDetection::Supported(FileFormat::PDF)
            );
        }

        #[test]
        fn test_zip_detected_as_cbz() {
            // ZIP magic bytes: PK\x03\x04
            // Generic ZIP (no EPUB mimetype) should be detected as CBZ
            let zip_data = [
                0x50, 0x4B, 0x03, 0x04, // PK signature
                0x14, 0x00, 0x00, 0x00, // version, flags
                0x00, 0x00, // compression method
                0x00, 0x00, 0x00, 0x00, // file time/date
                0x00, 0x00, 0x00, 0x00, // CRC-32
                0x00, 0x00, 0x00, 0x00, // compressed size
                0x00, 0x00, 0x00, 0x00, // uncompressed size
                0x08, 0x00, // filename length (8)
                0x00, 0x00, // extra field length
                b't', b'e', b's', b't', b'.', b'j', b'p', b'g', // filename: "test.jpg"
            ];
            assert_eq!(
                FileFormat::detect_from_bytes(&zip_data),
                FileFormatDetection::Supported(FileFormat::CBZ)
            );
        }

        #[test]
        fn test_epub_zip_detected() {
            // EPUB is a ZIP with "mimetype" as first file containing "application/epub+zip"
            // This is a simplified EPUB header
            let epub_data = [
                0x50, 0x4B, 0x03, 0x04, // PK signature
                0x14, 0x00, 0x00, 0x00, // version, flags
                0x00, 0x00, // compression method (stored, no compression)
                0x00, 0x00, 0x00, 0x00, // file time/date
                0x00, 0x00, 0x00, 0x00, // CRC-32
                0x14, 0x00, 0x00, 0x00, // compressed size (20)
                0x14, 0x00, 0x00, 0x00, // uncompressed size (20)
                0x08, 0x00, // filename length (8)
                0x00, 0x00, // extra field length
                b'm', b'i', b'm', b'e', b't', b'y', b'p', b'e', // filename: "mimetype"
                b'a', b'p', b'p', b'l', b'i', b'c', b'a', b't', b'i', b'o', b'n', b'/', b'e', b'p',
                b'u', b'b', b'+', b'z', b'i', b'p', // content: "application/epub+zip"
            ];
            assert_eq!(
                FileFormat::detect_from_bytes(&epub_data),
                FileFormatDetection::Supported(FileFormat::EPUB)
            );
        }

        #[test]
        fn test_rar_magic_bytes() {
            // RAR5 magic bytes: Rar!\x1a\x07\x01\x00
            let rar_data = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];
            assert_eq!(
                FileFormat::detect_from_bytes(&rar_data),
                FileFormatDetection::Supported(FileFormat::CBR)
            );
        }

        #[test]
        fn test_rar4_magic_bytes() {
            // RAR4 magic bytes: Rar!\x1a\x07\x00
            let rar_data = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00];
            assert_eq!(
                FileFormat::detect_from_bytes(&rar_data),
                FileFormatDetection::Supported(FileFormat::CBR)
            );
        }

        #[test]
        fn test_unsupported_format() {
            // JPEG magic bytes - image format, not a document format
            let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0];
            let result = FileFormat::detect_from_bytes(&jpeg_data);
            assert!(matches!(result, FileFormatDetection::Unsupported(_)));
            if let FileFormatDetection::Unsupported(mime) = result {
                assert_eq!(mime, "image/jpeg");
            }
        }

        #[test]
        fn test_unknown_format() {
            // Random bytes that don't match any known format
            let unknown_data = [0x12, 0x34, 0x56, 0x78];
            assert_eq!(
                FileFormat::detect_from_bytes(&unknown_data),
                FileFormatDetection::Unknown
            );
        }

        #[test]
        fn test_empty_data() {
            assert_eq!(
                FileFormat::detect_from_bytes(&[]),
                FileFormatDetection::Unknown
            );
        }
    }
}
