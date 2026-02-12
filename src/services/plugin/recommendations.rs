//! Recommendation Provider Protocol Types
//!
//! Defines the JSON-RPC request/response types for recommendation provider operations.
//! Recommendation providers generate personalized suggestions based on the user's
//! library, ratings, and reading history.
//!
//! ## Architecture
//!
//! Recommendation operations are initiated by the host (Codex) and sent to the plugin.
//! The plugin analyzes the user's library data and returns recommendations, optionally
//! using external APIs (e.g., AniList recommendations) and caching results via the
//! storage system.
//!
//! ## Methods
//!
//! - `recommendations/get` - Get personalized recommendations
//! - `recommendations/updateProfile` - Update taste profile from new activity
//! - `recommendations/clear` - Clear cached recommendations

use serde::{Deserialize, Serialize};

use super::protocol::{SeriesStatus, UserLibraryEntry};

// =============================================================================
// Recommendation Request
// =============================================================================

/// Parameters for `recommendations/get` method
///
/// Sends the user's library data to the plugin so it can generate
/// personalized recommendations. The plugin may use external APIs,
/// cached taste profiles, or both.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationRequest {
    /// User's library entries (series with ratings, progress, etc.)
    pub library: Vec<UserLibraryEntry>,
    /// Maximum number of recommendations to return
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// External IDs to exclude (e.g., series the user already has)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_ids: Vec<String>,
}

// =============================================================================
// Recommendation Response
// =============================================================================

/// Response from `recommendations/get` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResponse {
    /// List of personalized recommendations
    pub recommendations: Vec<Recommendation>,
    /// When this set of recommendations was generated (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    /// Whether these are cached results or freshly generated
    #[serde(default)]
    pub cached: bool,
}

/// A single recommendation from the plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// External ID on the source service (e.g., AniList media ID)
    pub external_id: String,
    /// URL to the entry on the external service
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,

    /// Title of the recommended series/book
    pub title: String,
    /// Cover image URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// Summary/description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Genres
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,

    /// Confidence/relevance score (0.0 to 1.0)
    pub score: f64,
    /// Human-readable reason for this recommendation
    /// e.g., "Because you rated Berserk 10/10"
    pub reason: String,
    /// Titles that influenced this recommendation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub based_on: Vec<String>,

    /// Codex series ID if matched to an existing series in the library
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_series_id: Option<String>,
    /// Whether this series is already in the user's library
    #[serde(default)]
    pub in_library: bool,

    /// Publication status of the series (ongoing, ended, hiatus, abandoned, unknown)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<SeriesStatus>,
    /// Total expected number of books/volumes in the series
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_book_count: Option<i32>,
    /// Average user rating on the source service (0-100 scale)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    /// Popularity ranking/count on the source service
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popularity: Option<i32>,
}

// =============================================================================
// Profile Update
// =============================================================================

/// Parameters for `recommendations/updateProfile` method
///
/// Notifies the plugin of new user activity so it can update the
/// taste profile used for generating recommendations.
#[allow(dead_code)] // Protocol contract: updateProfile method not yet invoked by host
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateRequest {
    /// Updated library entries (may be partial - only changed entries)
    pub entries: Vec<UserLibraryEntry>,
}

/// Response from `recommendations/updateProfile` method
#[allow(dead_code)] // Protocol contract: updateProfile method not yet invoked by host
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateResponse {
    /// Whether the profile was successfully updated
    pub updated: bool,
    /// Number of entries processed
    #[serde(default)]
    pub entries_processed: u32,
}

// =============================================================================
// Clear Recommendations
// =============================================================================

/// Response from `recommendations/clear` method
///
/// Clears cached recommendations, forcing a fresh generation on next request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationClearResponse {
    /// Whether the clear was successful
    pub cleared: bool,
}

// =============================================================================
// Dismiss Recommendation
// =============================================================================

/// Parameters for `recommendations/dismiss` method
///
/// Tells the plugin that the user is not interested in a recommendation,
/// so it can be excluded from future results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationDismissRequest {
    /// External ID of the recommendation to dismiss
    pub external_id: String,
    /// Reason for dismissal (optional, may help improve future recommendations)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<DismissReason>,
}

/// Reason for dismissing a recommendation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DismissReason {
    /// User is not interested
    NotInterested,
    /// User has already read it
    AlreadyRead,
    /// User already owns it (not in Codex library though)
    AlreadyOwned,
}

