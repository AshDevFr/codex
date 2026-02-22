//! OPDS 2.0 Module
//!
//! This module contains everything for the OPDS 2.0 (JSON-based) catalog protocol.
//! OPDS 2.0 is the successor to OPDS 1.2, using JSON instead of Atom XML.
//!
//! # Structure
//!
//! - `dto` - Data Transfer Objects for OPDS 2.0 feeds and publications
//! - `handlers` - Request handlers for catalog and search endpoints
//! - `routes` - Route definitions mapping URLs to handlers
//!
//! # Endpoints
//!
//! - `GET /` - Root catalog (navigation feed)
//! - `GET /libraries` - List all libraries
//! - `GET /libraries/{id}` - Library series catalog
//! - `GET /series/{id}` - Series books catalog (publications feed)
//! - `GET /recent` - Recently added books
//! - `GET /search` - Search endpoint

pub mod dto;
pub mod handlers;
mod routes;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the OPDS 2.0 router
pub fn router(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}
