//! API v1 Handlers
//!
//! This module contains all request handlers for API v1.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::dto::common::PaginationLinkBuilder;

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

pub mod api_keys;
pub mod auth;
pub mod books;
pub mod bulk;
pub mod bulk_metadata;
pub mod cleanup;
pub mod duplicates;
pub mod events;
pub mod filesystem;
pub mod health;
pub mod info;
pub mod libraries;
pub mod metrics;
pub mod oidc;
pub mod pages;
pub mod pdf_cache;
pub mod plugin_actions;
pub mod plugin_storage;
pub mod plugins;
pub mod read_progress;
pub mod recommendations;
pub mod scan;
pub mod series;
pub mod series_exports;
pub mod settings;
pub mod setup;
pub mod sharing_tags;
pub mod task_metrics;
pub mod task_queue;
pub mod user_plugins;
pub mod user_preferences;
pub mod users;

pub use auth::*;
pub use books::*;
pub use bulk::*;
pub use bulk_metadata::*;
pub use duplicates::*;
pub use events::*;
pub use filesystem::*;
pub use health::*;
pub use libraries::*;
pub use metrics::*;
pub use pages::*;
pub use read_progress::*;
pub use scan::*;
pub use series::*;
pub use users::*;
