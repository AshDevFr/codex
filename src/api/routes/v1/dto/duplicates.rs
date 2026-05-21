use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A group of duplicate books
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Unique identifier for the duplicate group
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// SHA-256 hash of the file content
    #[schema(example = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")]
    pub file_hash: String,

    /// List of book IDs that share this hash
    pub book_ids: Vec<Uuid>,

    /// Number of duplicate copies found
    #[schema(example = 3)]
    pub duplicate_count: i32,

    /// When the duplicate was first detected
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub created_at: String,

    /// When the group was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: String,
}

/// Response for listing duplicates
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListDuplicatesResponse {
    /// List of duplicate groups
    pub duplicates: Vec<DuplicateGroup>,

    /// Total number of duplicate groups
    #[schema(example = 5)]
    pub total_groups: usize,

    /// Total number of books that are duplicates
    #[schema(example = 15)]
    pub total_duplicate_books: usize,
}

/// Response for triggering a duplicate scan
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TriggerDuplicateScanResponse {
    /// Task ID for tracking the scan progress
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub task_id: Uuid,

    /// Status message
    #[schema(example = "Duplicate scan started")]
    pub message: String,
}

/// A single series participating in a duplicate group, hydrated with the
/// fields the duplicate-detection UI needs to render each row.
///
/// Returning the title, library, book count, and last-updated timestamp on
/// the list endpoint lets the client render groups in one round trip, instead
/// of issuing a `GET /series/{id}` for every member.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDuplicateMember {
    /// Series UUID.
    pub id: Uuid,

    /// Display title, falling back to `series.name` when no metadata exists.
    #[schema(example = "Fairy Tail")]
    pub title: String,

    /// Library this series belongs to.
    pub library_id: Uuid,

    /// Library display name.
    #[schema(example = "Manga")]
    pub library_name: String,

    /// Number of (non-deleted) books in the series.
    #[schema(example = 63)]
    pub book_count: i64,

    /// Series row's last-updated timestamp.
    #[schema(example = "2026-02-15T00:00:00Z")]
    pub updated_at: String,
}

/// A group of duplicate series.
///
/// Two detection methods are surfaced through `match_type`:
/// - `external_id`: high-confidence match where two or more series resolve to
///   the same plugin/external identifier. Cross-library by design.
/// - `title`: medium-confidence match where two or more series in the same
///   library share the same normalized title.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDuplicateGroup {
    /// Unique identifier for the duplicate group
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// `external_id` or `title`
    #[schema(example = "external_id")]
    pub match_type: String,

    /// For `external_id` matches: "<source>:<external_id>" e.g. "plugin:mangabaka:12345".
    /// For `title` matches: the normalized search title (e.g. "naruto").
    #[schema(example = "plugin:mangabaka:12345")]
    pub match_key: String,

    /// Library this group is scoped to. Null for `external_id` matches.
    pub library_id: Option<Uuid>,

    /// Hydrated details for each series in the group, in the same order the
    /// detector emitted them. May be shorter than `duplicate_count` if a
    /// member series has since been deleted.
    pub members: Vec<SeriesDuplicateMember>,

    /// Number of series in the group at detection time. `members.len()` may
    /// be smaller if a member has been deleted between scan and read.
    #[schema(example = 2)]
    pub duplicate_count: i32,

    /// When the duplicate was first detected
    #[schema(example = "2026-05-20T10:30:00Z")]
    pub created_at: String,

    /// When the group was last updated
    #[schema(example = "2026-05-20T10:30:00Z")]
    pub updated_at: String,
}

/// Response for listing series duplicates.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListSeriesDuplicatesResponse {
    /// List of duplicate groups
    pub duplicates: Vec<SeriesDuplicateGroup>,

    /// Total number of duplicate groups (across both match types).
    #[schema(example = 3)]
    pub total_groups: usize,

    /// Total number of series that participate in any duplicate group.
    #[schema(example = 8)]
    pub total_duplicate_series: usize,

    /// Number of groups matched by external ID (high confidence).
    #[schema(example = 1)]
    pub external_id_groups: usize,

    /// Number of groups matched by normalized title (lower confidence).
    #[schema(example = 2)]
    pub title_groups: usize,
}

/// Query parameters for listing series duplicates.
#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListSeriesDuplicatesQuery {
    /// Optional filter: `external_id` or `title`.
    #[serde(default)]
    pub match_type: Option<String>,
}
