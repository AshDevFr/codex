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
        if let Ok(data) = std::fs::read(path)
            && let FileFormatDetection::Supported(format) = Self::detect_from_bytes(&data)
        {
            return Some(format);
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
    /// Chapter number derived from ComicInfo `<Number>`. ComicInfo overloads
    /// `<Number>` (issue / chapter / part) — Phase 12 of metadata-count-split
    /// reads it as a chapter unconditionally and lets users lock the field if
    /// their files use `<Number>` for issues instead.
    pub chapter: Option<f32>,
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
    /// Structured author information as JSON array, computed from individual role fields
    /// Format: [{"name": "...", "role": "writer|penciller|inker|..."}]
    pub authors_json: Option<String>,
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

/// A single position in the Readium positions list for EPUB books.
///
/// Positions are computed using the Readium algorithm (1 position per 1024 bytes
/// of each spine resource). This provides a canonical coordinate system for
/// cross-app reading position sync, matching Komga's implementation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpubPosition {
    /// Resource href within the EPUB (e.g., "OEBPS/chapter1.xhtml")
    pub href: String,
    /// Media type of the resource (e.g., "application/xhtml+xml")
    pub media_type: String,
    /// Progression within the resource (0.0-1.0)
    pub progression: f64,
    /// Sequential position number (1-based) across the entire book
    pub position: i32,
    /// Overall progression within the entire book (0.0-1.0)
    pub total_progression: f64,
}

/// Spine item extracted from the EPUB OPF manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpineItem {
    /// Full path within the EPUB archive
    pub href: String,
    /// Media type from the manifest
    pub media_type: String,
    /// Uncompressed file size in bytes
    pub file_size: u64,
    /// Number of text characters (excluding HTML markup, scripts, styles)
    pub char_count: u64,
}

/// Compute Readium positions list from spine items.
///
/// Uses the Readium algorithm: 1 position per 1024 bytes of each spine resource.
/// This matches Komga's implementation for cross-app compatibility.
pub fn compute_epub_positions(spine_items: &[SpineItem]) -> Vec<EpubPosition> {
    let mut positions = Vec::new();
    let mut next_position: i32 = 1;

    for item in spine_items {
        let position_count = (item.file_size as f64 / 1024.0).ceil().max(1.0) as usize;

        for p in 0..position_count {
            let progression = p as f64 / position_count as f64;
            positions.push(EpubPosition {
                href: item.href.clone(),
                media_type: item.media_type.clone(),
                progression,
                position: next_position,
                total_progression: 0.0, // computed below
            });
            next_position += 1;
        }
    }

    // Compute total_progression for each position
    let total = positions.len() as f64;
    for pos in &mut positions {
        pos.total_progression = pos.position as f64 / total;
    }

    positions
}

/// Normalize a client's totalProgression using the server's positions list.
///
/// Given the client's `href` and `total_progression`, finds the closest matching
/// position in the server's positions list and returns its authoritative
/// `total_progression` value along with the derived page number.
///
/// Returns `None` if positions is empty or href doesn't match any position.
pub fn normalize_progression(
    positions: &[EpubPosition],
    client_href: &str,
    client_total_progression: f64,
) -> Option<(f64, i32)> {
    if positions.is_empty() {
        return None;
    }

    // Strip fragment from href and URL-decode
    let href_clean = client_href.split('#').next().unwrap_or(client_href);
    let href_decoded = urlencoding::decode(href_clean).unwrap_or_else(|_| href_clean.into());

    // Find positions matching the href (try exact match, then suffix match)
    let matching: Vec<&EpubPosition> = positions
        .iter()
        .filter(|p| {
            p.href == href_decoded.as_ref()
                || href_decoded.ends_with(&p.href)
                || p.href.ends_with(href_decoded.as_ref())
        })
        .collect();

    if matching.is_empty() {
        // No href match; fall back to closest totalProgression across all positions
        let closest = positions.iter().min_by(|a, b| {
            let da = (a.total_progression - client_total_progression).abs();
            let db = (b.total_progression - client_total_progression).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })?;
        return Some((closest.total_progression, closest.position));
    }

    // Among matching positions, find the one closest to client's totalProgression
    let closest = matching.iter().min_by(|a, b| {
        let da = (a.total_progression - client_total_progression).abs();
        let db = (b.total_progression - client_total_progression).abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })?;

    Some((closest.total_progression, closest.position))
}

