//! Re-export of the cross-layer permission types.
//!
//! The canonical definitions live in [`crate::models::permissions`] so that
//! the db and utils layers can reference `UserRole` without depending on the
//! api layer. This module preserves the historic `codex::api::permissions::*`
//! path used by integration tests and downstream code.

pub use crate::models::permissions::*;
