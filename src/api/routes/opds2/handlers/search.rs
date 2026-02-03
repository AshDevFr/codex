//! OPDS 2.0 Search Handler
//!
//! Handler for searching books and series via OPDS 2.0.

use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, ReadProgressRepository, SeriesMetadataRepository,
    SeriesRepository,
};
use crate::require_permission;
use axum::extract::{Query, State};
use serde::Deserialize;
use std::sync::Arc;

use super::super::dto::{
    ImageLink, Opds2Feed, Opds2Link, Publication, PublicationMetadata, ReadingProgress,
};
use super::Opds2Response;

/// Search query parameters for OPDS 2.0
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct Opds2SearchParams {
    /// Search query string
    pub query: String,
}

/// OPDS 2.0 search endpoint
///
/// Searches books and series by title and returns an OPDS 2.0 publications feed
#[utoipa::path(
    get,
    path = "/opds/v2/search",
    params(
        Opds2SearchParams
    ),
    responses(
        (status = 200, description = "OPDS 2.0 search results", content_type = "application/opds+json", body = Opds2Feed),
        (status = 400, description = "Bad request - empty query"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS 2.0"
)]
pub async fn search(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<Opds2SearchParams>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let base_url = "/opds/v2";
    let query = params.query.trim();

    if query.is_empty() {
        return Err(ApiError::BadRequest(
            "Search query cannot be empty".to_string(),
        ));
    }

    let mut publications: Vec<Publication> = Vec::new();

    // Search series by name and add as navigation-like entries with links to books
    let series_list = SeriesRepository::search_by_name(&state.db, query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?;

    // For series results, we create publications with links to browse the series
    // Fetch series metadata for names (title is now in series_metadata table)
    for series in series_list.iter().take(20) {
        // Fetch series name from series_metadata
        let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
            .await
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| "Unknown Series".to_string());

        let metadata = PublicationMetadata::new(format!("Series: {}", series_name))
            .with_identifier(format!("urn:uuid:series-{}", series.id))
            .with_modified(series.updated_at);

        let pub_entry = Publication::new(metadata)
            .add_link(
                Opds2Link::navigation_link(
                    format!("{}/series/{}", base_url, series.id),
                    format!("Browse {} books", series_name),
                )
                .with_rel("subsection"),
            )
            .add_image(ImageLink::thumbnail(format!(
                "/api/v1/series/{}/thumbnail",
                series.id
            )));

        publications.push(pub_entry);
    }

    // Search books by name/title
    let books = BookRepository::search_by_name(&state.db, query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search books: {}", e)))?;

    let user_id = auth.user_id;

    // Add book entries
    for book in books.iter().take(20) {
        // Fetch book title from book_metadata
        let title = BookMetadataRepository::get_by_book_id(&state.db, book.id)
            .await
            .ok()
            .flatten()
            .and_then(|m| m.title)
            .unwrap_or_else(|| "Untitled".to_string());

        let mime_type = match book.format.as_str() {
            "cbz" | "zip" => "application/zip",
            "cbr" | "rar" => "application/x-rar-compressed",
            "epub" => "application/epub+zip",
            "pdf" => "application/pdf",
            _ => "application/octet-stream",
        };

        let metadata = PublicationMetadata::new(title)
            .with_identifier(format!("urn:uuid:{}", book.id))
            .with_modified(book.updated_at)
            .with_page_count(book.page_count);

        let mut pub_entry = Publication::new(metadata)
            .add_link(Opds2Link::acquisition_link(
                format!("/api/v1/books/{}/file", book.id),
                mime_type,
            ))
            .add_image(ImageLink::thumbnail(format!(
                "/api/v1/books/{}/thumbnail",
                book.id
            )));

        // Add reading progress if available
        if let Ok(Some(progress)) =
            ReadProgressRepository::get_by_user_and_book(&state.db, user_id, book.id).await
        {
            pub_entry = pub_entry.with_reading_progress(ReadingProgress::new(
                progress.current_page,
                book.page_count,
                progress.completed,
                Some(progress.updated_at),
            ));
        }

        publications.push(pub_entry);
    }

    let total = publications.len() as i64;

    // Simple URL encoding for the query parameter
    let encoded_query = query
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '&' => "%26".to_string(),
            '=' => "%3D".to_string(),
            '?' => "%3F".to_string(),
            '#' => "%23".to_string(),
            _ if c.is_ascii_alphanumeric() || "-_.~".contains(c) => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect::<String>();

    let feed = Opds2Feed::publications(
        format!("Search Results for '{}'", query),
        vec![
            Opds2Link::self_link(format!("{}/search?query={}", base_url, encoded_query)),
            Opds2Link::start_link(base_url),
        ],
        publications,
    )
    .with_pagination(total, total as i32, 1);

    Ok(Opds2Response(feed))
}
