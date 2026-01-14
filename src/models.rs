//! Data models for library organization strategies
//!
//! TODO: Remove allow(dead_code) once all strategy features are fully integrated

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

// ============================================================================
// Series Scanning Strategy
// ============================================================================

/// Series scanning strategy type for library organization
///
/// Determines how series are detected and organized from the filesystem structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum SeriesStrategy {
    /// Direct child folders of library = series (Komga-compatible)
    /// Example: /library/Batman/issue1.cbz → Series: "Batman"
    #[default]
    SeriesVolume,

    /// Parent folder = series, child folders = volumes/arcs
    /// All files at any depth under series folder = books in that series
    /// Example: /library/One Piece/Volume 01/Chapter 001.cbz → Series: "One Piece"
    SeriesVolumeChapter,

    /// All files at library root, series detected from filename or metadata
    /// Example: /library/[One Piece] v01.cbz → Series: "One Piece"
    Flat,

    /// Skip first N levels as organizational containers, then apply series_volume rules
    /// Example: /library/Marvel/Spider-Man/issue1.cbz → Series: "Spider-Man" (skip "Marvel")
    PublisherHierarchy,

    /// Calibre library structure: Author folder → Book title folder → book files
    /// Example: /library/Brandon Sanderson/Mistborn (45)/Mistborn.epub
    Calibre,

    /// User-defined regex patterns for series/book detection
    Custom,
}

impl SeriesStrategy {
    /// Convert to string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SeriesVolume => "series_volume",
            Self::SeriesVolumeChapter => "series_volume_chapter",
            Self::Flat => "flat",
            Self::PublisherHierarchy => "publisher_hierarchy",
            Self::Calibre => "calibre",
            Self::Custom => "custom",
        }
    }

    /// Get all available strategies
    pub fn all() -> Vec<Self> {
        vec![
            Self::SeriesVolume,
            Self::SeriesVolumeChapter,
            Self::Flat,
            Self::PublisherHierarchy,
            Self::Calibre,
            Self::Custom,
        ]
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::SeriesVolume => "Direct child folders = series (default, Komga-compatible)",
            Self::SeriesVolumeChapter => {
                "Parent folder = series, supports volume/chapter subfolders"
            }
            Self::Flat => "All files at root level, series from filename/metadata",
            Self::PublisherHierarchy => "Skip organizational levels (publisher/year) before series",
            Self::Calibre => "Calibre library structure (Author/Book folders)",
            Self::Custom => "Custom regex patterns for series detection",
        }
    }
}

impl FromStr for SeriesStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "series_volume" | "default" => Ok(Self::SeriesVolume),
            "series_volume_chapter" => Ok(Self::SeriesVolumeChapter),
            "flat" => Ok(Self::Flat),
            "publisher_hierarchy" => Ok(Self::PublisherHierarchy),
            "calibre" => Ok(Self::Calibre),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Unknown series strategy: {}", s)),
        }
    }
}

impl fmt::Display for SeriesStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Book Naming Strategy
// ============================================================================

/// Book naming strategy type for determining book titles
///
/// Determines how individual book titles and numbers are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum BookStrategy {
    /// Always use filename without extension (Komga-compatible)
    #[default]
    Filename,

    /// Use ComicInfo/metadata title if present, fallback to filename
    MetadataFirst,

    /// Use metadata only if meaningful (not generic like "Vol. 3"), else filename
    Smart,

    /// Generate title from series name + position (e.g., "One Piece v.01 c.001")
    SeriesName,

    /// User-defined regex patterns for title, volume, and chapter extraction
    Custom,
}

impl BookStrategy {
    /// Convert to string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filename => "filename",
            Self::MetadataFirst => "metadata_first",
            Self::Smart => "smart",
            Self::SeriesName => "series_name",
            Self::Custom => "custom",
        }
    }

    /// Get all available strategies
    pub fn all() -> Vec<Self> {
        vec![
            Self::Filename,
            Self::MetadataFirst,
            Self::Smart,
            Self::SeriesName,
            Self::Custom,
        ]
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Filename => "Use filename as book title (Komga-compatible)",
            Self::MetadataFirst => "Use metadata title if available, fallback to filename",
            Self::Smart => "Use metadata if meaningful, ignore generic titles like 'Vol. 3'",
            Self::SeriesName => "Generate uniform titles: 'Series v.01' or 'Series v.01 c.001'",
            Self::Custom => "User-defined regex for title, volume, and chapter extraction",
        }
    }
}

impl FromStr for BookStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "filename" => Ok(Self::Filename),
            "metadata_first" => Ok(Self::MetadataFirst),
            "smart" => Ok(Self::Smart),
            "series_name" => Ok(Self::SeriesName),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Unknown book strategy: {}", s)),
        }
    }
}

impl fmt::Display for BookStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Book Number Strategy
// ============================================================================

