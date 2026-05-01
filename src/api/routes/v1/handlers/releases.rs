//! HTTP handlers for the release ledger and release-source admin endpoints.
//!
//! Three groups of endpoints:
//!
//! 1. Per-series ledger reads (`GET /series/{id}/releases`) - read tracked
//!    series releases for the series detail Releases tab.
//! 2. Inbox + state transitions (`GET /releases`, `PATCH /releases/{id}`,
//!    `POST /releases/{id}/dismiss|mark-acquired`) - cross-series inbox UI.
//! 3. Source admin (`GET /release-sources`, `PATCH /release-sources/{id}`,
//!    `POST /release-sources/{id}/poll-now`) - admin-only source management.
//!
//! Phase 2 keeps `poll-now` as a stub returning HTTP 501; Phase 4 wires it
//! into the task queue.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use super::super::dto::common::{
    DEFAULT_PAGE, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, PaginatedResponse, PaginationLinkBuilder,
};
use super::super::dto::release::{
    PollNowResponse, ReleaseLedgerEntryDto, ReleaseLedgerListResponse, ReleaseSourceDto,
    ReleaseSourceListResponse, UpdateReleaseLedgerEntryRequest, UpdateReleaseSourceRequest,
};
use super::paginated_response;
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::entities::release_ledger::state as ledger_state;
use crate::db::repositories::{
    LedgerInboxFilter, ReleaseLedgerRepository, ReleaseSourceRepository, ReleaseSourceUpdate,
    SeriesRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent};

// =============================================================================
// Per-series ledger
// =============================================================================

/// Query parameters for the per-series ledger view.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct SeriesReleaseListParams {
    /// Filter by state. Defaults to all states (no filter) so the per-series
    /// view shows the full history.
    #[serde(default)]
    pub state: Option<String>,
    /// 1-indexed page number.
    #[serde(default = "default_page")]
    pub page: u64,
    /// Items per page (max 500, default 50).
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// List release-ledger entries for a series.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/releases",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        SeriesReleaseListParams,
    ),
    responses(
        (status = 200, description = "Paginated ledger entries for the series", body = PaginatedResponse<ReleaseLedgerEntryDto>),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn list_series_releases(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(params): Query<SeriesReleaseListParams>,
) -> Result<Response, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    // Validate state filter if present.
    if let Some(ref s) = params.state
        && !ledger_state::is_valid(s)
    {
        return Err(ApiError::BadRequest(format!("invalid state filter: {}", s)));
    }

    let rows = ReleaseLedgerRepository::list_for_series(
        &state.db,
        series_id,
        params.state.as_deref(),
        page_size,
        offset,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to list releases: {}", e)))?;

    // Total comes from a count query that respects the same state filter.
    let filter = LedgerInboxFilter {
        state: params.state.clone(),
        series_id: Some(series_id),
        ..Default::default()
    };
    // count_inbox always filters by state; if the caller didn't pass one, we
    // fall back to counting all states for the series instead of the inbox
    // default (`announced`). Run a manual count via list_for_series with a
    // large limit when the caller asked for "all states."
    let total = if params.state.is_some() {
        ReleaseLedgerRepository::count_inbox(&state.db, filter)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count releases: {}", e)))?
    } else {
        ReleaseLedgerRepository::list_for_series(&state.db, series_id, None, 0, 0)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count releases: {}", e)))?
            .len() as u64
    };

    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };

    let dtos: Vec<ReleaseLedgerEntryDto> = rows.into_iter().map(Into::into).collect();
    let base_path = format!("/api/v1/series/{}/releases", series_id);
    let mut builder = PaginationLinkBuilder::new(&base_path, page, page_size, total_pages);
    if let Some(ref s) = params.state {
        builder = builder.with_param("state", s);
    }
    let response = PaginatedResponse::with_builder(dtos, page, page_size, total, &builder);
    Ok(paginated_response(response, &builder))
}

// =============================================================================
// Inbox + state transitions
// =============================================================================

/// Query parameters for the cross-series inbox view.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct ReleaseInboxParams {
    /// Filter by state. Defaults to `announced`.
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub series_id: Option<Uuid>,
    #[serde(default)]
    pub source_id: Option<Uuid>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

