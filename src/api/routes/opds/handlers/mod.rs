//! OPDS 1.2 Handlers
//!
//! Handlers for OPDS catalog, search, and PSE (Page Streaming Extension) endpoints.

pub mod catalog;
pub mod pse;
pub mod search;

pub use catalog::*;
pub use pse::*;
pub use search::*;
