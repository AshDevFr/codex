pub mod connection;
pub mod entities;
pub mod repositories;

pub mod test_helpers;

// Re-export commonly used types
pub use connection::Database;

// Re-export SeaORM entities for use throughout the application
pub use entities::{book_metadata_records, books, libraries, pages, prelude::*, series};

// Re-export scanning strategies for convenience
pub use crate::models::{BookStrategy, ScanningStrategy, SeriesStrategy};

// Re-export CreateLibraryParams for convenience
pub use repositories::library::CreateLibraryParams;
