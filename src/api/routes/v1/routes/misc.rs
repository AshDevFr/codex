//! Miscellaneous routes
//!
//! Handles various utility routes including genres, tags, metrics, duplicates,
//! filesystem browsing, and real-time events.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;

/// Create miscellaneous routes
///
/// Routes are protected (authentication required) unless otherwise noted.
///
/// Routes:
/// - Genres: /genres (cleanup/delete require admin)
/// - Tags: /tags (cleanup/delete require admin)
/// - Metrics: /metrics/inventory, /metrics/tasks
/// - Duplicates: /duplicates
/// - Filesystem: /filesystem/browse, /filesystem/drives (admin only)
/// - Events: /events/stream
/// - Scans: /scans/active, /scans/stream
/// - Settings: /settings/branding (public), /settings/public
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Global scan routes
        .route("/scans/active", get(handlers::list_active_scans))
        .route("/scans/stream", get(handlers::scan_progress_stream))
        // Real-time event routes
        .route("/events/stream", get(handlers::entity_events_stream))
        // Global genre routes (cleanup/delete require admin)
        .route("/genres", get(handlers::list_genres))
        .route("/genres/cleanup", post(handlers::cleanup_genres))
        .route("/genres/{genre_id}", delete(handlers::delete_genre))
        // Global tag routes (cleanup/delete require admin)
        .route("/tags", get(handlers::list_tags))
        .route("/tags/cleanup", post(handlers::cleanup_tags))
        .route("/tags/{tag_id}", delete(handlers::delete_tag))
        // Metrics routes
        .route("/metrics/inventory", get(handlers::get_inventory_metrics))
        .route("/metrics/plugins", get(handlers::get_plugin_metrics))
        .route(
            "/metrics/tasks",
            get(handlers::task_metrics::get_task_metrics),
        )
        .route(
            "/metrics/tasks",
            delete(handlers::task_metrics::nuke_task_metrics),
        )
        .route(
            "/metrics/tasks/history",
            get(handlers::task_metrics::get_task_metrics_history),
        )
        .route(
            "/metrics/tasks/cleanup",
            post(handlers::task_metrics::trigger_metrics_cleanup),
        )
        // Duplicate detection routes
        .route("/duplicates", get(handlers::list_duplicates))
        .route("/duplicates/scan", post(handlers::trigger_duplicate_scan))
        .route(
            "/duplicates/{duplicate_id}",
            delete(handlers::delete_duplicate_group),
        )
        // Filesystem routes (admin only)
        .route("/filesystem/browse", get(handlers::browse_filesystem))
        .route("/filesystem/drives", get(handlers::list_drives))
        // App info route (public, no authentication required)
        .route("/info", get(handlers::info::get_app_info))
        // Branding settings route (public, no authentication required)
        .route(
            "/settings/branding",
            get(handlers::settings::get_branding_settings),
        )
        // Public settings routes (protected, all authenticated users)
        .route(
            "/settings/public",
            get(handlers::settings::get_public_settings),
        )
        // Static field-group catalog for the metadata-refresh job editor.
        .route(
            "/library-jobs/metadata-refresh/field-groups",
            get(handlers::library_jobs::list_field_groups),
        )
}
