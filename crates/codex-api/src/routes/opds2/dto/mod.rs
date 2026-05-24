//! OPDS 2.0 Data Transfer Objects
//!
//! These DTOs represent OPDS 2.0 (JSON-based) feed structures.

pub mod feed;
pub mod link;
pub mod metadata;
pub mod publication;

pub use feed::*;
pub use link::*;
pub use metadata::*;
pub use publication::*;
