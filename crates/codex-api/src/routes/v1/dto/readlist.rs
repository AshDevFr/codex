//! DTOs for read lists (shared, ordered groupings of books across series).

use chrono::{DateTime, Utc};
use codex_models::sort::ReadListBookSort;
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for listing a read list's books.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ReadListBooksQuery {
    /// Sort: `release`, `title`, `added`, or `manual`. When omitted, the
    /// read list's `ordered` flag picks the default (`manual` when set,
    /// `release` otherwise).
    #[param(inline)]
    pub sort: Option<ReadListBookSort>,
}

/// Deserialize a nullable field into a "double option" so the handler can tell
/// "field absent" (`None` → leave unchanged) from "field present and null"
/// (`Some(None)` → clear). Without this, serde collapses an explicit `null`
/// into the outer `None`.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// A read list.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadListDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "Civil War")]
    pub name: String,
    /// Optional description (Komga read lists carry a summary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// When true, members are kept in manual reading order; otherwise sorted by
    /// release date.
    #[schema(example = true)]
    pub ordered: bool,
    /// Number of member books visible to the requesting user.
    #[schema(example = 24)]
    pub book_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ReadListDto {
    pub fn from_model(model: codex_db::entities::read_lists::Model, book_count: u64) -> Self {
        Self {
            id: model.id,
            name: model.name,
            summary: model.summary,
            ordered: model.ordered,
            book_count,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// List of read lists.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadListListResponse {
    pub items: Vec<ReadListDto>,
    #[schema(example = 3)]
    pub total: usize,
}

/// Request to create a read list.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateReadListRequest {
    #[schema(example = "Civil War")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Defaults to `true` (manual reading order).
    #[serde(default = "default_true")]
    #[schema(example = true)]
    pub ordered: bool,
}

fn default_true() -> bool {
    true
}

/// Request to update a read list. Absent fields are left unchanged. To clear the
/// summary, send `summary: null` explicitly.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReadListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `Some(Some(text))` sets it, `Some(None)` clears it, absent leaves it.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub summary: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,
}

/// Request to add one or more books to a read list.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddBooksToReadListRequest {
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001"]))]
    pub book_ids: Vec<Uuid>,
}

/// Request to set the manual order of a read list's books.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderReadListBooksRequest {
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440002", "550e8400-e29b-41d4-a716-446655440001"]))]
    pub book_ids: Vec<Uuid>,
}
