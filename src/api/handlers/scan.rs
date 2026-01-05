use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::{
    dto::{ScanStatusDto, TriggerScanQuery},
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
};
use crate::db::repositories::LibraryRepository;
use crate::scanner::ScanMode;

use super::AppState;

/// Trigger a library scan
///
/// # Permission Required
/// - `libraries:write`
#[utoipa::path(
    post,
    path = "/libraries/{id}/scan",
    params(
        ("id" = Uuid, Path, description = "Library ID"),
        ("mode" = Option<String>, Query, description = "Scan mode: 'normal' or 'deep' (default: 'normal')")
    ),
    responses(
        (status = 200, description = "Scan started successfully", body = ScanStatusDto),
        (status = 400, description = "Invalid scan mode"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Library not found"),
        (status = 409, description = "Scan already in progress"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn trigger_scan(
    Path(library_id): Path<Uuid>,
    Query(params): Query<TriggerScanQuery>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<ScanStatusDto>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesWrite)?;

    // Check if library exists
    LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Parse scan mode
    let mode = ScanMode::from_str(&params.mode)
        .map_err(|e| ApiError::BadRequest(e))?;

    // Trigger the scan
    state
        .scan_manager
        .trigger_scan(library_id, mode)
        .await
        .map_err(|e| {
            if e.to_string().contains("already") {
                ApiError::Conflict(e.to_string())
            } else {
                ApiError::Internal(e.to_string())
            }
        })?;

    // Get and return the status
    let status = state
        .scan_manager
        .get_status(library_id)
        .await
        .ok_or_else(|| ApiError::NotFound("Scan status not found".to_string()))?;

    Ok(Json(status.into()))
}

/// Get scan status for a library
///
/// # Permission Required
/// - `libraries:read`
#[utoipa::path(
    get,
    path = "/libraries/{id}/scan-status",
    params(
        ("id" = Uuid, Path, description = "Library ID")
    ),
    responses(
        (status = 200, description = "Scan status retrieved", body = ScanStatusDto),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "No scan found for this library"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn get_scan_status(
    Path(library_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<ScanStatusDto>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesRead)?;

    // Get scan status
    let status = state
        .scan_manager
        .get_status(library_id)
        .await
        .ok_or_else(|| ApiError::NotFound("No scan found for this library".to_string()))?;

    Ok(Json(status.into()))
}

/// Cancel a running scan
///
/// # Permission Required
/// - `libraries:write`
#[utoipa::path(
    post,
    path = "/libraries/{id}/scan/cancel",
    params(
        ("id" = Uuid, Path, description = "Library ID")
    ),
    responses(
        (status = 204, description = "Scan cancelled successfully"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "No active scan found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn cancel_scan(
    Path(library_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesWrite)?;

    // Cancel the scan
    state
        .scan_manager
        .cancel_scan(library_id)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// List all active scans
///
/// # Permission Required
/// - `libraries:read`
#[utoipa::path(
    get,
    path = "/scans/active",
    responses(
        (status = 200, description = "List of active scans", body = Vec<ScanStatusDto>),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn list_active_scans(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<Vec<ScanStatusDto>>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesRead)?;

    // Get all active scans
    let scans = state.scan_manager.list_active().await;

    let dtos: Vec<ScanStatusDto> = scans.into_iter().map(|s| s.into()).collect();

    Ok(Json(dtos))
}
