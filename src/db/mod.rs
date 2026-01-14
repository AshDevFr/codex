pub mod connection;
pub mod entities;
pub mod repositories;

pub mod test_helpers;

// Re-export commonly used types
pub use connection::Database;

// Re-export SeaORM entities for use throughout the application

// Re-export scanning strategies for convenience
pub use crate::models::ScanningStrategy;

// Re-export CreateLibraryParams for convenience
