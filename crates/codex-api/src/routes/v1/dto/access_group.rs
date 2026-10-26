//! DTOs for access group operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use codex_db::entities::access_groups::RuleMatch;
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

    /// Whether new users join this group when their account is created
    #[schema(example = false)]
    pub is_default: bool,

    /// Number of members in the group
    #[schema(example = 5)]
    pub member_count: u64,

    /// Number of sharing tag grants in the group
    #[schema(example = 3)]
    pub grant_count: u64,

    /// Number of rules in the group, across sharing tags, tags, genres and libraries
    #[schema(example = 5)]
    pub rule_count: u64,

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

    /// Whether new users join this group when their account is created
    #[schema(example = false)]
    pub is_default: bool,

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

    /// Make this the default group (clearing any other default), or stop it
    /// being the default. Only affects accounts created afterwards.
    #[serde(default)]
    #[schema(example = true)]
    pub is_default: Option<bool>,
}

/// Result of adding every user to an access group
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddAllUsersResponse {
    /// Users who were not members yet and have been added
    #[schema(example = 12)]
    pub added: u64,
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
        rule_count: u64,
    ) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            is_default: model.is_default,
            member_count,
            grant_count,
            rule_count,
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

// ==================== Rules ====================

/// Every rule of an access group
///
/// Within a group, values of the same type combine as "any of" or "all of"
/// (per the match field for that type; libraries are always "any of"), and
/// different types combine with AND. Across groups, what each group allows is
/// combined with OR. Denies from every group always win. Admins and
/// maintainers are never restricted.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupRulesDto {
    /// How allowed sharing tags combine
    pub sharing_tag_match: RuleMatch,
    /// How allowed tags combine
    pub tag_match: RuleMatch,
    /// How allowed genres combine
    pub genre_match: RuleMatch,
    /// Rules on sharing tags
    pub sharing_tags: Vec<SharingTagRuleDto>,
    /// Rules on regular tags, matched by name
    pub tags: Vec<NameRuleDto>,
    /// Rules on genres, matched by name
    pub genres: Vec<NameRuleDto>,
    /// Rules on libraries
    pub libraries: Vec<LibraryRuleDto>,
}

/// A rule on a sharing tag
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharingTagRuleDto {
    /// Sharing tag ID
    pub sharing_tag_id: Uuid,
    /// Sharing tag name
    #[schema(example = "Kids")]
    pub name: String,
    /// Allow or deny
    pub access_mode: AccessMode,
}

/// A rule on a regular tag or a genre
///
/// Matched by name, case-insensitively, so the rule keeps applying when the
/// tag or genre is removed by cleanup and created again later.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NameRuleDto {
    /// Tag or genre name
    #[schema(example = "Adult")]
    pub name: String,
    /// Allow or deny
    pub access_mode: AccessMode,
}

/// A rule on a library
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRuleDto {
    /// Library ID
    pub library_id: Uuid,
    /// Library name
    #[schema(example = "Manga")]
    pub name: String,
    /// Allow or deny
    pub access_mode: AccessMode,
}

/// Replace every rule of an access group
///
/// Omitted lists are treated as empty and omitted match fields as "any".
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ReplaceAccessGroupRulesRequest {
    /// How allowed sharing tags combine
    pub sharing_tag_match: RuleMatch,
    /// How allowed tags combine
    pub tag_match: RuleMatch,
    /// How allowed genres combine
    pub genre_match: RuleMatch,
    /// Rules on sharing tags
    pub sharing_tags: Vec<SharingTagRuleInput>,
    /// Rules on regular tags, matched by name
    pub tags: Vec<NameRuleDto>,
    /// Rules on genres, matched by name
    pub genres: Vec<NameRuleDto>,
    /// Rules on libraries
    pub libraries: Vec<LibraryRuleInput>,
}

/// A sharing tag rule in a replace request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharingTagRuleInput {
    /// Sharing tag ID
    pub sharing_tag_id: Uuid,
    /// Allow or deny
    pub access_mode: AccessMode,
}

/// A library rule in a replace request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRuleInput {
    /// Library ID
    pub library_id: Uuid,
    /// Allow or deny
    pub access_mode: AccessMode,
}
