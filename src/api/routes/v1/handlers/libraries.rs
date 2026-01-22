use super::super::dto::{
    CreateLibraryRequest, DetectedSeriesDto, DetectedSeriesMetadataDto, LibraryDto,
    PreviewScanRequest, PreviewScanResponse, UpdateLibraryRequest,
};
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::entities::libraries;
use crate::db::repositories::{CreateLibraryParams, LibraryRepository};
use crate::models::{BookStrategy, NumberStrategy, SeriesStrategy};
use crate::require_permission;
use crate::scanner::strategies::create_strategy;
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// Helper function to convert a library entity to a DTO
async fn library_to_dto(db: &DatabaseConnection, library: libraries::Model) -> LibraryDto {
    // Get counts
    let book_count = crate::db::repositories::BookRepository::count_by_library(db, library.id)
        .await
        .ok();
    let series_count = crate::db::repositories::SeriesRepository::count_by_library(db, library.id)
        .await
        .ok();

    // Parse allowed_formats from JSON string
    let allowed_formats = library
        .allowed_formats
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok());

    // Parse strategy fields
    let series_strategy = library
        .series_strategy
        .parse::<SeriesStrategy>()
        .unwrap_or(SeriesStrategy::SeriesVolume);
    let book_strategy = library
        .book_strategy
        .parse::<BookStrategy>()
        .unwrap_or(BookStrategy::Filename);
    let number_strategy = library
        .number_strategy
        .parse::<NumberStrategy>()
        .unwrap_or(NumberStrategy::FileOrder);

    // Extract config values (already serde_json::Value)
    let series_config = library.series_config;
    let book_config = library.book_config;
    let number_config = library.number_config;

    LibraryDto {
        id: library.id,
        name: library.name,
        path: library.path,
        description: None, // No description field in libraries entity
        is_active: true,   // No is_active field in libraries entity
        series_strategy,
        series_config,
        book_strategy,
        book_config,
        number_strategy,
        number_config,
        scanning_config: library
            .scanning_config
            .and_then(|json| serde_json::from_str(&json).ok()),
        last_scanned_at: library.last_scanned_at,
        created_at: library.created_at,
        updated_at: library.updated_at,
        book_count,
        series_count,
        allowed_formats,
        excluded_patterns: library.excluded_patterns,
        default_reading_direction: library.default_reading_direction,
    }
}

