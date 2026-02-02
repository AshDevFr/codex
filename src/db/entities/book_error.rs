//! Book error types for structured error storage
//!
//! This module defines the error types and structures used to track
//! book analysis and processing errors. Errors are stored as a JSON
//! map keyed by error type, allowing multiple independent error types
//! to be tracked for a single book.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of errors that can occur during book processing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BookErrorType {
    /// Error detecting the file format
    FormatDetection,
    /// Error parsing the book file (archive extraction, etc.)
    Parser,
    /// Error extracting or parsing metadata
    Metadata,
    /// Error generating thumbnail
    Thumbnail,
    /// Error extracting pages from the book
    PageExtraction,
    /// Error rendering PDF pages (e.g., PDFium not available)
    PdfRendering,
    /// Book was analyzed successfully but contains zero pages
    ZeroPages,
    /// Other uncategorized errors
    Other,
}

impl BookErrorType {
    /// Get a human-readable label for this error type
    pub fn label(&self) -> &'static str {
        match self {
            BookErrorType::FormatDetection => "Format Detection",
            BookErrorType::Parser => "Parser Error",
            BookErrorType::Metadata => "Metadata Error",
            BookErrorType::Thumbnail => "Thumbnail Generation",
            BookErrorType::PageExtraction => "Page Extraction",
            BookErrorType::PdfRendering => "PDF Rendering",
            BookErrorType::ZeroPages => "Zero Pages",
            BookErrorType::Other => "Other Error",
        }
    }

    /// Get all error types for iteration
    #[cfg(test)]
    pub fn all() -> &'static [BookErrorType] {
        &[
            BookErrorType::FormatDetection,
            BookErrorType::Parser,
            BookErrorType::Metadata,
            BookErrorType::Thumbnail,
            BookErrorType::PageExtraction,
            BookErrorType::PdfRendering,
            BookErrorType::ZeroPages,
            BookErrorType::Other,
        ]
    }
}

impl std::fmt::Display for BookErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Information about a specific error
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookError {
    /// Human-readable error message
    pub message: String,
    /// Optional additional details (e.g., stack trace, context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// When the error occurred
    pub occurred_at: DateTime<Utc>,
}

impl BookError {
    /// Create a new BookError with the current timestamp
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
            occurred_at: Utc::now(),
        }
    }

    /// Create a new BookError with details
    #[cfg(test)]
    pub fn with_details(message: impl Into<String>, details: serde_json::Value) -> Self {
        Self {
            message: message.into(),
            details: Some(details),
            occurred_at: Utc::now(),
        }
    }
}

/// A collection of errors for a book, keyed by error type
pub type BookErrors = HashMap<BookErrorType, BookError>;

/// Helper functions for working with BookErrors
#[cfg(test)]
pub trait BookErrorsExt {
    /// Parse a JSON string into BookErrors
    fn from_json(json: &str) -> Result<BookErrors, serde_json::Error>;

    /// Serialize BookErrors to a JSON string
    fn to_json(&self) -> Result<String, serde_json::Error>;
}

#[cfg(test)]
impl BookErrorsExt for BookErrors {
    fn from_json(json: &str) -> Result<BookErrors, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Parse analysis_errors JSON string into BookErrors
pub fn parse_analysis_errors(json: Option<&str>) -> BookErrors {
    json.and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default()
}

/// Serialize BookErrors to JSON string for storage
pub fn serialize_analysis_errors(errors: &BookErrors) -> Option<String> {
    if errors.is_empty() {
        None
    } else {
        serde_json::to_string(errors).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_book_error_type_serialization() {
        let error_type = BookErrorType::Parser;
        let json = serde_json::to_string(&error_type).unwrap();
        assert_eq!(json, "\"parser\"");

        let parsed: BookErrorType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, BookErrorType::Parser);

        // Test ZeroPages serializes to snake_case
        let zero_pages = BookErrorType::ZeroPages;
        let json = serde_json::to_string(&zero_pages).unwrap();
        assert_eq!(json, "\"zero_pages\"");

        let parsed: BookErrorType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, BookErrorType::ZeroPages);
    }

    #[test]
    fn test_book_error_type_all_variants() {
        // Test all variants serialize/deserialize correctly
        for error_type in BookErrorType::all() {
            let json = serde_json::to_string(error_type).unwrap();
            let parsed: BookErrorType = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, error_type);
        }
    }

    #[test]
    fn test_book_error_creation() {
        let error = BookError::new("Test error message");
        assert_eq!(error.message, "Test error message");
        assert!(error.details.is_none());

        let error_with_details =
            BookError::with_details("Another error", json!({"file": "test.cbz"}));
        assert_eq!(error_with_details.message, "Another error");
        assert!(error_with_details.details.is_some());
    }

    #[test]
    fn test_book_error_serialization() {
        let error = BookError::new("Test error");
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"message\":\"Test error\""));
        assert!(json.contains("\"occurred_at\":"));

        let parsed: BookError = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.message, error.message);
    }

    #[test]
    fn test_book_errors_map() {
        let mut errors = BookErrors::new();
        errors.insert(BookErrorType::Parser, BookError::new("Parse failed"));
        errors.insert(
            BookErrorType::Thumbnail,
            BookError::new("Thumbnail generation failed"),
        );

        let json = errors.to_json().unwrap();
        let parsed = BookErrors::from_json(&json).unwrap();

        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains_key(&BookErrorType::Parser));
        assert!(parsed.contains_key(&BookErrorType::Thumbnail));
    }

    #[test]
    fn test_parse_analysis_errors() {
        // Test with valid JSON
        let json = r#"{"parser":{"message":"Failed","occurred_at":"2026-01-22T00:00:00Z"}}"#;
        let errors = parse_analysis_errors(Some(json));
        assert_eq!(errors.len(), 1);
        assert!(errors.contains_key(&BookErrorType::Parser));

        // Test with None
        let errors = parse_analysis_errors(None);
        assert!(errors.is_empty());

        // Test with invalid JSON (should return empty)
        let errors = parse_analysis_errors(Some("invalid"));
        assert!(errors.is_empty());
    }

    #[test]
    fn test_serialize_analysis_errors() {
        // Test with empty errors
        let errors = BookErrors::new();
        assert!(serialize_analysis_errors(&errors).is_none());

        // Test with errors
        let mut errors = BookErrors::new();
        errors.insert(BookErrorType::Other, BookError::new("Some error"));
        let json = serialize_analysis_errors(&errors);
        assert!(json.is_some());
        assert!(json.unwrap().contains("other"));
    }

    #[test]
    fn test_book_error_type_labels() {
        assert_eq!(BookErrorType::FormatDetection.label(), "Format Detection");
        assert_eq!(BookErrorType::Parser.label(), "Parser Error");
        assert_eq!(BookErrorType::Metadata.label(), "Metadata Error");
        assert_eq!(BookErrorType::Thumbnail.label(), "Thumbnail Generation");
        assert_eq!(BookErrorType::PageExtraction.label(), "Page Extraction");
        assert_eq!(BookErrorType::PdfRendering.label(), "PDF Rendering");
        assert_eq!(BookErrorType::ZeroPages.label(), "Zero Pages");
        assert_eq!(BookErrorType::Other.label(), "Other Error");
    }
}
