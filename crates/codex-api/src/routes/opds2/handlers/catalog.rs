//! OPDS 2.0 Catalog Handlers
//!
//! Handlers for browsing the OPDS 2.0 catalog (JSON-based).

use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter},
    permissions::Permission,
    routes::opds::handlers::OpdsPaginationParams,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use codex_db::repositories::{
    BookMetadataRepository, BookRepository, CollectionRepository, LibraryRepository,
    ReadListRepository, ReadProgressRepository, SeriesMetadataRepository, SeriesRepository,
    SettingsRepository,
};
use std::sync::Arc;
use uuid::Uuid;

use super::super::dto::{
    ImageLink, Opds2Feed, Opds2Link, Publication, PublicationMetadata, ReadingProgress,
};

/// OPDS 2.0 JSON response wrapper
pub struct Opds2Response(pub Opds2Feed);

impl IntoResponse for Opds2Response {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/opds+json;charset=utf-8")],
            Json(self.0),
        )
            .into_response()
    }
}

/// Root OPDS 2.0 catalog
///
/// Returns the main navigation feed with links to:
/// - All libraries
/// - Search
/// - Recent additions
#[utoipa::path(
    get,
    path = "/opds/v2",
    responses(
        (status = 200, description = "OPDS 2.0 root catalog", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS 2.0"
)]
pub async fn root(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let base_url = "/opds/v2";
    let app_name = SettingsRepository::get_app_name(&state.db).await;

    let feed = Opds2Feed::navigation(
        format!("{} OPDS 2.0 Catalog", app_name),
        vec![
            Opds2Link::self_link(base_url),
            Opds2Link::start_link(base_url),
            Opds2Link::search_template(format!("{}/search{{?query}}", base_url)),
        ],
        vec![
            Opds2Link::navigation_link(format!("{}/libraries", base_url), "All Libraries"),
            Opds2Link::new_link(format!("{}/recent", base_url), "Recent Additions"),
            Opds2Link::navigation_link(format!("{}/collections", base_url), "Collections"),
            Opds2Link::navigation_link(format!("{}/readlists", base_url), "Read Lists"),
        ],
    )
    .with_subtitle("Digital library server for comics, manga, and ebooks");

    Ok(Opds2Response(feed))
}

/// List all libraries (OPDS 2.0)
///
/// Returns a navigation feed with all available libraries
#[utoipa::path(
    get,
    path = "/opds/v2/libraries",
    responses(
        (status = 200, description = "OPDS 2.0 libraries feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS 2.0"
)]
pub async fn libraries(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let base_url = "/opds/v2";

    // Fetch all libraries
    let libraries = LibraryRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;

    let nav_links: Vec<Opds2Link> = libraries
        .iter()
        .map(|lib| {
            Opds2Link::navigation_link(
                format!("{}/libraries/{}", base_url, lib.id),
                lib.name.clone(),
            )
        })
        .collect();

    let feed = Opds2Feed::navigation(
        "All Libraries",
        vec![
            Opds2Link::self_link(format!("{}/libraries", base_url)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(base_url, "Home"),
        ],
        nav_links,
    );

    Ok(Opds2Response(feed))
}

/// List series in a library (OPDS 2.0)
///
/// Returns a navigation feed with all series in the specified library
#[utoipa::path(
    get,
    path = "/opds/v2/libraries/{library_id}",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        OpdsPaginationParams
    ),
    responses(
        (status = 200, description = "OPDS 2.0 library series feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS 2.0"
)]
pub async fn library_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(pagination): Query<OpdsPaginationParams>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let pagination = pagination.validate(100);
    let base_url = "/opds/v2";

    // Fetch library
    let library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;
    let visibility = content_filter.to_visibility();

    // Fetch all series in library
    let all_series = SeriesRepository::list_by_library(&state.db, library_id, visibility.as_ref())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let total = all_series.len() as i64;

    // Manual pagination
    let start = pagination.offset() as usize;
    let end = (start + pagination.page_size as usize).min(all_series.len());
    let series_list = all_series[start..end].to_vec();

    // Build navigation links with series names from series_metadata
    let mut nav_links = Vec::new();
    for series in &series_list {
        // Fetch series name from series_metadata
        let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
            .await
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| "Unknown Series".to_string());

        nav_links.push(Opds2Link::navigation_link(
            format!("{}/series/{}", base_url, series.id),
            series_name,
        ));
    }

    let mut links = vec![
        Opds2Link::self_link(format!(
            "{}/libraries/{}?page={}&pageSize={}",
            base_url, library_id, pagination.page, pagination.page_size
        )),
        Opds2Link::start_link(base_url),
        Opds2Link::up_link(format!("{}/libraries", base_url), "All Libraries"),
    ];

    // Add pagination links
    if pagination.page > 1 {
        links.push(Opds2Link::first_link(format!(
            "{}/libraries/{}?page=1&pageSize={}",
            base_url, library_id, pagination.page_size
        )));
        links.push(Opds2Link::prev_link(format!(
            "{}/libraries/{}?page={}&pageSize={}",
            base_url,
            library_id,
            pagination.page - 1,
            pagination.page_size
        )));
    }

    let total_pages = ((total as f64) / (pagination.page_size as f64)).ceil() as u32;
    if pagination.page < total_pages {
        links.push(Opds2Link::next_link(format!(
            "{}/libraries/{}?page={}&pageSize={}",
            base_url,
            library_id,
            pagination.page + 1,
            pagination.page_size
        )));
        links.push(Opds2Link::last_link(format!(
            "{}/libraries/{}?page={}&pageSize={}",
            base_url, library_id, total_pages, pagination.page_size
        )));
    }

    let feed = Opds2Feed::navigation(format!("{} - Series", library.name), links, nav_links)
        .with_pagination(total, pagination.page_size as i32, pagination.page as i32);

    Ok(Opds2Response(feed))
}

