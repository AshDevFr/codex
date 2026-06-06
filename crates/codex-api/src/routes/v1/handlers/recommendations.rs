//! Recommendation Handlers
//!
//! Handlers for personalized recommendation endpoints.
//! These endpoints allow users to get recommendations from plugins,
//! refresh cached recommendations, and dismiss individual suggestions.

use super::super::dto::recommendations::{
    DismissRecommendationRequest, DismissRecommendationResponse, RecommendationDto,
    RecommendationSourceDto, RecommendationsRefreshResponse, RecommendationsResponse,
};
use crate::extractors::auth::AuthContext;
use crate::{error::ApiError, extractors::AppState};
use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use codex_db::repositories::{
    PluginsRepository, SeriesExternalIdRepository, TaskRepository, UserPluginDataRepository,
    UserPluginsRepository,
};
use codex_services::plugin::protocol::PluginManifest;
use codex_services::plugin::recommendations::RecommendationResponse;
use codex_tasks::types::TaskType;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

type RecPlugin = (
    codex_db::entities::plugins::Model,
    codex_db::entities::user_plugins::Model,
);

/// Find ALL of the user's enabled recommendation-provider plugin instances.
///
/// A user can enable the same plugin more than once (as distinct instances,
/// e.g. scoped to different libraries); every enabled recommendation provider
/// contributes to the merged response.
async fn find_recommendation_plugins(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<RecPlugin>, ApiError> {
    let user_instances = UserPluginsRepository::get_enabled_for_user(db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get user plugins: {}", e)))?;

    let mut result = Vec::new();
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
                result.push((plugin, instance));
            }
        }
    }

    if result.is_empty() {
        return Err(ApiError::NotFound(
            "No recommendation plugin enabled. Enable a recommendation plugin in Settings > Integrations."
                .to_string(),
        ));
    }

    Ok(result)
}

/// Resolve a plugin's `external_id_source` (e.g. "anilist"), if declared.
fn plugin_source(plugin: &codex_db::entities::plugins::Model) -> Option<String> {
    plugin
        .manifest
        .as_ref()
        .and_then(|m| serde_json::from_value::<PluginManifest>(m.clone()).ok())
        .and_then(|m| m.capabilities.external_id_source)
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
    let instances = find_recommendation_plugins(&state.db, auth.user_id).await?;

    let mut all_recommendations: Vec<RecommendationDto> = Vec::new();
    let mut sources: Vec<RecommendationSourceDto> = Vec::new();

    for (plugin, instance) in &instances {
        let (recs, source) =
            collect_instance_recommendations(&state, auth.user_id, plugin, instance).await;
        all_recommendations.extend(recs);
        sources.push(source);
    }

    // Merge across instances: dedupe by external ID (highest score wins,
    // reasons combined), then order by score desc.
    let recommendations = merge_recommendations(all_recommendations);

    Ok(Json(RecommendationsResponse {
        recommendations,
        sources,
    }))
}

