//! API v1 Module
//!
//! This module contains everything for API v1: routes, handlers, and DTOs.
//! This structure enables true API versioning where each version can evolve independently.
//!
//! # Structure
//!
//! - `dto` - Data Transfer Objects for request/response serialization
//! - `handlers` - Request handlers containing business logic
//! - `routes` - Route definitions mapping URLs to handlers
//!
//! # Route Organization
//!
//! - `auth` - Authentication routes (login, register, logout, verify email)
//! - `setup` - Initial setup routes (when no users exist)
//! - `libraries` - Library management and scanning
//! - `series` - Series CRUD, metadata, genres, tags, covers, ratings
//! - `books` - Book CRUD, metadata, pages, progress, files
//! - `users` - User administration (admin only)
//! - `user` - Current user preferences, integrations, ratings
//! - `admin` - Admin settings, integrations, sharing tags, cleanup
//! - `tasks` - Task queue and thumbnail generation
//! - `misc` - Genres, tags, metrics, duplicates, filesystem, events

pub mod dto;
pub mod handlers;
mod routes;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the combined API v1 router
///
/// This function composes all domain-specific routers into a single router.
/// Routes are organized as follows:
///
/// - `/auth/*` - Authentication (nested)
/// - `/setup/*` - Initial setup (nested)
/// - `/libraries/*` - Library management (merged)
/// - `/series/*` - Series management (merged)
/// - `/books/*` - Book management (merged)
/// - `/users/*` - User administration (merged)
/// - `/user/*` - Current user (merged)
/// - `/admin/*` - Admin settings (merged)
/// - `/tasks/*` - Task queue (merged)
/// - Various utility routes (merged from misc)
pub fn routes(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}