/// Book number strategy type for determining book ordering numbers
///
/// Determines how individual book numbers are resolved for sorting and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum NumberStrategy {
    /// Use file position in sorted directory listing (default)
    /// Books are numbered 1, 2, 3... based on alphabetical sort order
    #[default]
    FileOrder,

    /// Use ComicInfo/metadata number field only, no fallback
    Metadata,

    /// Parse number from filename patterns (#001, v01, c001, etc.)
    Filename,

    /// Smart fallback chain: metadata → filename patterns → file order
    Smart,
}

impl NumberStrategy {
    /// Convert to string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileOrder => "file_order",
            Self::Metadata => "metadata",
            Self::Filename => "filename",
            Self::Smart => "smart",
        }
    }

    /// Get all available strategies
    pub fn all() -> Vec<Self> {
        vec![Self::FileOrder, Self::Metadata, Self::Filename, Self::Smart]
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::FileOrder => "Use file position in sorted directory listing (default)",
            Self::Metadata => "Use ComicInfo/metadata number field only",
            Self::Filename => "Parse number from filename patterns (#001, v01, c001, etc.)",
            Self::Smart => "Smart fallback: metadata → filename → file order",
        }
    }
}

impl FromStr for NumberStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file_order" => Ok(Self::FileOrder),
            "metadata" => Ok(Self::Metadata),
            "filename" => Ok(Self::Filename),
            "smart" => Ok(Self::Smart),
            _ => Err(format!("Unknown number strategy: {}", s)),
        }
    }
}

impl fmt::Display for NumberStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Strategy Configuration Types
// ============================================================================

/// Configuration for flat scanning strategy
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlatStrategyConfig {
    /// Regex patterns for extracting series name from filename
    /// Patterns are tried in order, first match wins
    #[serde(default = "FlatStrategyConfig::default_patterns")]
    pub filename_patterns: Vec<String>,

    /// If true, require metadata for series detection (no filename fallback)
    #[serde(default)]
    pub require_metadata: bool,
}

impl Default for FlatStrategyConfig {
    fn default() -> Self {
        Self {
            filename_patterns: Self::default_patterns(),
            require_metadata: false,
        }
    }
}

impl FlatStrategyConfig {
    fn default_patterns() -> Vec<String> {
        vec![
            r"\[([^\]]+)\]".to_string(), // [Series Name] format
            r"^([^-]+) -".to_string(),   // Series Name - format
            r"^([^_]+)_".to_string(),    // Series_Name format
        ]
    }
}

/// Configuration for publisher hierarchy strategy
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublisherHierarchyConfig {
    /// Number of directory levels to skip before series detection
    #[serde(default = "PublisherHierarchyConfig::default_skip_depth")]
    pub skip_depth: u32,

    /// Metadata field to store skipped folder names (e.g., "publisher")
    #[serde(default)]
    pub store_skipped_as: Option<String>,
}

impl Default for PublisherHierarchyConfig {
    fn default() -> Self {
        Self {
            skip_depth: Self::default_skip_depth(),
            store_skipped_as: Some("publisher".to_string()),
        }
    }
}

impl PublisherHierarchyConfig {
    fn default_skip_depth() -> u32 {
        1
    }
}

/// Configuration for Calibre strategy
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalibreStrategyConfig {
    /// Strip Calibre ID suffix from folder names (e.g., " (123)")
    #[serde(default = "default_true")]
    pub strip_id_suffix: bool,

    /// How to group books into series
    #[serde(default)]
    pub series_mode: CalibreSeriesMode,

    /// Read metadata.opf files for rich metadata
    #[serde(default = "default_true")]
    pub read_opf_metadata: bool,

    /// Use author folder name as author metadata
    #[serde(default = "default_true")]
    pub author_from_folder: bool,
}

impl Default for CalibreStrategyConfig {
    fn default() -> Self {
        Self {
            strip_id_suffix: true,
            series_mode: CalibreSeriesMode::default(),
            read_opf_metadata: true,
            author_from_folder: true,
        }
    }
}

/// How Calibre strategy groups books into series
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum CalibreSeriesMode {
    /// Each book is its own "series" of 1
    Standalone,

    /// Group all books by same author into a series
    ByAuthor,

    /// Use series field from OPF/embedded metadata
    #[default]
    FromMetadata,
}

/// Configuration for custom series strategy
///
/// Note: Volume/chapter extraction from filenames is handled by the book strategy,
/// not the series strategy. Use CustomBookConfig for regex-based volume/chapter parsing.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomStrategyConfig {
    /// Regex pattern with named capture groups for series detection
    /// Supported groups: publisher, series, book
    /// Example: "^(?P<publisher>[^/]+)/(?P<series>[^/]+)/(?P<book>.+)\\.(cbz|cbr|epub|pdf)$"
    pub pattern: String,

    /// Template for constructing series name from capture groups
    /// Example: "{publisher} - {series}"
    #[serde(default = "CustomStrategyConfig::default_template")]
    pub series_name_template: String,
}

