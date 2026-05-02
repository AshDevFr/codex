//! Recommendation DTOs
//!
//! Request and response types for the recommendations API endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A tag with relevance rank from the source service
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationTagDto {
    /// Tag name (e.g., "Isekai", "Gore")
    pub name: String,
    /// Relevance rank (0-100)
    pub rank: i32,
    /// Tag category (e.g., "Genre", "Theme")
    pub category: String,
}

/// A single recommendation for the user
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationDto {
    /// External ID on the source service
    pub external_id: String,
    /// URL to the entry on the external service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,
    /// Title of the recommended series/book
    pub title: String,
    /// Cover image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// Summary/description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Genres
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    /// Tags with relevance rank
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<RecommendationTagDto>>,
    /// Confidence/relevance score (0.0 to 1.0)
    pub score: f64,
    /// Human-readable reason for this recommendation
    pub reason: String,
    /// Titles that influenced this recommendation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub based_on: Vec<String>,
    /// Codex series ID if matched to an existing series
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_series_id: Option<String>,
    /// Whether this series is already in the user's library (as reported by the plugin)
    #[serde(default)]
    pub in_library: bool,
    /// Whether this series exists in the Codex library (matched via external IDs)
    #[serde(default)]
    pub in_codex: bool,
    /// Publication status (ongoing, ended, hiatus, abandoned, unknown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Media format (e.g., "MANGA", "NOVEL", "ONE_SHOT")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Country of origin ISO code (e.g., "JP", "KR", "CN")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_of_origin: Option<String>,
    /// Year the series started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_year: Option<i32>,
    /// Total expected number of volumes in the series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_volume_count: Option<i32>,
    /// Total expected number of chapters in the series. May be fractional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chapter_count: Option<f32>,
    /// Average user rating on the source service (0-100 scale)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    /// Popularity ranking/count on the source service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popularity: Option<i32>,
}

/// Response from GET /api/v1/user/recommendations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationsResponse {
    /// Personalized recommendations
    pub recommendations: Vec<RecommendationDto>,
    /// Plugin that provided these recommendations
    pub plugin_id: Uuid,
    /// Plugin display name
    pub plugin_name: String,
    /// When these recommendations were generated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    /// Whether these are cached results
    #[serde(default)]
    pub cached: bool,
    /// Status of a running/pending background task ("pending" or "running"), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    /// ID of the running/pending background task, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<Uuid>,
}

/// Response from POST /api/v1/user/recommendations/refresh
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationsRefreshResponse {
    /// Task ID for tracking the refresh operation
    pub task_id: Uuid,
    /// Human-readable status message
    pub message: String,
}

/// Request body for POST /api/v1/user/recommendations/{id}/dismiss
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DismissRecommendationRequest {
    /// Reason for dismissal
    #[serde(default)]
    pub reason: Option<String>,
}

/// Response from POST /api/v1/user/recommendations/{id}/dismiss
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DismissRecommendationResponse {
    /// Whether the dismissal was recorded
    pub dismissed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommendation_dto_skips_none_fields() {
        let dto = RecommendationDto {
            external_id: "1".to_string(),
            external_url: None,
            title: "Test".to_string(),
            cover_url: None,
            summary: None,
            genres: vec![],
            tags: None,
            score: 0.5,
            reason: "test".to_string(),
            based_on: vec![],
            codex_series_id: None,
            in_library: false,
            in_codex: false,
            status: None,
            format: None,
            country_of_origin: None,
            start_year: None,
            total_volume_count: None,
            total_chapter_count: None,
            rating: None,
            popularity: None,
        };
        let json = serde_json::to_value(&dto).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("externalUrl"));
        assert!(!obj.contains_key("coverUrl"));
        assert!(!obj.contains_key("summary"));
        assert!(!obj.contains_key("genres"));
        assert!(!obj.contains_key("basedOn"));
        assert!(!obj.contains_key("codexSeriesId"));
        assert!(!obj.contains_key("status"));
        assert!(!obj.contains_key("totalVolumeCount"));
        assert!(!obj.contains_key("totalChapterCount"));
        assert!(!obj.contains_key("rating"));
        assert!(!obj.contains_key("popularity"));
    }

    #[test]
    fn test_recommendations_response_serialization() {
        let resp = RecommendationsResponse {
            recommendations: vec![],
            plugin_id: Uuid::new_v4(),
            plugin_name: "AniList Recs".to_string(),
            generated_at: Some("2026-02-06T12:00:00Z".to_string()),
            cached: true,
            task_status: None,
            task_id: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["recommendations"].as_array().unwrap().is_empty());
        assert!(json["cached"].as_bool().unwrap());
        assert_eq!(json["pluginName"], "AniList Recs");
        // task_status and task_id should be absent when None
        assert!(json.get("taskStatus").is_none());
        assert!(json.get("taskId").is_none());
    }

    #[test]
    fn test_dismiss_request_with_reason() {
        let json = serde_json::json!({ "reason": "not_interested" });
        let req: DismissRecommendationRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.reason.unwrap(), "not_interested");
    }

    #[test]
    fn test_dismiss_request_without_reason() {
        let json = serde_json::json!({});
        let req: DismissRecommendationRequest = serde_json::from_value(json).unwrap();
        assert!(req.reason.is_none());
    }

    #[test]
    fn test_recommendations_response_with_task_status() {
        let task_id = Uuid::new_v4();
        let resp = RecommendationsResponse {
            recommendations: vec![],
            plugin_id: Uuid::new_v4(),
            plugin_name: "AniList Recs".to_string(),
            generated_at: None,
            cached: false,
            task_status: Some("pending".to_string()),
            task_id: Some(task_id),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["taskStatus"], "pending");
        assert_eq!(json["taskId"], task_id.to_string());
    }

    #[test]
    fn test_recommendations_response_with_running_status() {
        let task_id = Uuid::new_v4();
        let resp = RecommendationsResponse {
            recommendations: vec![],
            plugin_id: Uuid::new_v4(),
            plugin_name: "Test Plugin".to_string(),
            generated_at: Some("2026-02-11T10:00:00Z".to_string()),
            cached: true,
            task_status: Some("running".to_string()),
            task_id: Some(task_id),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["taskStatus"], "running");
        assert_eq!(json["taskId"], task_id.to_string());
        assert!(json["cached"].as_bool().unwrap());
    }
}
