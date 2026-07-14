//! DTOs for collections (shared, ordered groupings of series).

use chrono::{DateTime, Utc};
use codex_models::sort::CollectionSeriesSort;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for listing a collection's series.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct CollectionSeriesQuery {
    /// Sort for unordered collections: `title` (default), `added`, or `year`.
    /// Ignored when the collection is manually ordered.
    #[param(inline)]
    pub sort: Option<CollectionSeriesSort>,
}

/// A collection of series.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "Batman")]
    pub name: String,
    /// When true, members are kept in manual order; otherwise sorted by title.
    #[schema(example = false)]
    pub ordered: bool,
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
    /// Defaults to `false` (members sorted by title).
    #[serde(default)]
    #[schema(example = false)]
    pub ordered: bool,
}

/// Request to update a collection. Absent fields are left unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
