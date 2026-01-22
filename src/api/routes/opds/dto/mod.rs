//! OPDS Data Transfer Objects
//!
//! These DTOs represent OPDS 1.2 (Atom-based) feed structures.

pub mod entry;
pub mod feed;
pub mod link;

pub use entry::*;
pub use feed::*;
pub use link::*;
