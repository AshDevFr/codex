//! DTOs for exporting and importing one user's reading state.
//!
//! The document, request, and report shapes are defined in
//! `codex_services::reading_transfer::model` (re-exported below) rather than
//! here, because the service that produces and consumes them owns the shape;
//! this module only adds the query parameters for the export endpoint, which
//! are handler-only concerns.

pub use codex_services::reading_transfer::model::*;

use serde::Deserialize;
use utoipa::ToSchema;

fn default_true() -> bool {
    true
}

/// Query parameters for `GET /api/v1/reading-progress/export`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExportReadingProgressQuery {
    /// Sessions are opt-out: they are the only source of every reading
    /// statistic, so leaving them out is easy to do by accident and hard to
    /// notice until the numbers are gone.
    #[serde(default = "default_true")]
    pub include_sessions: bool,
}

impl Default for ExportReadingProgressQuery {
    fn default() -> Self {
        Self {
            include_sessions: true,
        }
    }
}
