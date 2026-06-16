//! Stub handlers for unimplemented Komga endpoints
//!
//! These handlers return empty results for endpoints that Komic expects
//! but Codex doesn't fully support. This prevents 404 errors in the client.

use super::super::dto::series::KomgaAuthorDto;
use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use axum::{Json, extract::State};
use codex_db::repositories::{GenreRepository, TagRepository};
use std::sync::Arc;

/// List genres
///
/// Returns all genres in the library.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/genres`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/genres",
    responses(
        (status = 200, description = "List of all genres", body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_genres(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<String>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let genres = GenreRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;

    let genre_names: Vec<String> = genres.into_iter().map(|g| g.name).collect();
    Ok(Json(genre_names))
}

/// List tags
///
/// Returns all tags in the library.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/tags`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/tags",
    responses(
        (status = 200, description = "List of all tags", body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_tags(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<String>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let tags = TagRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let tag_names: Vec<String> = tags.into_iter().map(|t| t.name).collect();
    Ok(Json(tag_names))
}

/// List authors v2 (stub - always returns empty array)
///
/// Returns all authors in the library (v2 endpoint used by Komic).
/// Currently returns empty as Codex doesn't aggregate authors separately.
///
/// ## Endpoint
/// `GET /{prefix}/api/v2/authors`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v2/authors",
    responses(
        (status = 200, description = "Empty list of authors", body = Vec<KomgaAuthorDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_authors_v2(
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<KomgaAuthorDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(vec![]))
}

/// List languages (stub - always returns empty array)
///
/// Returns all languages in the library.
/// Currently returns empty as Codex doesn't aggregate languages separately.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/languages`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/languages",
    responses(
        (status = 200, description = "Empty list of languages", body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_languages(
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<String>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(vec![]))
}

/// List publishers (stub - always returns empty array)
///
/// Returns all publishers in the library.
/// Currently returns empty as Codex doesn't aggregate publishers separately.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/publishers`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/publishers",
    responses(
        (status = 200, description = "Empty list of publishers", body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_publishers(
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<String>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(vec![]))
}

/// List age ratings (stub - always returns empty array)
///
/// Returns all age ratings in the library.
/// Currently returns empty as Codex doesn't aggregate age ratings separately.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/age-ratings`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/age-ratings",
    responses(
        (status = 200, description = "Empty list of age ratings", body = Vec<i32>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_age_ratings(
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<i32>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(vec![]))
}

/// List series release dates (stub - always returns empty array)
///
/// Returns all release dates used by series in the library.
/// Currently returns empty as Codex doesn't aggregate release dates separately.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/release-dates`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/release-dates",
    responses(
        (status = 200, description = "Empty list of release dates", body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_series_release_dates(
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<String>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(vec![]))
}
