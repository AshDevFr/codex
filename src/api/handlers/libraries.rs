use crate::api::{
    dto::{CreateLibraryRequest, LibraryDto, UpdateLibraryRequest},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::entities::libraries;
use crate::db::repositories::LibraryRepository;
use crate::require_permission;
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

    LibraryDto {
        id: library.id,
        name: library.name,
        path: library.path,
        description: None, // No description field in libraries entity
        is_active: true,   // No is_active field in libraries entity
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

    // Use the ScanningStrategy enum from the db module
    let mut library = LibraryRepository::create(
        &state.db,
        &request.name,
        &request.path,
        crate::db::ScanningStrategy::Default,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create library: {}", e)))?;

    // Track if we need to update after creation
    let mut needs_update = false;

    // Handle allowed_formats if provided
    if let Some(formats) = &request.allowed_formats {
        let formats_json = serde_json::to_string(formats)
            .map_err(|e| ApiError::BadRequest(format!("Invalid allowed formats: {}", e)))?;
        library.allowed_formats = Some(formats_json);
        needs_update = true;
    }

    // Handle excluded_patterns if provided
    if let Some(patterns) = &request.excluded_patterns {
        library.excluded_patterns = Some(patterns.clone());
        needs_update = true;
    }

    // Handle default_reading_direction if provided
    if let Some(direction) = &request.default_reading_direction {
        library.default_reading_direction = direction.clone();
        needs_update = true;
    }

    // Handle scanning_config if provided
    if let Some(config_dto) = &request.scanning_config {
        // Serialize the config to JSON string for database storage
        let config_json = serde_json::to_string(config_dto)
            .map_err(|e| ApiError::BadRequest(format!("Invalid scanning config: {}", e)))?;

        library.scanning_config = Some(config_json);
        needs_update = true;
    }

    // Save all optional fields if any were provided
    if needs_update {
        LibraryRepository::update(&state.db, &library)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update library config: {}", e)))?;
    }

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
    // Note: description and is_active fields don't exist in the entity
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
