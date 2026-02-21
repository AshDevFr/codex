//! Bulk metadata editing handlers
//!
//! Handlers for bulk metadata PATCH, bulk tag/genre add/remove,
//! and bulk metadata lock toggling for series and books.

use super::super::dto::bulk_metadata::*;
use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::entities::{book_metadata, series_metadata};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, GenreRepository, SeriesMetadataRepository,
    SeriesRepository, TagRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent};
use crate::require_permission;
use crate::utils::{
    json_merge_patch, parse_custom_metadata, serialize_custom_metadata,
    validate_custom_metadata_size,
};
use axum::{Json, extract::State};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};
use std::sync::Arc;
use uuid::Uuid;

/// Maximum number of series in a bulk request
const MAX_BULK_SERIES: usize = 100;
/// Maximum number of books in a bulk request
const MAX_BULK_BOOKS: usize = 500;

// ============================================================================
// Bulk Series Metadata PATCH
// ============================================================================

/// Bulk patch series metadata
///
/// Applies the same partial metadata update to multiple series at once.
/// Only provided fields will be updated. Changed fields are auto-locked.
/// Non-existent series are silently skipped.
#[utoipa::path(
    patch,
    path = "/api/v1/series/bulk/metadata",
    request_body = BulkPatchSeriesMetadataRequest,
    responses(
        (status = 200, description = "Metadata updated", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_patch_series_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkPatchSeriesMetadataRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No series specified".to_string(),
        }));
    }

    if request.series_ids.len() > MAX_BULK_SERIES {
        return Err(ApiError::BadRequest(format!(
            "Too many series (max {})",
            MAX_BULK_SERIES
        )));
    }

    let now = Utc::now();
    let mut updated_count = 0usize;

    // Pre-compute values that are the same for all items
    let publisher_opt = request.publisher.into_nested_option();
    let imprint_opt = request.imprint.into_nested_option();
    let status_opt = request.status.into_nested_option();
    let age_rating_opt = request.age_rating.into_nested_option();
    let language_opt = request.language.into_nested_option();
    let reading_direction_opt = request.reading_direction.into_nested_option();
    let year_opt = request.year.into_nested_option();
    let total_book_count_opt = request.total_book_count.into_nested_option();
    let custom_metadata_opt = request.custom_metadata.into_nested_option();
    let authors_opt = request.authors.into_nested_option();

    for series_id in &request.series_ids {
        // Verify series exists
        let series = match SeriesRepository::get_by_id(&state.db, *series_id).await {
            Ok(Some(s)) => s,
            _ => continue,
        };

        // Get existing metadata
        let existing = match SeriesMetadataRepository::get_by_series_id(&state.db, *series_id).await
        {
            Ok(Some(m)) => m,
            _ => continue,
        };

        let existing_custom_metadata = existing.custom_metadata.clone();
        let mut active: series_metadata::ActiveModel = existing.into();
        let mut has_changes = false;

        if let Some(ref opt) = publisher_opt {
            active.publisher = Set(opt.clone());
            has_changes = true;
        }
        if let Some(ref opt) = imprint_opt {
            active.imprint = Set(opt.clone());
            has_changes = true;
        }
        if let Some(ref opt) = status_opt {
            active.status = Set(opt.clone());
            has_changes = true;
        }
        if let Some(ref opt) = age_rating_opt {
            active.age_rating = Set(*opt);
            has_changes = true;
        }
        if let Some(ref opt) = language_opt {
            active.language = Set(opt.clone());
            has_changes = true;
        }
        if let Some(ref opt) = reading_direction_opt {
            active.reading_direction = Set(opt.clone());
            has_changes = true;
        }
        if let Some(ref opt) = year_opt {
            active.year = Set(*opt);
            has_changes = true;
        }
        if let Some(ref opt) = total_book_count_opt {
            active.total_book_count = Set(*opt);
            has_changes = true;
        }
        if let Some(ref opt) = custom_metadata_opt {
            let merged = if let Some(patch) = opt {
                let existing_cm = parse_custom_metadata(existing_custom_metadata.as_deref())
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                let merged = json_merge_patch(&existing_cm, patch);
                validate_custom_metadata_size(Some(&merged)).map_err(ApiError::BadRequest)?;
                Some(merged)
            } else {
                None
            };
            active.custom_metadata = Set(serialize_custom_metadata(merged.as_ref()));
            has_changes = true;
        }
        if let Some(ref opt) = authors_opt {
            active.authors_json = Set(opt
                .as_ref()
                .map(|authors| serde_json::to_string(authors).unwrap_or_default()));
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
            active.update(&state.db).await.map_err(|e| {
                ApiError::Internal(format!("Failed to update series metadata: {}", e))
            })?;
            updated_count += 1;

            let event = EntityChangeEvent {
                event: EntityEvent::SeriesUpdated {
                    series_id: *series_id,
                    library_id: series.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Updated metadata for {} series", updated_count),
    }))
}