/// Response from `recommendations/dismiss` method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationDismissResponse {
    /// Whether the dismissal was recorded
    pub dismissed: bool,
}

// =============================================================================
// Method Validation
// =============================================================================

/// Check if a method name is a recommendation method
#[allow(dead_code)] // Protocol contract: mirrors is_storage_method() for recommendation methods
pub fn is_recommendation_method(method: &str) -> bool {
    matches!(
        method,
        "recommendations/get"
            | "recommendations/updateProfile"
            | "recommendations/clear"
            | "recommendations/dismiss"
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // Recommendation Request Tests
    // =========================================================================

    #[test]
    fn test_recommendation_request_serialization() {
        let req = RecommendationRequest {
            library: vec![UserLibraryEntry {
                series_id: "uuid-1".to_string(),
                title: "Berserk".to_string(),
                alternate_titles: vec![],
                year: Some(1989),
                status: None,
                genres: vec!["Action".to_string(), "Dark Fantasy".to_string()],
                tags: vec![],
                total_book_count: Some(41),
                external_ids: vec![],
                reading_status: None,
                books_read: 41,
                books_owned: 41,
                user_rating: Some(95),
                user_notes: None,
                started_at: None,
                last_read_at: None,
                completed_at: None,
            }],
            limit: Some(10),
            exclude_ids: vec!["99999".to_string()],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["library"].as_array().unwrap().len(), 1);
        assert_eq!(json["library"][0]["title"], "Berserk");
        assert_eq!(json["limit"], 10);
        assert_eq!(json["excludeIds"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_recommendation_request_minimal() {
        let json = json!({
            "library": []
        });
        let req: RecommendationRequest = serde_json::from_value(json).unwrap();
        assert!(req.library.is_empty());
        assert!(req.limit.is_none());
        assert!(req.exclude_ids.is_empty());
    }

    #[test]
    fn test_recommendation_request_skips_empty_exclude_ids() {
        let req = RecommendationRequest {
            library: vec![],
            limit: None,
            exclude_ids: vec![],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(!json.as_object().unwrap().contains_key("excludeIds"));
        assert!(!json.as_object().unwrap().contains_key("limit"));
    }

    // =========================================================================
    // Recommendation Response Tests
    // =========================================================================

    #[test]
    fn test_recommendation_response_serialization() {
        let resp = RecommendationResponse {
            recommendations: vec![Recommendation {
                external_id: "12345".to_string(),
                external_url: Some("https://anilist.co/manga/12345".to_string()),
                title: "Vinland Saga".to_string(),
                cover_url: Some("https://img.anilist.co/cover.jpg".to_string()),
                summary: Some("A Viking epic".to_string()),
                genres: vec!["Action".to_string(), "Historical".to_string()],
                score: 0.95,
                reason: "Because you rated Berserk 10/10".to_string(),
                based_on: vec!["Berserk".to_string()],
                codex_series_id: None,
                in_library: false,
                status: Some(SeriesStatus::Ongoing),
                total_book_count: Some(27),
                rating: Some(85),
                popularity: Some(120000),
            }],
            generated_at: Some("2026-02-06T12:00:00Z".to_string()),
            cached: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["recommendations"].as_array().unwrap().len(), 1);
        assert_eq!(json["recommendations"][0]["title"], "Vinland Saga");
        assert_eq!(json["recommendations"][0]["score"], 0.95);
        assert_eq!(json["recommendations"][0]["status"], "ongoing");
        assert_eq!(json["recommendations"][0]["totalBookCount"], 27);
        assert_eq!(json["recommendations"][0]["rating"], 85);
        assert_eq!(json["recommendations"][0]["popularity"], 120000);
        assert_eq!(json["generatedAt"], "2026-02-06T12:00:00Z");
        assert!(!json["cached"].as_bool().unwrap());
    }

    #[test]
    fn test_recommendation_response_cached() {
        let resp = RecommendationResponse {
            recommendations: vec![],
            generated_at: Some("2026-02-05T00:00:00Z".to_string()),
            cached: true,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["cached"].as_bool().unwrap());
        assert!(json["recommendations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_recommendation_response_minimal() {
        let json = json!({
            "recommendations": []
        });
        let resp: RecommendationResponse = serde_json::from_value(json).unwrap();
        assert!(resp.recommendations.is_empty());
        assert!(resp.generated_at.is_none());
        assert!(!resp.cached);
    }

    // =========================================================================
    // Recommendation Tests
    // =========================================================================

    #[test]
    fn test_recommendation_full_serialization() {
        let rec = Recommendation {
            external_id: "54321".to_string(),
            external_url: Some("https://anilist.co/manga/54321".to_string()),
            title: "Monster".to_string(),
            cover_url: Some("https://img.anilist.co/monster.jpg".to_string()),
            summary: Some("A psychological thriller".to_string()),
            genres: vec!["Thriller".to_string(), "Mystery".to_string()],
            score: 0.88,
            reason: "Based on your interest in psychological thrillers".to_string(),
            based_on: vec!["Death Note".to_string(), "20th Century Boys".to_string()],
            codex_series_id: Some("codex-uuid-123".to_string()),
            in_library: true,
            status: Some(SeriesStatus::Ended),
            total_book_count: Some(18),
            rating: Some(92),
            popularity: Some(85000),
        };
        let json = serde_json::to_value(&rec).unwrap();
        assert_eq!(json["externalId"], "54321");
        assert_eq!(json["externalUrl"], "https://anilist.co/manga/54321");
        assert_eq!(json["title"], "Monster");
        assert_eq!(json["coverUrl"], "https://img.anilist.co/monster.jpg");
        assert_eq!(json["summary"], "A psychological thriller");
        assert_eq!(json["genres"].as_array().unwrap().len(), 2);
        assert_eq!(json["score"], 0.88);
        assert_eq!(
            json["reason"],
            "Based on your interest in psychological thrillers"
        );
        assert_eq!(json["basedOn"].as_array().unwrap().len(), 2);
        assert_eq!(json["codexSeriesId"], "codex-uuid-123");
        assert!(json["inLibrary"].as_bool().unwrap());
        assert_eq!(json["status"], "ended");
        assert_eq!(json["totalBookCount"], 18);
        assert_eq!(json["rating"], 92);
        assert_eq!(json["popularity"], 85000);
    }

    #[test]
    fn test_recommendation_minimal() {
        let json = json!({
            "externalId": "99",
            "title": "Some Manga",
            "score": 0.5,
            "reason": "You might like it"
        });
        let rec: Recommendation = serde_json::from_value(json).unwrap();
        assert_eq!(rec.external_id, "99");
        assert_eq!(rec.title, "Some Manga");
        assert_eq!(rec.score, 0.5);
        assert_eq!(rec.reason, "You might like it");
        assert!(rec.external_url.is_none());
        assert!(rec.cover_url.is_none());
        assert!(rec.summary.is_none());
        assert!(rec.genres.is_empty());
        assert!(rec.based_on.is_empty());
        assert!(rec.codex_series_id.is_none());
        assert!(!rec.in_library);
        assert!(rec.status.is_none());
        assert!(rec.total_book_count.is_none());
        assert!(rec.rating.is_none());
        assert!(rec.popularity.is_none());
    }

    #[test]
    fn test_recommendation_skips_none_fields() {
        let rec = Recommendation {
            external_id: "1".to_string(),
            external_url: None,
            title: "Test".to_string(),
            cover_url: None,
            summary: None,
            genres: vec![],
            score: 0.7,
            reason: "test".to_string(),
            based_on: vec![],
            codex_series_id: None,
            in_library: false,
            status: None,
            total_book_count: None,
            rating: None,
            popularity: None,
        };
        let json = serde_json::to_value(&rec).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("externalUrl"));
        assert!(!obj.contains_key("coverUrl"));
        assert!(!obj.contains_key("summary"));
        assert!(!obj.contains_key("genres"));
        assert!(!obj.contains_key("basedOn"));
        assert!(!obj.contains_key("codexSeriesId"));
        assert!(!obj.contains_key("status"));
        assert!(!obj.contains_key("totalBookCount"));
        assert!(!obj.contains_key("rating"));
        assert!(!obj.contains_key("popularity"));
    }

    // =========================================================================
    // Profile Update Tests
    // =========================================================================

    #[test]
    fn test_profile_update_request_serialization() {
        let req = ProfileUpdateRequest {
            entries: vec![UserLibraryEntry {
                series_id: "uuid-2".to_string(),
                title: "One Piece".to_string(),
                alternate_titles: vec![],
                year: Some(1997),
                status: None,
                genres: vec!["Adventure".to_string()],
                tags: vec![],
                total_book_count: None,
                external_ids: vec![],
                reading_status: None,
                books_read: 100,
                books_owned: 105,
                user_rating: Some(90),
                user_notes: None,
                started_at: None,
                last_read_at: None,
                completed_at: None,
            }],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["entries"].as_array().unwrap().len(), 1);
        assert_eq!(json["entries"][0]["title"], "One Piece");
    }

    #[test]
    fn test_profile_update_response_serialization() {
        let resp = ProfileUpdateResponse {
            updated: true,
            entries_processed: 5,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["updated"].as_bool().unwrap());
        assert_eq!(json["entriesProcessed"], 5);
    }

    #[test]
    fn test_profile_update_response_deserialization() {
        let json = json!({
            "updated": true
        });
        let resp: ProfileUpdateResponse = serde_json::from_value(json).unwrap();
        assert!(resp.updated);
        assert_eq!(resp.entries_processed, 0);
    }

    // =========================================================================
    // Clear Recommendations Tests
    // =========================================================================

    #[test]
    fn test_recommendation_clear_response_serialization() {
        let resp = RecommendationClearResponse { cleared: true };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["cleared"].as_bool().unwrap());
    }

    #[test]
    fn test_recommendation_clear_response_deserialization() {
        let json = json!({"cleared": false});
        let resp: RecommendationClearResponse = serde_json::from_value(json).unwrap();
        assert!(!resp.cleared);
    }

    // =========================================================================
    // Dismiss Recommendation Tests
    // =========================================================================

    #[test]
    fn test_dismiss_request_serialization() {
        let req = RecommendationDismissRequest {
            external_id: "12345".to_string(),
            reason: Some(DismissReason::NotInterested),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["externalId"], "12345");
        assert_eq!(json["reason"], "not_interested");
    }

    #[test]
    fn test_dismiss_request_minimal() {
        let json = json!({
            "externalId": "99"
        });
        let req: RecommendationDismissRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.external_id, "99");
        assert!(req.reason.is_none());
    }

    #[test]
    fn test_dismiss_reason_serialization() {
        assert_eq!(
            serde_json::to_value(DismissReason::NotInterested).unwrap(),
            json!("not_interested")
        );
        assert_eq!(
            serde_json::to_value(DismissReason::AlreadyRead).unwrap(),
            json!("already_read")
        );
        assert_eq!(
            serde_json::to_value(DismissReason::AlreadyOwned).unwrap(),
            json!("already_owned")
        );
    }

    #[test]
    fn test_dismiss_reason_deserialization() {
        let reason: DismissReason = serde_json::from_value(json!("not_interested")).unwrap();
        assert_eq!(reason, DismissReason::NotInterested);

        let reason: DismissReason = serde_json::from_value(json!("already_read")).unwrap();
        assert_eq!(reason, DismissReason::AlreadyRead);

        let reason: DismissReason = serde_json::from_value(json!("already_owned")).unwrap();
        assert_eq!(reason, DismissReason::AlreadyOwned);
    }

    #[test]
    fn test_dismiss_response_serialization() {
        let resp = RecommendationDismissResponse { dismissed: true };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["dismissed"].as_bool().unwrap());
    }

    #[test]
    fn test_dismiss_request_skips_none_reason() {
        let req = RecommendationDismissRequest {
            external_id: "1".to_string(),
            reason: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(!json.as_object().unwrap().contains_key("reason"));
    }

    // =========================================================================
    // is_recommendation_method Tests
    // =========================================================================

    #[test]
    fn test_is_recommendation_method() {
        assert!(is_recommendation_method("recommendations/get"));
        assert!(is_recommendation_method("recommendations/updateProfile"));
        assert!(is_recommendation_method("recommendations/clear"));
        assert!(is_recommendation_method("recommendations/dismiss"));
        assert!(!is_recommendation_method("sync/getUserInfo"));
        assert!(!is_recommendation_method("storage/get"));
        assert!(!is_recommendation_method("metadata/series/search"));
        assert!(!is_recommendation_method("initialize"));
        assert!(!is_recommendation_method("recommendations/unknown"));
    }
}