/// List books in a series (OPDS 2.0)
///
/// Returns a publications feed with all books in the specified series
#[utoipa::path(
    get,
    path = "/opds/v2/series/{series_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
    ),
    responses(
        (status = 200, description = "OPDS 2.0 series books feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS 2.0"
)]
pub async fn series_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let base_url = "/opds/v2";

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

    // Fetch reading progress for all books in one query per book
    // (could be optimized with a batch query in the future)
    let user_id = auth.user_id;
    let mut publications: Vec<Publication> = Vec::with_capacity(books.len());

    for book in &books {
        // Fetch book metadata for title and number
        let book_meta = BookMetadataRepository::get_by_book_id(&state.db, book.id)
            .await
            .ok()
            .flatten();
        let title = book_meta
            .as_ref()
            .and_then(|m| m.title.clone())
            .unwrap_or_else(|| "Untitled".to_string());
        let book_number = book_meta
            .as_ref()
            .and_then(|m| m.number)
            .and_then(|n| n.to_string().parse::<f64>().ok());

        // Determine MIME type based on format
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
            .with_page_count(book.page_count)
            .with_series(series_name.clone(), book_number);

        let mut publication = Publication::new(metadata)
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
            publication = publication.with_reading_progress(ReadingProgress::new(
                progress.current_page,
                book.page_count,
                progress.completed,
                Some(progress.updated_at),
            ));
        }

        publications.push(publication);
    }

    let total = publications.len() as i64;

    let feed = Opds2Feed::publications(
        format!("{} - Books", series_name),
        vec![
            Opds2Link::self_link(format!("{}/series/{}", base_url, series_id)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(
                format!("{}/libraries/{}", base_url, series.library_id),
                "Library",
            ),
        ],
        publications,
    )
    .with_pagination(total, total as i32, 1);

    Ok(Opds2Response(feed))
}

