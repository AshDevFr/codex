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
use uuid::Uuid;

fn default_true() -> bool {
    true
}

/// Query parameters for `GET /api/v1/reading-progress/export`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportReadingProgressQuery {
    /// Sessions are opt-out: they are the only source of every reading
    /// statistic, so leaving them out is easy to do by accident and hard to
    /// notice until the numbers are gone.
    #[serde(default = "default_true")]
    pub include_sessions: bool,

    /// Comma-separated library ids to export, e.g.
    /// `?libraryIds=<uuid>,<uuid>`. Omitted exports every library the reader
    /// has state for.
    ///
    /// Comma-separated rather than a repeated key because axum's `Query`
    /// extractor deserializes with `serde_urlencoded`, which collapses a
    /// repeated key instead of collecting it into a `Vec`.
    pub library_ids: Option<String>,
}

impl ExportReadingProgressQuery {
    /// Parse `library_ids`, rejecting anything that is not a uuid rather than
    /// silently exporting more than the caller asked for.
    pub fn parsed_library_ids(&self) -> Result<Option<Vec<Uuid>>, uuid::Error> {
        let Some(raw) = self.library_ids.as_deref() else {
            return Ok(None);
        };
        let ids = raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(Uuid::parse_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(ids))
    }
}

impl Default for ExportReadingProgressQuery {
    fn default() -> Self {
        Self {
            include_sessions: true,
            library_ids: None,
        }
    }
}
