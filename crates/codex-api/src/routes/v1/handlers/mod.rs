//! API v1 Handlers
//!
//! This module contains all request handlers for API v1.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use codex_models::reading_direction::ReadingDirection;
use serde::Serialize;

use super::dto::common::PaginationLinkBuilder;
use crate::error::ApiError;

/// Validate a client-supplied reading direction.
///
/// The database columns are `String` because rows predating validation may hold
/// anything, so the check lives here at the API boundary. Returns the canonical
/// lowercase form, which means a client sending `"RTL"` stores the same value as
/// one sending `"rtl"` and downstream resolution never has to case-fold.
pub fn validate_reading_direction(value: &str) -> Result<String, ApiError> {
    value
        .parse::<ReadingDirection>()
        .map(|direction| direction.as_str().to_string())
        .map_err(ApiError::BadRequest)
}

/// [`validate_reading_direction`] for optional fields, where `None` means
/// "no direction" rather than an invalid one.
pub fn validate_optional_reading_direction(
    value: Option<&str>,
) -> Result<Option<String>, ApiError> {
    value.map(validate_reading_direction).transpose()
}

/// Create a paginated response with Link headers (RFC 8288)
///
/// This helper wraps a serializable response with the appropriate Link header
/// for HATEOAS compliance.
///
/// # Example
/// ```ignore
/// let builder = PaginationLinkBuilder::new("/api/v1/books", page, page_size, total_pages);
/// let response = PaginatedResponse::with_builder(data, page, page_size, total, &builder);
/// Ok(paginated_response(response, &builder))
/// ```
pub fn paginated_response<T: Serialize>(data: T, link_builder: &PaginationLinkBuilder) -> Response {
    let link_header = link_builder.build_link_header();

    match serde_json::to_string(&data) {
        Ok(_) => {
            let mut response = (StatusCode::OK, Json(data)).into_response();

            if let Ok(header_value) = HeaderValue::from_str(&link_header) {
                response.headers_mut().insert(header::LINK, header_value);
            }

            response
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub mod access_groups;
pub mod api_keys;
pub mod auth;
pub mod books;
pub mod bulk;
pub mod bulk_metadata;
pub mod cleanup;
pub mod collections;
pub mod duplicates;
pub mod events;
pub mod filesystem;
pub mod filter_presets;
pub mod health;
pub mod info;
pub mod libraries;
pub mod library_jobs;
pub mod metrics;
pub mod observability;
pub mod oidc;
pub mod pages;
pub mod pdf_cache;
pub mod plugin_actions;
pub mod plugin_storage;
pub mod plugin_web_links;
pub mod plugins;
pub mod read_progress;
pub mod reading_sessions;
pub mod reading_stats;
pub mod readlists;
pub mod recommendations;
pub mod releases;
pub mod scan;
pub mod series;
pub mod series_exports;
pub mod settings;
pub mod setup;
pub mod sharing_tags;
pub mod task_metrics;
pub mod task_queue;
pub mod tracking;
pub mod user_plugins;
pub mod user_preferences;
pub mod users;
pub mod want_to_read;

pub use auth::*;
pub use books::*;
pub use bulk::*;
pub use bulk_metadata::*;
pub use collections::*;
pub use duplicates::*;
pub use events::*;
pub use filesystem::*;
pub use health::*;
pub use libraries::*;
pub use metrics::*;
pub use pages::*;
pub use read_progress::*;
pub use reading_sessions::*;
pub use reading_stats::*;
pub use readlists::*;
pub use scan::*;
pub use series::*;
pub use users::*;
pub use want_to_read::*;
