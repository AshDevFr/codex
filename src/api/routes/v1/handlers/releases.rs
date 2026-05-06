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
    BulkReleaseAction, BulkReleaseActionRequest, BulkReleaseActionResponse, DeleteReleaseResponse,
    PollNowResponse, ReleaseFacetsResponse, ReleaseLanguageFacetDto, ReleaseLedgerEntryDto,
    ReleaseLedgerListResponse, ReleaseLibraryFacetDto, ReleaseSeriesFacetDto, ReleaseSourceDto,
    ReleaseSourceListResponse, ResetReleaseSourceResponse, UpdateReleaseLedgerEntryRequest,
    UpdateReleaseSourceRequest,
};
use super::paginated_response;
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::entities::release_ledger::state as ledger_state;
use crate::db::repositories::{
    LedgerInboxFilter, LibraryRepository, PluginsRepository, ReleaseLedgerRepository,
    ReleaseSourceRepository, ReleaseSourceUpdate, SeriesRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent};

/// Hydrate ledger rows with series titles via a single batched lookup.
///
/// The DTO carries `series_title` so the inbox UI can render a human label
/// without a follow-up call. We do this in the handler (rather than a SQL
/// JOIN in the repo) to keep the repository surface narrow and reuse the
/// existing `SeriesRepository::get_by_ids` batch query.
async fn hydrate_ledger_dtos(
    db: &sea_orm::DatabaseConnection,
    rows: Vec<crate::db::entities::release_ledger::Model>,
) -> Result<Vec<ReleaseLedgerEntryDto>, ApiError> {
    let mut series_ids: Vec<Uuid> = rows.iter().map(|r| r.series_id).collect();
    series_ids.sort_unstable();
    series_ids.dedup();

    let title_by_id: std::collections::HashMap<Uuid, String> = if series_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        SeriesRepository::get_by_ids(db, &series_ids)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to load series titles: {}", e)))?
            .into_iter()
            .map(|s| (s.id, s.name))
            .collect()
    };

    Ok(rows
        .into_iter()
        .map(|row| {
            let title = title_by_id.get(&row.series_id).cloned().unwrap_or_default();
            ReleaseLedgerEntryDto::from_model_with_series_title(row, title)
        })
        .collect())
}

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

    let series = SeriesRepository::get_by_id(&state.db, series_id)
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

    // All rows belong to the same series, so we can reuse the title we
    // already loaded for the existence check rather than re-fetching it.
    let dtos: Vec<ReleaseLedgerEntryDto> = rows
        .into_iter()
        .map(|row| ReleaseLedgerEntryDto::from_model_with_series_title(row, series.name.clone()))
        .collect();
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
    /// Filter by state. Defaults to `announced`. Pass `all` to disable
    /// state filtering entirely (returns rows in every state).
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub series_id: Option<Uuid>,
    #[serde(default)]
    pub source_id: Option<Uuid>,
    #[serde(default)]
    pub language: Option<String>,
    /// Restrict to series belonging to this library.
    #[serde(default)]
    pub library_id: Option<Uuid>,
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

    // `all` is a sentinel meaning "no state filter"; otherwise validate
    // against the canonical set.
    let all_states = matches!(params.state.as_deref(), Some("all"));
    let normalised_state = if all_states {
        None
    } else {
        params.state.clone()
    };
    if let Some(ref s) = normalised_state
        && !ledger_state::is_valid(s)
    {
        return Err(ApiError::BadRequest(format!("invalid state filter: {}", s)));
    }

    let filter = LedgerInboxFilter {
        state: normalised_state,
        all_states,
        series_id: params.series_id,
        source_id: params.source_id,
        language: params.language.clone(),
        library_id: params.library_id,
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

    let dtos = hydrate_ledger_dtos(&state.db, rows).await?;
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
    if let Some(lib) = params.library_id {
        builder = builder.with_param("libraryId", &lib.to_string());
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

    // Look up the series for both the SSE event (library_id) and the DTO
    // (series_title). If the series was deleted concurrently we still return
    // the updated row, dropping the event and using an empty title — the
    // ledger row's series_id remains valid for navigation.
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .ok()
        .flatten();
    if let Some(ref s) = series {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: s.library_id,
                fields: Some(vec!["releases".to_string()]),
            },
            timestamp: Utc::now(),
            user_id: Some(user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    let title = series.map(|s| s.name).unwrap_or_default();
    Ok(Json(ReleaseLedgerEntryDto::from_model_with_series_title(
        updated, title,
    )))
}

// =============================================================================
// Inbox facets
// =============================================================================

/// Query parameters for the inbox facets endpoint.
///
/// The same shape as [`ReleaseInboxParams`] minus pagination, plus an
/// extra `excludeDimension` knob the handler ignores today (reserved for
/// when the frontend wants strict facet exclusion). For each facet
/// dimension we currently apply *all* other active filters (Solr-style
/// non-self-exclusion would require taking the dimension off before
/// filtering — a follow-up if any UI needs it).
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct ReleaseFacetsParams {
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub series_id: Option<Uuid>,
    #[serde(default)]
    pub source_id: Option<Uuid>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

/// Distinct values present in the inbox under the given filters.
///
/// Returns the languages, libraries, and series that have at least one
/// matching ledger row. The frontend uses this to populate cascading
/// Select dropdowns so users never have to type a UUID and never see
/// dropdown options that would yield zero results.
#[utoipa::path(
    get,
    path = "/api/v1/releases/facets",
    params(ReleaseFacetsParams),
    responses(
        (status = 200, description = "Facets for the inbox view", body = ReleaseFacetsResponse),
        (status = 400, description = "Invalid state filter"),
        (status = 403, description = "SeriesRead permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn list_release_facets(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<ReleaseFacetsParams>,
) -> Result<Json<ReleaseFacetsResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    let all_states = matches!(params.state.as_deref(), Some("all"));
    let normalised_state = if all_states {
        None
    } else {
        params.state.clone()
    };
    if let Some(ref s) = normalised_state
        && !ledger_state::is_valid(s)
    {
        return Err(ApiError::BadRequest(format!("invalid state filter: {}", s)));
    }

    let base_filter = LedgerInboxFilter {
        state: normalised_state,
        all_states,
        series_id: params.series_id,
        source_id: params.source_id,
        language: params.language.clone(),
        library_id: params.library_id,
    };

    // Each facet excludes its own dimension from the filter so it always
    // shows the full set of options (Solr-style facet exclusion). Without
    // this, picking a series in the dropdown would collapse the series
    // dropdown to that single series.
    let series_filter = LedgerInboxFilter {
        series_id: None,
        ..base_filter.clone()
    };
    let library_filter = LedgerInboxFilter {
        library_id: None,
        ..base_filter.clone()
    };
    let language_filter = LedgerInboxFilter {
        language: None,
        ..base_filter.clone()
    };

    let series_facets = ReleaseLedgerRepository::list_series_facets(&state.db, series_filter)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list series facets: {}", e)))?;
    let library_facets = ReleaseLedgerRepository::list_library_facets(&state.db, library_filter)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list library facets: {}", e)))?;
    let language_facets = ReleaseLedgerRepository::list_language_facets(&state.db, language_filter)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list language facets: {}", e)))?;

    // Hydrate series titles + library names in two batched lookups.
    let mut series_ids: Vec<Uuid> = series_facets.iter().map(|f| f.series_id).collect();
    series_ids.sort_unstable();
    series_ids.dedup();
    let mut library_ids: Vec<Uuid> = series_facets
        .iter()
        .map(|f| f.library_id)
        .chain(library_facets.iter().map(|f| f.library_id))
        .collect();
    library_ids.sort_unstable();
    library_ids.dedup();

    let series_models = SeriesRepository::get_by_ids(&state.db, &series_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load series: {}", e)))?;
    let series_titles: std::collections::HashMap<Uuid, String> =
        series_models.into_iter().map(|s| (s.id, s.name)).collect();
    let library_map = LibraryRepository::get_by_ids(&state.db, &library_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load libraries: {}", e)))?;

    let series_dtos: Vec<ReleaseSeriesFacetDto> = series_facets
        .into_iter()
        .map(|f| {
            let library_name = library_map
                .get(&f.library_id)
                .map(|l| l.name.clone())
                .unwrap_or_default();
            ReleaseSeriesFacetDto {
                series_id: f.series_id,
                series_title: series_titles.get(&f.series_id).cloned().unwrap_or_default(),
                library_id: f.library_id,
                library_name,
                count: f.count,
            }
        })
        .collect();
    let library_dtos: Vec<ReleaseLibraryFacetDto> = library_facets
        .into_iter()
        .map(|f| {
            let library_name = library_map
                .get(&f.library_id)
                .map(|l| l.name.clone())
                .unwrap_or_default();
            ReleaseLibraryFacetDto {
                library_id: f.library_id,
                library_name,
                count: f.count,
            }
        })
        .collect();
    let language_dtos: Vec<ReleaseLanguageFacetDto> = language_facets
        .into_iter()
        .map(|f| ReleaseLanguageFacetDto {
            language: f.language,
            count: f.count,
        })
        .collect();

    Ok(Json(ReleaseFacetsResponse {
        languages: language_dtos,
        libraries: library_dtos,
        series: series_dtos,
    }))
}

// =============================================================================
// Delete + bulk
// =============================================================================

/// Hard-delete a single ledger row.
///
/// Also clears the source's `etag` so the next poll bypasses
/// `If-None-Match` and re-records the deleted row in `announced` state
/// (assuming the upstream still lists it). This is the lever users want
/// when they marked something incorrectly and need to "get it back".
#[utoipa::path(
    delete,
    path = "/api/v1/releases/{release_id}",
    params(
        ("release_id" = Uuid, Path, description = "Ledger entry ID")
    ),
    responses(
        (status = 200, description = "Release deleted", body = DeleteReleaseResponse),
        (status = 404, description = "Ledger entry not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn delete_release(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(release_id): Path<Uuid>,
) -> Result<Json<DeleteReleaseResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;

    // Look up the row first to capture series_id (for SSE) and source_id
    // (for the etag clear). Returning a clean 404 here matches the rest
    // of the release endpoints.
    let existing = ReleaseLedgerRepository::get_by_id(&state.db, release_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch ledger entry: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Ledger entry not found".to_string()))?;
    let series_id = existing.series_id;
    let source_id = existing.source_id;

    let deleted = ReleaseLedgerRepository::delete(&state.db, release_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete ledger entry: {}", e)))?;

    // Best-effort etag clear: if it fails we still report the delete
    // succeeded (the row is already gone). User can manually `reset` the
    // source if they really need an etag flush.
    if deleted && let Err(e) = ReleaseSourceRepository::clear_etag(&state.db, source_id).await {
        tracing::warn!(
            "Failed to clear etag for source {} after deleting release {}: {}",
            source_id,
            release_id,
            e
        );
    }

    if deleted && let Ok(Some(s)) = SeriesRepository::get_by_id(&state.db, series_id).await {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: s.library_id,
                fields: Some(vec!["releases".to_string()]),
            },
            timestamp: Utc::now(),
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(DeleteReleaseResponse { deleted }))
}

/// Apply an action to a batch of ledger rows.
///
/// `dismiss`, `mark-acquired`, `ignore`, and `reset` all set state
/// in-place. `delete` removes the rows and clears the affected sources'
/// etags so the next poll re-fetches without `If-None-Match`. All run
/// as bulk SQL (no per-row round trips), so this scales to thousands of
/// rows in one call.
#[utoipa::path(
    post,
    path = "/api/v1/releases/bulk",
    request_body = BulkReleaseActionRequest,
    responses(
        (status = 200, description = "Bulk action applied", body = BulkReleaseActionResponse),
        (status = 400, description = "Empty ID list or invalid action"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn bulk_release_action(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<BulkReleaseActionRequest>,
) -> Result<Json<BulkReleaseActionResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;

    if request.ids.is_empty() {
        return Err(ApiError::BadRequest("ids must not be empty".to_string()));
    }
    // Soft cap to keep an unbounded list from melting the DB. 500 matches
    // MAX_PAGE_SIZE so a user can bulk-action a full inbox page.
    const MAX_BULK: usize = 500;
    if request.ids.len() > MAX_BULK {
        return Err(ApiError::BadRequest(format!(
            "too many ids: {} (max {})",
            request.ids.len(),
            MAX_BULK
        )));
    }

    // Snapshot affected sources + series before mutating, so we can clear
    // etags (delete) and emit SSE events (all actions). For dismiss/
    // mark-acquired we don't strictly need the source list, but loading
    // rows once keeps the code path uniform and lets us emit one
    // SeriesUpdated event per affected series.
    let rows_before = ReleaseLedgerRepository::find_by_ids(&state.db, &request.ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load ledger rows: {}", e)))?;
    let mut affected_series: Vec<Uuid> = rows_before.iter().map(|r| r.series_id).collect();
    affected_series.sort_unstable();
    affected_series.dedup();
    let mut affected_sources: Vec<Uuid> = rows_before.iter().map(|r| r.source_id).collect();
    affected_sources.sort_unstable();
    affected_sources.dedup();

    let affected: u64 = match request.action {
        BulkReleaseAction::Dismiss => ReleaseLedgerRepository::set_state_many(
            &state.db,
            &request.ids,
            ledger_state::DISMISSED,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to dismiss releases: {}", e)))?,
        BulkReleaseAction::MarkAcquired => ReleaseLedgerRepository::set_state_many(
            &state.db,
            &request.ids,
            ledger_state::MARKED_ACQUIRED,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark releases acquired: {}", e)))?,
        BulkReleaseAction::Ignore => {
            ReleaseLedgerRepository::set_state_many(&state.db, &request.ids, ledger_state::IGNORED)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to ignore releases: {}", e)))?
        }
        BulkReleaseAction::Reset => ReleaseLedgerRepository::set_state_many(
            &state.db,
            &request.ids,
            ledger_state::ANNOUNCED,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reset releases: {}", e)))?,
        BulkReleaseAction::Delete => {
            let count = ReleaseLedgerRepository::delete_many(&state.db, &request.ids)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to delete releases: {}", e)))?;
            if count > 0
                && let Err(e) =
                    ReleaseSourceRepository::clear_etag_many(&state.db, &affected_sources).await
            {
                tracing::warn!(
                    "Failed to clear etags for {} sources after bulk delete: {}",
                    affected_sources.len(),
                    e
                );
            }
            count
        }
    };

    // Emit one SeriesUpdated event per affected series so any open client
    // refreshes the per-series Releases panel + the inbox badge.
    if affected > 0 {
        let series_models = SeriesRepository::get_by_ids(&state.db, &affected_series)
            .await
            .ok()
            .unwrap_or_default();
        for s in series_models {
            let event = EntityChangeEvent {
                event: EntityEvent::SeriesUpdated {
                    series_id: s.id,
                    library_id: s.library_id,
                    fields: Some(vec!["releases".to_string()]),
                },
                timestamp: Utc::now(),
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkReleaseActionResponse {
        affected,
        action: request.action,
    }))
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
    let server_default = resolve_server_default_cron(&state.db).await;
    Ok(Json(ReleaseSourceListResponse {
        sources: sources
            .into_iter()
            .map(|m| ReleaseSourceDto::from_model_with_default(m, &server_default))
            .collect(),
    }))
}

/// Fetch the server-wide default cron schedule for release-source polling.
/// Falls back to the compile-time default on a settings-fetch failure
/// rather than 500-ing the request — the field is informational on the
/// response shape.
async fn resolve_server_default_cron(db: &sea_orm::DatabaseConnection) -> String {
    use crate::services::release::schedule::{DEFAULT_CRON_SCHEDULE, read_default_cron_schedule};
    use crate::services::settings::SettingsService;
    match SettingsService::new(db.clone()).await {
        Ok(svc) => read_default_cron_schedule(&svc).await,
        Err(e) => {
            tracing::warn!(
                "Failed to load settings service for cron resolution; using compile-time default: {}",
                e
            );
            DEFAULT_CRON_SCHEDULE.to_string()
        }
    }
}

/// PATCH a release source (admin-only).
///
/// Toggle `enabled`, override `cronSchedule`, or rename `displayName`.
/// Sending `cronSchedule: null` clears the override and reverts the row to
/// inheriting the server-wide `release_tracking.default_cron_schedule`.
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
        cron_schedule: request.cron_schedule,
        config: None, // config edits go through plugin admin, not here
    };

    let updated = ReleaseSourceRepository::update(&state.db, source_id, update)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.to_lowercase().contains("cron") {
                ApiError::BadRequest(msg)
            } else {
                ApiError::Internal(format!("Failed to update source: {}", e))
            }
        })?;

    // Best-effort reconcile so the scheduler picks up enable/disable or
    // interval changes without a restart. Reconcile failures don't block
    // the API response — the change is durable in the DB and the next
    // scheduler restart picks it up.
    if let Some(ref scheduler) = state.scheduler {
        let mut guard = scheduler.lock().await;
        if let Err(e) = guard.reconcile_release_sources().await {
            tracing::warn!(
                "Failed to reconcile release-source schedules after update: {}",
                e
            );
        }
    }

    let server_default = resolve_server_default_cron(&state.db).await;
    Ok(Json(ReleaseSourceDto::from_model_with_default(
        updated,
        &server_default,
    )))
}

/// Trigger a manual poll for a source.
///
/// Enqueues a `PollReleaseSource` task immediately. The task runs
/// asynchronously via the worker pool; the response confirms the enqueue,
/// not the poll outcome.
#[utoipa::path(
    post,
    path = "/api/v1/release-sources/{source_id}/poll-now",
    params(
        ("source_id" = Uuid, Path, description = "Source ID")
    ),
    responses(
        (status = 202, description = "Poll task enqueued", body = PollNowResponse),
        (status = 404, description = "Source not found"),
        (status = 403, description = "PluginsManage permission required"),
        (status = 409, description = "Source disabled"),
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

    // Confirm the source exists.
    let source = ReleaseSourceRepository::get_by_id(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch source: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Release source not found".to_string()))?;

    if !source.enabled {
        return Err(ApiError::Conflict(format!(
            "Source '{}' is disabled; enable it before polling",
            source.display_name
        )));
    }

    let outcome = crate::scheduler::release_sources::enqueue_poll_now(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue poll task: {}", e)))?;

    let (status, message) = if outcome.coalesced {
        (
            "already_running".to_string(),
            format!(
                "A poll for this source is already running (task_id={}); coalesced",
                outcome.task_id
            ),
        )
    } else {
        (
            "enqueued".to_string(),
            format!("Poll task enqueued (task_id={})", outcome.task_id),
        )
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(PollNowResponse { status, message }),
    ))
}

/// Reset a release source to a clean slate.
///
/// Deletes every `release_ledger` row owned by the source and clears the
/// source's transient poll state (`etag`, `last_polled_at`, `last_error`,
/// `last_error_at`, `last_summary`). User-managed fields (`enabled`,
/// `cron_schedule`, `display_name`, `config`) are preserved.
///
/// Intended for testing/troubleshooting: after a reset, the next poll
/// fetches the upstream feed without an `If-None-Match` header (so no 304
/// short-circuit) and re-records every release as `announced`. Does NOT
/// auto-enqueue a poll — call `POST /release-sources/{id}/poll-now` after
/// resetting if you want immediate re-fetch.
#[utoipa::path(
    post,
    path = "/api/v1/release-sources/{source_id}/reset",
    params(
        ("source_id" = Uuid, Path, description = "Source ID")
    ),
    responses(
        (status = 200, description = "Source reset", body = ResetReleaseSourceResponse),
        (status = 404, description = "Source not found"),
        (status = 403, description = "PluginsManage permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn reset_release_source(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(source_id): Path<Uuid>,
) -> Result<Json<ResetReleaseSourceResponse>, ApiError> {
    auth.require_permission(&Permission::PluginsManage)?;

    // Confirm existence to return a clean 404.
    ReleaseSourceRepository::get_by_id(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch source: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Release source not found".to_string()))?;

    let deleted = ReleaseLedgerRepository::delete_by_source(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to clear ledger: {}", e)))?;

    ReleaseSourceRepository::clear_poll_state(&state.db, source_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reset source state: {}", e)))?;

    Ok(Json(ResetReleaseSourceResponse {
        deleted_ledger_entries: deleted,
    }))
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

// =============================================================================
// Applicability lookup
// =============================================================================

/// Query string for `GET /api/v1/release-sources/applicability`.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ApplicabilityQuery {
    /// Optional library scope. When provided, only plugins that apply to
    /// this library are considered (a plugin's `library_ids` field is
    /// either empty = all, or contains this UUID).
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

/// Response shape for `GET /api/v1/release-sources/applicability`.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplicabilityResponse {
    /// `true` when at least one enabled `release_source` plugin applies to
    /// the requested library (or, if no `libraryId` was supplied, to *any*
    /// library). The frontend uses this to decide whether to render the
    /// per-series Tracking panel and Releases tab, or to show the
    /// bulk-track menu entry.
    pub applicable: bool,
    /// Plugin display names (or fallback to `name` when no manifest cached
    /// yet) of the enabled release-source plugins covering this library.
    /// Empty when `applicable` is `false`. Useful for surfacing "Powered by
    /// MangaUpdates, Nyaa" hints in the UI.
    pub plugin_display_names: Vec<String>,
}

/// Whether release tracking is available for a given library.
///
/// Read-only, requires only `SeriesRead`: the response carries no
/// admin-sensitive data (no plugin IDs, no configs, no library
/// allowlists), just the boolean and friendly display names. Used by the
/// frontend to:
///
/// - hide the per-series Tracking panel + Releases tab on libraries with
///   no applicable plugin (cleaner UX);
/// - decide whether to show the "Track for releases" / "Don't track for
///   releases" entries in the bulk-selection menu.
#[utoipa::path(
    get,
    path = "/api/v1/release-sources/applicability",
    params(ApplicabilityQuery),
    responses(
        (status = 200, description = "Applicability info", body = ApplicabilityResponse),
        (status = 403, description = "SeriesRead permission required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Releases"
)]
pub async fn get_release_tracking_applicability(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    axum::extract::Query(query): axum::extract::Query<ApplicabilityQuery>,
) -> Result<Json<ApplicabilityResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    let plugins = PluginsRepository::get_enabled(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load plugins: {}", e)))?;

    let mut display_names: Vec<String> = Vec::new();
    for plugin in plugins {
        // Capability check via the cached manifest. We deserialize the
        // shape lightly via the canonical `PluginManifest` struct so
        // a malformed manifest doesn't claim release-source capability.
        let Some(manifest_json) = plugin.manifest.as_ref() else {
            continue;
        };
        let Ok(manifest) = serde_json::from_value::<
            crate::services::plugin::protocol::PluginManifest,
        >(manifest_json.clone()) else {
            continue;
        };
        if manifest.capabilities.release_source.is_none() {
            continue;
        }

        // Library-scope check. The DB column is JSON; an empty array means
        // "all libraries". Anything not deserializing into a Vec<Uuid>
        // (NULL, non-array, etc.) is treated as "all libraries" too —
        // that matches the existing convention elsewhere in the codebase.
        let library_ids: Vec<Uuid> =
            serde_json::from_value(plugin.library_ids.clone()).unwrap_or_default();
        if let Some(lib) = query.library_id
            && !library_ids.is_empty()
            && !library_ids.contains(&lib)
        {
            continue;
        }

        let label = if plugin.display_name.trim().is_empty() {
            plugin.name.clone()
        } else {
            plugin.display_name.clone()
        };
        display_names.push(label);
    }

    Ok(Json(ApplicabilityResponse {
        applicable: !display_names.is_empty(),
        plugin_display_names: display_names,
    }))
}