/// List recent additions (OPDS 2.0)
///
/// Returns a publications feed with recently added books
#[utoipa::path(
    get,
    path = "/opds/v2/recent",
    params(
        OpdsPaginationParams
    ),
    responses(
        (status = 200, description = "OPDS 2.0 recent additions feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "OPDS 2.0"
)]
pub async fn recent(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(pagination): Query<OpdsPaginationParams>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let pagination = pagination.validate(50);
    let base_url = "/opds/v2";

    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;
    let visibility = content_filter.to_visibility();

    // Fetch recent books with their series
    // page is 0-indexed
    let (books, _total) = BookRepository::list_recently_added(
        &state.db,
        None, // All libraries
        false,
        0,
        pagination.page_size as u64,
        visibility.as_ref(),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let user_id = auth.user_id;
    let mut publications: Vec<Publication> = Vec::with_capacity(books.len());

    for book in &books {
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

        let mut publication = Publication::new(metadata)
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
            publication = publication.with_reading_progress(ReadingProgress::new(
                progress.current_page,
                book.page_count,
                progress.completed,
                Some(progress.updated_at),
            ));
        }

        publications.push(publication);
    }

    let total = publications.len() as i64;

    let feed = Opds2Feed::publications(
        "Recent Additions",
        vec![
            Opds2Link::self_link(format!("{}/recent", base_url)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(base_url, "Home"),
        ],
        publications,
    )
    .with_pagination(total, pagination.page_size as i32, pagination.page as i32);

    Ok(Opds2Response(feed))
}

/// List collections (OPDS 2.0 navigation feed)
#[utoipa::path(
    get,
    path = "/opds/v2/collections",
    responses(
        (status = 200, description = "OPDS 2.0 collections feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS 2.0"
)]
pub async fn list_collections(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    let base_url = "/opds/v2";

    let collections = CollectionRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collections: {}", e)))?;

    let nav_links: Vec<Opds2Link> = collections
        .iter()
        .map(|c| {
            Opds2Link::navigation_link(format!("{}/collections/{}", base_url, c.id), c.name.clone())
        })
        .collect();

    let feed = Opds2Feed::navigation(
        "Collections",
        vec![
            Opds2Link::self_link(format!("{}/collections", base_url)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(base_url, "Home"),
        ],
        nav_links,
    );

    Ok(Opds2Response(feed))
}

/// List the series in a collection (OPDS 2.0 navigation feed)
#[utoipa::path(
    get,
    path = "/opds/v2/collections/{collection_id}",
    params(("collection_id" = Uuid, Path, description = "Collection ID")),
    responses(
        (status = 200, description = "OPDS 2.0 collection series feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Collection not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS 2.0"
)]
pub async fn collection_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(collection_id): Path<Uuid>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    let base_url = "/opds/v2";

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

    let mut nav_links = Vec::with_capacity(series_list.len());
    for series in &series_list {
        let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
            .await
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| "Unknown Series".to_string());
        nav_links.push(Opds2Link::navigation_link(
            format!("{}/series/{}", base_url, series.id),
            series_name,
        ));
    }

    let feed = Opds2Feed::navigation(
        format!("{} - Series", collection.name),
        vec![
            Opds2Link::self_link(format!("{}/collections/{}", base_url, collection_id)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(format!("{}/collections", base_url), "Collections"),
        ],
        nav_links,
    );

    Ok(Opds2Response(feed))
}

/// List read lists (OPDS 2.0 navigation feed)
#[utoipa::path(
    get,
    path = "/opds/v2/readlists",
    responses(
        (status = 200, description = "OPDS 2.0 read lists feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS 2.0"
)]
pub async fn list_readlists(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;
    let base_url = "/opds/v2";

    let read_lists = ReadListRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read lists: {}", e)))?;

    let nav_links: Vec<Opds2Link> = read_lists
        .iter()
        .map(|r| {
            Opds2Link::navigation_link(format!("{}/readlists/{}", base_url, r.id), r.name.clone())
        })
        .collect();

    let feed = Opds2Feed::navigation(
        "Read Lists",
        vec![
            Opds2Link::self_link(format!("{}/readlists", base_url)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(base_url, "Home"),
        ],
        nav_links,
    );

    Ok(Opds2Response(feed))
}

/// List the books in a read list (OPDS 2.0 publications feed)
#[utoipa::path(
    get,
    path = "/opds/v2/readlists/{read_list_id}",
    params(("read_list_id" = Uuid, Path, description = "Read list ID")),
    responses(
        (status = 200, description = "OPDS 2.0 read list books feed", content_type = "application/opds+json", body = Opds2Feed),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Read list not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "OPDS 2.0"
)]
pub async fn readlist_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;
    let base_url = "/opds/v2";

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

    let user_id = auth.user_id;
    let mut publications: Vec<Publication> = Vec::with_capacity(books.len());
    for book in &books {
        let book_meta = BookMetadataRepository::get_by_book_id(&state.db, book.id)
            .await
            .ok()
            .flatten();
        let title = book_meta
            .as_ref()
            .and_then(|m| m.title.clone())
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

        let mut publication = Publication::new(metadata)
            .add_link(Opds2Link::acquisition_link(
                format!("/api/v1/books/{}/file", book.id),
                mime_type,
            ))
            .add_image(ImageLink::thumbnail(format!(
                "/api/v1/books/{}/thumbnail",
                book.id
            )));

        if let Ok(Some(progress)) =
            ReadProgressRepository::get_by_user_and_book(&state.db, user_id, book.id).await
        {
            publication = publication.with_reading_progress(ReadingProgress::new(
                progress.current_page,
                book.page_count,
                progress.completed,
                Some(progress.updated_at),
            ));
        }

        publications.push(publication);
    }

    let total = publications.len() as i64;

    let feed = Opds2Feed::publications(
        format!("{} - Books", read_list.name),
        vec![
            Opds2Link::self_link(format!("{}/readlists/{}", base_url, read_list_id)),
            Opds2Link::start_link(base_url),
            Opds2Link::up_link(format!("{}/readlists", base_url), "Read Lists"),
        ],
        publications,
    )
    .with_pagination(total, total as i32, 1);

    Ok(Opds2Response(feed))
}
