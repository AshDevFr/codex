pub mod connection;
pub mod models;
pub mod postgres;
pub mod sqlite;

// Re-export commonly used types
pub use connection::Database;
pub use models::{
    Book, BookMetadataRecord, Library, MetadataSource, Page, ReadProgress,
    ScanningStrategy, Series, User,
};
pub use postgres::PostgresDatabase;
pub use sqlite::SqliteDatabase;

