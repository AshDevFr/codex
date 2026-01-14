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
pub use book::{create_book_strategy, BookMetadata, BookNamingContext, BookNamingStrategy};

// Re-export common types for external strategy implementations
#[allow(unused_imports)]
pub use common::{DetectedBook, DetectedSeries};

// Re-export number strategy types
pub use number::{create_number_strategy, NumberContext, NumberMetadata};

// Re-export series strategy types
// ScanningStrategyImpl is public API for external strategy implementations
#[allow(unused_imports)]
pub use series::{create_strategy, ScanningStrategyImpl};