/// Read one instance's cached recommendations, handle staleness/auto-refresh,
/// enrich with Codex presence, stamp provenance, and return its contribution
/// plus a source-status entry.
async fn collect_instance_recommendations(
    state: &AppState,
    user_id: Uuid,
    plugin: &codex_db::entities::plugins::Model,
    instance: &codex_db::entities::user_plugins::Model,
) -> (Vec<RecommendationDto>, RecommendationSourceDto) {
    let source = plugin_source(plugin).unwrap_or_default();

    let cached_entry = UserPluginDataRepository::get(&state.db, instance.id, "recommendations")
        .await
        .unwrap_or(None);

    let cached_response = cached_entry.as_ref().and_then(|entry| {
        serde_json::from_value::<RecommendationResponse>(entry.data.clone()).ok()
    });

    let max_age_hours = plugin
        .config
        .get("recommendations_max_age_hours")
        .and_then(|v| v.as_i64())
        .unwrap_or(DEFAULT_RECOMMENDATIONS_MAX_AGE_HOURS);

    let is_stale = cached_entry.as_ref().is_none_or(|entry| {
        let age = Utc::now() - entry.updated_at;
        age.num_hours() >= max_age_hours
    });

    // Per-instance active-task guard so we never double-enqueue.
    let active_task = TaskRepository::find_pending_or_processing_task(
        &state.db,
        "user_plugin_recommendations",
        plugin.id,
        user_id,
    )
    .await
    .unwrap_or(None);

    // Auto-trigger a refresh for this instance if stale and nothing running.
    let (task_status, task_id) = if is_stale && active_task.is_none() {
        let task_type = TaskType::UserPluginRecommendations {
            plugin_id: plugin.id,
            user_id,
        };
        match TaskRepository::enqueue(&state.db, task_type, None).await {
            Ok(task_id) => {
                info!(
                    user_id = %user_id,
                    plugin_id = %plugin.id,
                    task_id = %task_id,
                    "Auto-enqueued recommendations refresh task"
                );
                (Some("pending".to_string()), Some(task_id))
            }
            Err(e) => {
                warn!(
                    user_id = %user_id,
                    plugin_id = %plugin.id,
                    error = %e,
                    "Failed to auto-enqueue refresh task"
                );
                (None, None)
            }
        }
    } else {
        // Map DB status "processing" → API "running" for frontend consistency.
        match active_task {
            Some((id, status)) => {
                let api_status = match status.as_str() {
                    "processing" => "running",
                    other => other,
                };
                (Some(api_status.to_string()), Some(id))
            }
            None => (None, None),
        }
    };

    let (mut recommendations, generated_at, cached) = match cached_response {
        Some(resp) => (
            resp.recommendations
                .into_iter()
                .map(|r| to_recommendation_dto(r, plugin.display_name.clone(), source.clone()))
                .collect(),
            resp.generated_at,
            true,
        ),
        None => (Vec::new(), None, false),
    };

    enrich_and_filter_codex_presence(&state.db, &mut recommendations, plugin).await;

    let source_dto = RecommendationSourceDto {
        plugin_id: plugin.id,
        plugin_name: plugin.display_name.clone(),
        source,
        generated_at,
        cached,
        task_status,
        task_id,
    };

    (recommendations, source_dto)
}

/// Merge recommendations from several instances into one list: dedupe by
/// external ID keeping the highest score, combining the distinct reasons and
/// unioning `based_on`; then order by score descending. Stable for equal
/// scores (insertion order preserved).
fn merge_recommendations(recs: Vec<RecommendationDto>) -> Vec<RecommendationDto> {
    use std::collections::HashMap;

    let mut order: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, RecommendationDto> = HashMap::new();

    for rec in recs {
        match by_id.get_mut(&rec.external_id) {
            None => {
                order.push(rec.external_id.clone());
                by_id.insert(rec.external_id.clone(), rec);
            }
            Some(existing) => {
                let combined_reason = combine_reasons(&existing.reason, &rec.reason);
                let combined_based_on = union_strings(&existing.based_on, &rec.based_on);
                let in_codex = existing.in_codex || rec.in_codex;
                let in_library = existing.in_library || rec.in_library;

                if rec.score > existing.score {
                    // Higher-scoring contributor becomes the base.
                    let mut winner = rec;
                    winner.reason = combined_reason;
                    winner.based_on = combined_based_on;
                    winner.in_codex = in_codex;
                    winner.in_library = in_library;
                    *existing = winner;
                } else {
                    existing.reason = combined_reason;
                    existing.based_on = combined_based_on;
                    existing.in_codex = in_codex;
                    existing.in_library = in_library;
                }
            }
        }
    }

    let mut merged: Vec<RecommendationDto> = order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect();

    // Stable sort: equal scores keep insertion order.
    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    merged
}

/// Combine two recommendation reasons, keeping distinct text joined with " · ".
fn combine_reasons(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    if b.is_empty() || a == b || a.contains(b) {
        return a.to_string();
    }
    if b.contains(a) {
        return b.to_string();
    }
    format!("{a} · {b}")
}

