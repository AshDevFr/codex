use super::super::dto::{OpdsEntry, OpdsFeed, OpdsLink};
use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter},
    permissions::Permission,
};
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use codex_db::repositories::{
    BookMetadataRepository, BookRepository, CollectionRepository, LibraryRepository,
    ReadListRepository, ReadProgressRepository, SeriesMetadataRepository, SeriesRepository,
    SettingsRepository,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Pagination parameters for OPDS feeds
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
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
    tag = "OPDS"
)]
pub async fn root_catalog(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    let feed = OpdsFeed::with_author(
        "urn:uuid:codex-root",
        format!("{} OPDS Catalog", app_name),
        now,
        false,
        &app_name,
    )
    .with_subtitle("Digital library server for comics, manga, and ebooks")
    .add_link(OpdsLink::self_link(base_url.to_string()))
    .add_link(OpdsLink::start_link(base_url.to_string()))
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
    )
    .add_entry(
        OpdsEntry::new("urn:uuid:codex-collections", "Collections", now)
            .with_content("text", "Browse collections of series")
            .add_link(OpdsLink::subsection_link(
                format!("{}/collections", base_url),
                "Collections",
            )),
    )
    .add_entry(
        OpdsEntry::new("urn:uuid:codex-readlists", "Read Lists", now)
            .with_content("text", "Browse ordered reading lists")
            .add_link(OpdsLink::subsection_link(
                format!("{}/readlists", base_url),
                "Read Lists",
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
    tag = "OPDS"
)]
pub async fn list_libraries(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    // Fetch all libraries
    let libraries = LibraryRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;

    let mut feed = OpdsFeed::with_author(
        "urn:uuid:codex-libraries",
        "All Libraries",
        now,
        false,
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!("{}/libraries", base_url)))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(base_url.to_string(), "Home"));

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
    path = "/opds/libraries/{library_id}",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
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
    tag = "OPDS"
)]
pub async fn library_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(pagination): Query<OpdsPaginationParams>,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let pagination = pagination.validate(100);
    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    // Fetch library
    let library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;
    let visibility = content_filter.to_visibility();

    // Fetch all series in library (no built-in pagination)
    let all_series = SeriesRepository::list_by_library(&state.db, library_id, visibility.as_ref())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let total = all_series.len() as u64;

    // Manual pagination
    let start = pagination.offset() as usize;
    let end = (start + pagination.page_size as usize).min(all_series.len());
    let series_list = all_series[start..end].to_vec();

    let mut feed = OpdsFeed::with_author(
        format!("urn:uuid:library-{}", library_id),
        format!("{} - Series", library.name),
        now,
        false,
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/libraries/{}?page={}&pageSize={}",
        base_url, library_id, pagination.page, pagination.page_size
    )))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(
        format!("{}/libraries", base_url),
        "All Libraries",
    ))
    .with_pagination(total, pagination.page_size, pagination.offset());

    // Add pagination links
    if pagination.page > 1 {
        feed = feed.add_link(OpdsLink::prev_link(format!(
            "{}/libraries/{}?page={}&pageSize={}",
            base_url,
            library_id,
            pagination.page - 1,
            pagination.page_size
        )));
    }

    let total_pages = (total as f64 / pagination.page_size as f64).ceil() as u32;
    if pagination.page < total_pages {
        feed = feed.add_link(OpdsLink::next_link(format!(
            "{}/libraries/{}?page={}&pageSize={}",
            base_url,
            library_id,
            pagination.page + 1,
            pagination.page_size
        )));
    }

    // Add series entries
    // Fetch series metadata for names (title is now in series_metadata table)
    for series in series_list {
        // Fetch series name from series_metadata
        let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
            .await
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| "Unknown Series".to_string());

        let mut entry = OpdsEntry::new(
            format!("urn:uuid:series-{}", series.id),
            series_name.clone(),
            series.updated_at,
        )
        .add_link(OpdsLink::subsection_link(
            format!("{}/series/{}", base_url, series.id),
            series_name.clone(),
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

/// List collections (navigation feed)
#[utoipa::path(
    get,
    path = "/opds/collections",
    responses(
        (status = 200, description = "OPDS collections feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS"
)]
pub async fn list_collections(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    let collections = CollectionRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collections: {}", e)))?;

    let mut feed = OpdsFeed::with_author(
        "urn:uuid:codex-collections",
        "Collections",
        now,
        false,
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!("{}/collections", base_url)))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(base_url.to_string(), "Home"));

    for collection in collections {
        let entry = OpdsEntry::new(
            format!("urn:uuid:collection-{}", collection.id),
            collection.name.clone(),
            collection.updated_at,
        )
        .with_content("text", format!("Browse series in {}", collection.name))
        .add_link(OpdsLink::subsection_link(
            format!("{}/collections/{}", base_url, collection.id),
            collection.name.clone(),
        ))
        .add_link(OpdsLink::thumbnail_link(format!(
            "/api/v1/collections/{}/thumbnail",
            collection.id
        )))
        .add_link(OpdsLink::cover_link(format!(
            "/api/v1/collections/{}/thumbnail",
            collection.id
        )));
        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsResponse(xml))
}

