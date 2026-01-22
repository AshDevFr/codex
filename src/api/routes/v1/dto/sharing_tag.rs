//! DTOs for sharing tag operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::entities::user_sharing_tags::AccessMode;

/// Sharing tag data transfer object
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharingTagDto {
    /// Unique sharing tag identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Display name of the sharing tag
    #[schema(example = "Kids Content")]
    pub name: String,

    /// Optional description
    #[schema(example = "Content appropriate for children")]
    pub description: Option<String>,

    /// Number of series tagged with this sharing tag
    #[schema(example = 42)]
    pub series_count: u64,

    /// Number of users with grants for this sharing tag
    #[schema(example = 5)]
    pub user_count: u64,

    /// Creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Simplified sharing tag for lists (without counts)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharingTagSummaryDto {
    /// Unique sharing tag identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Display name of the sharing tag
    #[schema(example = "Kids Content")]
    pub name: String,

    /// Optional description
    #[schema(example = "Content appropriate for children")]
    pub description: Option<String>,
}

/// Create sharing tag request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharingTagRequest {
    /// Display name for the sharing tag (must be unique)
    #[schema(example = "Kids Content")]
    pub name: String,

    /// Optional description
    #[schema(example = "Content appropriate for children")]
    pub description: Option<String>,
}

/// Update sharing tag request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSharingTagRequest {
    /// New display name (must be unique)
    #[schema(example = "Family Content")]
    pub name: Option<String>,

    /// New description (set to null to remove)
    #[schema(example = "Content appropriate for the whole family")]
    pub description: Option<Option<String>>,
}

/// User sharing tag grant data transfer object
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSharingTagGrantDto {
    /// Unique grant identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Sharing tag ID
    #[schema(example = "660e8400-e29b-41d4-a716-446655440000")]
    pub sharing_tag_id: Uuid,

    /// Sharing tag name
    #[schema(example = "Kids Content")]
    pub sharing_tag_name: String,

    /// Access mode: allow or deny
    #[schema(example = "allow")]
    pub access_mode: AccessMode,

    /// Grant creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
}

/// Set user sharing tag grant request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetUserSharingTagGrantRequest {
    /// Sharing tag ID to grant access to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub sharing_tag_id: Uuid,

    /// Access mode: allow or deny
    #[schema(example = "allow")]
    pub access_mode: AccessMode,
}

/// Bulk set series sharing tags request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetSeriesSharingTagsRequest {
    /// List of sharing tag IDs to apply to the series
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440000"]))]
    pub sharing_tag_ids: Vec<Uuid>,
}

/// Add/remove single sharing tag from series request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModifySeriesSharingTagRequest {
    /// Sharing tag ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub sharing_tag_id: Uuid,
}

/// Response for list of sharing tags
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharingTagListResponse {
    /// List of sharing tags
    pub items: Vec<SharingTagDto>,

    /// Total count
    #[schema(example = 10)]
    pub total: usize,
}

/// Response for user's sharing tag grants
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSharingTagGrantsResponse {
    /// User ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,

    /// List of grants
    pub grants: Vec<UserSharingTagGrantDto>,
}

// Conversion implementations

impl From<crate::db::entities::sharing_tags::Model> for SharingTagSummaryDto {
    fn from(model: crate::db::entities::sharing_tags::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
        }
    }
}

impl SharingTagDto {
    /// Create a DTO from model with counts
    pub fn from_model_with_counts(
        model: crate::db::entities::sharing_tags::Model,
        series_count: u64,
        user_count: u64,
    ) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            series_count,
            user_count,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl UserSharingTagGrantDto {
    /// Create a DTO from grant model and sharing tag model
    pub fn from_models(
        grant: crate::db::entities::user_sharing_tags::Model,
        tag: crate::db::entities::sharing_tags::Model,
    ) -> Self {
        Self {
            id: grant.id,
            sharing_tag_id: grant.sharing_tag_id,
            sharing_tag_name: tag.name,
            access_mode: grant.get_access_mode(),
            created_at: grant.created_at,
        }
    }
}
