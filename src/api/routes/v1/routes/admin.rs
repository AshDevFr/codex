//! Admin routes
//!
//! Handles administrative operations including settings, system integrations,
//! sharing tags, and cleanup tasks.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;

/// Create admin routes
///
/// All routes are protected and require admin permissions.
///
/// Routes:
/// - Settings: /admin/settings
/// - Integrations: /admin/integrations
/// - Sharing tags: /admin/sharing-tags
/// - Cleanup: /admin/cleanup-orphans
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Settings routes (admin only)
        .route("/admin/settings", get(handlers::settings::list_settings))
        .route(
            "/admin/settings/:setting_key",
            get(handlers::settings::get_setting),
        )
        .route(
            "/admin/settings/:setting_key",
            put(handlers::settings::update_setting),
        )
        .route(
            "/admin/settings/bulk",
            post(handlers::settings::bulk_update_settings),
        )
        .route(
            "/admin/settings/:setting_key/reset",
            post(handlers::settings::reset_setting),
        )
        .route(
            "/admin/settings/:setting_key/history",
            get(handlers::settings::get_setting_history),
        )
        // System integrations routes (admin only)
        .route(
            "/admin/integrations",
            get(handlers::system_integrations::list_system_integrations),
        )
        .route(
            "/admin/integrations",
            post(handlers::system_integrations::create_system_integration),
        )
        .route(
            "/admin/integrations/:id",
            get(handlers::system_integrations::get_system_integration),
        )
        .route(
            "/admin/integrations/:id",
            patch(handlers::system_integrations::update_system_integration),
        )
        .route(
            "/admin/integrations/:id",
            delete(handlers::system_integrations::delete_system_integration),
        )
        .route(
            "/admin/integrations/:id/enable",
            post(handlers::system_integrations::enable_system_integration),
        )
        .route(
            "/admin/integrations/:id/disable",
            post(handlers::system_integrations::disable_system_integration),
        )
        .route(
            "/admin/integrations/:id/test",
            post(handlers::system_integrations::test_system_integration),
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
            "/admin/sharing-tags/:tag_id",
            get(handlers::sharing_tags::get_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/:tag_id",
            patch(handlers::sharing_tags::update_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/:tag_id",
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
}
