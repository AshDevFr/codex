use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use super::super::dto::{
    DuplicateGroup, ListDuplicatesResponse, ListSeriesDuplicatesQuery,
    ListSeriesDuplicatesResponse, SeriesDuplicateGroup, TriggerDuplicateScanResponse,
};
use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::entities::series_duplicates::{MATCH_TYPE_EXTERNAL_ID, MATCH_TYPE_TITLE};
use crate::db::repositories::{
    BookDuplicatesRepository, SeriesDuplicatesRepository, TaskRepository,
};
use crate::tasks::types::TaskType;

/// List all duplicate book groups
///
/// # Permission Required
/// - `books:read`
#[utoipa::path(
    get,
    path = "/api/v1/duplicates",
    responses(
        (status = 200, description = "List of duplicate groups", body = ListDuplicatesResponse),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Duplicates"
)]
pub async fn list_duplicates(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<ListDuplicatesResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    let duplicates = BookDuplicatesRepository::find_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list duplicates: {}", e)))?;

    let total_groups = duplicates.len();
    let total_duplicate_books: usize = duplicates.iter().map(|d| d.duplicate_count as usize).sum();

    let duplicate_groups = duplicates
        .into_iter()
        .map(|d| DuplicateGroup {
            id: d.id,
            file_hash: d.file_hash,
            book_ids: serde_json::from_str(&d.book_ids).unwrap_or_default(),
            duplicate_count: d.duplicate_count,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListDuplicatesResponse {
        duplicates: duplicate_groups,
        total_groups,
        total_duplicate_books,
    }))
}

/// Trigger a manual duplicate detection scan
///
/// # Permission Required
/// - `books:write`
#[utoipa::path(
    post,
    path = "/api/v1/duplicates/scan",
    responses(
        (status = 200, description = "Scan triggered", body = TriggerDuplicateScanResponse),
        (status = 403, description = "Permission denied"),
        (status = 409, description = "Scan already in progress"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Duplicates"
)]
pub async fn trigger_duplicate_scan(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<TriggerDuplicateScanResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksWrite)?;

    // Check if there's already a pending/processing duplicate scan
    use crate::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let existing_scan = Tasks::find()
        .filter(tasks::Column::TaskType.eq("find_duplicates"))
        .filter(tasks::Column::Status.is_in(vec!["pending", "processing"]))
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check existing scans: {}", e)))?;

    if existing_scan.is_some() {
        return Err(ApiError::Conflict(
            "Duplicate scan is already in progress or pending".to_string(),
        ));
    }

    // Enqueue the duplicate scan task
    let task_type = TaskType::FindDuplicates;
    let task_id = TaskRepository::enqueue(&state.db, task_type, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue duplicate scan: {}", e)))?;

    Ok(Json(TriggerDuplicateScanResponse {
        task_id,
        message: "Duplicate detection scan has been queued".to_string(),
    }))
}

/// Delete a specific duplicate group (does not delete books, just the duplicate record)
///
/// # Permission Required
/// - `books:write`
#[utoipa::path(
    delete,
    path = "/api/v1/duplicates/{duplicate_id}",
    params(
        ("duplicate_id" = Uuid, Path, description = "Duplicate group ID")
    ),
    responses(
        (status = 204, description = "Duplicate group deleted"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Duplicate group not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Duplicates"
)]
pub async fn delete_duplicate_group(
    State(state): State<Arc<AppState>>,
    Path(duplicate_id): Path<Uuid>,
    auth: AuthContext,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksWrite)?;

    // Check if the duplicate group exists
    use crate::db::entities::book_duplicates::Entity as BookDuplicates;
    use sea_orm::EntityTrait;

    let exists = BookDuplicates::find_by_id(duplicate_id)
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check duplicate group: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound(format!(
            "Duplicate group {} not found",
            duplicate_id
        )));
    }

    BookDuplicatesRepository::delete_group(&state.db, duplicate_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete duplicate group: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// List all series duplicate groups.
///
/// Optionally filter by `match_type` (`external_id` or `title`).
///
/// # Permission Required
/// - `series:read`
#[utoipa::path(
    get,
    path = "/api/v1/duplicates/series",
    params(ListSeriesDuplicatesQuery),
    responses(
        (status = 200, description = "List of series duplicate groups", body = ListSeriesDuplicatesResponse),
        (status = 400, description = "Invalid match_type"),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Duplicates"
)]
pub async fn list_series_duplicates(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSeriesDuplicatesQuery>,
    auth: AuthContext,
) -> Result<Json<ListSeriesDuplicatesResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    let groups = match query.match_type.as_deref() {
        None => SeriesDuplicatesRepository::find_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to list series duplicates: {}", e)))?,
        Some(value @ (MATCH_TYPE_EXTERNAL_ID | MATCH_TYPE_TITLE)) => {
            SeriesDuplicatesRepository::find_by_match_type(&state.db, value)
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("Failed to list series duplicates: {}", e))
                })?
        }
        Some(other) => {
            return Err(ApiError::BadRequest(format!(
                "Invalid match_type '{}': expected 'external_id' or 'title'",
                other
            )));
        }
    };

    let total_groups = groups.len();
    let mut external_id_groups = 0;
    let mut title_groups = 0;
    let mut total_duplicate_series = 0usize;

    let duplicate_groups: Vec<SeriesDuplicateGroup> = groups
        .into_iter()
        .map(|d| {
            total_duplicate_series += d.duplicate_count as usize;
            match d.match_type.as_str() {
                MATCH_TYPE_EXTERNAL_ID => external_id_groups += 1,
                MATCH_TYPE_TITLE => title_groups += 1,
                _ => {}
            }
            SeriesDuplicateGroup {
                id: d.id,
                match_type: d.match_type,
                match_key: d.match_key,
                library_id: d.library_id,
                series_ids: serde_json::from_str(&d.series_ids).unwrap_or_default(),
                duplicate_count: d.duplicate_count,
                created_at: d.created_at.to_rfc3339(),
                updated_at: d.updated_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(ListSeriesDuplicatesResponse {
        duplicates: duplicate_groups,
        total_groups,
        total_duplicate_series,
        external_id_groups,
        title_groups,
    }))
}

/// Delete a series duplicate group (does not delete the underlying series).
///
/// # Permission Required
/// - `series:write`
#[utoipa::path(
    delete,
    path = "/api/v1/duplicates/series/{duplicate_id}",
    params(
        ("duplicate_id" = Uuid, Path, description = "Series duplicate group ID")
    ),
    responses(
        (status = 204, description = "Duplicate group deleted"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Duplicate group not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Duplicates"
)]
pub async fn delete_series_duplicate_group(
    State(state): State<Arc<AppState>>,
    Path(duplicate_id): Path<Uuid>,
    auth: AuthContext,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;

    use crate::db::entities::prelude::SeriesDuplicates;
    use sea_orm::EntityTrait;

    let exists = SeriesDuplicates::find_by_id(duplicate_id)
        .one(&state.db)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to check series duplicate group: {}", e))
        })?;

    if exists.is_none() {
        return Err(ApiError::NotFound(format!(
            "Series duplicate group {} not found",
            duplicate_id
        )));
    }

    SeriesDuplicatesRepository::delete_group(&state.db, duplicate_id)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to delete series duplicate group: {}", e))
        })?;

    Ok(StatusCode::NO_CONTENT)
}
