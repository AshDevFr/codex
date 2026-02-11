//! Series routes
//!
//! Handles series management including CRUD operations, metadata, genres, tags,
//! covers, ratings, and more.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;

/// Create series routes
///
/// All routes are protected (authentication required).
///
/// Routes include:
/// - Series CRUD: /series, /series/:id
/// - Series collections: /series/in-progress, /series/recently-added, etc.
/// - Metadata: /series/:id/metadata, /series/:id/genres, /series/:id/tags
/// - Covers: /series/:id/thumbnail, /series/:id/covers
/// - Ratings: /series/:id/rating, /series/:id/external-ratings
/// - Alternate titles and external links
/// - Mark as read/unread
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Series CRUD routes
        .route("/series", get(handlers::list_series))
        .route("/series/search", post(handlers::search_series))
        .route("/series/list", post(handlers::list_series_filtered))
        .route(
            "/series/list/alphabetical-groups",
            post(handlers::list_series_alphabetical_groups),
        )
        .route("/series/:series_id", get(handlers::get_series))
        .route("/series/:series_id", patch(handlers::patch_series))
        .route("/series/:series_id/books", get(handlers::get_series_books))
        // Series collection routes
        .route(
            "/series/in-progress",
            get(handlers::list_in_progress_series),
        )
        .route(
            "/series/recently-added",
            get(handlers::list_recently_added_series),
        )
        .route(
            "/series/recently-updated",
            get(handlers::list_recently_updated_series),
        )
        .route(
            "/series/:series_id/purge-deleted",
            delete(handlers::purge_series_deleted_books),
        )
        // Series cover routes
        .route(
            "/series/:series_id/thumbnail",
            get(handlers::get_series_thumbnail),
        )
        // Note: POST /series/:series_id/thumbnail/generate is in tasks.rs
        .route(
            "/series/:series_id/cover",
            post(handlers::upload_series_cover),
        )
        .route(
            "/series/:series_id/cover/source",
            patch(handlers::set_series_cover_source),
        )
        // Series covers routes (multi-cover management)
        .route(
            "/series/:series_id/covers",
            get(handlers::list_series_covers),
        )
        .route(
            "/series/:series_id/covers/selected",
            delete(handlers::reset_series_cover),
        )
        .route(
            "/series/:series_id/covers/:cover_id/select",
            put(handlers::select_series_cover),
        )
        .route(
            "/series/:series_id/covers/:cover_id/image",
            get(handlers::get_series_cover_image),
        )
        .route(
            "/series/:series_id/covers/:cover_id",
            delete(handlers::delete_series_cover),
        )
        // Series analysis routes
        .route(
            "/series/:series_id/analyze",
            post(handlers::trigger_series_analysis),
        )
        .route(
            "/series/:series_id/analyze-unanalyzed",
            post(handlers::trigger_series_unanalyzed_analysis),
        )
        // Series download route
        .route(
            "/series/:series_id/download",
            get(handlers::download_series),
        )
        // Series metadata routes
        .route(
            "/series/:series_id/metadata",
            get(handlers::get_series_metadata),
        )
        .route(
            "/series/:series_id/metadata",
            put(handlers::replace_series_metadata),
        )
        .route(
            "/series/:series_id/metadata",
            patch(handlers::patch_series_metadata),
        )
        .route(
            "/series/:series_id/metadata/locks",
            get(handlers::get_metadata_locks),
        )
        .route(
            "/series/:series_id/metadata/locks",
            put(handlers::update_metadata_locks),
        )
        // Series genres routes
        .route(
            "/series/:series_id/genres",
            get(handlers::get_series_genres),
        )
        .route(
            "/series/:series_id/genres",
            put(handlers::set_series_genres),
        )
        .route(
            "/series/:series_id/genres",
            post(handlers::add_series_genre),
        )
        .route(
            "/series/:series_id/genres/:genre_id",
            delete(handlers::remove_series_genre),
        )
        // Series tags routes
        .route("/series/:series_id/tags", get(handlers::get_series_tags))
        .route("/series/:series_id/tags", put(handlers::set_series_tags))
        .route("/series/:series_id/tags", post(handlers::add_series_tag))
        .route(
            "/series/:series_id/tags/:tag_id",
            delete(handlers::remove_series_tag),
        )
        // Series sharing tags routes (admin only)
        .route(
            "/series/:series_id/sharing-tags",
            get(handlers::sharing_tags::get_series_sharing_tags),
        )
        .route(
            "/series/:series_id/sharing-tags",
            put(handlers::sharing_tags::set_series_sharing_tags),
        )
        .route(
            "/series/:series_id/sharing-tags",
            post(handlers::sharing_tags::add_series_sharing_tag),
        )
        .route(
            "/series/:series_id/sharing-tags/:tag_id",
            delete(handlers::sharing_tags::remove_series_sharing_tag),
        )
        // Series user rating routes
        .route(
            "/series/:series_id/rating",
            get(handlers::get_series_rating),
        )
        .route(
            "/series/:series_id/rating",
            put(handlers::set_series_rating),
        )
        .route(
            "/series/:series_id/rating",
            delete(handlers::delete_series_rating),
        )
        // Series alternate titles routes
        .route(
            "/series/:series_id/alternate-titles",
            get(handlers::get_series_alternate_titles),
        )
        .route(
            "/series/:series_id/alternate-titles",
            post(handlers::create_alternate_title),
        )
        .route(
            "/series/:series_id/alternate-titles/:title_id",
            patch(handlers::update_alternate_title),
        )
        .route(
            "/series/:series_id/alternate-titles/:title_id",
            delete(handlers::delete_alternate_title),
        )
        // Series external ratings routes
        .route(
            "/series/:series_id/external-ratings",
            get(handlers::get_series_external_ratings),
        )
        .route(
            "/series/:series_id/external-ratings",
            post(handlers::create_external_rating),
        )
        .route(
            "/series/:series_id/external-ratings/:source",
            delete(handlers::delete_external_rating),
        )
        // Series average rating route
        .route(
            "/series/:series_id/ratings/average",
            get(handlers::get_series_average_rating),
        )
        // Series external links routes
        .route(
            "/series/:series_id/external-links",
            get(handlers::get_series_external_links),
        )
        .route(
            "/series/:series_id/external-links",
            post(handlers::create_external_link),
        )
        .route(
            "/series/:series_id/external-links/:source",
            delete(handlers::delete_external_link),
        )
        // Series external IDs routes
        .route(
            "/series/:series_id/external-ids",
            get(handlers::list_series_external_ids),
        )
        .route(
            "/series/:series_id/external-ids",
            post(handlers::create_series_external_id),
        )
        .route(
            "/series/:series_id/external-ids/:external_id_id",
            delete(handlers::delete_series_external_id),
        )
        // Mark series as read/unread routes
        .route(
            "/series/:series_id/read",
            post(handlers::mark_series_as_read),
        )
        .route(
            "/series/:series_id/unread",
            post(handlers::mark_series_as_unread),
        )
        // Bulk operations
        .route(
            "/series/bulk/read",
            post(handlers::bulk_mark_series_as_read),
        )
        .route(
            "/series/bulk/unread",
            post(handlers::bulk_mark_series_as_unread),
        )
        .route("/series/bulk/analyze", post(handlers::bulk_analyze_series))
        .route(
            "/series/bulk/thumbnails/generate",
            post(handlers::bulk_generate_series_thumbnails),
        )
        .route(
            "/series/bulk/thumbnails/books/generate",
            post(handlers::bulk_generate_series_book_thumbnails),
        )
        .route(
            "/series/bulk/titles/reprocess",
            post(handlers::bulk_reprocess_series_titles),
        )
        // Series metadata from plugins (Phase 4)
        .route(
            "/series/:series_id/metadata/search-title",
            get(handlers::plugin_actions::get_series_search_title),
        )
        .route(
            "/series/:series_id/metadata/preview",
            post(handlers::plugin_actions::preview_series_metadata),
        )
        .route(
            "/series/:series_id/metadata/apply",
            post(handlers::plugin_actions::apply_series_metadata),
        )
        .route(
            "/series/:series_id/metadata/auto-match",
            post(handlers::plugin_actions::auto_match_series_metadata),
        )
        // Task-based auto-match endpoints (Phase 5.5 - Worker plugin integration)
        .route(
            "/series/:series_id/metadata/auto-match/task",
            post(handlers::plugin_actions::enqueue_auto_match_task),
        )
        .route(
            "/series/metadata/auto-match/task/bulk",
            post(handlers::plugin_actions::enqueue_bulk_auto_match_tasks),
        )
        // Reprocess series titles (batch)
        .route(
            "/series/titles/reprocess",
            post(handlers::task_queue::reprocess_series_titles),
        )
        // Reprocess single series title
        .route(
            "/series/:series_id/title/reprocess",
            post(handlers::task_queue::reprocess_series_title),
        )
}
