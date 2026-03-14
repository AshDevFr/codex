//! KOReader sync API module
//!
//! This module provides the KOReader sync API, allowing KOReader e-readers
//! to sync reading progress with Codex.
//!
//! ## Configuration
//!
//! The KOReader API is disabled by default. Enable it via configuration:
//!
//! ```yaml
//! koreader_api:
//!   enabled: true
//! ```
//!
//! Or via environment variable:
//!
//! ```bash
//! CODEX_KOREADER_API_ENABLED=true
//! ```
//!
//! ## Endpoints
//!
//! When enabled, the following endpoints are available at `/koreader/`:
//!
//! - `POST /users/create` - Always returns 403 (registration handled by Codex)
//! - `GET /users/auth` - Verify authentication
//! - `GET /syncs/progress/{document}` - Get reading progress by KOReader hash
//! - `PUT /syncs/progress` - Update reading progress

pub mod dto;
pub mod handlers;
pub mod routes;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the KOReader sync API router
///
/// This router is mounted at `/koreader` when the KOReader API is enabled.
pub fn router(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}
