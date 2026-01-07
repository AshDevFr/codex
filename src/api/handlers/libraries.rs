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
    path = "/api/v1/libraries/{id}",
    params(
        ("id" = Uuid, Path, description = "Library ID")
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
    Path(id): Path<Uuid>,
) -> Result<Json<LibraryDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let library = LibraryRepository::get_by_id(&state.db, id)
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

    // Handle scanning_config if provided
    if let Some(config_dto) = request.scanning_config {
        // Serialize the config to JSON string for database storage
        let config_json = serde_json::to_string(&config_dto)
            .map_err(|e| ApiError::BadRequest(format!("Invalid scanning config: {}", e)))?;

        library.scanning_config = Some(config_json);
        LibraryRepository::update(&state.db, &library)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update library config: {}", e)))?;

        // Trigger auto-scan if enabled
        if config_dto.auto_scan_on_create {
            let scan_mode = crate::scanner::ScanMode::from_str(&config_dto.scan_mode)
                .map_err(|e| ApiError::BadRequest(e))?;

            state
                .scan_manager
                .trigger_scan(library.id, scan_mode)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to trigger auto-scan: {}", e)))?;
        }
    }

    Ok(Json(library_to_dto(&state.db, library).await))
}

/// Update a library
#[utoipa::path(
    put,
    path = "/api/v1/libraries/{id}",
    params(
        ("id" = Uuid, Path, description = "Library ID")
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
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateLibraryRequest>,
) -> Result<Json<LibraryDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;

    // Fetch existing library
    let mut library = LibraryRepository::get_by_id(&state.db, id)
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

    // Fetch the updated library since update returns ()
    let updated = LibraryRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch updated library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found after update".to_string()))?;

    Ok(Json(library_to_dto(&state.db, updated).await))
}

/// Delete a library
#[utoipa::path(
    delete,
    path = "/api/v1/libraries/{id}",
    params(
        ("id" = Uuid, Path, description = "Library ID")
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
    Path(id): Path<Uuid>,
) -> Result<(), ApiError> {
    require_permission!(auth, Permission::LibrariesDelete)?;

    LibraryRepository::delete(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete library: {}", e)))?;

    Ok(())
}

/// Purge deleted books from a library
#[utoipa::path(
    delete,
    path = "/api/v1/libraries/{id}/purge-deleted",
    params(
        ("id" = Uuid, Path, description = "Library ID")
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
    let count =
        crate::db::repositories::BookRepository::purge_deleted_in_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to purge deleted books: {}", e)))?;

    Ok(Json(count))
}