/// Cross-series inbox: announced (or filtered) ledger entries, paginated.
#[utoipa::path(
    get,
    path = "/api/v1/releases",
    params(ReleaseInboxParams),
    responses(
        (status = 200, description = "Paginated inbox entries", body = PaginatedResponse<ReleaseLedgerEntryDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn list_release_inbox(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<ReleaseInboxParams>,
) -> Result<Response, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    if let Some(ref s) = params.state
        && !ledger_state::is_valid(s)
    {
        return Err(ApiError::BadRequest(format!("invalid state filter: {}", s)));
    }

    let filter = LedgerInboxFilter {
        state: params.state.clone(),
        series_id: params.series_id,
        source_id: params.source_id,
        language: params.language.clone(),
    };
    let rows = ReleaseLedgerRepository::list_inbox(&state.db, filter.clone(), page_size, offset)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list inbox: {}", e)))?;
    let total = ReleaseLedgerRepository::count_inbox(&state.db, filter)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count inbox: {}", e)))?;
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };

    let dtos: Vec<ReleaseLedgerEntryDto> = rows.into_iter().map(Into::into).collect();
    let mut builder = PaginationLinkBuilder::new("/api/v1/releases", page, page_size, total_pages);
    if let Some(ref s) = params.state {
        builder = builder.with_param("state", s);
    }
    if let Some(sid) = params.series_id {
        builder = builder.with_param("seriesId", &sid.to_string());
    }
    if let Some(src) = params.source_id {
        builder = builder.with_param("sourceId", &src.to_string());
    }
    if let Some(ref lang) = params.language {
        builder = builder.with_param("language", lang);
    }
    let response = PaginatedResponse::with_builder(dtos, page, page_size, total, &builder);
    Ok(paginated_response(response, &builder))
}

/// PATCH a ledger entry's state (general-purpose state transition).
#[utoipa::path(
    patch,
    path = "/api/v1/releases/{release_id}",
    params(
        ("release_id" = Uuid, Path, description = "Ledger entry ID")
    ),
    request_body = UpdateReleaseLedgerEntryRequest,
    responses(
        (status = 200, description = "Updated ledger entry", body = ReleaseLedgerEntryDto),
        (status = 400, description = "Invalid state"),
        (status = 404, description = "Ledger entry not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn update_release_entry(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(release_id): Path<Uuid>,
    Json(request): Json<UpdateReleaseLedgerEntryRequest>,
) -> Result<Json<ReleaseLedgerEntryDto>, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;

    let new_state = request
        .state
        .ok_or_else(|| ApiError::BadRequest("state is required".to_string()))?;

    update_state_internal(&state, auth.user_id, release_id, &new_state).await
}

/// Convenience POST: dismiss a release.
#[utoipa::path(
    post,
    path = "/api/v1/releases/{release_id}/dismiss",
    params(
        ("release_id" = Uuid, Path, description = "Ledger entry ID")
    ),
    responses(
        (status = 200, description = "Release dismissed", body = ReleaseLedgerEntryDto),
        (status = 404, description = "Ledger entry not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn dismiss_release(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(release_id): Path<Uuid>,
) -> Result<Json<ReleaseLedgerEntryDto>, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;
    update_state_internal(&state, auth.user_id, release_id, ledger_state::DISMISSED).await
}

/// Convenience POST: mark a release acquired.
#[utoipa::path(
    post,
    path = "/api/v1/releases/{release_id}/mark-acquired",
    params(
        ("release_id" = Uuid, Path, description = "Ledger entry ID")
    ),
    responses(
        (status = 200, description = "Release marked acquired", body = ReleaseLedgerEntryDto),
        (status = 404, description = "Ledger entry not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn mark_release_acquired(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(release_id): Path<Uuid>,
) -> Result<Json<ReleaseLedgerEntryDto>, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;
    update_state_internal(
        &state,
        auth.user_id,
        release_id,
        ledger_state::MARKED_ACQUIRED,
    )
    .await
}

