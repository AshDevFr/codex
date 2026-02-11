//! Recommendation Handlers
//!
//! Handlers for personalized recommendation endpoints.
//! These endpoints allow users to get recommendations from plugins,
//! refresh cached recommendations, and dismiss individual suggestions.

use super::super::dto::recommendations::{
    DismissRecommendationRequest, DismissRecommendationResponse, RecommendationDto,
    RecommendationsRefreshResponse, RecommendationsResponse,
};
use crate::api::extractors::auth::AuthContext;
use crate::api::{error::ApiError, extractors::AppState};
use crate::db::repositories::{PluginsRepository, TaskRepository, UserPluginsRepository};
use crate::services::plugin::library::build_user_library;
use crate::services::plugin::protocol::{PluginManifest, methods};
use crate::services::plugin::recommendations::{
    RecommendationDismissRequest, RecommendationRequest, RecommendationResponse,
};
use crate::tasks::types::TaskType;
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Find the user's recommendation plugin.
///
/// Returns the plugin definition and user plugin instance for the first enabled
/// recommendation provider plugin the user has connected.
async fn find_recommendation_plugin(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<
    (
        crate::db::entities::plugins::Model,
        crate::db::entities::user_plugins::Model,
    ),
    ApiError,
> {
    let user_instances = UserPluginsRepository::get_enabled_for_user(db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get user plugins: {}", e)))?;

    for instance in user_instances {
        let plugin = PluginsRepository::get_by_id(db, instance.plugin_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to get plugin: {}", e)))?;

        if let Some(plugin) = plugin {
            let is_rec_provider = plugin
                .manifest
                .as_ref()
                .and_then(|m| serde_json::from_value::<PluginManifest>(m.clone()).ok())
                .map(|m| m.capabilities.user_recommendation_provider)
                .unwrap_or(false);

            if is_rec_provider {
                return Ok((plugin, instance));
            }
        }
    }

    Err(ApiError::NotFound(
        "No recommendation plugin enabled. Enable a recommendation plugin in Settings > Integrations."
            .to_string(),
    ))
}

/// Get personalized recommendations
///
/// Returns recommendations from the user's enabled recommendation plugin.
/// The plugin may return cached results or generate fresh recommendations.
#[utoipa::path(
    get,
    path = "/api/v1/user/recommendations",
    responses(
        (status = 200, description = "Personalized recommendations", body = RecommendationsResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "No recommendation plugin enabled"),
    ),
    tag = "Recommendations"
)]
pub async fn get_recommendations(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<RecommendationsResponse>, ApiError> {
    let (plugin, _instance) = find_recommendation_plugin(&state.db, auth.user_id).await?;

    debug!(
        user_id = %auth.user_id,
        plugin_id = %plugin.id,
        "Fetching recommendations from plugin"
    );

    // Spawn plugin and call recommendations/get
    let (handle, _context) = state
        .plugin_manager
        .get_user_plugin_handle(plugin.id, auth.user_id)
        .await
        .map_err(|e| {
            warn!(
                plugin_id = %plugin.id,
                error = %e,
                "Failed to spawn recommendation plugin"
            );
            ApiError::Internal(format!("Failed to start recommendation plugin: {}", e))
        })?;

    // Build user's library data to seed recommendations
    let library = match build_user_library(&state.db, auth.user_id).await {
        Ok(lib) => lib,
        Err(e) => {
            warn!(
                user_id = %auth.user_id,
                error = %e,
                "Failed to build user library for recommendations"
            );
            // Stop the handle before returning the error
            if let Err(stop_err) = handle.stop().await {
                warn!(plugin_id = %plugin.id, error = %stop_err, "Failed to stop recommendation plugin handle");
            }
            return Err(ApiError::Internal(format!(
                "Failed to build library data: {}",
                e
            )));
        }
    };

    debug!(
        user_id = %auth.user_id,
        library_entries = library.len(),
        "Sending library data to recommendation plugin"
    );

    let request = RecommendationRequest {
        library,
        limit: Some(20),
        exclude_ids: vec![],
    };

    let result = handle
        .call_method::<RecommendationRequest, RecommendationResponse>(
            methods::RECOMMENDATIONS_GET,
            request,
        )
        .await;

    // Always stop the user plugin handle to clean up the spawned process
    if let Err(e) = handle.stop().await {
        warn!(plugin_id = %plugin.id, error = %e, "Failed to stop recommendation plugin handle");
    }

    let response = result.map_err(|e| {
        warn!(
            plugin_id = %plugin.id,
            error = %e,
            "Failed to get recommendations from plugin"
        );
        ApiError::Internal(format!("Recommendation plugin error: {}", e))
    })?;

    // Convert plugin response to API DTO
    let recommendations = response
        .recommendations
        .into_iter()
        .map(to_recommendation_dto)
        .collect();

    Ok(Json(RecommendationsResponse {
        recommendations,
        plugin_id: plugin.id,
        plugin_name: plugin.display_name.clone(),
        generated_at: response.generated_at,
        cached: response.cached,
    }))
}

/// Refresh recommendations
///
/// Enqueues a background task to regenerate recommendations by clearing
/// the cache and updating the taste profile.
#[utoipa::path(
    post,
    path = "/api/v1/user/recommendations/refresh",
    responses(
        (status = 200, description = "Refresh task enqueued", body = RecommendationsRefreshResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "No recommendation plugin enabled"),
        (status = 409, description = "Recommendation refresh already in progress"),
    ),
    tag = "Recommendations"
)]
pub async fn refresh_recommendations(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<RecommendationsRefreshResponse>, ApiError> {
    let (plugin, _instance) = find_recommendation_plugin(&state.db, auth.user_id).await?;

    // Check for duplicate pending/processing recommendation task
    let has_existing = TaskRepository::has_pending_or_processing(
        &state.db,
        "user_plugin_recommendations",
        plugin.id,
        auth.user_id,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to check existing tasks: {}", e)))?;

    if has_existing {
        return Err(ApiError::Conflict(
            "Recommendation refresh already in progress".to_string(),
        ));
    }

    let task_type = TaskType::UserPluginRecommendations {
        plugin_id: plugin.id,
        user_id: auth.user_id,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to enqueue recommendations task: {}", e))
        })?;

    info!(
        user_id = %auth.user_id,
        plugin_id = %plugin.id,
        task_id = %task_id,
        "Enqueued recommendations refresh task"
    );

    Ok(Json(RecommendationsRefreshResponse {
        task_id,
        message: format!("Refreshing recommendations from {}", plugin.display_name),
    }))
}

/// Convert a plugin Recommendation to an API RecommendationDto
///
/// This is extracted for testability — the handler maps the plugin's response
/// into the API response type field-by-field.
fn to_recommendation_dto(
    r: crate::services::plugin::recommendations::Recommendation,
) -> RecommendationDto {
    RecommendationDto {
        external_id: r.external_id,
        external_url: r.external_url,
        title: r.title,
        cover_url: r.cover_url,
        summary: r.summary,
        genres: r.genres,
        score: r.score,
        reason: r.reason,
        based_on: r.based_on,
        codex_series_id: r.codex_series_id,
        in_library: r.in_library,
    }
}

/// Dismiss a recommendation
///
/// Tells the recommendation plugin that the user is not interested in a
/// particular recommendation, so it can be excluded from future results.
#[utoipa::path(
    post,
    path = "/api/v1/user/recommendations/{external_id}/dismiss",
    params(
        ("external_id" = String, Path, description = "External ID of the recommendation to dismiss")
    ),
    request_body = DismissRecommendationRequest,
    responses(
        (status = 200, description = "Recommendation dismissed", body = DismissRecommendationResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "No recommendation plugin enabled"),
    ),
    tag = "Recommendations"
)]
pub async fn dismiss_recommendation(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(external_id): Path<String>,
    Json(request): Json<DismissRecommendationRequest>,
) -> Result<Json<DismissRecommendationResponse>, ApiError> {
    let (plugin, _instance) = find_recommendation_plugin(&state.db, auth.user_id).await?;

    debug!(
        user_id = %auth.user_id,
        plugin_id = %plugin.id,
        external_id = %external_id,
        "Dismissing recommendation"
    );

    // Spawn plugin and call recommendations/dismiss
    let (handle, _context) = state
        .plugin_manager
        .get_user_plugin_handle(plugin.id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to start recommendation plugin: {}", e)))?;

    let dismiss_request = RecommendationDismissRequest {
        external_id: external_id.clone(),
        reason: request.reason.and_then(|r| match r.as_str() {
            "not_interested" => {
                Some(crate::services::plugin::recommendations::DismissReason::NotInterested)
            }
            "already_read" => {
                Some(crate::services::plugin::recommendations::DismissReason::AlreadyRead)
            }
            "already_owned" => {
                Some(crate::services::plugin::recommendations::DismissReason::AlreadyOwned)
            }
            _ => None,
        }),
    };

    let result = handle
        .call_method::<RecommendationDismissRequest, crate::services::plugin::recommendations::RecommendationDismissResponse>(
            methods::RECOMMENDATIONS_DISMISS,
            dismiss_request,
        )
        .await;

    // Always stop the user plugin handle to clean up the spawned process
    if let Err(e) = handle.stop().await {
        warn!(plugin_id = %plugin.id, error = %e, "Failed to stop recommendation plugin handle");
    }

    let response = result.map_err(|e| {
        warn!(
            plugin_id = %plugin.id,
            error = %e,
            "Failed to dismiss recommendation"
        );
        ApiError::Internal(format!("Recommendation plugin error: {}", e))
    })?;

    Ok(Json(DismissRecommendationResponse {
        dismissed: response.dismissed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plugin::recommendations::Recommendation;

    /// Verify that the Recommendation → RecommendationDto mapping preserves all fields
    /// when all optional fields are populated.
    #[test]
    fn test_to_recommendation_dto_full_fields() {
        let rec = Recommendation {
            external_id: "12345".to_string(),
            external_url: Some("https://anilist.co/manga/12345".to_string()),
            title: "Vinland Saga".to_string(),
            cover_url: Some("https://img.anilist.co/cover.jpg".to_string()),
            summary: Some("A Viking epic about revenge and redemption".to_string()),
            genres: vec!["Action".to_string(), "Historical".to_string()],
            score: 0.95,
            reason: "Because you rated Berserk 10/10".to_string(),
            based_on: vec!["Berserk".to_string(), "Vagabond".to_string()],
            codex_series_id: Some("codex-uuid-abc".to_string()),
            in_library: true,
        };

        let dto = to_recommendation_dto(rec);

        assert_eq!(dto.external_id, "12345");
        assert_eq!(
            dto.external_url.as_deref(),
            Some("https://anilist.co/manga/12345")
        );
        assert_eq!(dto.title, "Vinland Saga");
        assert_eq!(
            dto.cover_url.as_deref(),
            Some("https://img.anilist.co/cover.jpg")
        );
        assert_eq!(
            dto.summary.as_deref(),
            Some("A Viking epic about revenge and redemption")
        );
        assert_eq!(dto.genres, vec!["Action", "Historical"]);
        assert!((dto.score - 0.95).abs() < f64::EPSILON);
        assert_eq!(dto.reason, "Because you rated Berserk 10/10");
        assert_eq!(dto.based_on, vec!["Berserk", "Vagabond"]);
        assert_eq!(dto.codex_series_id.as_deref(), Some("codex-uuid-abc"));
        assert!(dto.in_library);
    }

    /// Verify that the mapping handles minimal recommendations (None/empty optional fields).
    #[test]
    fn test_to_recommendation_dto_minimal_fields() {
        let rec = Recommendation {
            external_id: "99".to_string(),
            external_url: None,
            title: "Some Manga".to_string(),
            cover_url: None,
            summary: None,
            genres: vec![],
            score: 0.5,
            reason: "You might like it".to_string(),
            based_on: vec![],
            codex_series_id: None,
            in_library: false,
        };

        let dto = to_recommendation_dto(rec);

        assert_eq!(dto.external_id, "99");
        assert!(dto.external_url.is_none());
        assert_eq!(dto.title, "Some Manga");
        assert!(dto.cover_url.is_none());
        assert!(dto.summary.is_none());
        assert!(dto.genres.is_empty());
        assert!((dto.score - 0.5).abs() < f64::EPSILON);
        assert_eq!(dto.reason, "You might like it");
        assert!(dto.based_on.is_empty());
        assert!(dto.codex_series_id.is_none());
        assert!(!dto.in_library);
    }

    /// Verify the full RecommendationsResponse can be serialized with the expected JSON shape.
    #[test]
    fn test_recommendations_response_json_shape() {
        let recs = vec![
            to_recommendation_dto(Recommendation {
                external_id: "1".to_string(),
                external_url: Some("https://example.com/1".to_string()),
                title: "Manga A".to_string(),
                cover_url: Some("https://img.example.com/a.jpg".to_string()),
                summary: Some("Description A".to_string()),
                genres: vec!["Action".to_string()],
                score: 0.9,
                reason: "Based on your library".to_string(),
                based_on: vec!["Source A".to_string()],
                codex_series_id: None,
                in_library: false,
            }),
            to_recommendation_dto(Recommendation {
                external_id: "2".to_string(),
                external_url: None,
                title: "Manga B".to_string(),
                cover_url: None,
                summary: None,
                genres: vec![],
                score: 0.7,
                reason: "Popular in your genre".to_string(),
                based_on: vec![],
                codex_series_id: Some("series-id".to_string()),
                in_library: true,
            }),
        ];

        let plugin_id = Uuid::new_v4();
        let response = RecommendationsResponse {
            recommendations: recs,
            plugin_id,
            plugin_name: "AniList Recommendations".to_string(),
            generated_at: Some("2026-02-09T12:00:00Z".to_string()),
            cached: true,
        };

        let json = serde_json::to_value(&response).unwrap();

        // Top-level fields
        assert_eq!(json["pluginId"], plugin_id.to_string());
        assert_eq!(json["pluginName"], "AniList Recommendations");
        assert_eq!(json["generatedAt"], "2026-02-09T12:00:00Z");
        assert!(json["cached"].as_bool().unwrap());

        // Recommendations array
        let recs_arr = json["recommendations"].as_array().unwrap();
        assert_eq!(recs_arr.len(), 2);

        // First recommendation (full fields)
        let rec0 = &recs_arr[0];
        assert_eq!(rec0["externalId"], "1");
        assert_eq!(rec0["externalUrl"], "https://example.com/1");
        assert_eq!(rec0["title"], "Manga A");
        assert_eq!(rec0["coverUrl"], "https://img.example.com/a.jpg");
        assert_eq!(rec0["summary"], "Description A");
        assert_eq!(rec0["genres"].as_array().unwrap().len(), 1);
        assert_eq!(rec0["score"], 0.9);
        assert_eq!(rec0["reason"], "Based on your library");
        assert_eq!(rec0["basedOn"].as_array().unwrap().len(), 1);
        assert!(!rec0["inLibrary"].as_bool().unwrap());
        // codexSeriesId should be absent (None)
        assert!(rec0.get("codexSeriesId").is_none());

        // Second recommendation (minimal fields — optional fields absent)
        let rec1 = &recs_arr[1];
        assert_eq!(rec1["externalId"], "2");
        assert!(rec1.get("externalUrl").is_none());
        assert_eq!(rec1["title"], "Manga B");
        assert!(rec1.get("coverUrl").is_none());
        assert!(rec1.get("summary").is_none());
        assert!(rec1.get("genres").is_none()); // empty vec is skipped
        assert_eq!(rec1["score"], 0.7);
        assert!(rec1.get("basedOn").is_none()); // empty vec is skipped
        assert_eq!(rec1["codexSeriesId"], "series-id");
        assert!(rec1["inLibrary"].as_bool().unwrap());
    }
}
