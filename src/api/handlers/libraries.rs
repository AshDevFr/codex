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
use std::sync::Arc;
use uuid::Uuid;

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

    let dtos: Vec<LibraryDto> = libraries
        .into_iter()
        .map(|lib| LibraryDto {
            id: lib.id,
            name: lib.name,
            path: lib.path,
            description: None, // No description field in libraries entity
            is_active: true, // No is_active field in libraries entity
            last_scanned_at: lib.last_scanned_at,
            created_at: lib.created_at,
            updated_at: lib.updated_at,
        })
        .collect();

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

    let dto = LibraryDto {
        id: library.id,
        name: library.name,
        path: library.path,
        description: None, // No description field in libraries entity
        is_active: true, // No is_active field in libraries entity
        last_scanned_at: library.last_scanned_at,
        created_at: library.created_at,
        updated_at: library.updated_at,
    };

    Ok(Json(dto))
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
    let library = LibraryRepository::create(
        &state.db,
        &request.name,
        &request.path,
        crate::db::ScanningStrategy::Default,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create library: {}", e)))?;

    let dto = LibraryDto {
        id: library.id,
        name: library.name,
        path: library.path,
        description: None, // No description field
        is_active: true, // No is_active field
        last_scanned_at: library.last_scanned_at,
        created_at: library.created_at,
        updated_at: library.updated_at,
    };

    Ok(Json(dto))
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

    let dto = LibraryDto {
        id: updated.id,
        name: updated.name,
        path: updated.path,
        description: None, // No description field
        is_active: true, // No is_active field
        last_scanned_at: updated.last_scanned_at,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    };

    Ok(Json(dto))
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
