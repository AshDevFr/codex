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
    let library = build_user_library(&state.db, auth.user_id)
        .await
        .map_err(|e| {
            warn!(
                user_id = %auth.user_id,
                error = %e,
                "Failed to build user library for recommendations"
            );
            ApiError::Internal(format!("Failed to build library data: {}", e))
        })?;

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

    let response = handle
        .call_method::<RecommendationRequest, RecommendationResponse>(
            methods::RECOMMENDATIONS_GET,
            request,
        )
        .await
        .map_err(|e| {
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
        .map(|r| RecommendationDto {
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
        })
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

    let response = handle
        .call_method::<RecommendationDismissRequest, crate::services::plugin::recommendations::RecommendationDismissResponse>(
            methods::RECOMMENDATIONS_DISMISS,
            dismiss_request,
        )
        .await
        .map_err(|e| {
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
