//! Scanning strategies for library organization
//!
//! This module provides different strategies for detecting series and organizing
//! books within a library based on filesystem structure, metadata, or custom patterns.

pub mod book;
pub mod common;
pub mod number;
pub mod series;

// Re-export book naming types
pub use book::{
    create_book_strategy, filename_without_extension, BookMetadata, BookNamingContext,
    BookNamingStrategy, FilenameStrategy, MetadataFirstStrategy, SeriesNameStrategy, SmartStrategy,
};

// Re-export common types
pub use common::{DetectedBook, DetectedSeries, SeriesMetadata};

// Re-export number strategy types
pub use number::{create_number_strategy, BookNumberStrategy, NumberContext, NumberMetadata};

// Re-export series strategy types
pub use series::{
    create_strategy, CalibreStrategy, CustomStrategy, FlatStrategy, PublisherHierarchyStrategy,
    ScanningStrategyImpl, SeriesVolumeChapterStrategy, SeriesVolumeStrategy,
};