async fn update_state_internal(
    state: &Arc<AuthState>,
    user_id: Uuid,
    release_id: Uuid,
    new_state: &str,
) -> Result<Json<ReleaseLedgerEntryDto>, ApiError> {
    if !ledger_state::is_valid(new_state) {
        return Err(ApiError::BadRequest(format!(
            "invalid state: {}",
            new_state
        )));
    }

    // Fetch the row first so we have series_id for the SSE event.
    let existing = ReleaseLedgerRepository::get_by_id(&state.db, release_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch ledger entry: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Ledger entry not found".to_string()))?;
    let series_id = existing.series_id;

    let updated = ReleaseLedgerRepository::set_state(&state.db, release_id, new_state)
        .await
        .map_err(|e| {
            if e.to_string().contains("invalid state") {
                ApiError::BadRequest(e.to_string())
            } else {
                ApiError::Internal(format!("Failed to update ledger entry: {}", e))
            }
        })?;

    // Look up the series to get library_id for the SSE event payload. If the
    // series was deleted concurrently we still return the updated row -
    // dropping the event is safe.
    if let Ok(Some(series)) = SeriesRepository::get_by_id(&state.db, series_id).await {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: series.library_id,
                fields: Some(vec!["releases".to_string()]),
            },
            timestamp: Utc::now(),
            user_id: Some(user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(updated.into()))
}

// =============================================================================
// Source admin
// =============================================================================

/// List all configured release sources (admin-only).
#[utoipa::path(
    get,
    path = "/api/v1/release-sources",
    responses(
        (status = 200, description = "Source list", body = ReleaseSourceListResponse),
        (status = 403, description = "PluginsManage permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn list_release_sources(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<ReleaseSourceListResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;
    let sources = ReleaseSourceRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list sources: {}", e)))?;
    Ok(Json(ReleaseSourceListResponse {
        sources: sources.into_iter().map(Into::into).collect(),
    }))
}

/// PATCH a release source (admin-only).
///
/// Toggle `enabled`, override `pollIntervalS`, or rename `displayName`.
#[utoipa::path(
    patch,
    path = "/api/v1/release-sources/{source_id}",
    params(
        ("source_id" = Uuid, Path, description = "Source ID")
    ),
    request_body = UpdateReleaseSourceRequest,
    responses(
        (status = 200, description = "Source updated", body = ReleaseSourceDto),
        (status = 400, description = "Invalid update payload"),
        (status = 404, description = "Source not found"),
        (status = 403, description = "PluginsManage permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn update_release_source(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(source_id): Path<Uuid>,
    Json(request): Json<UpdateReleaseSourceRequest>,
) -> Result<Json<ReleaseSourceDto>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Confirm existence to return a clean 404 instead of a generic 500.
    ReleaseSourceRepository::get_by_id(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch source: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Release source not found".to_string()))?;

    let update = ReleaseSourceUpdate {
        display_name: request.display_name,
        enabled: request.enabled,
        poll_interval_s: request.poll_interval_s,
        config: None, // config edits go through plugin admin, not here
    };

    let updated = ReleaseSourceRepository::update(&state.db, source_id, update)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("positive") {
                ApiError::BadRequest(msg)
            } else {
                ApiError::Internal(format!("Failed to update source: {}", e))
            }
        })?;

    Ok(Json(updated.into()))
}

/// Trigger a manual poll for a source.
///
/// **Phase 2 stub**: returns `501 Not Implemented`. Phase 4 wires this into
/// the task queue (`PollReleaseSource` task type).
#[utoipa::path(
    post,
    path = "/api/v1/release-sources/{source_id}/poll-now",
    params(
        ("source_id" = Uuid, Path, description = "Source ID")
    ),
    responses(
        (status = 501, description = "Not implemented yet (Phase 4)", body = PollNowResponse),
        (status = 404, description = "Source not found"),
        (status = 403, description = "PluginsManage permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn poll_release_source_now(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(source_id): Path<Uuid>,
) -> Result<(StatusCode, Json<PollNowResponse>), ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Confirm the source exists - fail fast with 404 even though we don't act.
    ReleaseSourceRepository::get_by_id(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch source: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Release source not found".to_string()))?;

    Ok((
        StatusCode::NOT_IMPLEMENTED,
        Json(PollNowResponse {
            status: "not_implemented".to_string(),
            message:
                "Manual poll-now not implemented yet. Phase 4 (PollReleaseSource task) wires this."
                    .to_string(),
        }),
    ))
}

// =============================================================================
// OpenAPI placeholder
// =============================================================================

// `ReleaseLedgerListResponse` is unused in handlers (we return paginated
// responses) but kept in the DTO module for potential simpler clients. Pull it
// in here to silence the unused-import lint.
#[allow(dead_code)]
fn _opening_api_keepalive() -> ReleaseLedgerListResponse {
    ReleaseLedgerListResponse { entries: vec![] }
}
