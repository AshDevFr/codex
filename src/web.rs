//! Frontend static file serving module
//!
//! This module provides functionality to serve the embedded React frontend.
//! The frontend is embedded into the binary when the `embed-frontend` feature is enabled.

#[cfg(feature = "embed-frontend")]
use axum::{
    body::Body,
    http::{HeaderValue, Response, header},
};
use axum::{
    http::{StatusCode, Uri},
    response::IntoResponse,
};

#[cfg(feature = "embed-frontend")]
use rust_embed::RustEmbed;

// Embed the frontend dist directory when the feature is enabled
#[cfg(feature = "embed-frontend")]
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct StaticAssets;

/// Serves static files from the embedded frontend (production mode)
/// Falls back to index.html for client-side routing (SPA)
#[cfg(feature = "embed-frontend")]
pub async fn serve_static(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Handle root path
    let path = if path.is_empty() { "index.html" } else { path };

    // Try to serve the requested file
    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let body = Body::from(content.data);

            Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.as_ref()).unwrap(),
                )
                .body(body)
                .unwrap()
        }
        // If file not found, serve index.html for client-side routing
        None => match StaticAssets::get("index.html") {
            Some(content) => {
                let body = Body::from(content.data);

                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html"))
                    .body(body)
                    .unwrap()
            }
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 - Frontend not found"))
                .unwrap(),
        },
    }
}

/// Handler for when frontend is not embedded (dev mode)
/// Returns a helpful message directing users to the development environment
#[cfg(not(feature = "embed-frontend"))]
pub async fn serve_static(_uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "Frontend not embedded. Use the dev environment with docker-compose or run the Vite dev server separately.",
    )
}
