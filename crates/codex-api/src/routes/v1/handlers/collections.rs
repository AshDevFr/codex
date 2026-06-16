//! Handlers for collections (shared, ordered groupings of series).
//!
//! Reads require `CollectionsRead` (granted to every role); create/modify
//! require `CollectionsWrite`; delete requires `CollectionsDelete` (write/delete
//! are in the Maintainer bundle). Member lists and counts are filtered through
//! the requesting user's sharing-tag visibility.

use super::super::dto::{
    AddSeriesToCollectionRequest, CollectionDto, CollectionListResponse, CreateCollectionRequest,
    ReorderCollectionSeriesRequest, SeriesDto, UpdateCollectionRequest,
};
use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::Redirect,
};
use codex_db::repositories::{
    CollectionRepository, SeriesRepository, visibility::SeriesVisibility,
};
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_collections,
        create_collection,
        get_collection,
        update_collection,
        delete_collection,
        get_collection_series,
        add_collection_series,
        remove_collection_series,
        reorder_collection_series,
        get_collection_thumbnail,
        get_series_collections,
    ),
    components(schemas(
        CollectionDto,
        CollectionListResponse,
        CreateCollectionRequest,
        UpdateCollectionRequest,
        AddSeriesToCollectionRequest,
        ReorderCollectionSeriesRequest,
    )),
    tags(
        (name = "Collections", description = "Shared, ordered groupings of series")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct CollectionsApi;

fn internal<E: std::fmt::Display>(context: &str) -> impl Fn(E) -> ApiError + '_ {
    move |e| ApiError::Internal(format!("{context}: {e}"))
}

/// Build a CollectionDto with the requesting user's visible member count.
async fn collection_dto(
    db: &sea_orm::DatabaseConnection,
    model: codex_db::entities::collections::Model,
    vis: Option<&SeriesVisibility>,
) -> Result<CollectionDto, ApiError> {
    let count = CollectionRepository::count_series(db, model.id, vis)
        .await
        .map_err(internal("Failed to count collection series"))?;
    Ok(CollectionDto::from_model(model, count))
}

async fn user_visibility(
    state: &AuthState,
    user_id: Uuid,
) -> Result<Option<SeriesVisibility>, ApiError> {
    let filter = ContentFilter::for_user(&state.db, user_id)
        .await
        .map_err(internal("Failed to load content filter"))?;
    Ok(filter.to_visibility())
}

/// List all collections.
#[utoipa::path(
    get,
    path = "/api/v1/collections",
    responses(
        (status = 200, description = "Collections", body = CollectionListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn list_collections(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<CollectionListResponse>, ApiError> {
    require_permission!(auth, Permission::CollectionsRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let collections = CollectionRepository::list_all(&state.db)
        .await
        .map_err(internal("Failed to list collections"))?;

    let mut items = Vec::with_capacity(collections.len());
    for model in collections {
        items.push(collection_dto(&state.db, model, vis.as_ref()).await?);
    }
    let total = items.len();
    Ok(Json(CollectionListResponse { items, total }))
}

/// Create a collection.
#[utoipa::path(
    post,
    path = "/api/v1/collections",
    request_body = CreateCollectionRequest,
    responses(
        (status = 201, description = "Created", body = CollectionDto),
        (status = 400, description = "Invalid name"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "A collection with that name already exists"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn create_collection(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateCollectionRequest>,
) -> Result<(StatusCode, Json<CollectionDto>), ApiError> {
    require_permission!(auth, Permission::CollectionsWrite)?;

    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest(
            "Collection name cannot be empty".to_string(),
        ));
    }
    if CollectionRepository::get_by_name(&state.db, name)
        .await
        .map_err(internal("Failed to check collection name"))?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "A collection named '{name}' already exists"
        )));
    }

    let model = CollectionRepository::create(&state.db, name, request.ordered)
        .await
        .map_err(internal("Failed to create collection"))?;
    Ok((
        StatusCode::CREATED,
        Json(CollectionDto::from_model(model, 0)),
    ))
}

