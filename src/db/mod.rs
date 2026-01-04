pub mod connection;
pub mod entities;
pub mod postgres;
pub mod repositories;
pub mod sqlite;

pub mod test_helpers;

// Re-export commonly used types
pub use connection::Database;

// Re-export SeaORM entities for use throughout the application
pub use entities::{
    books, libraries, pages, series, book_metadata_records,
    prelude::*,
};

// Re-export ScanningStrategy for convenience
pub use crate::models::ScanningStrategy;

