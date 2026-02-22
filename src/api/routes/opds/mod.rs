//! OPDS 1.2 Module
//!
//! This module contains everything for the OPDS 1.2 (Atom-based) catalog protocol.
//! OPDS (Open Publication Distribution System) allows e-reader applications
//! to browse and download content from digital libraries.
//!
//! # Structure
//!
//! - `dto` - Data Transfer Objects for OPDS feeds and entries
//! - `handlers` - Request handlers for catalog, search, and PSE endpoints
//! - `routes` - Route definitions mapping URLs to handlers
//!
//! # Endpoints
//!
//! - `GET /` - Root catalog
//! - `GET /libraries` - List all libraries
//! - `GET /libraries/{id}` - Library series catalog
//! - `GET /series/{id}` - Series books catalog
//! - `GET /books/{id}/pages` - Book pages (PSE)
//! - `GET /search.xml` - OpenSearch descriptor
//! - `GET /search` - Search endpoint

pub mod dto;
pub mod handlers;
mod routes;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the OPDS 1.2 router
pub fn router(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}