/// List all libraries
#[utoipa::path(
    get,
    path = "/api/v1/libraries",
    responses(
        (status = 200, description = "List of libraries", body = Vec<LibraryDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn list_libraries(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<Vec<LibraryDto>>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let libraries = LibraryRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;

    let mut dtos = Vec::new();
    for library in libraries {
        dtos.push(library_to_dto(&state.db, library).await);
    }

    Ok(Json(dtos))
}

/// Get library by ID
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    responses(
        (status = 200, description = "Library details", body = LibraryDto),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn get_library(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<LibraryDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    Ok(Json(library_to_dto(&state.db, library).await))
}

/// Create a new library
#[utoipa::path(
    post,
    path = "/api/v1/libraries",
    request_body = CreateLibraryRequest,
    responses(
        (status = 201, description = "Library created", body = LibraryDto),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn create_library(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateLibraryRequest>,
) -> Result<Json<LibraryDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;

    // Validate path exists
    if !std::path::Path::new(&request.path).exists() {
        return Err(ApiError::BadRequest(format!(
            "Path does not exist: {}",
            request.path
        )));
    }

    // Build CreateLibraryParams with all strategy and config fields
    let series_strategy = request.series_strategy.unwrap_or_default();
    let book_strategy = request.book_strategy.unwrap_or_default();
    let number_strategy = request.number_strategy.unwrap_or_default();

    // Use configs directly (already serde_json::Value)
    let series_config = request.series_config.clone();
    let book_config = request.book_config.clone();
    let number_config = request.number_config.clone();

    // Validate the strategy can be created (validates config is appropriate for strategy)
    let series_config_str = series_config.as_ref().map(|v| v.to_string());
    create_strategy(series_strategy, series_config_str.as_deref()).map_err(|e| {
        ApiError::BadRequest(format!("Invalid series strategy configuration: {}", e))
    })?;

    // Build params
    let mut params = CreateLibraryParams::new(&request.name, &request.path)
        .with_series_strategy(series_strategy)
        .with_series_config(series_config)
        .with_book_strategy(book_strategy)
        .with_book_config(book_config)
        .with_number_strategy(number_strategy)
        .with_number_config(number_config);

    // Add optional fields
    if let Some(config_dto) = &request.scanning_config {
        let config_json = serde_json::to_string(config_dto)
            .map_err(|e| ApiError::BadRequest(format!("Invalid scanning config: {}", e)))?;
        params = params.with_scanning_config(Some(config_json));
    }

    if let Some(formats) = &request.allowed_formats {
        let formats_json = serde_json::to_string(formats)
            .map_err(|e| ApiError::BadRequest(format!("Invalid allowed formats: {}", e)))?;
        params.allowed_formats = Some(formats_json);
    }

    if let Some(patterns) = &request.excluded_patterns {
        params.excluded_patterns = Some(patterns.clone());
    }

    if let Some(direction) = &request.default_reading_direction {
        params.default_reading_direction = Some(direction.clone());
    }

    // Create library with all params at once
    let library = LibraryRepository::create_with_params(&state.db, params)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create library: {}", e)))?;

    // Trigger scan immediately after creation if requested
    if request.scan_immediately {
        let task_type = crate::tasks::types::TaskType::ScanLibrary {
            library_id: library.id,
            mode: "normal".to_string(),
        };

        crate::db::repositories::TaskRepository::enqueue(&state.db, task_type, 0, None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to trigger auto-scan: {}", e)))?;
    }

    // Reload scheduler to pick up new library's scanning schedule
    if let Some(scheduler) = &state.scheduler {
        if let Err(e) = scheduler.lock().await.reload_schedules().await {
            tracing::warn!("Failed to reload scheduler after library creation: {}", e);
            // Don't fail the request - library was created successfully
        }
    }

    Ok(Json(library_to_dto(&state.db, library).await))
}

/// Update a library (partial update)
#[utoipa::path(
    patch,
    path = "/api/v1/libraries/{library_id}",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    request_body = UpdateLibraryRequest,
    responses(
        (status = 200, description = "Library updated", body = LibraryDto),
        (status = 404, description = "Library not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn update_library(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Json(request): Json<UpdateLibraryRequest>,
) -> Result<Json<LibraryDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;

    // Fetch existing library
    let mut library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Update fields
    if let Some(name) = request.name {
        library.name = name;
    }
    if let Some(path) = request.path {
        // Validate path exists
        if !std::path::Path::new(&path).exists() {
            return Err(ApiError::BadRequest(format!(
                "Path does not exist: {}",
                path
            )));
        }
        library.path = path;
    }
    // Handle allowed_formats if provided
    if let Some(formats) = request.allowed_formats {
        let formats_json = serde_json::to_string(&formats)
            .map_err(|e| ApiError::BadRequest(format!("Invalid allowed formats: {}", e)))?;
        library.allowed_formats = Some(formats_json);
    }
    // Handle excluded_patterns if provided
    if let Some(patterns) = request.excluded_patterns {
        library.excluded_patterns = Some(patterns);
    }
    // Handle default_reading_direction if provided
    if let Some(direction) = request.default_reading_direction {
        library.default_reading_direction = direction;
    }
    // Handle scanning_config if provided
    if let Some(config_dto) = request.scanning_config {
        // Serialize the config to JSON string for database storage
        let config_json = serde_json::to_string(&config_dto)
            .map_err(|e| ApiError::BadRequest(format!("Invalid scanning config: {}", e)))?;

        library.scanning_config = Some(config_json);
    }
    // Handle book_strategy if provided (mutable)
    if let Some(book_strategy) = request.book_strategy {
        library.book_strategy = book_strategy.as_str().to_string();
    }
    // Handle book_config if provided (mutable)
    if let Some(book_config) = request.book_config {
        library.book_config = Some(book_config);
    }
    // Handle number_strategy if provided (mutable)
    if let Some(number_strategy) = request.number_strategy {
        library.number_strategy = number_strategy.as_str().to_string();
    }
    // Handle number_config if provided (mutable)
    if let Some(number_config) = request.number_config {
        library.number_config = Some(number_config);
    }
    // Note: description and is_active fields don't exist in the entity
    // Note: series_strategy and series_config are IMMUTABLE after creation
    library.updated_at = Utc::now();

    LibraryRepository::update(&state.db, &library)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update library: {}", e)))?;

    // Emit LibraryUpdated event
    {
        use crate::events::{EntityChangeEvent, EntityEvent};

        let event = EntityChangeEvent {
            event: EntityEvent::LibraryUpdated { library_id },
            user_id: Some(auth.user_id),
            timestamp: Utc::now(),
        };

        if let Err(e) = state.event_broadcaster.emit(event) {
            tracing::warn!(
                "Failed to emit LibraryUpdated event for library {}: {:?}",
                library_id,
                e
            );
        }
    }

    // Fetch the updated library since update returns ()
    let updated = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch updated library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found after update".to_string()))?;

    // Reload scheduler to pick up updated library's scanning schedule
    if let Some(scheduler) = &state.scheduler {
        if let Err(e) = scheduler.lock().await.reload_schedules().await {
            tracing::warn!("Failed to reload scheduler after library update: {}", e);
            // Don't fail the request - library was updated successfully
        }
    }

    Ok(Json(library_to_dto(&state.db, updated).await))
}

/// Delete a library
#[utoipa::path(
    delete,
    path = "/api/v1/libraries/{library_id}",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    responses(
        (status = 204, description = "Library deleted"),
        (status = 404, description = "Library not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn delete_library(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<(), ApiError> {
    require_permission!(auth, Permission::LibrariesDelete)?;

    LibraryRepository::delete(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete library: {}", e)))?;

    // Emit LibraryDeleted event
    {
        use crate::events::{EntityChangeEvent, EntityEvent};

        let event = EntityChangeEvent {
            event: EntityEvent::LibraryDeleted { library_id },
            user_id: Some(auth.user_id),
            timestamp: Utc::now(),
        };

        if let Err(e) = state.event_broadcaster.emit(event) {
            tracing::warn!(
                "Failed to emit LibraryDeleted event for library {}: {:?}",
                library_id,
                e
            );
        }
    }

    // Reload scheduler to remove deleted library's scanning schedule
    if let Some(scheduler) = &state.scheduler {
        if let Err(e) = scheduler.lock().await.reload_schedules().await {
            tracing::warn!("Failed to reload scheduler after library deletion: {}", e);
            // Don't fail the request - library was deleted successfully
        }
    }

    Ok(())
}

/// Purge deleted books from a library
#[utoipa::path(
    delete,
    path = "/api/v1/libraries/{library_id}/purge-deleted",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    responses(
        (status = 200, description = "Number of books purged", body = u64),
        (status = 404, description = "Library not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn purge_deleted_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<u64>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;

    // Verify library exists
    LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Purge deleted books
    let count = crate::db::repositories::BookRepository::purge_deleted_in_library(
        &state.db,
        library_id,
        Some(&state.event_broadcaster),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to purge deleted books: {}", e)))?;

    Ok(Json(count))
}

/// Preview scan a path with a given strategy
///
/// This endpoint allows users to preview how a scanning strategy would organize
/// files without actually creating a library or importing anything. Useful for
/// testing strategy configurations before committing to them.
#[utoipa::path(
    post,
    path = "/api/v1/libraries/preview-scan",
    request_body = PreviewScanRequest,
    responses(
        (status = 200, description = "Preview scan results", body = PreviewScanResponse),
        (status = 400, description = "Invalid request or path"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "libraries"
)]
pub async fn preview_scan(
    State(_state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<PreviewScanRequest>,
) -> Result<Json<PreviewScanResponse>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;

    // Validate path exists
    let library_path = std::path::Path::new(&request.path);
    if !library_path.exists() {
        return Err(ApiError::BadRequest(format!(
            "Path does not exist: {}",
            request.path
        )));
    }

    if !library_path.is_dir() {
        return Err(ApiError::BadRequest(format!(
            "Path is not a directory: {}",
            request.path
        )));
    }

    // Parse series strategy and config
    let series_strategy = request.series_strategy.unwrap_or_default();
    let series_config_str = request.series_config.as_ref().map(|v| v.to_string());

    // Create the strategy
    let strategy = create_strategy(series_strategy, series_config_str.as_deref()).map_err(|e| {
        ApiError::BadRequest(format!("Invalid series strategy configuration: {}", e))
    })?;

    // Discover files in the path (similar to scanner's discover_files)
    let discovered_files = discover_preview_files(library_path)
        .map_err(|e| ApiError::Internal(format!("Failed to scan directory: {}", e)))?;

    let total_files = discovered_files.len();

    // Use the strategy to organize files
    let series_map = strategy
        .organize_files(&discovered_files, library_path)
        .map_err(|e| ApiError::Internal(format!("Failed to organize files: {}", e)))?;

    // Convert to DTOs
    let detected_series: Vec<DetectedSeriesDto> = series_map
        .into_iter()
        .map(|(name, detected)| {
            let sample_books: Vec<String> = detected
                .books
                .iter()
                .take(5)
                .filter_map(|b| b.path.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .collect();

            DetectedSeriesDto {
                name,
                path: detected.path,
                book_count: detected.books.len(),
                sample_books,
                metadata: if detected.metadata.publisher.is_some()
                    || detected.metadata.author.is_some()
                {
                    Some(DetectedSeriesMetadataDto {
                        publisher: detected.metadata.publisher,
                        author: detected.metadata.author,
                    })
                } else {
                    None
                },
            }
        })
        .collect();

    Ok(Json(PreviewScanResponse {
        detected_series,
        total_files,
    }))
}

/// Discover files for preview scan (simplified version of scanner's discover_files)
fn discover_preview_files(
    library_path: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    use std::fs;

    let mut files = Vec::new();

    fn visit_dir(
        dir: &std::path::Path,
        files: &mut Vec<std::path::PathBuf>,
    ) -> Result<(), std::io::Error> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, files)?;
            } else if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                // Check for supported formats
                if matches!(ext_lower.as_str(), "cbz" | "cbr" | "epub" | "pdf") {
                    files.push(path);
                }
            }
        }
        Ok(())
    }

    visit_dir(library_path, &mut files)?;
    Ok(files)
}
