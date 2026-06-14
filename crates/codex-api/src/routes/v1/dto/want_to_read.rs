//! DTOs for the per-user want-to-read queue.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// What a want-to-read entry points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum WantToReadItemType {
    Series,
    Book,
}

/// A single entry in a user's want-to-read queue. Exactly one of `series_id` /
/// `book_id` is populated, matching `item_type`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WantToReadEntryDto {
    /// Queue entry ID.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Whether this entry flags a series or a book.
    pub item_type: WantToReadItemType,
    /// The flagged series (set when `item_type` is `series`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<Uuid>,
    /// The flagged book (set when `item_type` is `book`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_id: Option<Uuid>,
    /// When the entry was added to the queue.
    #[schema(example = "2026-06-15T18:45:00Z")]
    pub added_at: DateTime<Utc>,
}

impl From<codex_db::entities::want_to_read::Model> for WantToReadEntryDto {
    fn from(model: codex_db::entities::want_to_read::Model) -> Self {
        let item_type = if model.series_id.is_some() {
            WantToReadItemType::Series
        } else {
            WantToReadItemType::Book
        };
        Self {
            id: model.id,
            item_type,
            series_id: model.series_id,
            book_id: model.book_id,
            added_at: model.added_at,
        }
    }
}

/// A user's want-to-read queue.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WantToReadListResponse {
    /// Queue entries.
    pub items: Vec<WantToReadEntryDto>,
    /// Total number of entries.
    #[schema(example = 7)]
    pub total: usize,
}

/// Request to add an entry to the queue. Exactly one of `series_id` / `book_id`
/// must be provided.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddWantToReadRequest {
    /// Flag a series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<Uuid>,
    /// Flag a book.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_id: Option<Uuid>,
}

/// Query parameters for listing the queue.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct WantToReadListQuery {
    /// Sort by add time. Accepts `added_at:asc` for oldest-first; any other
    /// value (or omitted) yields newest-first (`added_at:desc`).
    #[param(example = "added_at:desc")]
    pub sort: Option<String>,
}

impl WantToReadListQuery {
    /// Whether to sort ascending (oldest-first). Defaults to descending.
    pub fn ascending(&self) -> bool {
        matches!(self.sort.as_deref(), Some("added_at:asc") | Some("asc"))
    }
}
