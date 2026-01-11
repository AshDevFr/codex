use crate::api::{
    dto::opds2::{
        ImageLink, Opds2Feed, Opds2Link, Publication, PublicationMetadata, ReadingProgress,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState},
    handlers::opds::OpdsPaginationParams,
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
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

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
    tag = "opds2"
)]
pub async fn opds2_root(
    State(_state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let base_url = "/opds/v2";

    let feed = Opds2Feed::navigation(
        "Codex OPDS 2.0 Catalog",
        vec![
            Opds2Link::self_link(base_url),
            Opds2Link::start_link(base_url),
            Opds2Link::search_template(format!("{}/search{{?query}}", base_url)),
        ],
        vec![
            Opds2Link::navigation_link(format!("{}/libraries", base_url), "All Libraries"),
            Opds2Link::new_link(format!("{}/recent", base_url), "Recent Additions"),
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
    tag = "opds2"
)]
pub async fn opds2_libraries(
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
    path = "/opds/v2/libraries/{id}",
    params(
        ("id" = Uuid, Path, description = "Library ID"),
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
    tag = "opds2"
)]
pub async fn opds2_library_series(
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

    // Fetch all series in library
    let all_series = SeriesRepository::list_by_library(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let total = all_series.len() as i64;

    // Manual pagination
    let start = pagination.offset() as usize;
    let end = (start + pagination.page_size as usize).min(all_series.len());
    let series_list = all_series[start..end].to_vec();

    let nav_links: Vec<Opds2Link> = series_list
        .iter()
        .map(|series| {
            Opds2Link::navigation_link(
                format!("{}/series/{}", base_url, series.id),
                series.name.clone(),
            )
        })
        .collect();

    let mut links = vec![
        Opds2Link::self_link(format!(
            "{}/libraries/{}?page={}&page_size={}",
            base_url, library_id, pagination.page, pagination.page_size
        )),
        Opds2Link::start_link(base_url),
        Opds2Link::up_link(format!("{}/libraries", base_url), "All Libraries"),
    ];

    // Add pagination links
    if pagination.page > 1 {
        links.push(Opds2Link::first_link(format!(
            "{}/libraries/{}?page=1&page_size={}",
            base_url, library_id, pagination.page_size
        )));
        links.push(Opds2Link::prev_link(format!(
            "{}/libraries/{}?page={}&page_size={}",
            base_url,
            library_id,
            pagination.page - 1,
            pagination.page_size
        )));
    }

    let total_pages = ((total as f64) / (pagination.page_size as f64)).ceil() as u32;
    if pagination.page < total_pages {
        links.push(Opds2Link::next_link(format!(
            "{}/libraries/{}?page={}&page_size={}",
            base_url,
            library_id,
            pagination.page + 1,
            pagination.page_size
        )));
        links.push(Opds2Link::last_link(format!(
            "{}/libraries/{}?page={}&page_size={}",
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
    path = "/opds/v2/series/{id}",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
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
    tag = "opds2"
)]
pub async fn opds2_series_books(
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

    // Fetch books (excluding deleted books)
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    // Fetch reading progress for all books in one query per book
    // (could be optimized with a batch query in the future)
    let user_id = auth.user_id;
    let mut publications: Vec<Publication> = Vec::with_capacity(books.len());

    for book in &books {
        let title = book.title.clone().unwrap_or_else(|| "Untitled".to_string());

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
            .with_series(
                series.name.clone(),
                book.number
                    .as_ref()
                    .and_then(|n| n.to_string().parse::<f64>().ok()),
            );

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
        format!("{} - Books", series.name),
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
    tag = "opds2"
)]
pub async fn opds2_recent(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(pagination): Query<OpdsPaginationParams>,
) -> Result<Opds2Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let pagination = pagination.validate(50);
    let base_url = "/opds/v2";

    // Fetch recent books with their series
    // page is 0-indexed
    let (books, _total) = BookRepository::list_recently_added(
        &state.db,
        None, // All libraries
        false,
        0,
        pagination.page_size as u64,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let user_id = auth.user_id;
    let mut publications: Vec<Publication> = Vec::with_capacity(books.len());

    for book in &books {
        let title = book.title.clone().unwrap_or_else(|| "Untitled".to_string());

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
