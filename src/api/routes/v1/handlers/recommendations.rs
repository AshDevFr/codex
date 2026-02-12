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
use crate::db::repositories::{
    PluginsRepository, SeriesExternalIdRepository, TaskRepository, UserPluginDataRepository,
    UserPluginsRepository,
};
use crate::services::plugin::protocol::PluginManifest;
use crate::services::plugin::recommendations::RecommendationResponse;
use crate::tasks::types::TaskType;
use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
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

/// Default max age for recommendations in hours before considered stale
const DEFAULT_RECOMMENDATIONS_MAX_AGE_HOURS: i64 = 24;

/// Get personalized recommendations
///
/// Returns cached recommendations from the database. If no cached data exists
/// or the data is stale, an empty list is returned and a background refresh
/// task is auto-triggered. The frontend should use SSE task progress events
/// to know when fresh data is ready.
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
    let (plugin, instance) = find_recommendation_plugin(&state.db, auth.user_id).await?;

    debug!(
        user_id = %auth.user_id,
        plugin_id = %plugin.id,
        "Reading cached recommendations from DB"
    );

    // Read cached recommendations from user_plugin_data
    let cached_entry = UserPluginDataRepository::get(&state.db, instance.id, "recommendations")
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read cached recommendations: {}", e)))?;

    // Try to deserialize cached data
    let cached_response = cached_entry.as_ref().and_then(|entry| {
        serde_json::from_value::<RecommendationResponse>(entry.data.clone()).ok()
    });

    // Check staleness
    let max_age_hours = plugin
        .config
        .get("recommendations_max_age_hours")
        .and_then(|v| v.as_i64())
        .unwrap_or(DEFAULT_RECOMMENDATIONS_MAX_AGE_HOURS);

    let is_stale = cached_entry.as_ref().is_none_or(|entry| {
        let age = Utc::now() - entry.updated_at;
        age.num_hours() >= max_age_hours
    });

    let has_data = cached_response.is_some();

    // Check for active task
    let active_task = TaskRepository::find_pending_or_processing_task(
        &state.db,
        "user_plugin_recommendations",
        plugin.id,
        auth.user_id,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to check task status: {}", e)))?;

    // Auto-trigger refresh if empty or stale and no task already running
    if is_stale && active_task.is_none() {
        debug!(
            user_id = %auth.user_id,
            plugin_id = %plugin.id,
            has_data = has_data,
            "Recommendations empty or stale, auto-triggering refresh task"
        );

        let task_type = TaskType::UserPluginRecommendations {
            plugin_id: plugin.id,
            user_id: auth.user_id,
        };

        match TaskRepository::enqueue(&state.db, task_type, 0, None).await {
            Ok(task_id) => {
                info!(
                    user_id = %auth.user_id,
                    plugin_id = %plugin.id,
                    task_id = %task_id,
                    "Auto-enqueued recommendations refresh task"
                );
                // Return response with the newly created task info
                let (mut recommendations, generated_at, cached) = match cached_response {
                    Some(resp) => (
                        resp.recommendations
                            .into_iter()
                            .map(to_recommendation_dto)
                            .collect(),
                        resp.generated_at,
                        true,
                    ),
                    None => (vec![], None, false),
                };

                enrich_and_filter_codex_presence(&state.db, &mut recommendations, &plugin).await;

                return Ok(Json(RecommendationsResponse {
                    recommendations,
                    plugin_id: plugin.id,
                    plugin_name: plugin.display_name.clone(),
                    generated_at,
                    cached,
                    task_status: Some("pending".to_string()),
                    task_id: Some(task_id),
                }));
            }
            Err(e) => {
                warn!(
                    user_id = %auth.user_id,
                    plugin_id = %plugin.id,
                    error = %e,
                    "Failed to auto-enqueue refresh task"
                );
            }
        }
    }

    // Map DB status "processing" → API "running" for frontend consistency
    let (task_status, task_id) = match active_task {
        Some((id, status)) => {
            let api_status = match status.as_str() {
                "processing" => "running",
                other => other,
            };
            (Some(api_status.to_string()), Some(id))
        }
        None => (None, None),
    };

    // Build response from cached data
    let (mut recommendations, generated_at, cached) = match cached_response {
        Some(resp) => (
            resp.recommendations
                .into_iter()
                .map(to_recommendation_dto)
                .collect(),
            resp.generated_at,
            true,
        ),
        None => (vec![], None, false),
    };

    enrich_and_filter_codex_presence(&state.db, &mut recommendations, &plugin).await;

    Ok(Json(RecommendationsResponse {
        recommendations,
        plugin_id: plugin.id,
        plugin_name: plugin.display_name.clone(),
        generated_at,
        cached,
        task_status,
        task_id,
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

/// Enrich recommendation DTOs with Codex library presence and filter out
/// series that already exist in the user's Codex library.
///
/// For each recommendation, checks whether its `external_id` maps to a Codex series
/// via `series_external_ids`. When matched, the recommendation is removed from the
/// list since the user already has it locally — there's no point recommending it.
async fn enrich_and_filter_codex_presence(
    db: &sea_orm::DatabaseConnection,
    recommendations: &mut Vec<RecommendationDto>,
    plugin: &crate::db::entities::plugins::Model,
) {
    // Resolve the external_id_source from the plugin manifest
    let source = plugin
        .manifest
        .as_ref()
        .and_then(|m| serde_json::from_value::<PluginManifest>(m.clone()).ok())
        .and_then(|m| m.capabilities.external_id_source);

    let Some(source) = source else {
        debug!("Plugin has no external_id_source — skipping Codex enrichment");
        return;
    };

    // Collect all external IDs from recommendations
    let external_ids: Vec<String> = recommendations
        .iter()
        .map(|r| r.external_id.clone())
        .collect();

    if external_ids.is_empty() {
        return;
    }

    // Batch lookup
    match SeriesExternalIdRepository::find_by_external_ids_and_source(db, &external_ids, &source)
        .await
    {
        Ok(matches) => {
            let before_count = recommendations.len();
            // Remove recommendations that map to a local Codex series
            recommendations.retain(|rec| !matches.contains_key(&rec.external_id));
            let filtered = before_count - recommendations.len();
            debug!(
                matched = matches.len(),
                filtered = filtered,
                remaining = recommendations.len(),
                total = before_count,
                "Enriched recommendations and filtered out local series"
            );
        }
        Err(e) => {
            warn!(error = %e, "Failed to enrich recommendations with Codex presence");
        }
    }
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
        in_codex: false,
        status: r.status.map(|s| s.to_string()),
        total_book_count: r.total_book_count,
        rating: r.rating,
        popularity: r.popularity,
    }
}

/// Dismiss a recommendation
///
/// Removes the recommendation from the cached list immediately and enqueues
/// a background task to notify the plugin asynchronously. Returns instantly.
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
    let (plugin, instance) = find_recommendation_plugin(&state.db, auth.user_id).await?;

    debug!(
        user_id = %auth.user_id,
        plugin_id = %plugin.id,
        external_id = %external_id,
        "Dismissing recommendation (non-blocking)"
    );

    // 1. Read cached recommendations from DB
    let cached_entry = UserPluginDataRepository::get(&state.db, instance.id, "recommendations")
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read cached recommendations: {}", e)))?;

    // 2. Filter out the dismissed entry and write back
    if let Some(entry) = cached_entry
        && let Ok(mut cached) = serde_json::from_value::<RecommendationResponse>(entry.data.clone())
    {
        let before_count = cached.recommendations.len();
        cached
            .recommendations
            .retain(|r| r.external_id != external_id);

        if cached.recommendations.len() < before_count {
            let updated_data = serde_json::to_value(&cached).map_err(|e| {
                ApiError::Internal(format!("Failed to serialize recommendations: {}", e))
            })?;

            UserPluginDataRepository::set(
                &state.db,
                instance.id,
                "recommendations",
                updated_data,
                None,
            )
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to update cached recommendations: {}", e))
            })?;
        }
    }

    // 3. Parse dismiss reason
    let reason = request.reason.and_then(|r| match r.as_str() {
        "not_interested" => Some("not_interested".to_string()),
        "already_read" => Some("already_read".to_string()),
        "already_owned" => Some("already_owned".to_string()),
        _ => None,
    });

    // 4. Enqueue async task to notify plugin
    let task_type = TaskType::UserPluginRecommendationDismiss {
        plugin_id: plugin.id,
        user_id: auth.user_id,
        external_id: external_id.clone(),
        reason,
    };

    if let Err(e) = TaskRepository::enqueue(&state.db, task_type, 0, None).await {
        warn!(
            plugin_id = %plugin.id,
            external_id = %external_id,
            error = %e,
            "Failed to enqueue dismiss task (dismissal from cache still succeeded)"
        );
    }

    Ok(Json(DismissRecommendationResponse { dismissed: true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::error::ApiError;
    use crate::services::plugin::handle::PluginError;
    use crate::services::plugin::process::ProcessError;
    use crate::services::plugin::recommendations::Recommendation;
    use crate::services::plugin::rpc::RpcError;
    use std::time::Duration;

    /// Map a `PluginError` to the appropriate `ApiError` with proper HTTP status codes.
    fn plugin_error_to_api_error(err: PluginError) -> ApiError {
        match &err {
            PluginError::Rpc(rpc_err) => match rpc_err {
                RpcError::RateLimited {
                    retry_after_seconds,
                } => ApiError::TooManyRequests(format!(
                    "Plugin rate limited. Retry after {} seconds",
                    retry_after_seconds
                )),
                RpcError::Timeout(duration) => ApiError::ServiceUnavailable(format!(
                    "Plugin request timed out after {:.0}s",
                    duration.as_secs_f64()
                )),
                RpcError::AuthFailed(msg) => {
                    ApiError::Unauthorized(format!("Plugin authentication failed: {}", msg))
                }
                RpcError::ConfigError(msg) => {
                    ApiError::ServiceUnavailable(format!("Plugin configuration error: {}", msg))
                }
                RpcError::Process(_) => ApiError::ServiceUnavailable(format!(
                    "Plugin process terminated unexpectedly: {}",
                    rpc_err
                )),
                _ => ApiError::Internal(format!("Plugin error: {}", err)),
            },
            PluginError::Process(_) => {
                ApiError::ServiceUnavailable(format!("Plugin process error: {}", err))
            }
            PluginError::Disabled { reason } => {
                ApiError::ServiceUnavailable(format!("Plugin disabled: {}", reason))
            }
            _ => ApiError::Internal(format!("Plugin error: {}", err)),
        }
    }

    /// Verify that the Recommendation → RecommendationDto mapping preserves all fields
    /// when all optional fields are populated.
    #[test]
    fn test_to_recommendation_dto_full_fields() {
        use crate::db::entities::SeriesStatus;

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
            status: Some(SeriesStatus::Ongoing),
            total_book_count: Some(27),
            rating: Some(90),
            popularity: Some(50000),
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
        assert!(!dto.in_codex); // in_codex defaults to false before enrichment
        assert_eq!(dto.status.as_deref(), Some("ongoing"));
        assert_eq!(dto.total_book_count, Some(27));
        assert_eq!(dto.rating, Some(90));
        assert_eq!(dto.popularity, Some(50000));
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
            status: None,
            total_book_count: None,
            rating: None,
            popularity: None,
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
        assert!(!dto.in_codex);
        assert!(dto.status.is_none());
        assert!(dto.total_book_count.is_none());
        assert!(dto.rating.is_none());
        assert!(dto.popularity.is_none());
    }

    /// Verify the full RecommendationsResponse can be serialized with the expected JSON shape.
    #[test]
    fn test_recommendations_response_json_shape() {
        use crate::db::entities::SeriesStatus;

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
                status: Some(SeriesStatus::Ongoing),
                total_book_count: Some(30),
                rating: Some(88),
                popularity: Some(75000),
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
                status: None,
                total_book_count: None,
                rating: None,
                popularity: None,
            }),
        ];

        let plugin_id = Uuid::new_v4();
        let response = RecommendationsResponse {
            recommendations: recs,
            plugin_id,
            plugin_name: "AniList Recommendations".to_string(),
            generated_at: Some("2026-02-09T12:00:00Z".to_string()),
            cached: true,
            task_status: None,
            task_id: None,
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
        assert!(!rec0["inCodex"].as_bool().unwrap());
        // codexSeriesId should be absent (None)
        assert!(rec0.get("codexSeriesId").is_none());
        assert_eq!(rec0["status"], "ongoing");
        assert_eq!(rec0["totalBookCount"], 30);
        assert_eq!(rec0["rating"], 88);
        assert_eq!(rec0["popularity"], 75000);

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
        assert!(!rec1["inCodex"].as_bool().unwrap());
        // Optional fields should be absent (None)
        assert!(rec1.get("status").is_none());
        assert!(rec1.get("totalBookCount").is_none());
        assert!(rec1.get("rating").is_none());
        assert!(rec1.get("popularity").is_none());
    }

    /// Verify that RecommendationResponse round-trips through serde_json::Value.
    /// This is the exact path used by the task handler (serialize to Value → write to DB)
    /// and the GET endpoint (read from DB → deserialize from Value).
    #[test]
    fn test_recommendation_response_round_trip_through_json_value() {
        use crate::services::plugin::recommendations::RecommendationResponse;

        let original = RecommendationResponse {
            recommendations: vec![Recommendation {
                external_id: "42".to_string(),
                external_url: Some("https://anilist.co/manga/42".to_string()),
                title: "Test Manga".to_string(),
                cover_url: Some("https://img.example.com/42.jpg".to_string()),
                summary: Some("A test manga".to_string()),
                genres: vec!["Action".to_string()],
                score: 0.85,
                reason: "Because you liked testing".to_string(),
                based_on: vec!["Unit Tests".to_string()],
                codex_series_id: None,
                in_library: false,
                status: None,
                total_book_count: None,
                rating: None,
                popularity: None,
            }],
            generated_at: Some("2026-02-11T16:00:00Z".to_string()),
            cached: false,
        };

        // Serialize to Value (what the task handler does before writing to DB)
        let value = serde_json::to_value(&original).unwrap();

        // Deserialize from Value (what the GET endpoint does when reading from DB)
        let restored: RecommendationResponse = serde_json::from_value(value).unwrap();

        assert_eq!(restored.recommendations.len(), 1);
        assert_eq!(restored.recommendations[0].external_id, "42");
        assert_eq!(restored.recommendations[0].title, "Test Manga");
        assert_eq!(
            restored.generated_at.as_deref(),
            Some("2026-02-11T16:00:00Z")
        );
        assert!(!restored.cached);
    }

    /// Verify that an empty RecommendationResponse round-trips correctly.
    /// This covers the case where a plugin returns zero recommendations.
    #[test]
    fn test_empty_recommendation_response_round_trip() {
        use crate::services::plugin::recommendations::RecommendationResponse;

        let original = RecommendationResponse {
            recommendations: vec![],
            generated_at: Some("2026-02-11T16:00:00Z".to_string()),
            cached: false,
        };

        let value = serde_json::to_value(&original).unwrap();
        let restored: RecommendationResponse = serde_json::from_value(value).unwrap();

        assert!(restored.recommendations.is_empty());
        assert_eq!(
            restored.generated_at.as_deref(),
            Some("2026-02-11T16:00:00Z")
        );
    }

    // --- plugin_error_to_api_error mapping tests ---

    #[test]
    fn test_rate_limited_maps_to_429() {
        let err = PluginError::Rpc(RpcError::RateLimited {
            retry_after_seconds: 60,
        });
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::TooManyRequests(ref msg) if msg.contains("60")),
            "Expected TooManyRequests, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_timeout_maps_to_503() {
        let err = PluginError::Rpc(RpcError::Timeout(Duration::from_secs(30)));
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::ServiceUnavailable(ref msg) if msg.contains("30")),
            "Expected ServiceUnavailable, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_auth_failed_maps_to_401() {
        let err = PluginError::Rpc(RpcError::AuthFailed("Invalid API key".to_string()));
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::Unauthorized(ref msg) if msg.contains("Invalid API key")),
            "Expected Unauthorized, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_config_error_maps_to_503() {
        let err = PluginError::Rpc(RpcError::ConfigError("API key is required".to_string()));
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::ServiceUnavailable(ref msg) if msg.contains("API key is required")),
            "Expected ServiceUnavailable, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_process_terminated_maps_to_503() {
        let err = PluginError::Rpc(RpcError::Process(ProcessError::ProcessTerminated));
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::ServiceUnavailable(_)),
            "Expected ServiceUnavailable, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_plugin_process_error_maps_to_503() {
        let err = PluginError::Process(ProcessError::ProcessTerminated);
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::ServiceUnavailable(_)),
            "Expected ServiceUnavailable, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_plugin_disabled_maps_to_503() {
        let err = PluginError::Disabled {
            reason: "Too many failures".to_string(),
        };
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::ServiceUnavailable(ref msg) if msg.contains("Too many failures")),
            "Expected ServiceUnavailable, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_not_initialized_maps_to_500() {
        let err = PluginError::NotInitialized;
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::Internal(_)),
            "Expected Internal, got {:?}",
            api_err
        );
    }

    #[test]
    fn test_generic_rpc_error_maps_to_500() {
        let err = PluginError::Rpc(RpcError::Cancelled);
        let api_err = plugin_error_to_api_error(err);
        assert!(
            matches!(api_err, ApiError::Internal(_)),
            "Expected Internal, got {:?}",
            api_err
        );
    }
}
