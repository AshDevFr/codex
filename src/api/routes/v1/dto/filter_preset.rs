//! DTOs for filter preset endpoints
//!
//! Unified preset storage: the same `FilterPresetDto` powers both list-page
//! saved filters (`scope = "list"`) and the advanced search page
//! (`scope = "search"`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::filter::{BookCondition, SeriesCondition};

/// Preset scope: where the saved filter is used.
pub const SCOPE_LIST: &str = "list";
pub const SCOPE_SEARCH: &str = "search";

/// Preset target entity.
pub const TARGET_SERIES: &str = "series";
pub const TARGET_BOOKS: &str = "books";

/// Request body for creating a new filter preset.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFilterPresetRequest {
    pub name: String,
    /// Where this preset is used: `"list"` or `"search"`.
    pub scope: String,
    /// Target entity: `"series"` or `"books"`.
    pub target: String,
    /// The saved condition. Must parse as `SeriesCondition` when
    /// `target = "series"` and as `BookCondition` when `target = "books"`.
    #[schema(value_type = Object)]
    pub condition: serde_json::Value,
    /// Optional saved text query (only meaningful for `scope = "search"` or
    /// list pages that have a search box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Optional saved sort key, mirrors the URL `sort` query parameter
    /// (e.g. `"title:asc"`, `"year:desc"`, `"relevance"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// Optional library scope; `None` means the preset applies globally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_id: Option<Uuid>,
}

/// Request body for updating an existing filter preset.
///
/// Treated as a full replacement of the mutable fields. `scope` and `target`
/// are immutable since the condition is validated against them on create.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFilterPresetRequest {
    pub name: String,
    #[schema(value_type = Object)]
    pub condition: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_id: Option<Uuid>,
}

/// Filter preset response payload.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilterPresetDto {
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub target: String,
    #[schema(value_type = Object)]
    pub condition: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FilterPresetDto {
    pub fn from_model(m: &crate::db::entities::filter_presets::Model) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            scope: m.scope.clone(),
            target: m.target.clone(),
            condition: m.condition.clone(),
            query: m.query.clone(),
            sort: m.sort.clone(),
            library_id: m.library_id,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

/// Response payload for listing filter presets.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilterPresetListResponse {
    pub presets: Vec<FilterPresetDto>,
}

/// Query string parameters for `GET /api/v1/filter-presets`.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListFilterPresetsQuery {
    /// Filter by scope (`"list"` or `"search"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Filter by target (`"series"` or `"books"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Filter by library id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_id: Option<Uuid>,
}

/// Validate a scope string. Returns `Err` for unknown values.
pub fn validate_scope(scope: &str) -> Result<(), String> {
    match scope {
        SCOPE_LIST | SCOPE_SEARCH => Ok(()),
        other => Err(format!(
            "scope must be '{SCOPE_LIST}' or '{SCOPE_SEARCH}', got '{other}'"
        )),
    }
}

/// Validate a target string. Returns `Err` for unknown values.
pub fn validate_target(target: &str) -> Result<(), String> {
    match target {
        TARGET_SERIES | TARGET_BOOKS => Ok(()),
        other => Err(format!(
            "target must be '{TARGET_SERIES}' or '{TARGET_BOOKS}', got '{other}'"
        )),
    }
}

/// Validate a condition payload by attempting to parse it into the right
/// variant based on `target`. Returns the parse error message if it fails,
/// so callers can surface it directly as a `BadRequest`.
pub fn validate_condition(target: &str, condition: &serde_json::Value) -> Result<(), String> {
    match target {
        TARGET_SERIES => serde_json::from_value::<SeriesCondition>(condition.clone())
            .map(|_| ())
            .map_err(|e| format!("condition does not parse as a SeriesCondition: {e}")),
        TARGET_BOOKS => serde_json::from_value::<BookCondition>(condition.clone())
            .map(|_| ())
            .map_err(|e| format!("condition does not parse as a BookCondition: {e}")),
        // validate_target should have caught this; defensive fallback.
        other => Err(format!("unknown target '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_scope_accepts_known_values() {
        assert!(validate_scope("list").is_ok());
        assert!(validate_scope("search").is_ok());
        assert!(validate_scope("other").is_err());
    }

    #[test]
    fn validate_target_accepts_known_values() {
        assert!(validate_target("series").is_ok());
        assert!(validate_target("books").is_ok());
        assert!(validate_target("nope").is_err());
    }

    #[test]
    fn validate_condition_parses_series_payload() {
        let cond = serde_json::json!({
            "allOf": [
                { "title": { "operator": "contains", "value": "ABC" } }
            ]
        });
        assert!(validate_condition("series", &cond).is_ok());
    }

    #[test]
    fn validate_condition_parses_book_payload() {
        let cond = serde_json::json!({
            "title": { "operator": "contains", "value": "punch" }
        });
        assert!(validate_condition("books", &cond).is_ok());
    }

    #[test]
    fn validate_condition_rejects_wrong_target() {
        // `bookType` is a Book-only variant; should fail under "series".
        let cond = serde_json::json!({
            "bookType": { "operator": "is", "value": "manga" }
        });
        assert!(validate_condition("series", &cond).is_err());
    }
}
