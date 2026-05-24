//! OPDS 2.0 Handlers
//!
//! Handlers for OPDS 2.0 catalog and search endpoints (JSON-based).

pub mod catalog;
pub mod search;

pub use catalog::*;
pub use search::*;