/// Get a collection.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{collection_id}",
    responses(
        (status = 200, description = "Collection", body = CollectionDto),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn get_collection(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
) -> Result<Json<CollectionDto>, ApiError> {
    require_permission!(auth, Permission::CollectionsRead)?;
    let model = CollectionRepository::get_by_id(&state.db, collection_id)
        .await
        .map_err(internal("Failed to fetch collection"))?
        .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;
    let vis = user_visibility(&state, auth.user_id).await?;
    Ok(Json(collection_dto(&state.db, model, vis.as_ref()).await?))
}

/// Update a collection (rename / toggle ordered).
#[utoipa::path(
    patch,
    path = "/api/v1/collections/{collection_id}",
    request_body = UpdateCollectionRequest,
    responses(
        (status = 200, description = "Updated", body = CollectionDto),
        (status = 400, description = "Invalid name"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 409, description = "Name already in use"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn update_collection(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
    Json(request): Json<UpdateCollectionRequest>,
) -> Result<Json<CollectionDto>, ApiError> {
    require_permission!(auth, Permission::CollectionsWrite)?;

    if let Some(ref new_name) = request.name {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(ApiError::BadRequest(
                "Collection name cannot be empty".to_string(),
            ));
        }
        if let Some(existing) = CollectionRepository::get_by_name(&state.db, trimmed)
            .await
            .map_err(internal("Failed to check collection name"))?
            && existing.id != collection_id
        {
            return Err(ApiError::Conflict(format!(
                "A collection named '{trimmed}' already exists"
            )));
        }
    }

    let model = CollectionRepository::update(
        &state.db,
        collection_id,
        request.name.as_deref().map(str::trim),
        request.ordered,
    )
    .await
    .map_err(internal("Failed to update collection"))?
    .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;

    let vis = user_visibility(&state, auth.user_id).await?;
    Ok(Json(collection_dto(&state.db, model, vis.as_ref()).await?))
}

/// Delete a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{collection_id}",
    responses(
        (status = 204, description = "Deleted"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn delete_collection(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::CollectionsDelete)?;
    let deleted = CollectionRepository::delete(&state.db, collection_id)
        .await
        .map_err(internal("Failed to delete collection"))?;
    if !deleted {
        return Err(ApiError::NotFound("Collection not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Get the series in a collection (visibility-filtered, in stored order).
#[utoipa::path(
    get,
    path = "/api/v1/collections/{collection_id}/series",
    responses(
        (status = 200, description = "Member series", body = [SeriesDto]),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn get_collection_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::CollectionsRead)?;
    ensure_collection_exists(&state, collection_id).await?;

    let vis = user_visibility(&state, auth.user_id).await?;
    let members = CollectionRepository::get_series(&state.db, collection_id, vis.as_ref())
        .await
        .map_err(internal("Failed to fetch collection series"))?;

    let dtos =
        super::series::series_to_dtos_batched(&state.db, members, Some(auth.user_id)).await?;
    Ok(Json(dtos))
}

/// Add one or more series to a collection.
#[utoipa::path(
    post,
    path = "/api/v1/collections/{collection_id}/series",
    request_body = AddSeriesToCollectionRequest,
    responses(
        (status = 200, description = "Updated collection", body = CollectionDto),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Collection or series not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn add_collection_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
    Json(request): Json<AddSeriesToCollectionRequest>,
) -> Result<Json<CollectionDto>, ApiError> {
    require_permission!(auth, Permission::CollectionsWrite)?;
    let model = CollectionRepository::get_by_id(&state.db, collection_id)
        .await
        .map_err(internal("Failed to fetch collection"))?
        .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;

    for series_id in &request.series_ids {
        if SeriesRepository::get_by_id(&state.db, *series_id)
            .await
            .map_err(internal("Failed to look up series"))?
            .is_none()
        {
            return Err(ApiError::NotFound(format!("Series {series_id} not found")));
        }
        CollectionRepository::add_series(&state.db, collection_id, *series_id)
            .await
            .map_err(internal("Failed to add series to collection"))?;
    }

    let vis = user_visibility(&state, auth.user_id).await?;
    Ok(Json(collection_dto(&state.db, model, vis.as_ref()).await?))
}

/// Remove a series from a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{collection_id}/series/{series_id}",
    responses(
        (status = 204, description = "Removed (or was not a member)"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn remove_collection_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((collection_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::CollectionsWrite)?;
    CollectionRepository::remove_series(&state.db, collection_id, series_id)
        .await
        .map_err(internal("Failed to remove series from collection"))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Set the manual order of a collection's series.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{collection_id}/series",
    request_body = ReorderCollectionSeriesRequest,
    responses(
        (status = 204, description = "Reordered"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn reorder_collection_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
    Json(request): Json<ReorderCollectionSeriesRequest>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::CollectionsWrite)?;
    ensure_collection_exists(&state, collection_id).await?;
    CollectionRepository::reorder(&state.db, collection_id, &request.series_ids)
        .await
        .map_err(internal("Failed to reorder collection series"))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Get a collection's thumbnail (the first visible member series' cover).
#[utoipa::path(
    get,
    path = "/api/v1/collections/{collection_id}/thumbnail",
    responses(
        (status = 307, description = "Redirect to the first member series thumbnail"),
        (status = 404, description = "No visible member series"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn get_collection_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(collection_id): Path<Uuid>,
) -> Result<Redirect, ApiError> {
    auth.require_permission(&Permission::CollectionsRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    let members = CollectionRepository::get_series(&state.db, collection_id, vis.as_ref())
        .await
        .map_err(internal("Failed to fetch collection series"))?;
    let first = members
        .first()
        .ok_or_else(|| ApiError::NotFound("Collection has no visible series".to_string()))?;
    Ok(Redirect::temporary(&format!(
        "/api/v1/series/{}/thumbnail",
        first.id
    )))
}

/// List the collections that contain a given series.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/collections",
    responses(
        (status = 200, description = "Collections containing the series", body = CollectionListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Collections"
)]
pub async fn get_series_collections(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<CollectionListResponse>, ApiError> {
    require_permission!(auth, Permission::CollectionsRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let collections = CollectionRepository::get_collections_for_series(&state.db, series_id)
        .await
        .map_err(internal("Failed to fetch collections for series"))?;

    let mut items = Vec::with_capacity(collections.len());
    for model in collections {
        items.push(collection_dto(&state.db, model, vis.as_ref()).await?);
    }
    let total = items.len();
    Ok(Json(CollectionListResponse { items, total }))
}

async fn ensure_collection_exists(state: &AuthState, collection_id: Uuid) -> Result<(), ApiError> {
    if CollectionRepository::get_by_id(&state.db, collection_id)
        .await
        .map_err(internal("Failed to fetch collection"))?
        .is_none()
    {
        return Err(ApiError::NotFound("Collection not found".to_string()));
    }
    Ok(())
}
