use crate::api::{
    dto::{OpdsEntry, OpdsFeed, OpdsLink},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{BookMetadataRepository, BookRepository};
use crate::require_permission;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// Response wrapper for OPDS-PSE feeds
pub struct OpdsPseResponse(String);

impl IntoResponse for OpdsPseResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/atom+xml;profile=opds-catalog;charset=utf-8",
            )],
            self.0,
        )
            .into_response()
    }
}

/// OPDS-PSE: List all pages in a book
///
/// Returns a PSE page feed with individual page links for streaming.
/// This allows OPDS clients to read books page-by-page without downloading the entire file.
#[utoipa::path(
    get,
    path = "/opds/books/{book_id}/pages",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
    ),
    responses(
        (status = 200, description = "OPDS-PSE page feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "opds"
)]
pub async fn opds_book_pages(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<OpdsPseResponse, ApiError> {
    require_permission!(auth, Permission::PagesRead)?;

    let now = Utc::now();
    let base_url = "/opds";

    // Fetch book
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Fetch book title from book_metadata
    let title = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.title)
        .unwrap_or_else(|| "Untitled".to_string());

    let mut feed = OpdsFeed::new(
        format!("urn:uuid:book-{}-pages", book_id),
        format!("{} - Pages", title),
        now,
        true, // Include PSE namespace
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/books/{}/pages",
        base_url, book_id
    )))
    .add_link(OpdsLink::start_link(format!("{}", base_url)))
    .add_link(OpdsLink::up_link(
        format!("{}/series/{}", base_url, book.series_id),
        "Series",
    ));

    // Add page entries
    for page_num in 1..=book.page_count {
        let entry = OpdsEntry::new(
            format!("urn:uuid:book-{}-page-{}", book_id, page_num),
            format!("Page {}", page_num),
            book.updated_at,
        )
        .add_link(
            OpdsLink::new(
                "http://vaemendis.net/opds-pse/page",
                format!("/api/v1/books/{}/pages/{}", book_id, page_num),
            )
            .with_type("image/jpeg"),
        ); // Default to JPEG, actual format determined by page handler

        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsPseResponse(xml))
}
