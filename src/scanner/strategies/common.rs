//! Common types shared across scanning strategies
//!
//! This module contains types used by both series detection and book naming strategies.
//!
//! TODO: Remove allow(dead_code) once all scanning strategy features are fully integrated

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

/// Metadata extracted for a series during scanning
#[derive(Debug, Clone, Default)]
pub struct SeriesMetadata {
    /// Publisher name (from folder structure for publisher_hierarchy)
    pub publisher: Option<String>,
    /// Author name (from folder structure for calibre)
    pub author: Option<String>,
    /// Additional metadata as key-value pairs
    pub extra: HashMap<String, String>,
}

/// Detected book information during scanning
#[derive(Debug, Clone)]
pub struct DetectedBook {
    /// Full path to the book file
    pub path: PathBuf,
    /// Detected book number (if extractable)
    pub number: Option<f32>,
    /// Volume number (for series_volume_chapter strategy)
    pub volume: Option<String>,
    /// Relative path within series folder
    pub relative_path: Option<String>,
    /// Resolved title (based on book naming strategy)
    pub title: Option<String>,
    /// Chapter number within volume (for series_volume_chapter strategy)
    pub chapter_number: Option<f32>,
}

impl DetectedBook {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            number: None,
            volume: None,
            relative_path: None,
            title: None,
            chapter_number: None,
        }
    }

    pub fn with_number(mut self, number: f32) -> Self {
        self.number = Some(number);
        self
    }

    pub fn with_volume(mut self, volume: impl Into<String>) -> Self {
        self.volume = Some(volume.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_chapter_number(mut self, chapter: f32) -> Self {
        self.chapter_number = Some(chapter);
        self
    }
}

/// Detected series information during scanning
#[derive(Debug, Clone)]
pub struct DetectedSeries {
    /// Series name
    pub name: String,
    /// Path to series folder (relative to library root)
    pub path: Option<String>,
    /// Books in this series
    pub books: Vec<DetectedBook>,
    /// Series metadata
    pub metadata: SeriesMetadata,
}

impl DetectedSeries {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: None,
            books: Vec::new(),
            metadata: SeriesMetadata::default(),
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn add_book(&mut self, book: DetectedBook) {
        self.books.push(book);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detected_series_builder() {
        let mut series = DetectedSeries::new("Batman").with_path("Batman");

        series
            .add_book(DetectedBook::new(PathBuf::from("/lib/Batman/issue1.cbz")).with_number(1.0));
        series
            .add_book(DetectedBook::new(PathBuf::from("/lib/Batman/issue2.cbz")).with_number(2.0));

        assert_eq!(series.name, "Batman");
        assert_eq!(series.path, Some("Batman".to_string()));
        assert_eq!(series.books.len(), 2);
        assert_eq!(series.books[0].number, Some(1.0));
    }

    #[test]
    fn test_detected_book_builder() {
        let book = DetectedBook::new(PathBuf::from("/lib/book.cbz"))
            .with_number(5.0)
            .with_volume("Volume 2")
            .with_title("My Title")
            .with_chapter_number(10.0);

        assert_eq!(book.number, Some(5.0));
        assert_eq!(book.volume, Some("Volume 2".to_string()));
        assert_eq!(book.title, Some("My Title".to_string()));
        assert_eq!(book.chapter_number, Some(10.0));
    }

    #[test]
    fn test_series_metadata_default() {
        let metadata = SeriesMetadata::default();
        assert!(metadata.publisher.is_none());
        assert!(metadata.author.is_none());
        assert!(metadata.extra.is_empty());
    }
}
