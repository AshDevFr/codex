//! Stub handlers for unimplemented Komga endpoints
//!
//! These handlers return empty results for endpoints that Komic expects
//! but Codex doesn't fully support. This prevents 404 errors in the client.

use super::super::dto::pagination::KomgaPage;
use super::super::dto::series::KomgaAuthorDto;
use super::super::dto::stubs::{KomgaCollectionDto, KomgaReadListDto, StubPaginationQuery};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{GenreRepository, TagRepository};
use crate::require_permission;
use axum::{
    Json,
    extract::{Query, State},
};
use std::sync::Arc;

/// List collections (stub - always returns empty)
///
/// Komga collections are user-created groupings of series.
/// Codex doesn't support this feature, so we return empty results.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/collections`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections",
    responses(
        (status = 200, description = "Empty list of collections", body = KomgaPage<KomgaCollectionDto>),
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
pub async fn list_collections(
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<StubPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaCollectionDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(KomgaPage::new(vec![], query.page, query.size, 0)))
}

/// List read lists (stub - always returns empty)
///
/// Komga read lists are user-created lists of books to read.
/// Codex doesn't support this feature, so we return empty results.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/readlists`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/readlists",
    responses(
        (status = 200, description = "Empty list of read lists", body = KomgaPage<KomgaReadListDto>),
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
pub async fn list_readlists(
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<StubPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaReadListDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    Ok(Json(KomgaPage::new(vec![], query.page, query.size, 0)))
}

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
