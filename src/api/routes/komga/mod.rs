//! Komga-compatible API module
//!
//! This module provides a Komga-compatible API layer that allows Codex to work with
//! applications designed for Komga, such as the Komic iOS app.
//!
//! ## Features
//!
//! - Full compatibility with Komga API structure and response formats
//! - Support for library browsing, series navigation, and book reading
//! - Reading progress sync
//! - Basic authentication support
//!
//! ## Configuration
//!
//! The Komga API is disabled by default. Enable it via configuration:
//!
//! ```yaml
//! komga_api:
//!   enabled: true
//!   prefix: "komgav1"  # Optional, default
//! ```
//!
//! Or via environment variables:
//!
//! ```bash
//! CODEX_KOMGA_API_ENABLED=true
//! CODEX_KOMGA_API_PREFIX=komgav1
//! ```
//!
//! ## Endpoints
//!
//! When enabled, the following endpoint groups are available at `/{prefix}/api/v1/`:
//!
//! - `/libraries` - Library listing and details
//! - `/series` - Series listing, search, and details
//! - `/books` - Book listing, details, and file downloads
//! - `/books/{id}/pages` - Page listing and streaming
//! - `/books/{id}/read-progress` - Reading progress management
//! - `/users/me` - Current user information

pub mod dto;
pub mod handlers;
pub mod routes;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the Komga-compatible API router
///
/// This router is meant to be nested under `/{prefix}/api/v1/` where the prefix
/// is configurable (default: `komgav1`).
///
/// # Example
///
/// ```ignore
/// // In main router setup:
/// if config.komga_api.enabled {
///     let prefix = format!("/{}/api/v1", config.komga_api.prefix);
///     router = router.nest(&prefix, komga::router(state.clone()));
/// }
/// ```
pub fn router(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}
