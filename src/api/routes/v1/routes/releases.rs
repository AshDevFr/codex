//! Release-tracking routes (cross-series inbox + source admin).
//!
//! Per-series ledger (`/series/{id}/releases`) lives in `series.rs` to keep
//! all series-scoped routes together; this module wires the cross-series
//! inbox and the admin source-management endpoints.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{get, patch, post},
};
use std::sync::Arc;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Inbox + state transitions
        .route("/releases", get(handlers::releases::list_release_inbox))
        .route(
            "/releases/{release_id}",
            patch(handlers::releases::update_release_entry),
        )
        .route(
            "/releases/{release_id}/dismiss",
            post(handlers::releases::dismiss_release),
        )
        .route(
            "/releases/{release_id}/mark-acquired",
            post(handlers::releases::mark_release_acquired),
        )
        // Applicability (SeriesRead required) — used by the frontend to
        // hide release-tracking UI on libraries not covered by any plugin.
        .route(
            "/release-sources/applicability",
            get(handlers::releases::get_release_tracking_applicability),
        )
        // Source admin (PluginsManage required)
        .route(
            "/release-sources",
            get(handlers::releases::list_release_sources),
        )
        .route(
            "/release-sources/{source_id}",
            patch(handlers::releases::update_release_source),
        )
        .route(
            "/release-sources/{source_id}/poll-now",
            post(handlers::releases::poll_release_source_now),
        )
}