/// Convert character-based totalProgression (epub.js) to byte-based (Readium canonical).
///
/// epub.js divides the book into character-weighted chunks, while Readium uses
/// byte-weighted chunks. For multi-byte content (CJK, accented text), these
/// diverge significantly. This function maps a char-based percentage to the
/// equivalent byte-based percentage using per-spine-item char/byte counts.
pub fn char_to_byte_progression(spine_items: &[SpineItem], char_prog: f64) -> f64 {
    if spine_items.is_empty() {
        return char_prog;
    }

    let total_chars: u64 = spine_items.iter().map(|s| s.char_count.max(1)).sum();
    let total_bytes: u64 = spine_items.iter().map(|s| s.file_size).sum();

    if total_chars == 0 || total_bytes == 0 {
        return char_prog;
    }

    let target_chars = (char_prog * total_chars as f64) as u64;
    let mut accumulated_chars: u64 = 0;
    let mut accumulated_bytes: u64 = 0;

    for item in spine_items {
        let item_chars = item.char_count.max(1);
        if accumulated_chars + item_chars >= target_chars {
            let within_item_frac = if item_chars > 0 {
                (target_chars - accumulated_chars) as f64 / item_chars as f64
            } else {
                0.0
            };
            let byte_offset = accumulated_bytes as f64 + within_item_frac * item.file_size as f64;
            return (byte_offset / total_bytes as f64).clamp(0.0, 1.0);
        }
        accumulated_chars += item_chars;
        accumulated_bytes += item.file_size;
    }

    1.0
}

