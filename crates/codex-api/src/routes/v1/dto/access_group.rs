//! DTOs for access group operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use codex_db::entities::user_sharing_tags::AccessMode;

/// Access group data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupDto {
    /// Unique access group identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Display name of the access group
    #[schema(example = "Manga Readers")]
    pub name: String,

    /// Optional description
    #[schema(example = "Users who can access manga content")]
    pub description: Option<String>,

    /// Number of members in the group
    #[schema(example = 5)]
    pub member_count: u64,

    /// Number of tag grants in the group
    #[schema(example = 3)]
    pub grant_count: u64,

    /// Creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Access group detail (includes members, grants, and OIDC mappings)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupDetailDto {
    /// Unique access group identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Display name of the access group
    #[schema(example = "Manga Readers")]
    pub name: String,

    /// Optional description
    #[schema(example = "Users who can access manga content")]
    pub description: Option<String>,

    /// Members of the group
    pub members: Vec<AccessGroupMemberDto>,

    /// Tag grants for the group
    pub grants: Vec<AccessGroupGrantDto>,

    /// OIDC mappings for the group
    pub oidc_mappings: Vec<AccessGroupOidcMappingDto>,

    /// Creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// Access group member
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupMemberDto {
    /// User ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,

    /// Username
    #[schema(example = "alice")]
    pub username: String,

    /// Membership source (manual or oidc)
    #[schema(example = "manual")]
    pub source: String,

    /// When the user was added to the group
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
}

/// Access group tag grant
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupGrantDto {
    /// Sharing tag ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub sharing_tag_id: Uuid,

    /// Sharing tag name
    #[schema(example = "manga")]
    pub sharing_tag_name: String,

    /// Access mode: allow or deny
    #[schema(example = "allow")]
    pub access_mode: AccessMode,

    /// Grant creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
}

/// OIDC mapping for an access group
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupOidcMappingDto {
    /// Mapping ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// OIDC group name from the IdP
    #[schema(example = "library-staff")]
    pub oidc_group_name: String,

    /// Mapping creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
}

/// Create access group request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccessGroupRequest {
    /// Display name for the access group (must be unique)
    #[schema(example = "Manga Readers")]
    pub name: String,

    /// Optional description
    #[schema(example = "Users who can access manga content")]
    pub description: Option<String>,
}

/// Update access group request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccessGroupRequest {
    /// New display name (must be unique)
    #[schema(example = "Manga & Comic Readers")]
    pub name: Option<String>,

    /// New description (set to null to remove)
    #[schema(example = "Updated description")]
    pub description: Option<Option<String>>,
}

/// Add members to an access group request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddAccessGroupMembersRequest {
    /// User IDs to add to the group
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440000"]))]
    pub user_ids: Vec<Uuid>,
}

/// Add a tag grant to an access group request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddAccessGroupGrantRequest {
    /// Sharing tag ID to grant access to
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub sharing_tag_id: Uuid,

    /// Access mode: allow or deny
    #[schema(example = "allow")]
    pub access_mode: AccessMode,
}

/// Add an OIDC mapping to an access group request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddAccessGroupOidcMappingRequest {
    /// OIDC group name from the IdP
    #[schema(example = "library-staff")]
    pub oidc_group_name: String,
}

/// Access group summary (for user's group list)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupSummaryDto {
    /// Unique access group identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Display name
    #[schema(example = "Manga Readers")]
    pub name: String,

    /// Optional description
    #[schema(example = "Users who can access manga content")]
    pub description: Option<String>,
}

/// Effective grants response for a user (debug endpoint)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveGrantsResponse {
    /// User ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,

    /// Effective grants with source attribution
    pub grants: Vec<EffectiveGrantDto>,
}

/// A single effective grant with source attribution
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveGrantDto {
    /// Sharing tag ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub sharing_tag_id: Uuid,

    /// Sharing tag name
    #[schema(example = "manga")]
    pub sharing_tag_name: String,

    /// Access mode: allow or deny
    #[schema(example = "allow")]
    pub access_mode: AccessMode,

    /// Sources of this grant
    pub sources: Vec<GrantSourceDto>,
}

/// Source of a grant (user or group)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrantSourceDto {
    /// Source kind: "user" or "group"
    #[schema(example = "group")]
    pub kind: String,

    /// Group ID (only present for group sources)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub group_id: Option<Uuid>,

    /// Group name (only present for group sources)
    #[schema(example = "Manga Readers")]
    pub group_name: Option<String>,
}

// Conversion implementations

impl AccessGroupDto {
    pub fn from_model_with_counts(
        model: codex_db::entities::access_groups::Model,
        member_count: u64,
        grant_count: u64,
    ) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            member_count,
            grant_count,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<codex_db::entities::access_groups::Model> for AccessGroupSummaryDto {
    fn from(model: codex_db::entities::access_groups::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
        }
    }
}