/// Union of two string lists preserving order, first-seen wins.
fn union_strings(a: &[String], b: &[String]) -> Vec<String> {
    let mut out = a.to_vec();
    for s in b {
        if !out.contains(s) {
            out.push(s.clone());
        }
    }
    out
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
    let instances = find_recommendation_plugins(&state.db, auth.user_id).await?;

    // Enqueue a refresh for every instance that doesn't already have one
    // running. Conflict only if ALL instances are already refreshing.
    let mut task_ids = Vec::new();
    let mut names = Vec::new();
    let mut already_running = 0usize;

    for (plugin, _instance) in &instances {
        let has_existing = TaskRepository::has_pending_or_processing(
            &state.db,
            "user_plugin_recommendations",
            plugin.id,
            auth.user_id,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check existing tasks: {}", e)))?;

        if has_existing {
            already_running += 1;
            continue;
        }

        let task_type = TaskType::UserPluginRecommendations {
            plugin_id: plugin.id,
            user_id: auth.user_id,
        };

        let task_id = TaskRepository::enqueue(&state.db, task_type, None)
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
        task_ids.push(task_id);
        names.push(plugin.display_name.clone());
    }

    if task_ids.is_empty() && already_running == instances.len() {
        return Err(ApiError::Conflict(
            "Recommendation refresh already in progress".to_string(),
        ));
    }

    Ok(Json(RecommendationsRefreshResponse {
        task_ids,
        message: format!("Refreshing recommendations from {}", names.join(", ")),
    }))
}

/// Enrich recommendation DTOs with Codex library presence.
///
/// For each recommendation, checks whether its `external_id` maps to a Codex series
/// via `series_external_ids`. When matched, sets `in_codex = true` and populates
/// `codex_series_id` so the frontend can link to the local series.
async fn enrich_and_filter_codex_presence(
    db: &sea_orm::DatabaseConnection,
    recommendations: &mut [RecommendationDto],
    plugin: &codex_db::entities::plugins::Model,
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
            let mut enriched = 0;
            for rec in recommendations.iter_mut() {
                if let Some(ext_id_record) = matches.get(&rec.external_id) {
                    rec.in_codex = true;
                    rec.codex_series_id = Some(ext_id_record.series_id.to_string());
                    enriched += 1;
                }
            }
            debug!(
                matched = matches.len(),
                enriched = enriched,
                total = external_ids.len(),
                "Enriched recommendations with Codex library presence"
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
    r: codex_services::plugin::recommendations::Recommendation,
    source_plugin: String,
    source: String,
) -> RecommendationDto {
    use super::super::dto::recommendations::RecommendationTagDto;

    RecommendationDto {
        external_id: r.external_id,
        external_url: r.external_url,
        title: r.title,
        cover_url: r.cover_url,
        summary: r.summary,
        genres: r.genres,
        tags: r.tags.map(|tags| {
            tags.into_iter()
                .map(|t| RecommendationTagDto {
                    name: t.name,
                    rank: t.rank,
                    category: t.category,
                })
                .collect()
        }),
        score: r.score,
        reason: r.reason,
        based_on: r.based_on,
        codex_series_id: r.codex_series_id,
        in_library: r.in_library,
        in_codex: false,
        status: r.status.map(|s| s.to_string()),
        format: r.format,
        country_of_origin: r.country_of_origin,
        start_year: r.start_year,
        total_volume_count: r.total_volume_count,
        total_chapter_count: r.total_chapter_count,
        rating: r.rating,
        popularity: r.popularity,
        source_plugin,
        source,
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
    let instances = find_recommendation_plugins(&state.db, auth.user_id).await?;

    debug!(
        user_id = %auth.user_id,
        external_id = %external_id,
        instances = instances.len(),
        "Dismissing recommendation across all instances (non-blocking)"
    );

    // Parse dismiss reason once.
    let reason = request.reason.and_then(|r| match r.as_str() {
        "not_interested" => Some("not_interested".to_string()),
        "already_read" => Some("already_read".to_string()),
        "already_owned" => Some("already_owned".to_string()),
        _ => None,
    });

    // The same external ID can appear in multiple instances' caches (the same
    // title recommended by more than one provider). Remove it from each cache
    // that has it and notify only those plugins.
    for (plugin, instance) in &instances {
        let cached_entry = UserPluginDataRepository::get(&state.db, instance.id, "recommendations")
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to read cached recommendations: {}", e))
            })?;

        let removed = if let Some(entry) = cached_entry
            && let Ok(mut cached) =
                serde_json::from_value::<RecommendationResponse>(entry.data.clone())
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
                true
            } else {
                false
            }
        } else {
            false
        };

        // Only notify plugins whose cache actually contained the item.
        if removed {
            let task_type = TaskType::UserPluginRecommendationDismiss {
                plugin_id: plugin.id,
                user_id: auth.user_id,
                external_id: external_id.clone(),
                reason: reason.clone(),
            };

            if let Err(e) = TaskRepository::enqueue(&state.db, task_type, None).await {
                warn!(
                    plugin_id = %plugin.id,
                    external_id = %external_id,
                    error = %e,
                    "Failed to enqueue dismiss task (dismissal from cache still succeeded)"
                );
            }
        }
    }

    Ok(Json(DismissRecommendationResponse { dismissed: true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ApiError;
    use codex_services::plugin::handle::PluginError;
    use codex_services::plugin::process::ProcessError;
    use codex_services::plugin::recommendations::Recommendation;
    use codex_services::plugin::rpc::RpcError;
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
        use codex_db::entities::SeriesStatus;

        let rec = Recommendation {
            external_id: "12345".to_string(),
            external_url: Some("https://anilist.co/manga/12345".to_string()),
            title: "Vinland Saga".to_string(),
            cover_url: Some("https://img.anilist.co/cover.jpg".to_string()),
            summary: Some("A Viking epic about revenge and redemption".to_string()),
            genres: vec!["Action".to_string(), "Historical".to_string()],
            tags: None,
            score: 0.95,
            reason: "Because you rated Berserk 10/10".to_string(),
            based_on: vec!["Berserk".to_string(), "Vagabond".to_string()],
            codex_series_id: Some("codex-uuid-abc".to_string()),
            in_library: true,
            status: Some(SeriesStatus::Ongoing),
            format: Some("MANGA".to_string()),
            country_of_origin: Some("JP".to_string()),
            start_year: Some(2005),
            total_volume_count: Some(27),
            total_chapter_count: None,
            rating: Some(90),
            popularity: Some(50000),
        };

        let dto = to_recommendation_dto(rec, "AniList Recs".to_string(), "anilist".to_string());

        assert_eq!(dto.source_plugin, "AniList Recs");
        assert_eq!(dto.source, "anilist");
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
        assert_eq!(dto.total_volume_count, Some(27));
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
            tags: None,
            score: 0.5,
            reason: "You might like it".to_string(),
            based_on: vec![],
            codex_series_id: None,
            in_library: false,
            status: None,
            format: None,
            country_of_origin: None,
            start_year: None,
            total_volume_count: None,
            total_chapter_count: None,
            rating: None,
            popularity: None,
        };

        let dto = to_recommendation_dto(rec, "AniList Recs".to_string(), "anilist".to_string());

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
        assert!(dto.total_volume_count.is_none());
        assert!(dto.rating.is_none());
        assert!(dto.popularity.is_none());
    }

    /// Verify the full RecommendationsResponse can be serialized with the expected JSON shape.
    #[test]
    fn test_recommendations_response_json_shape() {
        use codex_db::entities::SeriesStatus;

        let recs = vec![
            to_recommendation_dto(
                Recommendation {
                    external_id: "1".to_string(),
                    external_url: Some("https://example.com/1".to_string()),
                    title: "Manga A".to_string(),
                    cover_url: Some("https://img.example.com/a.jpg".to_string()),
                    summary: Some("Description A".to_string()),
                    genres: vec!["Action".to_string()],
                    tags: None,
                    score: 0.9,
                    reason: "Based on your library".to_string(),
                    based_on: vec!["Source A".to_string()],
                    codex_series_id: None,
                    in_library: false,
                    status: Some(SeriesStatus::Ongoing),
                    format: Some("MANGA".to_string()),
                    country_of_origin: Some("JP".to_string()),
                    start_year: Some(2005),
                    total_volume_count: Some(30),
                    total_chapter_count: None,
                    rating: Some(88),
                    popularity: Some(75000),
                },
                "AniList Recs".to_string(),
                "anilist".to_string(),
            ),
            to_recommendation_dto(
                Recommendation {
                    external_id: "2".to_string(),
                    external_url: None,
                    title: "Manga B".to_string(),
                    cover_url: None,
                    summary: None,
                    genres: vec![],
                    tags: None,
                    score: 0.7,
                    reason: "Popular in your genre".to_string(),
                    based_on: vec![],
                    codex_series_id: Some("series-id".to_string()),
                    in_library: true,
                    status: None,
                    format: None,
                    country_of_origin: None,
                    start_year: None,
                    total_volume_count: None,
                    total_chapter_count: None,
                    rating: None,
                    popularity: None,
                },
                "AniList Recs".to_string(),
                "anilist".to_string(),
            ),
        ];

        let plugin_id = Uuid::new_v4();
        let response = RecommendationsResponse {
            recommendations: recs,
            sources: vec![RecommendationSourceDto {
                plugin_id,
                plugin_name: "AniList Recommendations".to_string(),
                source: "anilist".to_string(),
                generated_at: Some("2026-02-09T12:00:00Z".to_string()),
                cached: true,
                task_status: None,
                task_id: None,
            }],
        };

        let json = serde_json::to_value(&response).unwrap();

        // Source provenance
        let sources = json["sources"].as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["pluginId"], plugin_id.to_string());
        assert_eq!(sources[0]["pluginName"], "AniList Recommendations");
        assert_eq!(sources[0]["generatedAt"], "2026-02-09T12:00:00Z");
        assert!(sources[0]["cached"].as_bool().unwrap());

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
        assert_eq!(rec0["totalVolumeCount"], 30);
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
        assert!(rec1.get("totalVolumeCount").is_none());
        assert!(rec1.get("rating").is_none());
        assert!(rec1.get("popularity").is_none());
    }

    /// Build a minimal RecommendationDto for merge tests.
    fn merge_test_dto(
        external_id: &str,
        score: f64,
        reason: &str,
        source_plugin: &str,
    ) -> RecommendationDto {
        RecommendationDto {
            external_id: external_id.to_string(),
            external_url: None,
            title: format!("Title {external_id}"),
            cover_url: None,
            summary: None,
            genres: vec![],
            tags: None,
            score,
            reason: reason.to_string(),
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
            source_plugin: source_plugin.to_string(),
            source: "anilist".to_string(),
        }
    }

    #[test]
    fn test_merge_dedupes_keeps_highest_score_and_combines_reasons() {
        // Same external ID "42" from two instances; "1" and "3" unique.
        let recs = vec![
            merge_test_dto("1", 0.5, "low one", "Manga"),
            merge_test_dto("42", 0.6, "from manga", "Manga"),
            merge_test_dto("42", 0.9, "from comics", "Comics"),
            merge_test_dto("3", 0.95, "top", "Comics"),
        ];

        let merged = merge_recommendations(recs);

        // "42" deduped to one entry.
        assert_eq!(merged.len(), 3);

        // Ordered by score desc: 3 (0.95), 42 (0.9), 1 (0.5).
        assert_eq!(merged[0].external_id, "3");
        assert_eq!(merged[1].external_id, "42");
        assert_eq!(merged[2].external_id, "1");

        // The deduped "42" kept the highest score and the winning source,
        // and combined both reasons.
        let dup = &merged[1];
        assert!((dup.score - 0.9).abs() < f64::EPSILON);
        assert_eq!(dup.source_plugin, "Comics");
        assert!(dup.reason.contains("from comics"));
        assert!(dup.reason.contains("from manga"));
    }

    #[test]
    fn test_merge_identical_reason_not_duplicated() {
        let recs = vec![
            merge_test_dto("7", 0.8, "same reason", "Manga"),
            merge_test_dto("7", 0.6, "same reason", "Comics"),
        ];
        let merged = merge_recommendations(recs);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].reason, "same reason");
        // Higher score wins its source.
        assert_eq!(merged[0].source_plugin, "Manga");
    }

    /// Verify that RecommendationResponse round-trips through serde_json::Value.
    /// This is the exact path used by the task handler (serialize to Value → write to DB)
    /// and the GET endpoint (read from DB → deserialize from Value).
    #[test]
    fn test_recommendation_response_round_trip_through_json_value() {
        use codex_services::plugin::recommendations::RecommendationResponse;

        let original = RecommendationResponse {
            recommendations: vec![Recommendation {
                external_id: "42".to_string(),
                external_url: Some("https://anilist.co/manga/42".to_string()),
                title: "Test Manga".to_string(),
                cover_url: Some("https://img.example.com/42.jpg".to_string()),
                summary: Some("A test manga".to_string()),
                genres: vec!["Action".to_string()],
                tags: None,
                score: 0.85,
                reason: "Because you liked testing".to_string(),
                based_on: vec!["Unit Tests".to_string()],
                codex_series_id: None,
                in_library: false,
                status: None,
                format: None,
                country_of_origin: None,
                start_year: None,
                total_volume_count: None,
                total_chapter_count: None,
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
        use codex_services::plugin::recommendations::RecommendationResponse;

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
