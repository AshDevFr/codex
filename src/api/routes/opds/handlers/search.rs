use super::super::dto::{OpdsEntry, OpdsFeed, OpdsLink};
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, ReadProgressRepository, SeriesMetadataRepository,
    SeriesRepository, SettingsRepository,
};
use crate::require_permission;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;

/// Generate OpenSearch descriptor XML with dynamic app name
fn generate_opensearch_descriptor(app_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
  <ShortName>{app_name}</ShortName>
  <Description>Search your {app_name} digital library</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <OutputEncoding>UTF-8</OutputEncoding>
  <Url type="application/atom+xml;profile=opds-catalog" template="/opds/search?q={{searchTerms}}"/>
</OpenSearchDescription>"#,
        app_name = app_name
    )
}

/// OpenSearch descriptor response
pub struct OpenSearchResponse(String);

impl IntoResponse for OpenSearchResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/opensearchdescription+xml;charset=utf-8",
            )],
            self.0,
        )
            .into_response()
    }
}

/// OPDS search response
pub struct OpdsSearchResponse(String);

impl IntoResponse for OpdsSearchResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/atom+xml;profile=opds-catalog;kind=acquisition;charset=utf-8",
            )],
            self.0,
        )
            .into_response()
    }
}

/// Search query parameters
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchParams {
    /// Search query string
    pub q: String,
}

/// OpenSearch descriptor endpoint
///
/// Returns the OpenSearch XML descriptor for OPDS clients
#[utoipa::path(
    get,
    path = "/opds/search.xml",
    responses(
        (status = 200, description = "OpenSearch descriptor", content_type = "application/opensearchdescription+xml"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS"
)]
pub async fn opensearch_descriptor(
    State(state): State<Arc<AuthState>>,
    _auth: AuthContext,
) -> Result<OpenSearchResponse, ApiError> {
    let app_name = SettingsRepository::get_app_name(&state.db).await;
    Ok(OpenSearchResponse(generate_opensearch_descriptor(
        &app_name,
    )))
}

/// OPDS search endpoint
///
/// Searches books and series by title and returns an OPDS acquisition feed
#[utoipa::path(
    get,
    path = "/opds/search",
    params(
        SearchParams
    ),
    responses(
        (status = 200, description = "OPDS search results", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS"
)]
pub async fn search(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<SearchParams>,
) -> Result<OpdsSearchResponse, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let query = params.q.trim();
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    if query.is_empty() {
        return Err(ApiError::BadRequest(
            "Search query cannot be empty".to_string(),
        ));
    }

    let mut feed = OpdsFeed::with_author(
        format!("urn:uuid:codex-search-{}", query),
        format!("Search Results for '{}'", query),
        now,
        true, // Include PSE namespace
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/search?q={}",
        base_url, query
    )))
    .add_link(OpdsLink::start_link(base_url.to_string()));

    // Search series by name
    let series_list = SeriesRepository::search_by_name(&state.db, query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?;

    // Add series entries (navigation to series books)
    // Fetch series metadata for names (title is now in series_metadata table)
    for series in series_list.iter().take(20) {
        // Fetch series name from series_metadata
        let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
            .await
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| "Unknown Series".to_string());

        let entry = OpdsEntry::new(
            format!("urn:uuid:series-{}", series.id),
            series_name.clone(),
            series.updated_at,
        )
        .add_link(OpdsLink::subsection_link(
            format!("{}/series/{}", base_url, series.id),
            series_name.clone(),
        ));

        feed = feed.add_entry(entry);
    }

    // Search books by name/title
    let books = BookRepository::search_by_name(&state.db, query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search books: {}", e)))?;

    // Add book entries
    for book in books.iter().take(20) {
        // Fetch book title from book_metadata
        let title = BookMetadataRepository::get_by_book_id(&state.db, book.id)
            .await
            .ok()
            .flatten()
            .and_then(|m| m.title)
            .unwrap_or_else(|| "Untitled".to_string());

        let mut entry = OpdsEntry::new(
            format!("urn:uuid:book-{}", book.id),
            title.clone(),
            book.updated_at,
        );

        // Add acquisition link (download whole book)
        let mime_type = match book.format.as_str() {
            "cbz" | "zip" => "application/zip",
            "cbr" | "rar" => "application/x-rar-compressed",
            "epub" => "application/epub+zip",
            "pdf" => "application/pdf",
            _ => "application/octet-stream",
        };

        entry = entry.add_link(OpdsLink::acquisition_link(
            format!("/api/v1/books/{}/file", book.id),
            mime_type,
        ));

        // Fetch reading progress for this book
        let last_read =
            ReadProgressRepository::get_by_user_and_book(&state.db, auth.user_id, book.id)
                .await
                .ok()
                .flatten()
                .map(|progress| progress.current_page as u32);

        // Add PSE streaming link with reading progress
        entry = entry.add_link(OpdsLink::pse_stream_link(
            format!("{}/books/{}/pages", base_url, book.id),
            book.page_count as u32,
            last_read,
        ));

        // Add thumbnail link
        entry = entry.add_link(OpdsLink::thumbnail_link(format!(
            "/api/v1/books/{}/thumbnail",
            book.id
        )));

        // Add cover link
        entry = entry.add_link(OpdsLink::cover_link(format!(
            "/api/v1/books/{}/thumbnail",
            book.id
        )));

        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsSearchResponse(xml))
}
