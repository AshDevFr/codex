//! Handlers for series export endpoints

use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::extractors::auth::{AppState, AuthContext};
use crate::db::repositories::{SeriesExportRepository, TaskRepository};
use crate::services::book_export_collector::BookExportField;
use crate::services::series_export_collector::ExportField;
use crate::tasks::types::TaskType;

use super::super::dto::series_export::{
    CreateSeriesExportRequest, ExportFieldCatalogResponse, ExportFieldDto, ExportPresetsDto,
    SeriesExportDto, SeriesExportListResponse,
};

/// Default concurrent export limit per user
const DEFAULT_MAX_CONCURRENT: u64 = 3;
/// Default retention days
const DEFAULT_RETENTION_DAYS: u64 = 7;

/// POST /user/exports/series - Create a new series export job
#[utoipa::path(
    post,
    path = "/api/v1/user/exports/series",
    request_body = CreateSeriesExportRequest,
    responses(
        (status = 202, description = "Export job created", body = SeriesExportDto),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Concurrent export limit reached"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Series Exports"
)]
pub async fn create_export(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<CreateSeriesExportRequest>,
) -> Result<(StatusCode, Json<SeriesExportDto>), ApiError> {
    let user_id = auth.user_id;

    // Validate format
    if !["json", "csv", "md"].contains(&request.format.as_str()) {
        return Err(ApiError::BadRequest(
            "Format must be 'json', 'csv', or 'md'".to_string(),
        ));
    }

    // Validate export type
    if !["series", "books", "both"].contains(&request.export_type.as_str()) {
        return Err(ApiError::BadRequest(
            "Export type must be 'series', 'books', or 'both'".to_string(),
        ));
    }

    // "both" mode does not support CSV (two different schemas)
    if request.export_type == "both" && request.format == "csv" {
        return Err(ApiError::BadRequest(
            "CSV format is not supported for 'both' export type. Use JSON or Markdown.".to_string(),
        ));
    }

    // Validate at least one library
    if request.library_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one library must be selected".to_string(),
        ));
    }

    // Validate series fields (required for "series" and "both")
    let needs_series_fields = request.export_type != "books";
    if needs_series_fields && request.fields.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one series field must be selected".to_string(),
        ));
    }
    for key in &request.fields {
        if ExportField::parse(key).is_none() {
            return Err(ApiError::BadRequest(format!("Unknown series field: {key}")));
        }
    }

    // Validate book fields (required for "books" and "both")
    let needs_book_fields = request.export_type != "series";
    if needs_book_fields && request.book_fields.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one book field must be selected".to_string(),
        ));
    }
    for key in &request.book_fields {
        if BookExportField::parse(key).is_none() {
            return Err(ApiError::BadRequest(format!("Unknown book field: {key}")));
        }
    }

    // Check concurrent export limit
    let settings = &state.settings_service;
    let max_concurrent = settings
        .get_uint("exports.max_concurrent_per_user", DEFAULT_MAX_CONCURRENT)
        .await
        .unwrap_or(DEFAULT_MAX_CONCURRENT);

    let active_count = SeriesExportRepository::count_non_terminal_by_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check active exports: {e}")))?;

    if active_count >= max_concurrent {
        return Err(ApiError::Conflict(format!(
            "You already have {active_count} active exports (limit: {max_concurrent})"
        )));
    }

    // Compute expiry
    let retention_days = settings
        .get_uint("exports.retention_days", DEFAULT_RETENTION_DAYS)
        .await
        .unwrap_or(DEFAULT_RETENTION_DAYS);
    let expires_at = Utc::now() + Duration::days(retention_days as i64);

    // Create export record
    let library_ids_json = serde_json::to_value(&request.library_ids)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize library_ids: {e}")))?;
    let fields_json = serde_json::to_value(&request.fields)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize fields: {e}")))?;
    let book_fields_json = if request.book_fields.is_empty() {
        None
    } else {
        Some(
            serde_json::to_value(&request.book_fields)
                .map_err(|e| ApiError::Internal(format!("Failed to serialize book_fields: {e}")))?,
        )
    };

    let export = SeriesExportRepository::create(
        &state.db,
        user_id,
        &request.format,
        &request.export_type,
        library_ids_json,
        fields_json,
        book_fields_json,
        expires_at,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create export: {e}")))?;

    // Enqueue background task
    let task_type = TaskType::ExportSeries {
        export_id: export.id,
        user_id,
    };

    TaskRepository::enqueue(&state.db, task_type, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue export task: {e}")))?;

    let dto = SeriesExportDto::from_model(&export);
    Ok((StatusCode::ACCEPTED, Json(dto)))
}

/// GET /user/exports/series - List current user's exports
#[utoipa::path(
    get,
    path = "/api/v1/user/exports/series",
    responses(
        (status = 200, description = "List of exports", body = SeriesExportListResponse),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Series Exports"
)]
pub async fn list_exports(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<SeriesExportListResponse>, ApiError> {
    let exports = SeriesExportRepository::list_by_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list exports: {e}")))?;

    let dtos: Vec<SeriesExportDto> = exports.iter().map(SeriesExportDto::from_model).collect();

    Ok(Json(SeriesExportListResponse { exports: dtos }))
}

/// GET /user/exports/series/{id} - Get a single export's details
#[utoipa::path(
    get,
    path = "/api/v1/user/exports/series/{id}",
    params(("id" = Uuid, Path, description = "Export ID")),
    responses(
        (status = 200, description = "Export details", body = SeriesExportDto),
        (status = 404, description = "Export not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Series Exports"
)]
pub async fn get_export(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<SeriesExportDto>, ApiError> {
    let export = SeriesExportRepository::find_by_id_and_user(&state.db, id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get export: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Export not found".to_string()))?;

    Ok(Json(SeriesExportDto::from_model(&export)))
}

/// GET /user/exports/series/{id}/download - Download the export file
#[utoipa::path(
    get,
    path = "/api/v1/user/exports/series/{id}/download",
    params(("id" = Uuid, Path, description = "Export ID")),
    responses(
        (status = 200, description = "Export file", content_type = "application/octet-stream"),
        (status = 404, description = "Export not found or file missing"),
        (status = 409, description = "Export not yet completed"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Series Exports"
)]
pub async fn download_export(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let export = SeriesExportRepository::find_by_id_and_user(&state.db, id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get export: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Export not found".to_string()))?;

    if export.status != "completed" {
        return Err(ApiError::Conflict(format!(
            "Export is not ready for download (status: {})",
            export.status
        )));
    }

    let file_path = export.file_path.as_deref().ok_or_else(|| {
        ApiError::Internal("Export completed but file path is missing".to_string())
    })?;

    // Read file contents
    let data = tokio::fs::read(file_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound("Export file no longer exists on disk".to_string())
        } else {
            ApiError::Internal(format!("Failed to read export file: {e}"))
        }
    })?;

    let content_type = match export.format.as_str() {
        "csv" => "text/csv; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        _ => "application/json; charset=utf-8",
    };

    let ext = match export.format.as_str() {
        "csv" => "csv",
        "md" => "md",
        _ => "json",
    };

    let timestamp = export.created_at.format("%Y%m%d_%H%M%S");
    let filename = format!("codex-series-export-{timestamp}.{ext}");

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, data.len())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(data))
        .unwrap())
}

