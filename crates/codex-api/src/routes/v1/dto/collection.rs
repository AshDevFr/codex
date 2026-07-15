//! DTOs for collections (shared, ordered groupings of series).

use chrono::{DateTime, Utc};
use codex_models::sort::{CollectionSeriesSort, SortDirection};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for listing a collection's series.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct CollectionSeriesQuery {
    /// Sort: `title`, `added`, `year`, or `manual`. When omitted, the
    /// collection's `ordered` flag picks the default (`manual` when set,
    /// `title` otherwise).
    #[param(inline)]
    pub sort: Option<CollectionSeriesSort>,
    /// Direction for the chosen sort (`asc` default). Ignored for `manual`,
    /// which always returns the user's arranged order.
    #[param(inline)]
    pub direction: Option<SortDirection>,
}

/// A collection of series.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "Batman")]
    pub name: String,
    /// Default presentation order when no sort is requested: manual when
    /// true, title otherwise.
    #[schema(example = false)]
    pub ordered: bool,
    /// Optional description.
    #[schema(example = "The Dark Knight's essential arcs.")]
    pub summary: Option<String>,
    /// Number of member series visible to the requesting user.
    #[schema(example = 12)]
    pub series_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CollectionDto {
    pub fn from_model(model: codex_db::entities::collections::Model, series_count: u64) -> Self {
        Self {
            id: model.id,
            name: model.name,
            ordered: model.ordered,
            summary: model.summary,
            series_count,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// List of collections.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CollectionListResponse {
    pub items: Vec<CollectionDto>,
    #[schema(example = 3)]
    pub total: usize,
}

/// Request to create a collection.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionRequest {
    #[schema(example = "Batman")]
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Defaults to `false` (members default to sorting by title).
    #[serde(default)]
    #[schema(example = false)]
    pub ordered: bool,
}

/// Deserialize a nullable field into a "double option" so the handler can tell
/// "field absent" (`None` → leave unchanged) from "field present and null"
/// (`Some(None)` → clear). Without this, serde collapses an explicit `null`
/// into the outer `None`.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

/// Request to update a collection. Absent fields are left unchanged. To clear
/// the summary, send `summary: null` explicitly.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option"
    )]
    #[schema(value_type = Option<String>)]
    pub summary: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,
}

/// Request to add one or more series to a collection.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddSeriesToCollectionRequest {
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001"]))]
    pub series_ids: Vec<Uuid>,
}

/// Request to set the manual order of a collection's series. IDs not currently
/// members are ignored; omitted members keep their existing position.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderCollectionSeriesRequest {
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440002", "550e8400-e29b-41d4-a716-446655440001"]))]
    pub series_ids: Vec<Uuid>,
}