/// List the series in a collection (navigation feed)
#[utoipa::path(
    get,
    path = "/opds/collections/{collection_id}",
    params(("collection_id" = Uuid, Path, description = "Collection ID")),
    responses(
        (status = 200, description = "OPDS collection series feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Collection not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS"
)]
pub async fn collection_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    let collection = CollectionRepository::get_by_id(&state.db, collection_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collection: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;

    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;
    let visibility = content_filter.to_visibility();

    let series_list =
        CollectionRepository::get_series(&state.db, collection_id, visibility.as_ref())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch collection series: {}", e)))?;

    let mut feed = OpdsFeed::with_author(
        format!("urn:uuid:collection-{}", collection_id),
        format!("{} - Series", collection.name),
        now,
        false,
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/collections/{}",
        base_url, collection_id
    )))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(
        format!("{}/collections", base_url),
        "Collections",
    ));

    for series in series_list {
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
        ))
        .add_link(OpdsLink::thumbnail_link(format!(
            "/api/v1/series/{}/thumbnail",
            series.id
        )))
        .add_link(OpdsLink::cover_link(format!(
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

/// List read lists (navigation feed)
#[utoipa::path(
    get,
    path = "/opds/readlists",
    responses(
        (status = 200, description = "OPDS read lists feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS"
)]
pub async fn list_readlists(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    let read_lists = ReadListRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read lists: {}", e)))?;

    let mut feed = OpdsFeed::with_author(
        "urn:uuid:codex-readlists",
        "Read Lists",
        now,
        false,
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!("{}/readlists", base_url)))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(base_url.to_string(), "Home"));

    for read_list in read_lists {
        let entry = OpdsEntry::new(
            format!("urn:uuid:readlist-{}", read_list.id),
            read_list.name.clone(),
            read_list.updated_at,
        )
        .with_content(
            "text",
            read_list
                .summary
                .clone()
                .unwrap_or_else(|| format!("Books in {}", read_list.name)),
        )
        .add_link(OpdsLink::subsection_link(
            format!("{}/readlists/{}", base_url, read_list.id),
            read_list.name.clone(),
        ))
        .add_link(OpdsLink::thumbnail_link(format!(
            "/api/v1/readlists/{}/thumbnail",
            read_list.id
        )))
        .add_link(OpdsLink::cover_link(format!(
            "/api/v1/readlists/{}/thumbnail",
            read_list.id
        )));
        feed = feed.add_entry(entry);
    }

    let xml = feed
        .to_xml()
        .map_err(|e| ApiError::Internal(format!("Failed to serialize OPDS feed: {}", e)))?;

    Ok(OpdsResponse(xml))
}

/// List the books in a read list (acquisition feed)
#[utoipa::path(
    get,
    path = "/opds/readlists/{read_list_id}",
    params(("read_list_id" = Uuid, Path, description = "Read list ID")),
    responses(
        (status = 200, description = "OPDS read list books feed", content_type = "application/atom+xml"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Read list not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS"
)]
pub async fn readlist_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    let read_list = ReadListRepository::get_by_id(&state.db, read_list_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read list: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;

    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;
    let visibility = content_filter.to_visibility();

    let books = ReadListRepository::get_books(&state.db, read_list_id, visibility.as_ref())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read list books: {}", e)))?;

    let mut feed = OpdsFeed::with_author(
        format!("urn:uuid:readlist-{}", read_list_id),
        format!("{} - Books", read_list.name),
        now,
        true, // Include PSE namespace
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/readlists/{}",
        base_url, read_list_id
    )))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(
        format!("{}/readlists", base_url),
        "Read Lists",
    ));

    for book in books {
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

        let last_read =
            ReadProgressRepository::get_by_user_and_book(&state.db, auth.user_id, book.id)
                .await
                .ok()
                .flatten()
                .map(|progress| progress.current_page as u32);

        let entry = OpdsEntry::new(
            format!("urn:uuid:book-{}", book.id),
            title.clone(),
            book.updated_at,
        )
        .add_link(OpdsLink::acquisition_link(
            format!("/api/v1/books/{}/file", book.id),
            mime_type,
        ))
        .add_link(OpdsLink::pse_stream_link(
            format!("{}/books/{}/pages", base_url, book.id),
            book.page_count as u32,
            last_read,
        ))
        .add_link(OpdsLink::thumbnail_link(format!(
            "/api/v1/books/{}/thumbnail",
            book.id
        )))
        .add_link(OpdsLink::cover_link(format!(
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

/// List books in a series
///
/// Returns an acquisition feed with all books in the specified series
#[utoipa::path(
    get,
    path = "/opds/series/{series_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
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
    tag = "OPDS"
)]
pub async fn series_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<OpdsResponse, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let now = Utc::now();
    let base_url = "/opds";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    // Fetch series
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Fetch series metadata for name
    let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .ok()
        .flatten()
        .map(|m| m.title)
        .unwrap_or_else(|| "Unknown Series".to_string());

    // Fetch books (excluding deleted books)
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let mut feed = OpdsFeed::with_author(
        format!("urn:uuid:series-{}", series_id),
        format!("{} - Books", series_name),
        now,
        true, // Include PSE namespace
        &app_name,
    )
    .add_link(OpdsLink::self_link(format!(
        "{}/series/{}",
        base_url, series_id
    )))
    .add_link(OpdsLink::start_link(base_url.to_string()))
    .add_link(OpdsLink::up_link(
        format!("{}/libraries/{}", base_url, series.library_id),
        "Library",
    ));

    // Add book entries
    for book in books {
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
