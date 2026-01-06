use crate::api::{
    dto::{OpdsEntry, OpdsFeed, OpdsLink},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository,
};
use crate::require_permission;
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Pagination parameters for OPDS feeds
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct OpdsPaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    50
}

impl OpdsPaginationParams {
    pub fn validate(mut self, max_size: u32) -> Self {
        if self.page < 1 {
            self.page = 1;
        }
        if self.page_size < 1 {
            self.page_size = default_page_size();
        }
        if self.page_size > max_size {
            self.page_size = max_size;
        }
        self
    }

    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.page_size
    }
}

/// Response wrapper for OPDS feeds
pub struct OpdsResponse(String);

impl IntoResponse for OpdsResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/atom+xml;profile=opds-catalog;kind=navigation;charset=utf-8",
            )],
            self.0,
        )
            .into_response()
    }
}

/// Root OPDS catalog
///
/// Returns the main navigation feed with links to:
/// - All libraries
/// - Search
/// - Recent additions
#[utoipa::path(
    get,
    path = "/opds",
    responses(
        (status = 200, description = "OPDS root catalog", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "opds"
)]
pub async fn root_catalog(
    State(_state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let now = Utc::now();
    let base_url = "/opds";

    let feed = OpdsFeed::new("urn:uuid:codex-root", "Codex OPDS Catalog", now, false)
        .with_subtitle("Digital library server for comics, manga, and ebooks")
        .add_link(OpdsLink::self_link(format!("{}", base_url)))
        .add_link(OpdsLink::start_link(format!("{}", base_url)))
        .add_link(OpdsLink::search_link(format!("{}/search.xml", base_url)))
        // Navigation entries
        .add_entry(
            OpdsEntry::new("urn:uuid:codex-libraries", "All Libraries", now)
                .with_content("text", "Browse all available libraries")
                .add_link(OpdsLink::subsection_link(
                    format!("{}/libraries", base_url),
                    "All Libraries",
                )),
        )
        .add_entry(
            OpdsEntry::new("urn:uuid:codex-recent", "Recent Additions", now)
                .with_content("text", "Recently added books and series")
                .add_link(OpdsLink::subsection_link(
                    format!("{}/recent", base_url),
                    "Recent Additions",
                )),
        );

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsResponse(xml))
}

/// List all libraries
///
/// Returns a navigation feed with all available libraries
#[utoipa::path(
    get,
    path = "/opds/libraries",
    responses(
        (status = 200, description = "OPDS libraries feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "opds"
)]
pub async fn opds_list_libraries(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let now = Utc::now();
    let base_url = "/opds";

    // Fetch all libraries
    let libraries = LibraryRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;

    let mut feed = OpdsFeed::new("urn:uuid:codex-libraries", "All Libraries", now, false)
        .add_link(OpdsLink::self_link(format!("{}/libraries", base_url)))
        .add_link(OpdsLink::start_link(format!("{}", base_url)))
        .add_link(OpdsLink::up_link(format!("{}", base_url), "Home"));

    // Add library entries
    for library in libraries {
        let entry = OpdsEntry::new(
            format!("urn:uuid:library-{}", library.id),
            library.name.clone(),
            library.updated_at,
        )
        .with_content("text", format!("Browse series in {}", library.name))
        .add_link(OpdsLink::subsection_link(
            format!("{}/libraries/{}", base_url, library.id),
            library.name,
        ));

        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsResponse(xml))
}

/// List series in a library
///
/// Returns an acquisition feed with all series in the specified library
#[utoipa::path(
    get,
    path = "/opds/libraries/{id}",
    params(
        ("id" = Uuid, Path, description = "Library ID"),
        OpdsPaginationParams
    ),
    responses(
        (status = 200, description = "OPDS library series feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "opds"
)]
pub async fn opds_library_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(pagination): Query<OpdsPaginationParams>,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let pagination = pagination.validate(100);
    let now = Utc::now();
    let base_url = "/opds";

    // Fetch library
    let library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Fetch all series in library (no built-in pagination)
    let all_series = SeriesRepository::list_by_library(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let total = all_series.len() as u64;

    // Manual pagination
    let start = pagination.offset() as usize;
    let end = (start + pagination.page_size as usize).min(all_series.len());
    let series_list = all_series[start..end].to_vec();

    let mut feed = OpdsFeed::new(
        format!("urn:uuid:library-{}", library_id),
        format!("{} - Series", library.name),
        now,
        false,
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/libraries/{}?page={}&page_size={}",
        base_url, library_id, pagination.page, pagination.page_size
    )))
    .add_link(OpdsLink::start_link(format!("{}", base_url)))
    .add_link(OpdsLink::up_link(
        format!("{}/libraries", base_url),
        "All Libraries",
    ))
    .with_pagination(total, pagination.page_size, pagination.offset());

    // Add pagination links
    if pagination.page > 1 {
        feed = feed.add_link(OpdsLink::prev_link(format!(
            "{}/libraries/{}?page={}&page_size={}",
            base_url,
            library_id,
            pagination.page - 1,
            pagination.page_size
        )));
    }

    let total_pages = (total as f64 / pagination.page_size as f64).ceil() as u32;
    if pagination.page < total_pages {
        feed = feed.add_link(OpdsLink::next_link(format!(
            "{}/libraries/{}?page={}&page_size={}",
            base_url,
            library_id,
            pagination.page + 1,
            pagination.page_size
        )));
    }

    // Add series entries
    for series in series_list {
        let mut entry = OpdsEntry::new(
            format!("urn:uuid:series-{}", series.id),
            series.name.clone(),
            series.updated_at,
        )
        .with_summary("text", series.summary.unwrap_or_default())
        .add_link(OpdsLink::subsection_link(
            format!("{}/series/{}", base_url, series.id),
            series.name.clone(),
        ));

        // Add series thumbnail link
        entry = entry.add_link(OpdsLink::thumbnail_link(format!(
            "/api/v1/series/{}/thumbnail",
            series.id
        )));

        // Add series cover link (same as thumbnail, but full-size)
        entry = entry.add_link(OpdsLink::cover_link(format!(
            "/api/v1/series/{}/thumbnail",
            series.id
        )));

        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsResponse(xml))
}

/// List books in a series
///
/// Returns an acquisition feed with all books in the specified series
#[utoipa::path(
    get,
    path = "/opds/series/{id}",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
    ),
    responses(
        (status = 200, description = "OPDS series books feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "opds"
)]
pub async fn opds_series_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let now = Utc::now();
    let base_url = "/opds";

    // Fetch series
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Fetch books (excluding deleted books)
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let mut feed = OpdsFeed::new(
        format!("urn:uuid:series-{}", series_id),
        format!("{} - Books", series.name),
        now,
        true, // Include PSE namespace
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/series/{}",
        base_url, series_id
    )))
    .add_link(OpdsLink::start_link(format!("{}", base_url)))
    .add_link(OpdsLink::up_link(
        format!("{}/libraries/{}", base_url, series.library_id),
        "Library",
    ));

    // Add book entries
    for book in books {
        let title = book.title.clone().unwrap_or_else(|| "Untitled".to_string());

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

        // Add cover link (same as thumbnail, but full-size)
        entry = entry.add_link(OpdsLink::cover_link(format!(
            "/api/v1/books/{}/thumbnail",
            book.id
        )));

        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsResponse(xml))
}
