pub mod connection;
pub mod entities;
pub mod repositories;
pub mod trace;

// Available to codex-db's own `#[cfg(test)]` modules and to downstream crates
// that opt into the `test-utils` feature (e.g. the root binary's dev-deps).
#[cfg(any(test, feature = "test-utils"))]
pub mod test_helpers;

// Re-export commonly used types
pub use connection::Database;

// Re-export SeaORM entities for use throughout the application

// Re-export scanning strategies for convenience
pub use codex_models::ScanningStrategy;

// Re-export CreateLibraryParams for convenience
