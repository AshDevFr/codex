//! Re-export of preprocessing value types.
//!
//! The canonical home is [`crate::models::preprocessing`] so the db layer
//! can speak these types without depending on services. This module keeps
//! the historical `services::metadata::preprocessing::types::*` path alive
//! for the local processing logic in sibling modules.

pub use crate::models::preprocessing::*;