/// DELETE /user/exports/series/{id} - Delete an export and its file
#[utoipa::path(
    delete,
    path = "/api/v1/user/exports/series/{id}",
    params(("id" = Uuid, Path, description = "Export ID")),
    responses(
        (status = 204, description = "Export deleted"),
        (status = 404, description = "Export not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Series Exports"
)]
pub async fn delete_export(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let export = SeriesExportRepository::find_by_id_and_user(&state.db, id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get export: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Export not found".to_string()))?;

    // Delete file via ExportStorage if available, otherwise by path directly
    if let Some(ref storage) = state.export_storage {
        let _ = storage.delete(auth.user_id, id, &export.format).await;
    } else if let Some(ref file_path) = export.file_path {
        let _ = tokio::fs::remove_file(file_path).await;
    }

    // Delete DB record
    SeriesExportRepository::delete_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete export: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /user/exports/series/fields - Get the field catalog
#[utoipa::path(
    get,
    path = "/api/v1/user/exports/series/fields",
    responses(
        (status = 200, description = "Field catalog", body = ExportFieldCatalogResponse),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Series Exports"
)]
pub async fn get_field_catalog() -> Json<ExportFieldCatalogResponse> {
    let fields: Vec<ExportFieldDto> = ExportField::ALL
        .iter()
        .map(|f| ExportFieldDto {
            key: f.as_str().to_string(),
            label: f.label().to_string(),
            multi_value: f.is_multi_value(),
            user_specific: f.is_user_specific(),
            is_anchor: f.is_anchor(),
        })
        .collect();

    let book_fields: Vec<ExportFieldDto> = BookExportField::ALL
        .iter()
        .map(|f| ExportFieldDto {
            key: f.as_str().to_string(),
            label: f.label().to_string(),
            multi_value: f.is_multi_value(),
            user_specific: f.is_user_specific(),
            is_anchor: f.is_anchor(),
        })
        .collect();

    let presets = ExportPresetsDto {
        llm_select: ExportField::LLM_SELECT
            .iter()
            .map(|f| f.as_str().to_string())
            .collect(),
        llm_select_books: BookExportField::LLM_SELECT
            .iter()
            .map(|f| f.as_str().to_string())
            .collect(),
    };

    Json(ExportFieldCatalogResponse {
        fields,
        book_fields,
        presets,
    })
}
