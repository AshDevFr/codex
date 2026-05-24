//! Scanning strategies for library organization
//!
//! This module provides different strategies for detecting series and organizing
//! books within a library based on filesystem structure, metadata, or custom patterns.

pub mod book;
pub mod common;
pub mod number;
pub mod series;

// Re-export book naming types
// BookNamingStrategy is public API for external strategy implementations
#[allow(unused_imports)]
pub use book::{BookMetadata, BookNamingContext, BookNamingStrategy, create_book_strategy};

// Re-export common types for external strategy implementations
#[allow(unused_imports)]
pub use common::{DetectedBook, DetectedSeries};

// Re-export number strategy types
pub use number::{NumberContext, NumberMetadata, create_number_strategy};

// Re-export series strategy types
// ScanningStrategyImpl is public API for external strategy implementations
#[allow(unused_imports)]
pub use series::{ScanningStrategyImpl, create_strategy};
