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
//!   prefix: "komga"  # Optional, default
//! ```
//!
//! Or via environment variables:
//!
//! ```bash
//! CODEX_KOMGA_API_ENABLED=true
//! CODEX_KOMGA_API_PREFIX=komga
//! ```
//!
//! ## Endpoints
//!
//! When enabled, the following endpoint groups are available:
//!
//! ### API v1 (`/{prefix}/api/v1/`)
//! - `/libraries` - Library listing and details
//! - `/series` - Series listing, search, and details
//! - `/books` - Book listing, details, and file downloads
//! - `/books/{id}/pages` - Page listing and streaming
//! - `/books/{id}/read-progress` - Reading progress management
//! - `/users/me` - Current user information
//! - `/collections` - Collections (stub - always empty)
//! - `/readlists` - Read lists (stub - always empty)
//! - `/genres` - Genres (stub - always empty)
//! - `/tags` - Tags (stub - always empty)
//!
//! ### API v2 (`/{prefix}/api/v2/`)
//! - All routes from v1 (except v1-specific stubs)
//! - `/authors` - Authors (stub - always empty)

pub mod dto;
pub mod handlers;
pub mod routes;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the combined Komga-compatible API router with both v1 and v2 endpoints
///
/// This router is meant to be nested under `/{prefix}/` where the prefix
/// is configurable (default: `komga`).
///
/// The router mounts:
/// - `/api/v1/*` - Komga API v1 endpoints
/// - `/api/v2/*` - Komga API v2 endpoints
///
/// # Example
///
/// ```text
/// // In main router setup:
/// if config.komga_api.enabled {
///     let prefix = format!("/{}", config.komga_api.prefix);
///     router = router.nest(&prefix, komga::router(state.clone()));
/// }
/// ```
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api/v1", routes::create_v1_router(state.clone()))
        .nest("/api/v2", routes::create_v2_router(state))
}
