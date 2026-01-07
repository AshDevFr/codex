use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::{self, Stream};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt as _;
use uuid::Uuid;

use crate::api::{
    dto::{ScanStatusDto, TriggerScanQuery},
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use crate::scanner::{
    analyze_book, analyze_library_books, analyze_series_books, AnalyzerConfig, ScanMode,
};

use super::AppState;

/// Trigger a library scan
///
/// # Permission Required
/// - `libraries:write`
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{id}/scan",
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
    let mode = ScanMode::from_str(&params.mode).map_err(|e| ApiError::BadRequest(e))?;

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
    path = "/api/v1/libraries/{id}/scan-status",
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
    path = "/api/v1/libraries/{id}/scan/cancel",
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
    path = "/api/v1/scans/active",
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

/// Stream scan progress updates via Server-Sent Events
///
/// # Permission Required
/// - `libraries:read`
///
/// This endpoint streams real-time scan progress updates for all libraries.
/// Clients should listen to this stream to receive live updates during scanning.
#[utoipa::path(
    get,
    path = "/api/v1/scans/stream",
    responses(
        (status = 200, description = "SSE stream of scan progress updates"),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn scan_progress_stream(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesRead)?;

    // Subscribe to scan progress updates
    let mut receiver = state.scan_manager.subscribe();

    // Create SSE stream
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(progress) => {
                    let dto: ScanStatusDto = progress.into();
                    if let Ok(json) = serde_json::to_string(&dto) {
                        yield Ok(Event::default().data(json));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Client is too slow, skip lagged messages
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // Channel closed, end stream
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// Trigger analysis of unanalyzed books in a library
///
/// # Permission Required
/// - `libraries:write`
///
/// This endpoint triggers the analysis phase for all unanalyzed books in a library.
/// Books are analyzed in parallel based on the configured concurrency setting.
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{id}/analyze",
    params(
        ("id" = Uuid, Path, description = "Library ID"),
        ("concurrency" = Option<usize>, Query, description = "Number of concurrent analysis tasks (default: 4)")
    ),
    responses(
        (status = 200, description = "Analysis completed successfully"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn trigger_analysis(
    Path(library_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<crate::api::dto::scan::AnalysisResult>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesWrite)?;

    // Check if library exists
    LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Get concurrency parameter or use default
    let concurrency = params
        .get("concurrency")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    // Validate concurrency range
    if concurrency < 1 || concurrency > 16 {
        return Err(ApiError::BadRequest(
            "Concurrency must be between 1 and 16".to_string(),
        ));
    }

    let config = AnalyzerConfig {
        max_concurrent: concurrency,
    };

    // Run analysis
    let result = analyze_library_books(&state.db, library_id, config, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Analysis failed: {}", e)))?;

    Ok(Json(result.into()))
}

/// Trigger analysis of unanalyzed books in a series
///
/// # Permission Required
/// - `series:write`
#[utoipa::path(
    post,
    path = "/api/v1/series/{id}/analyze",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
        ("concurrency" = Option<usize>, Query, description = "Number of concurrent analysis tasks (default: 4)")
    ),
    responses(
        (status = 200, description = "Analysis completed successfully"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn trigger_series_analysis(
    Path(series_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<crate::api::dto::scan::AnalysisResult>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::SeriesWrite)?;

    // Check if series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get concurrency parameter or use default
    let concurrency = params
        .get("concurrency")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    // Validate concurrency range
    if concurrency < 1 || concurrency > 16 {
        return Err(ApiError::BadRequest(
            "Concurrency must be between 1 and 16".to_string(),
        ));
    }

    let config = AnalyzerConfig {
        max_concurrent: concurrency,
    };

    // Run analysis
    let result = analyze_series_books(&state.db, series_id, config, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Analysis failed: {}", e)))?;

    Ok(Json(result.into()))
}

/// Trigger analysis of a single book (force reanalysis)
///
/// # Permission Required
/// - `books:write`
#[utoipa::path(
    post,
    path = "/api/v1/books/{id}/analyze",
    params(
        ("id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "Analysis completed successfully"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Scans"
)]
pub async fn trigger_book_analysis(
    Path(book_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<crate::api::dto::scan::AnalysisResult>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksWrite)?;

    // Check if book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Run analysis
    let result = analyze_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Analysis failed: {}", e)))?;

    Ok(Json(result.into()))
}