impl Default for CustomStrategyConfig {
    fn default() -> Self {
        Self {
            pattern: r"^(?P<series>[^/]+)/(?P<book>.+)\.(cbz|cbr|epub|pdf)$".to_string(),
            series_name_template: Self::default_template(),
        }
    }
}

impl CustomStrategyConfig {
    fn default_template() -> String {
        "{series}".to_string()
    }
}

/// Configuration for smart book naming strategy
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct SmartBookConfig {
    /// Additional patterns to consider as "generic" titles (beyond defaults)
    #[serde(default)]
    pub additional_generic_patterns: Vec<String>,
}

/// Configuration for custom book naming strategy
///
/// Allows user-defined regex patterns for extracting title, volume, and chapter
/// from filenames.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomBookConfig {
    /// Regex pattern with named capture groups for extraction
    /// Supported groups: volume, chapter, title, series
    /// Example: "(?P<series>.+?)_v(?P<volume>\\d+)_c(?P<chapter>\\d+)"
    pub pattern: String,

    /// Template for constructing display title from captured groups
    /// Available placeholders: {series}, {volume}, {chapter}, {title}, {filename}
    /// Example: "{series} v.{volume} c.{chapter}"
    #[serde(default)]
    pub title_template: Option<String>,

    /// Fallback strategy if pattern doesn't match
    /// Options: "filename", "metadata_first", "smart"
    #[serde(default = "CustomBookConfig::default_fallback")]
    pub fallback: String,
}

impl Default for CustomBookConfig {
    fn default() -> Self {
        Self {
            pattern: r"(?P<title>.+)".to_string(),
            title_template: None,
            fallback: Self::default_fallback(),
        }
    }
}

impl CustomBookConfig {
    fn default_fallback() -> String {
        "filename".to_string()
    }
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Legacy Compatibility
// ============================================================================

/// Legacy scanning strategy enum for backward compatibility
/// Maps to SeriesStrategy::SeriesVolume
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanningStrategy {
    /// Default: Direct child folders = series (maps to SeriesStrategy::SeriesVolume)
    Default,
}

impl ScanningStrategy {
    /// Convert to string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
        }
    }
}

impl FromStr for ScanningStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            _ => Err(format!("Unknown scanning strategy: {}", s)),
        }
    }
}

impl From<ScanningStrategy> for SeriesStrategy {
    fn from(_: ScanningStrategy) -> Self {
        SeriesStrategy::SeriesVolume
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_strategy_roundtrip() {
        for strategy in SeriesStrategy::all() {
            let s = strategy.as_str();
            let parsed: SeriesStrategy = s.parse().unwrap();
            assert_eq!(parsed, strategy, "Failed roundtrip for {}", s);
        }
    }

    #[test]
    fn test_series_strategy_default_compatibility() {
        // "default" should map to SeriesVolume for backward compatibility
        assert_eq!(
            "default".parse::<SeriesStrategy>().unwrap(),
            SeriesStrategy::SeriesVolume
        );
    }

    #[test]
    fn test_book_strategy_roundtrip() {
        for strategy in BookStrategy::all() {
            let s = strategy.as_str();
            let parsed: BookStrategy = s.parse().unwrap();
            assert_eq!(parsed, strategy, "Failed roundtrip for {}", s);
        }
    }

    #[test]
    fn test_series_strategy_serde() {
        let strategy = SeriesStrategy::SeriesVolumeChapter;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"series_volume_chapter\"");

        let parsed: SeriesStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, strategy);
    }

    #[test]
    fn test_book_strategy_serde() {
        let strategy = BookStrategy::MetadataFirst;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"metadata_first\"");

        let parsed: BookStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, strategy);
    }

    #[test]
    fn test_flat_strategy_config_default() {
        let config = FlatStrategyConfig::default();
        assert!(!config.filename_patterns.is_empty());
        assert!(!config.require_metadata);
    }

    #[test]
    fn test_calibre_config_default() {
        let config = CalibreStrategyConfig::default();
        assert!(config.strip_id_suffix);
        assert!(config.read_opf_metadata);
        assert!(config.author_from_folder);
        assert_eq!(config.series_mode, CalibreSeriesMode::FromMetadata);
    }

    #[test]
    fn test_number_strategy_roundtrip() {
        for strategy in NumberStrategy::all() {
            let s = strategy.as_str();
            let parsed: NumberStrategy = s.parse().unwrap();
            assert_eq!(parsed, strategy, "Failed roundtrip for {}", s);
        }
    }

    #[test]
    fn test_number_strategy_serde() {
        let strategy = NumberStrategy::FileOrder;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"file_order\"");

        let parsed: NumberStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, strategy);
    }

    #[test]
    fn test_number_strategy_default() {
        assert_eq!(NumberStrategy::default(), NumberStrategy::FileOrder);
    }
}