/// Convert byte-based totalProgression (Readium/KOReader) to character-based (epub.js).
///
/// Inverse of `char_to_byte_progression`. Maps a byte-weighted percentage to
/// the equivalent character-weighted percentage.
pub fn byte_to_char_progression(spine_items: &[SpineItem], byte_prog: f64) -> f64 {
    if spine_items.is_empty() {
        return byte_prog;
    }

    let total_chars: u64 = spine_items.iter().map(|s| s.char_count.max(1)).sum();
    let total_bytes: u64 = spine_items.iter().map(|s| s.file_size).sum();

    if total_chars == 0 || total_bytes == 0 {
        return byte_prog;
    }

    let target_bytes = (byte_prog * total_bytes as f64) as u64;
    let mut accumulated_chars: u64 = 0;
    let mut accumulated_bytes: u64 = 0;

    for item in spine_items {
        if accumulated_bytes + item.file_size >= target_bytes {
            let within_item_frac = if item.file_size > 0 {
                (target_bytes - accumulated_bytes) as f64 / item.file_size as f64
            } else {
                0.0
            };
            let char_offset =
                accumulated_chars as f64 + within_item_frac * item.char_count.max(1) as f64;
            return (char_offset / total_chars as f64).clamp(0.0, 1.0);
        }
        accumulated_chars += item.char_count.max(1);
        accumulated_bytes += item.file_size;
    }

    1.0
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
    /// EPUB Readium positions list (only for EPUB format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epub_positions: Option<Vec<EpubPosition>>,
    /// EPUB spine items with byte/char counts (for cross-device sync normalization)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epub_spine_items: Option<Vec<SpineItem>>,
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

    mod epub_positions {
        use super::*;

        fn sample_spine() -> Vec<SpineItem> {
            vec![
                SpineItem {
                    href: "OEBPS/chapter1.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 2048, // 2 positions
                    char_count: 1500,
                },
                SpineItem {
                    href: "OEBPS/chapter2.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 3072, // 3 positions
                    char_count: 2500,
                },
            ]
        }

        #[test]
        fn test_compute_positions_count() {
            let positions = compute_epub_positions(&sample_spine());
            assert_eq!(positions.len(), 5); // 2 + 3
        }

        #[test]
        fn test_compute_positions_sequential() {
            let positions = compute_epub_positions(&sample_spine());
            for (i, pos) in positions.iter().enumerate() {
                assert_eq!(pos.position, (i + 1) as i32);
            }
        }

        #[test]
        fn test_compute_positions_total_progression() {
            let positions = compute_epub_positions(&sample_spine());
            assert!((positions[0].total_progression - 1.0 / 5.0).abs() < 1e-10);
            assert!((positions[4].total_progression - 5.0 / 5.0).abs() < 1e-10);
        }

        #[test]
        fn test_compute_positions_min_one_per_resource() {
            let spine = vec![SpineItem {
                href: "tiny.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                file_size: 100,
                char_count: 80,
            }];
            let positions = compute_epub_positions(&spine);
            assert_eq!(positions.len(), 1);
        }

        #[test]
        fn test_normalize_exact_match() {
            let positions = compute_epub_positions(&sample_spine());
            let (tp, pos) = normalize_progression(&positions, "OEBPS/chapter1.xhtml", 0.2).unwrap();
            assert_eq!(pos, 1);
            assert!((tp - 1.0 / 5.0).abs() < 1e-10);
        }

        #[test]
        fn test_normalize_suffix_match() {
            let positions = compute_epub_positions(&sample_spine());
            let result = normalize_progression(&positions, "chapter2.xhtml", 0.7);
            assert!(result.is_some());
            let (_, pos) = result.unwrap();
            assert!((3..=5).contains(&pos));
        }

        #[test]
        fn test_normalize_with_fragment() {
            let positions = compute_epub_positions(&sample_spine());
            let result = normalize_progression(&positions, "OEBPS/chapter1.xhtml#section1", 0.2);
            assert!(result.is_some());
        }

        #[test]
        fn test_normalize_url_encoded() {
            let spine = vec![SpineItem {
                href: "OEBPS/chapter 1.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                file_size: 1024,
                char_count: 800,
            }];
            let positions = compute_epub_positions(&spine);
            let result = normalize_progression(&positions, "OEBPS/chapter%201.xhtml", 0.5);
            assert!(result.is_some());
        }

        #[test]
        fn test_normalize_empty_positions() {
            assert!(normalize_progression(&[], "test.xhtml", 0.5).is_none());
        }

        #[test]
        fn test_normalize_no_href_match_falls_back() {
            let positions = compute_epub_positions(&sample_spine());
            let result = normalize_progression(&positions, "nonexistent.xhtml", 0.6);
            assert!(result.is_some());
        }
    }

    mod progression_conversion {
        use super::*;

        /// ASCII-heavy content: chars ~= bytes minus markup, so conversion is near-identity
        fn ascii_spine() -> Vec<SpineItem> {
            vec![
                SpineItem {
                    href: "ch1.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 1000,
                    char_count: 800, // ~80% text, 20% markup
                },
                SpineItem {
                    href: "ch2.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 1000,
                    char_count: 800,
                },
            ]
        }

        /// CJK content: 3 bytes per char, so char_count << file_size
        fn cjk_spine() -> Vec<SpineItem> {
            vec![
                SpineItem {
                    href: "ch1.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 3000, // 1000 CJK chars * 3 bytes each
                    char_count: 1000,
                },
                SpineItem {
                    href: "ch2.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 6000, // 2000 CJK chars * 3 bytes each
                    char_count: 2000,
                },
            ]
        }

        /// Mixed content: one ASCII chapter, one CJK chapter
        fn mixed_spine() -> Vec<SpineItem> {
            vec![
                SpineItem {
                    href: "ch1.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 1000, // ASCII
                    char_count: 1000,
                },
                SpineItem {
                    href: "ch2.xhtml".to_string(),
                    media_type: "application/xhtml+xml".to_string(),
                    file_size: 3000, // CJK: 1000 chars * 3 bytes
                    char_count: 1000,
                },
            ]
        }

        #[test]
        fn test_ascii_near_identity() {
            let spine = ascii_spine();
            // For uniform char/byte ratio, conversion should be near-identity
            let result = char_to_byte_progression(&spine, 0.5);
            assert!(
                (result - 0.5).abs() < 0.01,
                "ASCII content should be near-identity, got {}",
                result
            );
        }

        #[test]
        fn test_cjk_uniform_ratio_near_identity() {
            let spine = cjk_spine();
            // Both chapters have same 3:1 byte:char ratio, so conversion is near-identity
            let result = char_to_byte_progression(&spine, 0.5);
            assert!(
                (result - 0.5).abs() < 0.01,
                "Uniform CJK ratio should be near-identity, got {}",
                result
            );
        }

        #[test]
        fn test_mixed_content_diverges() {
            let spine = mixed_spine();
            // 50% chars = halfway through (1000 of 2000 chars = end of ch1)
            // In bytes: ch1 is 1000 bytes, total is 4000 bytes
            // So 50% char = 25% bytes
            let result = char_to_byte_progression(&spine, 0.5);
            assert!(
                (result - 0.25).abs() < 0.01,
                "Mixed content: 50% chars should map to ~25% bytes, got {}",
                result
            );
        }

        #[test]
        fn test_roundtrip() {
            let spine = mixed_spine();
            for prog in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
                let byte_prog = char_to_byte_progression(&spine, prog);
                let back = byte_to_char_progression(&spine, byte_prog);
                assert!(
                    (back - prog).abs() < 0.01,
                    "Roundtrip failed for {}: got {} -> {} -> {}",
                    prog,
                    prog,
                    byte_prog,
                    back
                );
            }
        }

        #[test]
        fn test_boundaries() {
            let spine = mixed_spine();
            let start = char_to_byte_progression(&spine, 0.0);
            let end = char_to_byte_progression(&spine, 1.0);
            assert!((start - 0.0).abs() < 0.01);
            assert!((end - 1.0).abs() < 0.01);
        }

        #[test]
        fn test_empty_spine() {
            assert!((char_to_byte_progression(&[], 0.5) - 0.5).abs() < f64::EPSILON);
            assert!((byte_to_char_progression(&[], 0.5) - 0.5).abs() < f64::EPSILON);
        }

        #[test]
        fn test_single_item() {
            let spine = vec![SpineItem {
                href: "ch1.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                file_size: 3000,
                char_count: 1000,
            }];
            // Single item: conversion should still be identity (all within one resource)
            let result = char_to_byte_progression(&spine, 0.5);
            assert!(
                (result - 0.5).abs() < 0.01,
                "Single item should be near-identity, got {}",
                result
            );
        }

        #[test]
        fn test_byte_to_char_mixed() {
            let spine = mixed_spine();
            // 25% bytes = end of ch1 (1000/4000 bytes) = 50% chars (1000/2000 chars)
            let result = byte_to_char_progression(&spine, 0.25);
            assert!(
                (result - 0.5).abs() < 0.01,
                "25% bytes should map to ~50% chars in mixed content, got {}",
                result
            );
        }
    }
}