// ============================================================================
// Bulk Book Metadata PATCH
// ============================================================================

/// Bulk patch book metadata
///
/// Applies the same partial metadata update to multiple books at once.
/// Only provided fields will be updated. Changed fields are auto-locked.
/// Non-existent books are silently skipped.
#[utoipa::path(
    patch,
    path = "/api/v1/books/bulk/metadata",
    request_body = BulkPatchBookMetadataRequest,
    responses(
        (status = 200, description = "Metadata updated", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_patch_book_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkPatchBookMetadataRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    if request.book_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No books specified".to_string(),
        }));
    }

    if request.book_ids.len() > MAX_BULK_BOOKS {
        return Err(ApiError::BadRequest(format!(
            "Too many books (max {})",
            MAX_BULK_BOOKS
        )));
    }

    let now = Utc::now();
    let mut updated_count = 0usize;

    let publisher_opt = request.publisher.into_nested_option();
    let imprint_opt = request.imprint.into_nested_option();
    let genre_opt = request.genre.into_nested_option();
    let language_iso_opt = request.language_iso.into_nested_option();
    let book_type_opt = request.book_type.into_nested_option();
    let translator_opt = request.translator.into_nested_option();
    let edition_opt = request.edition.into_nested_option();
    let original_title_opt = request.original_title.into_nested_option();
    let original_year_opt = request.original_year.into_nested_option();
    let black_and_white_opt = request.black_and_white.into_nested_option();
    let manga_opt = request.manga.into_nested_option();
    let custom_metadata_opt = request.custom_metadata.into_nested_option();
    let authors_opt = request.authors.into_nested_option();

    for book_id in &request.book_ids {
        // Verify book exists
        let book = match BookRepository::get_by_id(&state.db, *book_id).await {
            Ok(Some(b)) => b,
            _ => continue,
        };

        // Get existing metadata
        let existing = match BookMetadataRepository::get_by_book_id(&state.db, *book_id).await {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(_) => continue,
        };

        let existing_custom_metadata = existing.custom_metadata.clone();
        let mut active: book_metadata::ActiveModel = existing.into();
        let mut has_changes = false;

        if let Some(ref opt) = publisher_opt {
            active.publisher = Set(opt.clone());
            if opt.is_some() {
                active.publisher_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = imprint_opt {
            active.imprint = Set(opt.clone());
            if opt.is_some() {
                active.imprint_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = genre_opt {
            active.genre = Set(opt.clone());
            if opt.is_some() {
                active.genre_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = language_iso_opt {
            active.language_iso = Set(opt.clone());
            if opt.is_some() {
                active.language_iso_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = book_type_opt {
            active.book_type = Set(opt.clone());
            if opt.is_some() {
                active.book_type_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = translator_opt {
            active.translator = Set(opt.clone());
            if opt.is_some() {
                active.translator_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = edition_opt {
            active.edition = Set(opt.clone());
            if opt.is_some() {
                active.edition_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = original_title_opt {
            active.original_title = Set(opt.clone());
            if opt.is_some() {
                active.original_title_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = original_year_opt {
            active.original_year = Set(*opt);
            if opt.is_some() {
                active.original_year_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = black_and_white_opt {
            active.black_and_white = Set(*opt);
            if opt.is_some() {
                active.black_and_white_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = manga_opt {
            active.manga = Set(*opt);
            if opt.is_some() {
                active.manga_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = custom_metadata_opt {
            let merged = if let Some(patch) = opt {
                let existing_cm = parse_custom_metadata(existing_custom_metadata.as_deref())
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                let merged = json_merge_patch(&existing_cm, patch);
                validate_custom_metadata_size(Some(&merged)).map_err(ApiError::BadRequest)?;
                Some(merged)
            } else {
                None
            };
            active.custom_metadata = Set(serialize_custom_metadata(merged.as_ref()));
            if opt.is_some() {
                active.custom_metadata_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(ref opt) = authors_opt {
            active.authors_json = Set(opt
                .as_ref()
                .map(|authors| serde_json::to_string(authors).unwrap_or_default()));
            if opt.is_some() {
                active.authors_json_lock = Set(true);
            }
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
            active.update(&state.db).await.map_err(|e| {
                ApiError::Internal(format!("Failed to update book metadata: {}", e))
            })?;
            updated_count += 1;

            let event = EntityChangeEvent {
                event: EntityEvent::BookUpdated {
                    book_id: *book_id,
                    series_id: book.series_id,
                    library_id: book.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Updated metadata for {} books", updated_count),
    }))
}

// ============================================================================
// Bulk Series Tags/Genres Add/Remove
// ============================================================================

/// Bulk add/remove tags for multiple series
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/tags",
    request_body = BulkModifySeriesTagsRequest,
    responses(
        (status = 200, description = "Tags modified", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_modify_series_tags(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkModifySeriesTagsRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No series specified".to_string(),
        }));
    }
    if request.series_ids.len() > MAX_BULK_SERIES {
        return Err(ApiError::BadRequest(format!(
            "Too many series (max {})",
            MAX_BULK_SERIES
        )));
    }
    if request.add.is_empty() && request.remove.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No tags to add or remove".to_string(),
        }));
    }

    // Pre-find/create tags to add and find tags to remove
    let mut tags_to_remove_ids: Vec<Uuid> = Vec::new();
    for name in &request.remove {
        if let Ok(Some(tag)) = TagRepository::get_by_name(&state.db, name).await {
            tags_to_remove_ids.push(tag.id);
        }
    }

    let mut updated_count = 0usize;
    let now = Utc::now();

    for series_id in &request.series_ids {
        // Verify series exists
        let series = match SeriesRepository::get_by_id(&state.db, *series_id).await {
            Ok(Some(s)) => s,
            _ => continue,
        };

        let mut modified = false;

        // Add tags
        for tag_name in &request.add {
            if TagRepository::add_tag_to_series(&state.db, *series_id, tag_name)
                .await
                .is_ok()
            {
                modified = true;
            }
        }

        // Remove tags
        for tag_id in &tags_to_remove_ids {
            if TagRepository::remove_tag_from_series(&state.db, *series_id, *tag_id)
                .await
                .unwrap_or(false)
            {
                modified = true;
            }
        }

        if modified {
            updated_count += 1;
            let event = EntityChangeEvent {
                event: EntityEvent::SeriesUpdated {
                    series_id: *series_id,
                    library_id: series.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Modified tags for {} series", updated_count),
    }))
}

/// Bulk add/remove genres for multiple series
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/genres",
    request_body = BulkModifySeriesGenresRequest,
    responses(
        (status = 200, description = "Genres modified", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_modify_series_genres(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkModifySeriesGenresRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No series specified".to_string(),
        }));
    }
    if request.series_ids.len() > MAX_BULK_SERIES {
        return Err(ApiError::BadRequest(format!(
            "Too many series (max {})",
            MAX_BULK_SERIES
        )));
    }
    if request.add.is_empty() && request.remove.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No genres to add or remove".to_string(),
        }));
    }

    let mut genres_to_remove_ids: Vec<Uuid> = Vec::new();
    for name in &request.remove {
        if let Ok(Some(genre)) = GenreRepository::get_by_name(&state.db, name).await {
            genres_to_remove_ids.push(genre.id);
        }
    }

    let mut updated_count = 0usize;
    let now = Utc::now();

    for series_id in &request.series_ids {
        let series = match SeriesRepository::get_by_id(&state.db, *series_id).await {
            Ok(Some(s)) => s,
            _ => continue,
        };

        let mut modified = false;

        for genre_name in &request.add {
            if GenreRepository::add_genre_to_series(&state.db, *series_id, genre_name)
                .await
                .is_ok()
            {
                modified = true;
            }
        }

        for genre_id in &genres_to_remove_ids {
            if GenreRepository::remove_genre_from_series(&state.db, *series_id, *genre_id)
                .await
                .unwrap_or(false)
            {
                modified = true;
            }
        }

        if modified {
            updated_count += 1;
            let event = EntityChangeEvent {
                event: EntityEvent::SeriesUpdated {
                    series_id: *series_id,
                    library_id: series.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Modified genres for {} series", updated_count),
    }))
}

/// Bulk add/remove tags for multiple books
#[utoipa::path(
    post,
    path = "/api/v1/books/bulk/tags",
    request_body = BulkModifyBookTagsRequest,
    responses(
        (status = 200, description = "Tags modified", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_modify_book_tags(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkModifyBookTagsRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    if request.book_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No books specified".to_string(),
        }));
    }
    if request.book_ids.len() > MAX_BULK_BOOKS {
        return Err(ApiError::BadRequest(format!(
            "Too many books (max {})",
            MAX_BULK_BOOKS
        )));
    }
    if request.add.is_empty() && request.remove.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No tags to add or remove".to_string(),
        }));
    }

    let mut tags_to_remove_ids: Vec<Uuid> = Vec::new();
    for name in &request.remove {
        if let Ok(Some(tag)) = TagRepository::get_by_name(&state.db, name).await {
            tags_to_remove_ids.push(tag.id);
        }
    }

    let mut updated_count = 0usize;
    let now = Utc::now();

    for book_id in &request.book_ids {
        let book = match BookRepository::get_by_id(&state.db, *book_id).await {
            Ok(Some(b)) => b,
            _ => continue,
        };

        let mut modified = false;

        for tag_name in &request.add {
            if TagRepository::add_tag_to_book(&state.db, *book_id, tag_name)
                .await
                .is_ok()
            {
                modified = true;
            }
        }

        for tag_id in &tags_to_remove_ids {
            if TagRepository::remove_tag_from_book(&state.db, *book_id, *tag_id)
                .await
                .unwrap_or(false)
            {
                modified = true;
            }
        }

        if modified {
            updated_count += 1;
            let event = EntityChangeEvent {
                event: EntityEvent::BookUpdated {
                    book_id: *book_id,
                    series_id: book.series_id,
                    library_id: book.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Modified tags for {} books", updated_count),
    }))
}

/// Bulk add/remove genres for multiple books
#[utoipa::path(
    post,
    path = "/api/v1/books/bulk/genres",
    request_body = BulkModifyBookGenresRequest,
    responses(
        (status = 200, description = "Genres modified", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_modify_book_genres(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkModifyBookGenresRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    if request.book_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No books specified".to_string(),
        }));
    }
    if request.book_ids.len() > MAX_BULK_BOOKS {
        return Err(ApiError::BadRequest(format!(
            "Too many books (max {})",
            MAX_BULK_BOOKS
        )));
    }
    if request.add.is_empty() && request.remove.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No genres to add or remove".to_string(),
        }));
    }

    let mut genres_to_remove_ids: Vec<Uuid> = Vec::new();
    for name in &request.remove {
        if let Ok(Some(genre)) = GenreRepository::get_by_name(&state.db, name).await {
            genres_to_remove_ids.push(genre.id);
        }
    }

    let mut updated_count = 0usize;
    let now = Utc::now();

    for book_id in &request.book_ids {
        let book = match BookRepository::get_by_id(&state.db, *book_id).await {
            Ok(Some(b)) => b,
            _ => continue,
        };

        let mut modified = false;

        for genre_name in &request.add {
            if GenreRepository::add_genre_to_book(&state.db, *book_id, genre_name)
                .await
                .is_ok()
            {
                modified = true;
            }
        }

        for genre_id in &genres_to_remove_ids {
            if GenreRepository::remove_genre_from_book(&state.db, *book_id, *genre_id)
                .await
                .unwrap_or(false)
            {
                modified = true;
            }
        }

        if modified {
            updated_count += 1;
            let event = EntityChangeEvent {
                event: EntityEvent::BookUpdated {
                    book_id: *book_id,
                    series_id: book.series_id,
                    library_id: book.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Modified genres for {} books", updated_count),
    }))
}

// ============================================================================
// Bulk Lock Updates
// ============================================================================

/// Bulk update metadata locks for multiple series
#[utoipa::path(
    put,
    path = "/api/v1/series/bulk/metadata/locks",
    request_body = BulkUpdateSeriesLocksRequest,
    responses(
        (status = 200, description = "Locks updated", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_update_series_locks(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkUpdateSeriesLocksRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No series specified".to_string(),
        }));
    }
    if request.series_ids.len() > MAX_BULK_SERIES {
        return Err(ApiError::BadRequest(format!(
            "Too many series (max {})",
            MAX_BULK_SERIES
        )));
    }

    let locks = &request.locks;
    let now = Utc::now();
    let mut updated_count = 0usize;

    for series_id in &request.series_ids {
        let series = match SeriesRepository::get_by_id(&state.db, *series_id).await {
            Ok(Some(s)) => s,
            _ => continue,
        };

        let existing = match SeriesMetadataRepository::get_by_series_id(&state.db, *series_id).await
        {
            Ok(Some(m)) => m,
            _ => continue,
        };

        let mut active: series_metadata::ActiveModel = existing.into();
        let mut has_changes = false;

        if let Some(v) = locks.title {
            active.title_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.title_sort {
            active.title_sort_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.summary {
            active.summary_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.publisher {
            active.publisher_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.imprint {
            active.imprint_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.status {
            active.status_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.age_rating {
            active.age_rating_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.language {
            active.language_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.reading_direction {
            active.reading_direction_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.year {
            active.year_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.total_book_count {
            active.total_book_count_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.genres {
            active.genres_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.tags {
            active.tags_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.custom_metadata {
            active.custom_metadata_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.cover {
            active.cover_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.authors_json_lock {
            active.authors_json_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.alternate_titles {
            active.alternate_titles_lock = Set(v);
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
            active
                .update(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to update locks: {}", e)))?;
            updated_count += 1;

            let event = EntityChangeEvent {
                event: EntityEvent::SeriesUpdated {
                    series_id: *series_id,
                    library_id: series.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Updated locks for {} series", updated_count),
    }))
}

/// Bulk update metadata locks for multiple books
#[utoipa::path(
    put,
    path = "/api/v1/books/bulk/metadata/locks",
    request_body = BulkUpdateBookLocksRequest,
    responses(
        (status = 200, description = "Locks updated", body = BulkMetadataUpdateResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_update_book_locks(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkUpdateBookLocksRequest>,
) -> Result<Json<BulkMetadataUpdateResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    if request.book_ids.is_empty() {
        return Ok(Json(BulkMetadataUpdateResponse {
            updated_count: 0,
            message: "No books specified".to_string(),
        }));
    }
    if request.book_ids.len() > MAX_BULK_BOOKS {
        return Err(ApiError::BadRequest(format!(
            "Too many books (max {})",
            MAX_BULK_BOOKS
        )));
    }

    let locks = &request.locks;
    let now = Utc::now();
    let mut updated_count = 0usize;

    for book_id in &request.book_ids {
        let book = match BookRepository::get_by_id(&state.db, *book_id).await {
            Ok(Some(b)) => b,
            _ => continue,
        };

        let existing = match BookMetadataRepository::get_by_book_id(&state.db, *book_id).await {
            Ok(Some(m)) => m,
            _ => continue,
        };

        let mut active: book_metadata::ActiveModel = existing.into();
        let mut has_changes = false;

        if let Some(v) = locks.title_lock {
            active.title_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.title_sort_lock {
            active.title_sort_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.number_lock {
            active.number_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.summary_lock {
            active.summary_lock = Set(v);
            has_changes = true;
        }
        // Map individual author lock fields to the consolidated authors_json_lock
        if let Some(v) = locks
            .writer_lock
            .or(locks.penciller_lock)
            .or(locks.inker_lock)
            .or(locks.colorist_lock)
            .or(locks.letterer_lock)
            .or(locks.cover_artist_lock)
            .or(locks.editor_lock)
        {
            active.authors_json_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.publisher_lock {
            active.publisher_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.imprint_lock {
            active.imprint_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.genre_lock {
            active.genre_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.language_iso_lock {
            active.language_iso_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.format_detail_lock {
            active.format_detail_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.black_and_white_lock {
            active.black_and_white_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.manga_lock {
            active.manga_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.year_lock {
            active.year_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.month_lock {
            active.month_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.day_lock {
            active.day_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.volume_lock {
            active.volume_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.count_lock {
            active.count_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.isbns_lock {
            active.isbns_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.book_type_lock {
            active.book_type_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.subtitle_lock {
            active.subtitle_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.authors_json_lock {
            active.authors_json_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.translator_lock {
            active.translator_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.edition_lock {
            active.edition_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.original_title_lock {
            active.original_title_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.original_year_lock {
            active.original_year_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.series_position_lock {
            active.series_position_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.series_total_lock {
            active.series_total_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.subjects_lock {
            active.subjects_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.awards_json_lock {
            active.awards_json_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.custom_metadata_lock {
            active.custom_metadata_lock = Set(v);
            has_changes = true;
        }
        if let Some(v) = locks.cover_lock {
            active.cover_lock = Set(v);
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
            active
                .update(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to update locks: {}", e)))?;
            updated_count += 1;

            let event = EntityChangeEvent {
                event: EntityEvent::BookUpdated {
                    book_id: *book_id,
                    series_id: book.series_id,
                    library_id: book.library_id,
                    fields: None,
                },
                timestamp: now,
                user_id: Some(auth.user_id),
            };
            let _ = state.event_broadcaster.emit(event);
        }
    }

    Ok(Json(BulkMetadataUpdateResponse {
        updated_count,
        message: format!("Updated locks for {} books", updated_count),
    }))
}
