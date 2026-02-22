//! Admin routes
//!
//! Handles administrative operations including settings, sharing tags, and cleanup tasks.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;

/// Create admin routes
///
/// All routes are protected and require admin permissions.
///
/// Routes:
/// - Settings: /admin/settings
/// - Plugins: /admin/plugins
/// - Sharing tags: /admin/sharing-tags
/// - Cleanup: /admin/cleanup-orphans
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Settings routes (admin only)
        .route("/admin/settings", get(handlers::settings::list_settings))
        .route(
            "/admin/settings/{setting_key}",
            get(handlers::settings::get_setting),
        )
        .route(
            "/admin/settings/{setting_key}",
            put(handlers::settings::update_setting),
        )
        .route(
            "/admin/settings/bulk",
            post(handlers::settings::bulk_update_settings),
        )
        .route(
            "/admin/settings/{setting_key}/reset",
            post(handlers::settings::reset_setting),
        )
        .route(
            "/admin/settings/{setting_key}/history",
            get(handlers::settings::get_setting_history),
        )
        // Sharing tags routes (admin only)
        .route(
            "/admin/sharing-tags",
            get(handlers::sharing_tags::list_sharing_tags),
        )
        .route(
            "/admin/sharing-tags",
            post(handlers::sharing_tags::create_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/{tag_id}",
            get(handlers::sharing_tags::get_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/{tag_id}",
            patch(handlers::sharing_tags::update_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/{tag_id}",
            delete(handlers::sharing_tags::delete_sharing_tag),
        )
        // Cleanup routes (admin only)
        .route(
            "/admin/cleanup-orphans/stats",
            get(handlers::cleanup::get_orphan_stats),
        )
        .route(
            "/admin/cleanup-orphans",
            post(handlers::cleanup::trigger_cleanup),
        )
        .route(
            "/admin/cleanup-orphans",
            delete(handlers::cleanup::delete_orphans),
        )
        // PDF cache management routes (admin only)
        .route(
            "/admin/pdf-cache/stats",
            get(handlers::pdf_cache::get_pdf_cache_stats),
        )
        .route(
            "/admin/pdf-cache/cleanup",
            post(handlers::pdf_cache::trigger_pdf_cache_cleanup),
        )
        .route(
            "/admin/pdf-cache",
            delete(handlers::pdf_cache::clear_pdf_cache),
        )
        // Plugin file storage routes (admin only)
        .route(
            "/admin/plugin-storage",
            get(handlers::plugin_storage::get_all_plugin_storage_stats),
        )
        .route(
            "/admin/plugin-storage/{name}",
            get(handlers::plugin_storage::get_plugin_storage_stats),
        )
        .route(
            "/admin/plugin-storage/{name}",
            delete(handlers::plugin_storage::cleanup_plugin_storage),
        )
        // Plugin management routes (admin only)
        .route("/admin/plugins", get(handlers::plugins::list_plugins))
        .route("/admin/plugins", post(handlers::plugins::create_plugin))
        .route("/admin/plugins/{id}", get(handlers::plugins::get_plugin))
        .route(
            "/admin/plugins/{id}",
            patch(handlers::plugins::update_plugin),
        )
        .route(
            "/admin/plugins/{id}",
            delete(handlers::plugins::delete_plugin),
        )
        .route(
            "/admin/plugins/{id}/enable",
            post(handlers::plugins::enable_plugin),
        )
        .route(
            "/admin/plugins/{id}/disable",
            post(handlers::plugins::disable_plugin),
        )
        .route(
            "/admin/plugins/{id}/test",
            post(handlers::plugins::test_plugin),
        )
        .route(
            "/admin/plugins/{id}/health",
            get(handlers::plugins::get_plugin_health),
        )
        .route(
            "/admin/plugins/{id}/reset",
            post(handlers::plugins::reset_plugin_failures),
        )
        .route(
            "/admin/plugins/{id}/failures",
            get(handlers::plugins::get_plugin_failures),
        )
}
