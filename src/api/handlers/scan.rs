use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::api::{
    dto::{ScanStatusDto, TriggerScanQuery},
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
};
use crate::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, TaskRepository,
};
use crate::scanner::{
    analyze_book, analyze_library_books, analyze_series_books, AnalyzerConfig, ScanMode,
};
use crate::tasks::types::TaskType;

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

    // Check if there's already a pending/processing scan for this library
    use crate::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let existing_scan = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library_id))
        .filter(tasks::Column::Status.is_in(vec!["pending", "processing"]))
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check existing scans: {}", e)))?;

    if existing_scan.is_some() {
        return Err(ApiError::Conflict(format!(
            "Library {} is already being scanned or scan is pending",
            library_id
        )));
    }

    // Enqueue the scan task
    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: mode.to_string(),
    };

    let _task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue scan: {}", e)))?;

    // Return a pending status since the scan was just queued
    use chrono::Utc;

    let status = ScanStatusDto {
        library_id,
        status: "pending".to_string(),
        files_total: 0,
        files_processed: 0,
        series_found: 0,
        books_found: 0,
        errors: vec![],
        started_at: Utc::now(),
        completed_at: None,
    };

    Ok(Json(status))
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

    // Find the most recent scan task for this library
    use crate::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let task = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library_id))
        .order_by_desc(tasks::Column::CreatedAt)
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to query scan tasks: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("No scan found for this library".to_string()))?;

    // Convert task to ScanStatusDto
    use chrono::Utc;

    let status = ScanStatusDto {
        library_id,
        status: task.status,
        files_total: 0,
        files_processed: 0,
        series_found: 0,
        books_found: 0,
        errors: vec![],
        started_at: task.started_at.unwrap_or_else(|| Utc::now()),
        completed_at: task.completed_at,
    };

    Ok(Json(status))
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

    // Find the active scan task for this library
    use crate::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let task = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library_id))
        .filter(tasks::Column::Status.is_in(vec!["pending", "processing"]))
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to query scan tasks: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("No active scan found for this library".to_string()))?;

    // Cancel the task
    TaskRepository::cancel(&state.db, task.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to cancel scan: {}", e)))?;

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

    // Get all active scan tasks
    use crate::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let tasks = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::Status.is_in(vec!["pending", "processing"]))
        .all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to query scan tasks: {}", e)))?;

    // Convert tasks to ScanStatusDto
    use chrono::Utc;

    let dtos: Vec<ScanStatusDto> = tasks
        .into_iter()
        .filter_map(|task| {
            task.library_id.map(|library_id| ScanStatusDto {
                library_id,
                status: task.status.clone(),
                files_total: 0,
                files_processed: 0,
                series_found: 0,
                books_found: 0,
                errors: vec![],
                started_at: task.started_at.unwrap_or_else(|| Utc::now()),
                completed_at: task.completed_at,
            })
        })
        .collect();

    Ok(Json(dtos))
}

/// Stream scan progress updates via Server-Sent Events
///
/// # Permission Required
/// - `libraries:read`
///
/// **DEPRECATED**: This endpoint is replaced by `/api/v1/tasks/stream` which provides
/// real-time updates for all task types including scans. This endpoint now filters
/// the task stream to only show scan_library tasks for backwards compatibility.
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

    // Subscribe to task progress events from the event broadcaster
    let mut receiver = state.event_broadcaster.subscribe_tasks();

    // Create SSE stream that filters for scan_library tasks only
    let db = state.db.clone();
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    // Only emit scan_library task events
                    if event.task_type == "scan_library" {
                        if let Some(library_id) = event.library_id {
                            let (files_processed, files_total) = if let Some(ref prog) = event.progress {
                                (prog.current, prog.total)
                            } else {
                                (0, 0)
                            };

                            let status_str = match event.status {
                                crate::events::TaskStatus::Pending => "pending",
                                crate::events::TaskStatus::Running => "running",
                                crate::events::TaskStatus::Completed => "completed",
                                crate::events::TaskStatus::Failed => "failed",
                            };

                            // For completed tasks, try to extract scan counts from task result
                            let (series_found, books_found) = if event.status == crate::events::TaskStatus::Completed {
                                // Query task result to get actual scan counts
                                match TaskRepository::get_by_id(&db, event.task_id).await {
                                    Ok(Some(task)) if task.result.is_some() => {
                                        if let Some(result) = task.result {
                                            let series = result.get("series_created")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0) as usize;
                                            let books = result.get("books_created")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0) as usize;
                                            (series, books)
                                        } else {
                                            (0, 0)
                                        }
                                    }
                                    _ => (0, 0),
                                }
                            } else {
                                (0, 0)
                            };

                            let dto = ScanStatusDto {
                                library_id,
                                status: status_str.to_string(),
                                files_total,
                                files_processed,
                                series_found,
                                books_found,
                                errors: vec![],
                                started_at: event.started_at,
                                completed_at: event.completed_at,
                            };
                            if let Ok(json) = serde_json::to_string(&dto) {
                                yield Ok(Event::default().data(json));
                            }
                        }
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
