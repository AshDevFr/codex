use super::super::dto::{
    AdjacentBooksResponse, BookDetailResponse, BookDto, BookFullMetadata, BookListRequest,
    BookListResponse, BookMetadataDto, BookMetadataLocks, FullBookListResponse, FullBookResponse,
    PaginationParams,
    book::{
        AddBookGenreRequest, AddBookTagRequest, BookAuthorDto, BookAwardDto, BookSortParam,
        BookType, BookTypeDto, SetBookGenresRequest, SetBookTagsRequest,
    },
    common::{
        DEFAULT_PAGE, DEFAULT_PAGE_SIZE, ListPaginationParams, MAX_PAGE_SIZE, PaginationLinkBuilder,
    },
    page::PageDto,
    series::{GenreDto, GenreListResponse, TagDto, TagListResponse},
};
use super::paginated_response;
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, GenreRepository, LibraryRepository, PageRepository,
    ReadProgressRepository, SeriesMetadataRepository, TagRepository,
};
use crate::require_permission;
use crate::services::FilterService;
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

/// Extract authors with a specific role from an `authors_json` string.
///
/// Parses the JSON array and returns names where the `role` field matches.
/// Returns an empty Vec if the JSON is None or invalid.
fn extract_authors_by_role(authors_json: &Option<String>, role: &str) -> Vec<String> {
    authors_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<Vec<serde_json::Value>>(json).ok())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let entry_role = entry.get("role").and_then(|r| r.as_str()).unwrap_or("");
                    if entry_role == role {
                        entry
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the first author name with a specific role from `authors_json`.
fn extract_first_author_by_role(authors_json: &Option<String>, role: &str) -> Option<String> {
    let authors = extract_authors_by_role(authors_json, role);
    if authors.is_empty() {
        None
    } else {
        Some(authors.join(", "))
    }
}

/// Build `authors_json` from individual role fields in a request DTO.
///
/// For each non-None field, creates author entries with the appropriate role.
/// Returns None if all fields are None.
fn build_authors_json_from_request(
    writer: &Option<String>,
    penciller: &Option<String>,
    inker: &Option<String>,
    colorist: &Option<String>,
    letterer: &Option<String>,
    cover_artist: &Option<String>,
    editor: &Option<String>,
) -> Option<String> {
    let fields: &[(&Option<String>, &str)] = &[
        (writer, "writer"),
        (penciller, "penciller"),
        (inker, "inker"),
        (colorist, "colorist"),
        (letterer, "letterer"),
        (cover_artist, "cover_artist"),
        (editor, "editor"),
    ];

    let mut entries = Vec::new();
    let mut has_any = false;

    for (field, role) in fields {
        if let Some(value) = field {
            has_any = true;
            for name in value.split(',') {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    entries.push(serde_json::json!({
                        "name": trimmed,
                        "role": role,
                    }));
                }
            }
        }
    }

    if !has_any {
        None
    } else {
        Some(serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string()))
    }
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// Query parameters for listing books
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct BookListQuery {
    /// Optional library filter
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Optional series filter
    #[serde(default)]
    pub series_id: Option<Uuid>,

    /// Page number (1-indexed, minimum 1)
    #[serde(default = "default_page")]
    pub page: u64,

    /// Number of items per page (max 100, default 50)
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// Sort parameter (format: "field,direction" e.g. "title,asc")
    #[serde(default)]
    pub sort: Option<String>,

    /// Return full data including metadata and locks.
    /// Default is false for backward compatibility.
    #[serde(default)]
    pub full: bool,
}

/// Query parameters for getting a single book
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct BookGetQuery {
    /// Return full data including metadata and locks.
    /// Default is false for backward compatibility.
    #[serde(default)]
    pub full: bool,
}

/// Helper function to convert books to DTOs with series information and read progress
pub async fn books_to_dtos(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    books: Vec<crate::db::entities::books::Model>,
) -> Result<Vec<BookDto>, ApiError> {
    // Collect unique series IDs and library IDs
    let series_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.series_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let library_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.library_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Collect book IDs for metadata lookup
    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();

    // Fetch series metadata (contains title, reading direction, etc.)
    let mut series_metadata_map: HashMap<Uuid, crate::db::entities::series_metadata::Model> =
        HashMap::new();
    for series_id in &series_ids {
        if let Ok(Some(metadata)) = SeriesMetadataRepository::get_by_series_id(db, *series_id).await
        {
            series_metadata_map.insert(*series_id, metadata);
        }
    }

    // Fetch book metadata for all books (contains title, number, etc.)
    let mut book_metadata_map: HashMap<Uuid, crate::db::entities::book_metadata::Model> =
        HashMap::new();
    for book_id in &book_ids {
        if let Ok(Some(metadata)) = BookMetadataRepository::get_by_book_id(db, *book_id).await {
            book_metadata_map.insert(*book_id, metadata);
        }
    }

    // Fetch libraries for name and default reading direction fallback
    let mut library_map: HashMap<Uuid, crate::db::entities::libraries::Model> = HashMap::new();
    for library_id in &library_ids {
        if let Ok(Some(library)) = LibraryRepository::get_by_id(db, *library_id).await {
            library_map.insert(*library_id, library);
        }
    }

    // Fetch read progress for all books
    let mut progress_map = HashMap::new();
    for book in &books {
        if let Ok(Some(progress)) =
            ReadProgressRepository::get_by_user_and_book(db, user_id, book.id).await
        {
            progress_map.insert(book.id, progress.into());
        }
    }

    // Convert books to DTOs
    let dtos = books
        .into_iter()
        .map(|book| {
            // Get library info
            let library = library_map.get(&book.library_id);
            let library_name = library
                .map(|l| l.name.clone())
                .unwrap_or_else(|| "Unknown Library".to_string());

            // Get series name from series_metadata.title
            let series_name = series_metadata_map
                .get(&book.series_id)
                .map(|m| m.title.clone())
                .unwrap_or_else(|| "Unknown Series".to_string());

            // Get book metadata
            let book_meta = book_metadata_map.get(&book.id);

            // Use title from book_metadata if available, otherwise use file_name (without extension)
            let title = book_meta.and_then(|m| m.title.clone()).unwrap_or_else(|| {
                // Extract filename without extension
                let file_name = &book.file_name;
                if let Some(pos) = file_name.rfind('.') {
                    file_name[..pos].to_string()
                } else {
                    file_name.clone()
                }
            });

            // Get title_sort from book_metadata
            let title_sort = book_meta.and_then(|m| m.title_sort.clone());

            // Get number from book_metadata
            let number = book_meta
                .and_then(|m| m.number)
                .map(|d| d.to_string().parse::<i32>().unwrap_or(0));

            let read_progress = progress_map.get(&book.id).cloned();

            // Determine effective reading direction: series metadata > library default
            let reading_direction = series_metadata_map
                .get(&book.series_id)
                .and_then(|m| m.reading_direction.clone())
                .or_else(|| library.map(|l| l.default_reading_direction.clone()));

            BookDto {
                id: book.id,
                library_id: book.library_id,
                library_name,
                series_id: book.series_id,
                series_name,
                title,
                title_sort,
                file_path: book.file_path,
                file_format: book.format,
                file_size: book.file_size,
                file_hash: book.file_hash,
                page_count: book.page_count,
                number,
                created_at: book.created_at,
                updated_at: book.updated_at,
                read_progress,
                analysis_error: book.analysis_error,
                deleted: book.deleted,
                analyzed: book.analyzed,
                reading_direction,
            }
        })
        .collect();

    Ok(dtos)
}

/// Helper function to convert books to FullBookResponse DTOs with batched queries
///
/// This function efficiently fetches all related data for multiple books in parallel
/// batched queries, avoiding N+1 query problems.
pub async fn books_to_full_dtos_batched(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    books: Vec<crate::db::entities::books::Model>,
) -> Result<Vec<FullBookResponse>, ApiError> {
    use chrono::Utc;

    if books.is_empty() {
        return Ok(vec![]);
    }

    // Collect IDs
    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();
    let series_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.series_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let library_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.library_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch all related data in parallel
    let (metadata_map, series_metadata_map, library_map, progress_map, genres_map, tags_map) = tokio::join!(
        BookMetadataRepository::get_by_book_ids(db, &book_ids),
        SeriesMetadataRepository::get_by_series_ids(db, &series_ids),
        LibraryRepository::get_by_ids(db, &library_ids),
        async {
            // Fetch read progress for all books
            let mut map = HashMap::new();
            for book_id in &book_ids {
                if let Ok(Some(progress)) =
                    ReadProgressRepository::get_by_user_and_book(db, user_id, *book_id).await
                {
                    map.insert(*book_id, progress.into());
                }
            }
            Ok::<_, anyhow::Error>(map)
        },
        GenreRepository::get_genres_for_book_ids(db, &book_ids),
        TagRepository::get_tags_for_book_ids(db, &book_ids),
    );

    // Handle errors
    let metadata_map = metadata_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book metadata: {}", e)))?;
    let series_metadata_map = series_metadata_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?;
    let library_map =
        library_map.map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;
    let progress_map = progress_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read progress: {}", e)))?;
    let genres_map = genres_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book genres: {}", e)))?;
    let tags_map =
        tags_map.map_err(|e| ApiError::Internal(format!("Failed to fetch book tags: {}", e)))?;

    // Convert to DTOs
    let mut results = Vec::with_capacity(books.len());

    for book in books {
        let book_id = book.id;

        // Get library info
        let library = library_map.get(&book.library_id);
        let library_name = library
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "Unknown Library".to_string());

        // Get series name from series_metadata.title
        let series_metadata = series_metadata_map.get(&book.series_id);
        let series_name = series_metadata
            .map(|m| m.title.clone())
            .unwrap_or_else(|| "Unknown Series".to_string());

        // Get book metadata (may not exist)
        let book_meta = metadata_map.get(&book_id);

        // Use title from book_metadata if available, otherwise use file_name (without extension)
        let title = book_meta.and_then(|m| m.title.clone()).unwrap_or_else(|| {
            let file_name = &book.file_name;
            if let Some(pos) = file_name.rfind('.') {
                file_name[..pos].to_string()
            } else {
                file_name.clone()
            }
        });

        // Get title_sort from book_metadata
        let title_sort = book_meta.and_then(|m| m.title_sort.clone());

        // Get number from book_metadata
        let number = book_meta
            .and_then(|m| m.number)
            .map(|d| d.to_string().parse::<i32>().unwrap_or(0));

        // Determine effective reading direction: series metadata > library default
        let reading_direction = series_metadata
            .and_then(|m| m.reading_direction.clone())
            .or_else(|| library.map(|l| l.default_reading_direction.clone()));

        let read_progress = progress_map.get(&book_id).cloned();

        // Build full metadata (even if no metadata record exists)
        let now = Utc::now();
        let full_metadata = if let Some(meta) = book_meta {
            // Parse authors JSON
            let authors = meta
                .authors_json
                .as_ref()
                .and_then(|json| serde_json::from_str::<Vec<BookAuthorDto>>(json).ok());
            // Parse awards JSON
            let awards = meta
                .awards_json
                .as_ref()
                .and_then(|json| serde_json::from_str::<Vec<BookAwardDto>>(json).ok());
            // Parse subjects (either JSON array or comma-separated)
            let subjects = meta.subjects.as_ref().map(|s| {
                if s.starts_with('[') {
                    serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| vec![s.clone()])
                } else {
                    s.split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect()
                }
            });
            // Parse custom metadata
            let custom_metadata = meta
                .custom_metadata
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok());

            BookFullMetadata {
                title: meta.title.clone(),
                title_sort: meta.title_sort.clone(),
                number: meta.number.map(|d| d.to_string()),
                summary: meta.summary.clone(),
                writer: extract_first_author_by_role(&meta.authors_json, "writer"),
                penciller: extract_first_author_by_role(&meta.authors_json, "penciller"),
                inker: extract_first_author_by_role(&meta.authors_json, "inker"),
                colorist: extract_first_author_by_role(&meta.authors_json, "colorist"),
                letterer: extract_first_author_by_role(&meta.authors_json, "letterer"),
                cover_artist: extract_first_author_by_role(&meta.authors_json, "cover_artist"),
                editor: extract_first_author_by_role(&meta.authors_json, "editor"),
                publisher: meta.publisher.clone(),
                imprint: meta.imprint.clone(),
                genre: meta.genre.clone(),
                language_iso: meta.language_iso.clone(),
                format_detail: meta.format_detail.clone(),
                black_and_white: meta.black_and_white,
                manga: meta.manga,
                year: meta.year,
                month: meta.month,
                day: meta.day,
                volume: meta.volume,
                count: meta.count,
                isbns: meta.isbns.clone(),
                // Phase 6 fields
                book_type: meta
                    .book_type
                    .as_ref()
                    .and_then(|s| s.parse::<BookType>().ok())
                    .map(BookTypeDto::from),
                subtitle: meta.subtitle.clone(),
                authors,
                translator: meta.translator.clone(),
                edition: meta.edition.clone(),
                original_title: meta.original_title.clone(),
                original_year: meta.original_year,
                series_position: meta
                    .series_position
                    .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                series_total: meta.series_total,
                subjects,
                awards,
                custom_metadata,
                release_date: None, // Computed from year/month/day if needed
                writers: extract_authors_by_role(&meta.authors_json, "writer"),
                pencillers: extract_authors_by_role(&meta.authors_json, "penciller"),
                inkers: extract_authors_by_role(&meta.authors_json, "inker"),
                colorists: extract_authors_by_role(&meta.authors_json, "colorist"),
                letterers: extract_authors_by_role(&meta.authors_json, "letterer"),
                cover_artists: extract_authors_by_role(&meta.authors_json, "cover_artist"),
                editors: extract_authors_by_role(&meta.authors_json, "editor"),
                locks: BookMetadataLocks {
                    title_lock: meta.title_lock,
                    title_sort_lock: meta.title_sort_lock,
                    number_lock: meta.number_lock,
                    summary_lock: meta.summary_lock,
                    writer_lock: meta.authors_json_lock,
                    penciller_lock: meta.authors_json_lock,
                    inker_lock: meta.authors_json_lock,
                    colorist_lock: meta.authors_json_lock,
                    letterer_lock: meta.authors_json_lock,
                    cover_artist_lock: meta.authors_json_lock,
                    editor_lock: meta.authors_json_lock,
                    publisher_lock: meta.publisher_lock,
                    imprint_lock: meta.imprint_lock,
                    genre_lock: meta.genre_lock,
                    language_iso_lock: meta.language_iso_lock,
                    format_detail_lock: meta.format_detail_lock,
                    black_and_white_lock: meta.black_and_white_lock,
                    manga_lock: meta.manga_lock,
                    year_lock: meta.year_lock,
                    month_lock: meta.month_lock,
                    day_lock: meta.day_lock,
                    volume_lock: meta.volume_lock,
                    count_lock: meta.count_lock,
                    isbns_lock: meta.isbns_lock,
                    book_type_lock: meta.book_type_lock,
                    subtitle_lock: meta.subtitle_lock,
                    authors_json_lock: meta.authors_json_lock,
                    translator_lock: meta.translator_lock,
                    edition_lock: meta.edition_lock,
                    original_title_lock: meta.original_title_lock,
                    original_year_lock: meta.original_year_lock,
                    series_position_lock: meta.series_position_lock,
                    series_total_lock: meta.series_total_lock,
                    subjects_lock: meta.subjects_lock,
                    awards_json_lock: meta.awards_json_lock,
                    custom_metadata_lock: meta.custom_metadata_lock,
                    cover_lock: meta.cover_lock,
                },
                created_at: meta.created_at,
                updated_at: meta.updated_at,
            }
        } else {
            // No metadata record - create empty metadata with all locks false
            BookFullMetadata {
                title: None,
                title_sort: None,
                number: None,
                summary: None,
                writer: None,
                penciller: None,
                inker: None,
                colorist: None,
                letterer: None,
                cover_artist: None,
                editor: None,
                publisher: None,
                imprint: None,
                genre: None,
                language_iso: None,
                format_detail: None,
                black_and_white: None,
                manga: None,
                year: None,
                month: None,
                day: None,
                volume: None,
                count: None,
                isbns: None,
                book_type: None,
                subtitle: None,
                authors: None,
                translator: None,
                edition: None,
                original_title: None,
                original_year: None,
                series_position: None,
                series_total: None,
                subjects: None,
                awards: None,
                custom_metadata: None,
                release_date: None,
                writers: vec![],
                pencillers: vec![],
                inkers: vec![],
                colorists: vec![],
                letterers: vec![],
                cover_artists: vec![],
                editors: vec![],
                locks: BookMetadataLocks {
                    title_lock: false,
                    title_sort_lock: false,
                    number_lock: false,
                    summary_lock: false,
                    writer_lock: false,
                    penciller_lock: false,
                    inker_lock: false,
                    colorist_lock: false,
                    letterer_lock: false,
                    cover_artist_lock: false,
                    editor_lock: false,
                    publisher_lock: false,
                    imprint_lock: false,
                    genre_lock: false,
                    language_iso_lock: false,
                    format_detail_lock: false,
                    black_and_white_lock: false,
                    manga_lock: false,
                    year_lock: false,
                    month_lock: false,
                    day_lock: false,
                    volume_lock: false,
                    count_lock: false,
                    isbns_lock: false,
                    book_type_lock: false,
                    subtitle_lock: false,
                    authors_json_lock: false,
                    translator_lock: false,
                    edition_lock: false,
                    original_title_lock: false,
                    original_year_lock: false,
                    series_position_lock: false,
                    series_total_lock: false,
                    subjects_lock: false,
                    awards_json_lock: false,
                    custom_metadata_lock: false,
                    cover_lock: false,
                },
                created_at: now,
                updated_at: now,
            }
        };

        // Build genre/tag DTOs from batch-fetched data
        let book_genres = genres_map
            .get(&book_id)
            .map(|gs| {
                gs.iter()
                    .map(|g| super::super::dto::series::GenreDto {
                        id: g.id,
                        name: g.name.clone(),
                        series_count: None,
                        created_at: g.created_at,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let book_tags = tags_map
            .get(&book_id)
            .map(|ts| {
                ts.iter()
                    .map(|t| super::super::dto::series::TagDto {
                        id: t.id,
                        name: t.name.clone(),
                        series_count: None,
                        created_at: t.created_at,
                    })
                    .collect()
            })
            .unwrap_or_default();

        results.push(FullBookResponse {
            id: book.id,
            library_id: book.library_id,
            library_name,
            series_id: book.series_id,
            series_name,
            title,
            title_sort,
            file_path: book.file_path,
            file_format: book.format,
            file_size: book.file_size,
            file_hash: book.file_hash,
            page_count: book.page_count,
            number,
            deleted: book.deleted,
            analyzed: book.analyzed,
            analysis_error: book.analysis_error,
            reading_direction,
            read_progress,
            metadata: full_metadata,
            genres: book_genres,
            tags: book_tags,
            created_at: book.created_at,
            updated_at: book.updated_at,
        });
    }

    Ok(results)
}

/// List books with pagination
#[utoipa::path(
    get,
    path = "/api/v1/books",
    params(BookListQuery),
    responses(
        (status = 200, description = "Paginated list of books (returns FullBookListResponse when full=true)", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1); // Treat page 0 as page 1 for backward compatibility
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };

    // Load content filter for sharing tags
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    // Fetch books based on filter
    let (books_list, total) = if let Some(ser_id) = query.series_id {
        // Check if the series is visible to the user
        if !content_filter.is_series_visible(ser_id) {
            let total_pages = 0u64;
            let link_builder =
                PaginationLinkBuilder::new("/api/v1/books", page, page_size, total_pages)
                    .with_param("series_id", &ser_id.to_string());
            let response =
                BookListResponse::with_builder(vec![], page, page_size, 0, &link_builder);
            return Ok(paginated_response(response, &link_builder));
        }

        // By default, don't include deleted books in API responses
        let books = BookRepository::list_by_series(&state.db, ser_id, false)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;
        let total = books.len() as u64;

        // Apply pagination manually (1-indexed: page 1 = offset 0)
        let offset = (page - 1) * page_size;
        let start = offset as usize;

        // If start is beyond the list, return empty results
        let paginated = if start >= books.len() {
            vec![]
        } else {
            let end = (start + page_size as usize).min(books.len());
            books[start..end].to_vec()
        };

        (paginated, total)
    } else {
        // List all books with pagination, then filter by sharing tags
        // Use i64::MAX as page_size to avoid SQLite integer overflow (u64::MAX > i64::MAX)
        let (books, _) = BookRepository::list_all(
            &state.db,
            false, // exclude deleted
            0,
            i64::MAX as u64,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

        // Filter books by sharing tags
        let filtered: Vec<_> = books
            .into_iter()
            .filter(|b| content_filter.is_book_visible(b.series_id))
            .collect();

        let total = filtered.len() as u64;

        // Apply pagination (1-indexed: page 1 = offset 0)
        let offset = (page - 1) * page_size;
        let start = offset as usize;

        let paginated = if start >= filtered.len() {
            vec![]
        } else {
            let end = (start + page_size as usize).min(filtered.len());
            filtered[start..end].to_vec()
        };

        (paginated, total)
    };

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/books", page, page_size, total_pages);
    if let Some(series_id) = query.series_id {
        link_builder = link_builder.with_param("series_id", &series_id.to_string());
    }
    if let Some(library_id) = query.library_id {
        link_builder = link_builder.with_param("library_id", &library_id.to_string());
    }
    if query.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Return full or basic response based on the full parameter
    if query.full {
        let full_dtos = books_to_full_dtos_batched(&state.db, auth.user_id, books_list).await?;
        let response =
            FullBookListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;
        let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// List books with advanced filtering
///
/// Supports complex filter conditions including nested AllOf/AnyOf logic,
/// genre/tag filtering with include/exclude, and more.
///
/// Pagination parameters (page, pageSize, sort, full) are passed as query parameters.
/// Filter conditions are passed in the request body.
#[utoipa::path(
    post,
    path = "/api/v1/books/list",
    params(ListPaginationParams),
    request_body = BookListRequest,
    responses(
        (status = 200, description = "Paginated list of filtered books (returns FullBookListResponse when full=true)", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_books_filtered(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(pagination): Query<ListPaginationParams>,
    Json(request): Json<BookListRequest>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed, from query params)
    let (page, page_size) = pagination.validated();
    // Convert to 0-indexed offset for repository methods
    let offset = (page - 1) * page_size;

    // If there's a condition, evaluate it to get matching book IDs (with user context for ReadStatus filtering)
    let filtered_ids: Option<HashSet<Uuid>> = if let Some(ref condition) = request.condition {
        let matching = FilterService::get_matching_books_for_user(
            &state.db,
            condition,
            None,
            Some(auth.user_id),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to evaluate filter: {}", e)))?;
        Some(matching)
    } else {
        None
    };

    // Fetch books based on filter results and full-text search
    let (books_list, total) = match (&filtered_ids, &request.full_text_search) {
        // Full-text search with filter conditions
        (Some(ids), Some(search_query)) if !search_query.trim().is_empty() => {
            if ids.is_empty() {
                (vec![], 0)
            } else {
                let id_vec: Vec<Uuid> = ids.iter().cloned().collect();
                BookRepository::search_by_title(
                    &state.db,
                    search_query,
                    None,
                    Some(&id_vec),
                    request.include_deleted,
                    Some((offset, page_size)),
                )
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to search books: {}", e)))?
            }
        }
        // Full-text search without filter conditions
        (None, Some(search_query)) if !search_query.trim().is_empty() => {
            BookRepository::search_by_title(
                &state.db,
                search_query,
                None,
                None,
                request.include_deleted,
                Some((offset, page_size)),
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to search books: {}", e)))?
        }
        // Filter conditions only (no full-text search)
        (Some(ids), _) => {
            if ids.is_empty() {
                (vec![], 0)
            } else {
                let id_vec: Vec<Uuid> = ids.iter().cloned().collect();
                BookRepository::list_by_ids(
                    &state.db,
                    &id_vec,
                    request.include_deleted,
                    offset,
                    page_size,
                )
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
            }
        }
        // No filter and no full-text search
        (None, _) => {
            BookRepository::list_all(&state.db, request.include_deleted, offset, page_size)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
        }
    };

    // Build pagination links with query params
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/books/list", page, page_size, total_pages);
    if let Some(ref sort_str) = pagination.sort {
        link_builder = link_builder.with_param("sort", sort_str);
    }
    if pagination.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Return full or basic response based on the full parameter
    if pagination.full {
        let full_dtos = books_to_full_dtos_batched(&state.db, auth.user_id, books_list).await?;
        let response =
            FullBookListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;
        let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// Get book by ID
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        BookGetQuery
    ),
    responses(
        (status = 200, description = "Book details (returns FullBookResponse when full=true)", body = BookDetailResponse),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_book(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Query(query): Query<BookGetQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check sharing tag access for the book's series
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if !content_filter.is_book_visible(book.series_id) {
        return Err(ApiError::NotFound("Book not found".to_string()));
    }

    // Return full or basic response based on the full parameter
    if query.full {
        let mut full_dtos = books_to_full_dtos_batched(&state.db, auth.user_id, vec![book]).await?;
        let full_book = full_dtos.pop().unwrap(); // Safe because we just passed a single book
        Ok(Json(full_book).into_response())
    } else {
        // Try to fetch metadata - now contains title, title_sort, number
        let metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
            .await
            .ok()
            .flatten()
            .map(|meta| {
                // Parse authors JSON
                let authors = meta
                    .authors_json
                    .as_ref()
                    .and_then(|json| serde_json::from_str::<Vec<BookAuthorDto>>(json).ok());
                // Parse awards JSON
                let awards = meta
                    .awards_json
                    .as_ref()
                    .and_then(|json| serde_json::from_str::<Vec<BookAwardDto>>(json).ok());
                // Parse subjects (either JSON array or comma-separated)
                let subjects = meta.subjects.as_ref().map(|s| {
                    if s.starts_with('[') {
                        serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| vec![s.clone()])
                    } else {
                        s.split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect()
                    }
                });
                // Parse custom metadata
                let custom_metadata = meta
                    .custom_metadata
                    .as_ref()
                    .and_then(|json| serde_json::from_str(json).ok());

                BookMetadataDto {
                    id: meta.id,
                    book_id: meta.book_id,
                    title: meta.title,
                    series: None, // Series name is fetched separately via series_metadata
                    number: meta.number.map(|d| d.to_string()),
                    summary: meta.summary,
                    publisher: meta.publisher,
                    imprint: meta.imprint,
                    genre: meta.genre,
                    page_count: None, // Page count is in books table, not metadata
                    language_iso: meta.language_iso,
                    release_date: None, // Release date is computed from year/month/day
                    writers: extract_authors_by_role(&meta.authors_json, "writer"),
                    pencillers: extract_authors_by_role(&meta.authors_json, "penciller"),
                    inkers: extract_authors_by_role(&meta.authors_json, "inker"),
                    colorists: extract_authors_by_role(&meta.authors_json, "colorist"),
                    letterers: extract_authors_by_role(&meta.authors_json, "letterer"),
                    cover_artists: extract_authors_by_role(&meta.authors_json, "cover_artist"),
                    editors: extract_authors_by_role(&meta.authors_json, "editor"),
                    // New Phase 6 fields
                    book_type: meta
                        .book_type
                        .as_ref()
                        .and_then(|s| s.parse::<BookType>().ok())
                        .map(BookTypeDto::from),
                    subtitle: meta.subtitle,
                    authors,
                    translator: meta.translator,
                    edition: meta.edition,
                    original_title: meta.original_title,
                    original_year: meta.original_year,
                    series_position: meta
                        .series_position
                        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                    series_total: meta.series_total,
                    subjects,
                    awards,
                    custom_metadata,
                    // Raw metadata fields (for edit form)
                    format_detail: meta.format_detail,
                    black_and_white: meta.black_and_white,
                    manga: meta.manga,
                    year: meta.year,
                    month: meta.month,
                    day: meta.day,
                    volume: meta.volume,
                    count: meta.count,
                    isbns: meta.isbns,
                }
            });

        let mut dtos = books_to_dtos(&state.db, auth.user_id, vec![book]).await?;
        let book_dto = dtos.pop().unwrap(); // Safe because we just passed a single book

        let response = BookDetailResponse {
            book: book_dto,
            metadata,
        };

        Ok(Json(response).into_response())
    }
}

/// Update book core fields (title, number)
///
/// Partially updates book_metadata fields. Only provided fields will be updated.
/// Absent fields are unchanged. Explicitly null fields will be cleared.
/// When a field is set to a non-null value, it is automatically locked.
#[utoipa::path(
    patch,
    path = "/api/v1/books/{book_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = PatchBookRequest,
    responses(
        (status = 200, description = "Book updated successfully", body = BookUpdateResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn patch_book(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<PatchBookRequest>,
) -> Result<Json<BookUpdateResponse>, ApiError> {
    use sea_orm::prelude::Decimal;

    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let now = Utc::now();
    let mut has_changes = false;

    // Get or create book_metadata record
    let existing_meta = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let updated_meta = if let Some(existing) = existing_meta {
        // Update existing metadata record
        let mut active: book_metadata::ActiveModel = existing.into();

        // Update title if provided (also lock it when set to non-null)
        if let Some(opt) = request.title.into_nested_option() {
            active.title = Set(opt.clone());
            if opt.is_some() {
                active.title_lock = Set(true);
            }
            has_changes = true;
        }

        // Update number if provided (convert f64 to Decimal, also lock when set)
        if let Some(opt) = request.number.into_nested_option() {
            let decimal_opt = opt.and_then(Decimal::from_f64_retain);
            active.number = Set(decimal_opt);
            if opt.is_some() {
                active.number_lock = Set(true);
            }
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
        }

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update book metadata: {}", e)))?
    } else {
        // Create new metadata record with provided fields
        has_changes = true;
        let title_opt = request.title.into_option();
        let number_opt = request.number.into_option();
        let decimal_opt = number_opt.and_then(Decimal::from_f64_retain);

        let active = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(title_opt.clone()),
            title_sort: Set(None),
            number: Set(decimal_opt),
            summary: Set(None),
            publisher: Set(None),
            imprint: Set(None),
            genre: Set(None),
            language_iso: Set(None),
            format_detail: Set(None),
            black_and_white: Set(None),
            manga: Set(None),
            year: Set(None),
            month: Set(None),
            day: Set(None),
            volume: Set(None),
            count: Set(None),
            isbns: Set(None),
            // New Phase 1 fields
            book_type: Set(None),
            subtitle: Set(None),
            authors_json: Set(None),
            translator: Set(None),
            edition: Set(None),
            original_title: Set(None),
            original_year: Set(None),
            series_position: Set(None),
            series_total: Set(None),
            subjects: Set(None),
            awards_json: Set(None),
            custom_metadata: Set(None),
            // Auto-lock fields that are set
            title_lock: Set(title_opt.is_some()),
            title_sort_lock: Set(false),
            number_lock: Set(number_opt.is_some()),
            summary_lock: Set(false),
            publisher_lock: Set(false),
            imprint_lock: Set(false),
            genre_lock: Set(false),
            language_iso_lock: Set(false),
            format_detail_lock: Set(false),
            black_and_white_lock: Set(false),
            manga_lock: Set(false),
            year_lock: Set(false),
            month_lock: Set(false),
            day_lock: Set(false),
            volume_lock: Set(false),
            count_lock: Set(false),
            isbns_lock: Set(false),
            // New Phase 1 lock fields
            book_type_lock: Set(false),
            subtitle_lock: Set(false),
            authors_json_lock: Set(false),
            translator_lock: Set(false),
            edition_lock: Set(false),
            original_title_lock: Set(false),
            original_year_lock: Set(false),
            series_position_lock: Set(false),
            series_total_lock: Set(false),
            subjects_lock: Set(false),
            awards_json_lock: Set(false),
            custom_metadata_lock: Set(false),
            cover_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active
            .insert(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create book metadata: {}", e)))?
    };

    // Emit update event
    if has_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::BookUpdated {
                book_id,
                series_id: book.series_id,
                library_id: book.library_id,
                fields: Some(vec!["title".to_string(), "number".to_string()]),
            },
            timestamp: now,
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(BookUpdateResponse {
        id: book_id,
        title: updated_meta.title,
        number: updated_meta
            .number
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        updated_at: updated_meta.updated_at,
    }))
}

/// Get adjacent books in the same series
///
/// Returns the previous and next books relative to the requested book,
/// ordered by book number within the series.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/adjacent",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
    ),
    responses(
        (status = 200, description = "Adjacent books", body = AdjacentBooksResponse),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_adjacent_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<AdjacentBooksResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let (prev, next) = BookRepository::get_adjacent_in_series(&state.db, book_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Book not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to get adjacent books: {}", e))
            }
        })?;

    // Convert to DTOs
    let prev_dto = if let Some(book) = prev {
        let mut dtos = books_to_dtos(&state.db, auth.user_id, vec![book]).await?;
        dtos.pop()
    } else {
        None
    };

    let next_dto = if let Some(book) = next {
        let mut dtos = books_to_dtos(&state.db, auth.user_id, vec![book]).await?;
        dtos.pop()
    } else {
        None
    };

    Ok(Json(AdjacentBooksResponse {
        prev: prev_dto,
        next: next_dto,
    }))
}

/// List books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_library_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Parse sort parameter
    let sort = query
        .sort
        .as_ref()
        .map(|s| BookSortParam::parse(s))
        .unwrap_or_default();

    // Use database-level sorting for all sort types
    let (books_list, total) = BookRepository::list_by_library_sorted(
        &state.db, library_id, &sort, false, // exclude deleted
        offset, page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch library books: {}", e)))?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder = PaginationLinkBuilder::new(
        &format!("/api/v1/libraries/{}/books", library_id),
        page,
        page_size,
        total_pages,
    );
    if query.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Return full or basic response based on the full parameter
    if query.full {
        let full_dtos = books_to_full_dtos_batched(&state.db, auth.user_id, books_list).await?;
        let response =
            FullBookListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;
        let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// List books with reading progress (in-progress books)
#[utoipa::path(
    get,
    path = "/api/v1/books/in-progress",
    params(BookListQuery),
    responses(
        (status = 200, description = "Paginated list of in-progress books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_in_progress_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Fetch books with reading progress (not completed)
    let (books_list, total) = BookRepository::list_with_progress(
        &state.db,
        auth.user_id,
        query.library_id,
        Some(false), // only in-progress (not completed)
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress books: {}", e)))?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/books/in-progress", page, page_size, total_pages);
    if let Some(library_id) = query.library_id {
        link_builder = link_builder.with_param("library_id", &library_id.to_string());
    }
    if query.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Return full or basic response based on the full parameter
    if query.full {
        let full_dtos = books_to_full_dtos_batched(&state.db, auth.user_id, books_list).await?;
        let response =
            FullBookListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;
        let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// List books with reading progress in a specific library (in-progress books)
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/in-progress",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of in-progress books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_library_in_progress_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Fetch books with reading progress (not completed) in this library
    let (books_list, total) = BookRepository::list_with_progress(
        &state.db,
        auth.user_id,
        Some(library_id),
        Some(false), // only in-progress (not completed)
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let link_builder = PaginationLinkBuilder::new(
        &format!("/api/v1/libraries/{}/books/in-progress", library_id),
        page,
        page_size,
        total_pages,
    );

    let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// List on-deck books (next unread book in series where user has completed at least one book)
#[utoipa::path(
    get,
    path = "/api/v1/books/on-deck",
    params(BookListQuery),
    responses(
        (status = 200, description = "Paginated list of on-deck books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_on_deck_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Fetch on-deck books
    let (books_list, total) =
        BookRepository::list_on_deck(&state.db, auth.user_id, query.library_id, offset, page_size)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch on-deck books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/books/on-deck", page, page_size, total_pages);
    if let Some(library_id) = query.library_id {
        link_builder = link_builder.with_param("library_id", &library_id.to_string());
    }

    let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// List on-deck books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/on-deck",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of on-deck books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_library_on_deck_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Fetch on-deck books in this library
    let (books_list, total) =
        BookRepository::list_on_deck(&state.db, auth.user_id, Some(library_id), offset, page_size)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch on-deck books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let link_builder = PaginationLinkBuilder::new(
        &format!("/api/v1/libraries/{}/books/on-deck", library_id),
        page,
        page_size,
        total_pages,
    );

    let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// List recently added books
#[utoipa::path(
    get,
    path = "/api/v1/books/recently-added",
    params(BookListQuery),
    responses(
        (status = 200, description = "Paginated list of recently added books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_recently_added_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Fetch recently added books
    let (books_list, total) = BookRepository::list_recently_added(
        &state.db,
        query.library_id,
        false, // exclude deleted
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch recently added books: {}", e)))?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/books/recently-added", page, page_size, total_pages);
    if let Some(library_id) = query.library_id {
        link_builder = link_builder.with_param("library_id", &library_id.to_string());
    }
    if query.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Return full or basic response based on the full parameter
    if query.full {
        let full_dtos = books_to_full_dtos_batched(&state.db, auth.user_id, books_list).await?;
        let response =
            FullBookListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;
        let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// List recently added books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/recently-added",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of recently added books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_library_recently_added_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };
    let offset = (page - 1) * page_size;

    // Fetch recently added books in this library
    let (books_list, total) = BookRepository::list_recently_added(
        &state.db,
        Some(library_id),
        false, // exclude deleted
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch recently added books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let link_builder = PaginationLinkBuilder::new(
        &format!("/api/v1/libraries/{}/books/recently-added", library_id),
        page,
        page_size,
        total_pages,
    );

    let response = BookListResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// Query parameters for recently read books
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct RecentBooksQuery {
    /// Maximum number of books to return (default: 50)
    #[serde(default = "default_recent_limit")]
    pub limit: u64,

    /// Filter by library ID (optional)
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

fn default_recent_limit() -> u64 {
    50
}

/// List recently read books (ordered by last read activity)
#[utoipa::path(
    get,
    path = "/api/v1/books/recently-read",
    params(RecentBooksQuery),
    responses(
        (status = 200, description = "List of recently read books", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_recently_read_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<RecentBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let books_list =
        BookRepository::list_recently_read(&state.db, auth.user_id, query.library_id, query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently read books: {}", e))
            })?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    Ok(Json(dtos))
}

/// List recently read books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/recently-read",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        ("limit" = Option<u64>, Query, description = "Maximum number of books to return (default: 50)")
    ),
    responses(
        (status = 200, description = "List of recently read books in library", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_library_recently_read_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<RecentBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let books_list =
        BookRepository::list_recently_read(&state.db, auth.user_id, Some(library_id), query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently read books: {}", e))
            })?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    Ok(Json(dtos))
}

/// Download book file
///
/// Streams the original book file (CBZ, CBR, EPUB, PDF) for download.
/// Used by OPDS clients for acquisition links.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/file",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "Book file", content_type = "application/octet-stream"),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_book_file(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Fetch book from database
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check sharing tag access for the book's series
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if !content_filter.is_book_visible(book.series_id) {
        return Err(ApiError::NotFound("Book not found".to_string()));
    }

    // Check if file exists
    let file_path = std::path::Path::new(&book.file_path);
    if !file_path.exists() {
        return Err(ApiError::NotFound(
            "Book file not found on disk".to_string(),
        ));
    }

    // Get file metadata for content-length
    let metadata = tokio::fs::metadata(&book.file_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read file metadata: {}", e)))?;

    // Determine content type based on format
    let content_type = match book.format.to_lowercase().as_str() {
        "cbz" | "zip" => "application/zip",
        "cbr" | "rar" => "application/x-rar-compressed",
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };

    // Open file for streaming
    let file = tokio::fs::File::open(&book.file_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to open book file: {}", e)))?;

    // Create a stream from the file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Build response with appropriate headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", book.file_name),
        )
        .body(body)
        .unwrap())
}

// ============================================================================
// Book Metadata Endpoints
// ============================================================================

use crate::api::routes::v1::dto::{
    BookMetadataResponse, PatchBookMetadataRequest, ReplaceBookMetadataRequest,
};
use crate::db::entities::book_metadata;
use crate::events::{EntityChangeEvent, EntityEvent};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};

/// Replace all book metadata (PUT)
///
/// Completely replaces all metadata fields. Omitted or null fields will be cleared.
/// If no metadata record exists, one will be created.
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/metadata",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = ReplaceBookMetadataRequest,
    responses(
        (status = 200, description = "Metadata replaced successfully", body = BookMetadataResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn replace_book_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<ReplaceBookMetadataRequest>,
) -> Result<Json<BookMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check if metadata record exists
    let existing = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let now = Utc::now();
    let updated = if let Some(existing) = existing {
        // Update existing record - full replacement
        // Auto-lock fields that are being set to non-null values
        let mut active: book_metadata::ActiveModel = existing.into();

        active.summary = Set(request.summary.clone());
        // Build authors_json from individual role fields
        active.authors_json = Set(build_authors_json_from_request(
            &request.writer,
            &request.penciller,
            &request.inker,
            &request.colorist,
            &request.letterer,
            &request.cover_artist,
            &request.editor,
        ));
        active.publisher = Set(request.publisher.clone());
        active.imprint = Set(request.imprint.clone());
        active.genre = Set(request.genre.clone());
        active.language_iso = Set(request.language_iso.clone());
        active.format_detail = Set(request.format_detail.clone());
        active.black_and_white = Set(request.black_and_white);
        active.manga = Set(request.manga);
        active.year = Set(request.year);
        active.month = Set(request.month);
        active.day = Set(request.day);
        active.volume = Set(request.volume);
        active.count = Set(request.count);
        active.isbns = Set(request.isbns.clone());

        // Auto-lock fields that are being set to non-null values
        if request.summary.is_some() {
            active.summary_lock = Set(true);
        }
        if request.writer.is_some()
            || request.penciller.is_some()
            || request.inker.is_some()
            || request.colorist.is_some()
            || request.letterer.is_some()
            || request.cover_artist.is_some()
            || request.editor.is_some()
        {
            active.authors_json_lock = Set(true);
        }
        if request.publisher.is_some() {
            active.publisher_lock = Set(true);
        }
        if request.imprint.is_some() {
            active.imprint_lock = Set(true);
        }
        if request.genre.is_some() {
            active.genre_lock = Set(true);
        }
        if request.language_iso.is_some() {
            active.language_iso_lock = Set(true);
        }
        if request.format_detail.is_some() {
            active.format_detail_lock = Set(true);
        }
        if request.black_and_white.is_some() {
            active.black_and_white_lock = Set(true);
        }
        if request.manga.is_some() {
            active.manga_lock = Set(true);
        }
        if request.year.is_some() {
            active.year_lock = Set(true);
        }
        if request.month.is_some() {
            active.month_lock = Set(true);
        }
        if request.day.is_some() {
            active.day_lock = Set(true);
        }
        if request.volume.is_some() {
            active.volume_lock = Set(true);
        }
        if request.count.is_some() {
            active.count_lock = Set(true);
        }
        if request.isbns.is_some() {
            active.isbns_lock = Set(true);
        }

        active.updated_at = Set(now);

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update metadata: {}", e)))?
    } else {
        // Create new record with locks set for non-null fields
        let new_authors_json = build_authors_json_from_request(
            &request.writer,
            &request.penciller,
            &request.inker,
            &request.colorist,
            &request.letterer,
            &request.cover_artist,
            &request.editor,
        );
        let any_author_set = new_authors_json.is_some();

        let active = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(None), // Title is not set via this endpoint (use PATCH /books/{id})
            title_sort: Set(None), // Title sort is not set via this endpoint
            number: Set(None), // Number is not set via this endpoint (use PATCH /books/{id})
            summary: Set(request.summary.clone()),
            publisher: Set(request.publisher.clone()),
            imprint: Set(request.imprint.clone()),
            genre: Set(request.genre.clone()),
            language_iso: Set(request.language_iso.clone()),
            format_detail: Set(request.format_detail.clone()),
            black_and_white: Set(request.black_and_white),
            manga: Set(request.manga),
            year: Set(request.year),
            month: Set(request.month),
            day: Set(request.day),
            volume: Set(request.volume),
            count: Set(request.count),
            isbns: Set(request.isbns.clone()),
            // New Phase 1 fields
            book_type: Set(None),
            subtitle: Set(None),
            authors_json: Set(new_authors_json),
            translator: Set(None),
            edition: Set(None),
            original_title: Set(None),
            original_year: Set(None),
            series_position: Set(None),
            series_total: Set(None),
            subjects: Set(None),
            awards_json: Set(None),
            custom_metadata: Set(None),
            // Set locks for non-null fields
            title_lock: Set(false),
            title_sort_lock: Set(false),
            number_lock: Set(false),
            summary_lock: Set(request.summary.is_some()),
            publisher_lock: Set(request.publisher.is_some()),
            imprint_lock: Set(request.imprint.is_some()),
            genre_lock: Set(request.genre.is_some()),
            language_iso_lock: Set(request.language_iso.is_some()),
            format_detail_lock: Set(request.format_detail.is_some()),
            black_and_white_lock: Set(request.black_and_white.is_some()),
            manga_lock: Set(request.manga.is_some()),
            year_lock: Set(request.year.is_some()),
            month_lock: Set(request.month.is_some()),
            day_lock: Set(request.day.is_some()),
            volume_lock: Set(request.volume.is_some()),
            count_lock: Set(request.count.is_some()),
            isbns_lock: Set(request.isbns.is_some()),
            // New Phase 1 lock fields
            book_type_lock: Set(false),
            subtitle_lock: Set(false),
            authors_json_lock: Set(any_author_set),
            translator_lock: Set(false),
            edition_lock: Set(false),
            original_title_lock: Set(false),
            original_year_lock: Set(false),
            series_position_lock: Set(false),
            series_total_lock: Set(false),
            subjects_lock: Set(false),
            awards_json_lock: Set(false),
            custom_metadata_lock: Set(false),
            cover_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active
            .insert(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create metadata: {}", e)))?
    };

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["metadata".to_string()]),
        },
        timestamp: now,
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    // Parse new fields from the updated record
    let authors = updated
        .authors_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<Vec<BookAuthorDto>>(json).ok());
    let awards = updated
        .awards_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<Vec<BookAwardDto>>(json).ok());
    let subjects = updated.subjects.as_ref().map(|s| {
        if s.starts_with('[') {
            serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| vec![s.clone()])
        } else {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        }
    });
    let custom_metadata = updated
        .custom_metadata
        .as_ref()
        .and_then(|json| serde_json::from_str(json).ok());

    Ok(Json(BookMetadataResponse {
        book_id: updated.book_id,
        summary: updated.summary,
        writer: extract_first_author_by_role(&updated.authors_json, "writer"),
        penciller: extract_first_author_by_role(&updated.authors_json, "penciller"),
        inker: extract_first_author_by_role(&updated.authors_json, "inker"),
        colorist: extract_first_author_by_role(&updated.authors_json, "colorist"),
        letterer: extract_first_author_by_role(&updated.authors_json, "letterer"),
        cover_artist: extract_first_author_by_role(&updated.authors_json, "cover_artist"),
        editor: extract_first_author_by_role(&updated.authors_json, "editor"),
        publisher: updated.publisher,
        imprint: updated.imprint,
        genre: updated.genre,
        language_iso: updated.language_iso,
        format_detail: updated.format_detail,
        black_and_white: updated.black_and_white,
        manga: updated.manga,
        year: updated.year,
        month: updated.month,
        day: updated.day,
        volume: updated.volume,
        count: updated.count,
        isbns: updated.isbns,
        // New Phase 6 fields
        book_type: updated
            .book_type
            .as_ref()
            .and_then(|s| s.parse::<BookType>().ok())
            .map(BookTypeDto::from),
        subtitle: updated.subtitle,
        authors,
        translator: updated.translator,
        edition: updated.edition,
        original_title: updated.original_title,
        original_year: updated.original_year,
        series_position: updated
            .series_position
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        series_total: updated.series_total,
        subjects,
        awards,
        custom_metadata,
        locks: BookMetadataLocks {
            title_lock: updated.title_lock,
            title_sort_lock: updated.title_sort_lock,
            number_lock: updated.number_lock,
            summary_lock: updated.summary_lock,
            writer_lock: updated.authors_json_lock,
            penciller_lock: updated.authors_json_lock,
            inker_lock: updated.authors_json_lock,
            colorist_lock: updated.authors_json_lock,
            letterer_lock: updated.authors_json_lock,
            cover_artist_lock: updated.authors_json_lock,
            editor_lock: updated.authors_json_lock,
            publisher_lock: updated.publisher_lock,
            imprint_lock: updated.imprint_lock,
            genre_lock: updated.genre_lock,
            language_iso_lock: updated.language_iso_lock,
            format_detail_lock: updated.format_detail_lock,
            black_and_white_lock: updated.black_and_white_lock,
            manga_lock: updated.manga_lock,
            year_lock: updated.year_lock,
            month_lock: updated.month_lock,
            day_lock: updated.day_lock,
            volume_lock: updated.volume_lock,
            count_lock: updated.count_lock,
            isbns_lock: updated.isbns_lock,
            book_type_lock: updated.book_type_lock,
            subtitle_lock: updated.subtitle_lock,
            authors_json_lock: updated.authors_json_lock,
            translator_lock: updated.translator_lock,
            edition_lock: updated.edition_lock,
            original_title_lock: updated.original_title_lock,
            original_year_lock: updated.original_year_lock,
            series_position_lock: updated.series_position_lock,
            series_total_lock: updated.series_total_lock,
            subjects_lock: updated.subjects_lock,
            awards_json_lock: updated.awards_json_lock,
            custom_metadata_lock: updated.custom_metadata_lock,
            cover_lock: updated.cover_lock,
        },
        updated_at: updated.updated_at,
    }))
}

/// Partially update book metadata (PATCH)
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
/// If no metadata record exists, one will be created with the provided fields.
#[utoipa::path(
    patch,
    path = "/api/v1/books/{book_id}/metadata",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = PatchBookMetadataRequest,
    responses(
        (status = 200, description = "Metadata updated successfully", body = BookMetadataResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn patch_book_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<PatchBookMetadataRequest>,
) -> Result<Json<BookMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check if metadata record exists
    let existing = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let now = Utc::now();
    let mut has_changes = false;

    let updated = if let Some(existing) = existing {
        // Partial update existing record with auto-locking
        let existing_authors_json = existing.authors_json.clone();
        let mut active: book_metadata::ActiveModel = existing.into();

        if let Some(opt) = request.summary.into_nested_option() {
            active.summary = Set(opt.clone());
            if opt.is_some() {
                active.summary_lock = Set(true);
            }
            has_changes = true;
        }
        // Handle individual author role fields by merging into authors_json.
        // Each role field (writer, penciller, etc.) updates only its entries
        // within the existing authors_json, preserving other roles.
        {
            let role_fields: Vec<(&str, Option<Option<String>>)> = vec![
                ("writer", request.writer.into_nested_option()),
                ("penciller", request.penciller.into_nested_option()),
                ("inker", request.inker.into_nested_option()),
                ("colorist", request.colorist.into_nested_option()),
                ("letterer", request.letterer.into_nested_option()),
                ("cover_artist", request.cover_artist.into_nested_option()),
                ("editor", request.editor.into_nested_option()),
            ];

            let mut any_author_change = false;
            // Parse existing authors_json into a mutable list
            let mut entries: Vec<serde_json::Value> = existing_authors_json
                .as_deref()
                .and_then(|j| serde_json::from_str(j).ok())
                .unwrap_or_default();

            for (role, patch_value) in role_fields {
                if let Some(opt) = patch_value {
                    any_author_change = true;
                    // Remove existing entries for this role
                    entries
                        .retain(|e| e.get("role").and_then(|r| r.as_str()).unwrap_or("") != role);
                    // Add new entries if the value is non-null
                    if let Some(ref value) = opt {
                        for name in value.split(',') {
                            let trimmed = name.trim();
                            if !trimmed.is_empty() {
                                entries.push(serde_json::json!({
                                    "name": trimmed,
                                    "role": role,
                                }));
                            }
                        }
                    }
                }
            }

            if any_author_change {
                let new_json = if entries.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string()))
                };
                active.authors_json = Set(new_json.clone());
                if new_json.is_some() {
                    active.authors_json_lock = Set(true);
                }
                has_changes = true;
            }
        }
        if let Some(opt) = request.publisher.into_nested_option() {
            active.publisher = Set(opt.clone());
            if opt.is_some() {
                active.publisher_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.imprint.into_nested_option() {
            active.imprint = Set(opt.clone());
            if opt.is_some() {
                active.imprint_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.genre.into_nested_option() {
            active.genre = Set(opt.clone());
            if opt.is_some() {
                active.genre_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.language_iso.into_nested_option() {
            active.language_iso = Set(opt.clone());
            if opt.is_some() {
                active.language_iso_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.format_detail.into_nested_option() {
            active.format_detail = Set(opt.clone());
            if opt.is_some() {
                active.format_detail_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.black_and_white.into_nested_option() {
            active.black_and_white = Set(opt);
            if opt.is_some() {
                active.black_and_white_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.manga.into_nested_option() {
            active.manga = Set(opt);
            if opt.is_some() {
                active.manga_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.year.into_nested_option() {
            active.year = Set(opt);
            if opt.is_some() {
                active.year_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.month.into_nested_option() {
            active.month = Set(opt);
            if opt.is_some() {
                active.month_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.day.into_nested_option() {
            active.day = Set(opt);
            if opt.is_some() {
                active.day_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.volume.into_nested_option() {
            active.volume = Set(opt);
            if opt.is_some() {
                active.volume_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.count.into_nested_option() {
            active.count = Set(opt);
            if opt.is_some() {
                active.count_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.isbns.into_nested_option() {
            active.isbns = Set(opt.clone());
            if opt.is_some() {
                active.isbns_lock = Set(true);
            }
            has_changes = true;
        }
        // New Phase 6 fields
        if let Some(opt) = request.book_type.into_nested_option() {
            let book_type_str = opt.as_ref().map(|bt| bt.to_string());
            active.book_type = Set(book_type_str);
            if opt.is_some() {
                active.book_type_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.subtitle.into_nested_option() {
            active.subtitle = Set(opt.clone());
            if opt.is_some() {
                active.subtitle_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.authors.into_nested_option() {
            let authors_json = opt
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());
            active.authors_json = Set(authors_json);
            if opt.is_some() {
                active.authors_json_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.translator.into_nested_option() {
            active.translator = Set(opt.clone());
            if opt.is_some() {
                active.translator_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.edition.into_nested_option() {
            active.edition = Set(opt.clone());
            if opt.is_some() {
                active.edition_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.original_title.into_nested_option() {
            active.original_title = Set(opt.clone());
            if opt.is_some() {
                active.original_title_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.original_year.into_nested_option() {
            active.original_year = Set(opt);
            if opt.is_some() {
                active.original_year_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.series_position.into_nested_option() {
            active.series_position = Set(opt.and_then(sea_orm::prelude::Decimal::from_f64_retain));
            if opt.is_some() {
                active.series_position_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.series_total.into_nested_option() {
            active.series_total = Set(opt);
            if opt.is_some() {
                active.series_total_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.subjects.into_nested_option() {
            let subjects_str = opt
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());
            active.subjects = Set(subjects_str);
            if opt.is_some() {
                active.subjects_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.awards.into_nested_option() {
            let awards_json = opt
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());
            active.awards_json = Set(awards_json);
            if opt.is_some() {
                active.awards_json_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.custom_metadata.into_nested_option() {
            let custom_str = opt
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());
            active.custom_metadata = Set(custom_str);
            if opt.is_some() {
                active.custom_metadata_lock = Set(true);
            }
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
        }

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update metadata: {}", e)))?
    } else {
        // Create new record with provided fields and auto-locking
        has_changes = true;
        let summary_opt = request.summary.into_option();
        // Merge individual author role fields into authors_json.
        // If the new-style authors field is provided, it takes precedence.
        // Otherwise, build from individual role fields for backward compatibility.
        let writer_opt = request.writer.into_option();
        let penciller_opt = request.penciller.into_option();
        let inker_opt = request.inker.into_option();
        let colorist_opt = request.colorist.into_option();
        let letterer_opt = request.letterer.into_option();
        let cover_artist_opt = request.cover_artist.into_option();
        let editor_opt = request.editor.into_option();
        let publisher_opt = request.publisher.into_option();
        let imprint_opt = request.imprint.into_option();
        let genre_opt = request.genre.into_option();
        let language_iso_opt = request.language_iso.into_option();
        let format_detail_opt = request.format_detail.into_option();
        let black_and_white_opt = request.black_and_white.into_option();
        let manga_opt = request.manga.into_option();
        let year_opt = request.year.into_option();
        let month_opt = request.month.into_option();
        let day_opt = request.day.into_option();
        let volume_opt = request.volume.into_option();
        let count_opt = request.count.into_option();
        let isbns_opt = request.isbns.into_option();
        // New Phase 6 fields
        let book_type_opt = request.book_type.into_option();
        let book_type_str = book_type_opt.as_ref().map(|bt| bt.to_string());
        let subtitle_opt = request.subtitle.into_option();
        let authors_opt = request.authors.into_option();
        let authors_json_opt = authors_opt
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let translator_opt = request.translator.into_option();
        let edition_opt = request.edition.into_option();
        let original_title_opt = request.original_title.into_option();
        let original_year_opt = request.original_year.into_option();
        let series_position_opt = request.series_position.into_option();
        let series_total_opt = request.series_total.into_option();
        let subjects_opt = request.subjects.into_option();
        let subjects_str = subjects_opt
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let awards_opt = request.awards.into_option();
        let awards_json_opt = awards_opt
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let custom_metadata_opt = request.custom_metadata.into_option();
        let custom_metadata_str = custom_metadata_opt
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        // Merge individual author fields into authors_json if the new-style
        // authors field was not provided.
        let merged_authors_json = if authors_json_opt.is_some() {
            authors_json_opt.clone()
        } else {
            build_authors_json_from_request(
                &writer_opt,
                &penciller_opt,
                &inker_opt,
                &colorist_opt,
                &letterer_opt,
                &cover_artist_opt,
                &editor_opt,
            )
        };
        let any_author_set = merged_authors_json.is_some();

        let active = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(None), // Title is not set via metadata replace (use PATCH /books/{id})
            title_sort: Set(None), // Title sort is not set via metadata replace
            number: Set(None), // Number is not set via metadata replace (use PATCH /books/{id})
            summary: Set(summary_opt.clone()),
            publisher: Set(publisher_opt.clone()),
            imprint: Set(imprint_opt.clone()),
            genre: Set(genre_opt.clone()),
            language_iso: Set(language_iso_opt.clone()),
            format_detail: Set(format_detail_opt.clone()),
            black_and_white: Set(black_and_white_opt),
            manga: Set(manga_opt),
            year: Set(year_opt),
            month: Set(month_opt),
            day: Set(day_opt),
            volume: Set(volume_opt),
            count: Set(count_opt),
            isbns: Set(isbns_opt.clone()),
            // New Phase 6 fields
            book_type: Set(book_type_str.clone()),
            subtitle: Set(subtitle_opt.clone()),
            authors_json: Set(merged_authors_json),
            translator: Set(translator_opt.clone()),
            edition: Set(edition_opt.clone()),
            original_title: Set(original_title_opt.clone()),
            original_year: Set(original_year_opt),
            series_position: Set(
                series_position_opt.and_then(sea_orm::prelude::Decimal::from_f64_retain)
            ),
            series_total: Set(series_total_opt),
            subjects: Set(subjects_str.clone()),
            awards_json: Set(awards_json_opt.clone()),
            custom_metadata: Set(custom_metadata_str.clone()),
            // Set locks for non-null fields
            title_lock: Set(false),
            title_sort_lock: Set(false),
            number_lock: Set(false),
            summary_lock: Set(summary_opt.is_some()),
            publisher_lock: Set(publisher_opt.is_some()),
            imprint_lock: Set(imprint_opt.is_some()),
            genre_lock: Set(genre_opt.is_some()),
            language_iso_lock: Set(language_iso_opt.is_some()),
            format_detail_lock: Set(format_detail_opt.is_some()),
            black_and_white_lock: Set(black_and_white_opt.is_some()),
            manga_lock: Set(manga_opt.is_some()),
            year_lock: Set(year_opt.is_some()),
            month_lock: Set(month_opt.is_some()),
            day_lock: Set(day_opt.is_some()),
            volume_lock: Set(volume_opt.is_some()),
            count_lock: Set(count_opt.is_some()),
            isbns_lock: Set(isbns_opt.is_some()),
            // New Phase 6 lock fields
            book_type_lock: Set(book_type_str.is_some()),
            subtitle_lock: Set(subtitle_opt.is_some()),
            authors_json_lock: Set(any_author_set),
            translator_lock: Set(translator_opt.is_some()),
            edition_lock: Set(edition_opt.is_some()),
            original_title_lock: Set(original_title_opt.is_some()),
            original_year_lock: Set(original_year_opt.is_some()),
            series_position_lock: Set(series_position_opt.is_some()),
            series_total_lock: Set(series_total_opt.is_some()),
            subjects_lock: Set(subjects_str.is_some()),
            awards_json_lock: Set(awards_json_opt.is_some()),
            custom_metadata_lock: Set(custom_metadata_str.is_some()),
            cover_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active
            .insert(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create metadata: {}", e)))?
    };

    // Emit update event if there were changes
    if has_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::BookUpdated {
                book_id,
                series_id: book.series_id,
                library_id: book.library_id,
                fields: None,
            },
            timestamp: now,
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    // Parse new fields from the updated record
    let authors = updated
        .authors_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<Vec<BookAuthorDto>>(json).ok());
    let awards = updated
        .awards_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<Vec<BookAwardDto>>(json).ok());
    let subjects = updated.subjects.as_ref().map(|s| {
        if s.starts_with('[') {
            serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| vec![s.clone()])
        } else {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        }
    });
    let custom_metadata = updated
        .custom_metadata
        .as_ref()
        .and_then(|json| serde_json::from_str(json).ok());

    Ok(Json(BookMetadataResponse {
        book_id: updated.book_id,
        summary: updated.summary,
        writer: extract_first_author_by_role(&updated.authors_json, "writer"),
        penciller: extract_first_author_by_role(&updated.authors_json, "penciller"),
        inker: extract_first_author_by_role(&updated.authors_json, "inker"),
        colorist: extract_first_author_by_role(&updated.authors_json, "colorist"),
        letterer: extract_first_author_by_role(&updated.authors_json, "letterer"),
        cover_artist: extract_first_author_by_role(&updated.authors_json, "cover_artist"),
        editor: extract_first_author_by_role(&updated.authors_json, "editor"),
        publisher: updated.publisher,
        imprint: updated.imprint,
        genre: updated.genre,
        language_iso: updated.language_iso,
        format_detail: updated.format_detail,
        black_and_white: updated.black_and_white,
        manga: updated.manga,
        year: updated.year,
        month: updated.month,
        day: updated.day,
        volume: updated.volume,
        count: updated.count,
        isbns: updated.isbns,
        // New Phase 6 fields
        book_type: updated
            .book_type
            .as_ref()
            .and_then(|s| s.parse::<BookType>().ok())
            .map(BookTypeDto::from),
        subtitle: updated.subtitle,
        authors,
        translator: updated.translator,
        edition: updated.edition,
        original_title: updated.original_title,
        original_year: updated.original_year,
        series_position: updated
            .series_position
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        series_total: updated.series_total,
        subjects,
        awards,
        custom_metadata,
        locks: BookMetadataLocks {
            title_lock: updated.title_lock,
            title_sort_lock: updated.title_sort_lock,
            number_lock: updated.number_lock,
            summary_lock: updated.summary_lock,
            writer_lock: updated.authors_json_lock,
            penciller_lock: updated.authors_json_lock,
            inker_lock: updated.authors_json_lock,
            colorist_lock: updated.authors_json_lock,
            letterer_lock: updated.authors_json_lock,
            cover_artist_lock: updated.authors_json_lock,
            editor_lock: updated.authors_json_lock,
            publisher_lock: updated.publisher_lock,
            imprint_lock: updated.imprint_lock,
            genre_lock: updated.genre_lock,
            language_iso_lock: updated.language_iso_lock,
            format_detail_lock: updated.format_detail_lock,
            black_and_white_lock: updated.black_and_white_lock,
            manga_lock: updated.manga_lock,
            year_lock: updated.year_lock,
            month_lock: updated.month_lock,
            day_lock: updated.day_lock,
            volume_lock: updated.volume_lock,
            count_lock: updated.count_lock,
            isbns_lock: updated.isbns_lock,
            book_type_lock: updated.book_type_lock,
            subtitle_lock: updated.subtitle_lock,
            authors_json_lock: updated.authors_json_lock,
            translator_lock: updated.translator_lock,
            edition_lock: updated.edition_lock,
            original_title_lock: updated.original_title_lock,
            original_year_lock: updated.original_year_lock,
            series_position_lock: updated.series_position_lock,
            series_total_lock: updated.series_total_lock,
            subjects_lock: updated.subjects_lock,
            awards_json_lock: updated.awards_json_lock,
            custom_metadata_lock: updated.custom_metadata_lock,
            cover_lock: updated.cover_lock,
        },
        updated_at: updated.updated_at,
    }))
}

// ============================================================================
// Book Metadata Lock Endpoints
// ============================================================================

use crate::api::routes::v1::dto::{
    BookUpdateResponse, PatchBookRequest, UpdateBookMetadataLocksRequest,
};

/// Get book metadata lock states
///
/// Returns which metadata fields are locked (protected from automatic updates).
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/metadata/locks",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "Lock states retrieved successfully", body = BookMetadataLocks),
        (status = 404, description = "Book or metadata not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_book_metadata_locks(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookMetadataLocks>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get metadata record
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book metadata not found".to_string()))?;

    Ok(Json(BookMetadataLocks {
        title_lock: metadata.title_lock,
        title_sort_lock: metadata.title_sort_lock,
        number_lock: metadata.number_lock,
        summary_lock: metadata.summary_lock,
        writer_lock: metadata.authors_json_lock,
        penciller_lock: metadata.authors_json_lock,
        inker_lock: metadata.authors_json_lock,
        colorist_lock: metadata.authors_json_lock,
        letterer_lock: metadata.authors_json_lock,
        cover_artist_lock: metadata.authors_json_lock,
        editor_lock: metadata.authors_json_lock,
        publisher_lock: metadata.publisher_lock,
        imprint_lock: metadata.imprint_lock,
        genre_lock: metadata.genre_lock,
        language_iso_lock: metadata.language_iso_lock,
        format_detail_lock: metadata.format_detail_lock,
        black_and_white_lock: metadata.black_and_white_lock,
        manga_lock: metadata.manga_lock,
        year_lock: metadata.year_lock,
        month_lock: metadata.month_lock,
        day_lock: metadata.day_lock,
        volume_lock: metadata.volume_lock,
        count_lock: metadata.count_lock,
        isbns_lock: metadata.isbns_lock,
        // New Phase 6 lock fields
        book_type_lock: metadata.book_type_lock,
        subtitle_lock: metadata.subtitle_lock,
        authors_json_lock: metadata.authors_json_lock,
        translator_lock: metadata.translator_lock,
        edition_lock: metadata.edition_lock,
        original_title_lock: metadata.original_title_lock,
        original_year_lock: metadata.original_year_lock,
        series_position_lock: metadata.series_position_lock,
        series_total_lock: metadata.series_total_lock,
        subjects_lock: metadata.subjects_lock,
        awards_json_lock: metadata.awards_json_lock,
        custom_metadata_lock: metadata.custom_metadata_lock,
        cover_lock: metadata.cover_lock,
    }))
}

/// Update book metadata lock states
///
/// Updates which metadata fields are locked. Only provided fields will be updated.
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/metadata/locks",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = UpdateBookMetadataLocksRequest,
    responses(
        (status = 200, description = "Lock states updated successfully", body = BookMetadataLocks),
        (status = 404, description = "Book or metadata not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn update_book_metadata_locks(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<UpdateBookMetadataLocksRequest>,
) -> Result<Json<BookMetadataLocks>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get existing metadata
    let existing = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book metadata not found".to_string()))?;

    // Update locks
    let now = Utc::now();
    let mut active: book_metadata::ActiveModel = existing.into();

    if let Some(v) = request.title_lock {
        active.title_lock = Set(v);
    }
    if let Some(v) = request.title_sort_lock {
        active.title_sort_lock = Set(v);
    }
    if let Some(v) = request.number_lock {
        active.number_lock = Set(v);
    }
    if let Some(v) = request.summary_lock {
        active.summary_lock = Set(v);
    }
    // Map individual author lock fields to the consolidated authors_json_lock.
    // Any of the individual lock fields being set will update authors_json_lock.
    if let Some(v) = request
        .writer_lock
        .or(request.penciller_lock)
        .or(request.inker_lock)
        .or(request.colorist_lock)
        .or(request.letterer_lock)
        .or(request.cover_artist_lock)
        .or(request.editor_lock)
    {
        active.authors_json_lock = Set(v);
    }
    if let Some(v) = request.publisher_lock {
        active.publisher_lock = Set(v);
    }
    if let Some(v) = request.imprint_lock {
        active.imprint_lock = Set(v);
    }
    if let Some(v) = request.genre_lock {
        active.genre_lock = Set(v);
    }
    if let Some(v) = request.language_iso_lock {
        active.language_iso_lock = Set(v);
    }
    if let Some(v) = request.format_detail_lock {
        active.format_detail_lock = Set(v);
    }
    if let Some(v) = request.black_and_white_lock {
        active.black_and_white_lock = Set(v);
    }
    if let Some(v) = request.manga_lock {
        active.manga_lock = Set(v);
    }
    if let Some(v) = request.year_lock {
        active.year_lock = Set(v);
    }
    if let Some(v) = request.month_lock {
        active.month_lock = Set(v);
    }
    if let Some(v) = request.day_lock {
        active.day_lock = Set(v);
    }
    if let Some(v) = request.volume_lock {
        active.volume_lock = Set(v);
    }
    if let Some(v) = request.count_lock {
        active.count_lock = Set(v);
    }
    if let Some(v) = request.isbns_lock {
        active.isbns_lock = Set(v);
    }
    // New Phase 6 lock fields
    if let Some(v) = request.book_type_lock {
        active.book_type_lock = Set(v);
    }
    if let Some(v) = request.subtitle_lock {
        active.subtitle_lock = Set(v);
    }
    if let Some(v) = request.authors_json_lock {
        active.authors_json_lock = Set(v);
    }
    if let Some(v) = request.translator_lock {
        active.translator_lock = Set(v);
    }
    if let Some(v) = request.edition_lock {
        active.edition_lock = Set(v);
    }
    if let Some(v) = request.original_title_lock {
        active.original_title_lock = Set(v);
    }
    if let Some(v) = request.original_year_lock {
        active.original_year_lock = Set(v);
    }
    if let Some(v) = request.series_position_lock {
        active.series_position_lock = Set(v);
    }
    if let Some(v) = request.series_total_lock {
        active.series_total_lock = Set(v);
    }
    if let Some(v) = request.subjects_lock {
        active.subjects_lock = Set(v);
    }
    if let Some(v) = request.awards_json_lock {
        active.awards_json_lock = Set(v);
    }
    if let Some(v) = request.custom_metadata_lock {
        active.custom_metadata_lock = Set(v);
    }
    if let Some(v) = request.cover_lock {
        active.cover_lock = Set(v);
    }

    active.updated_at = Set(now);

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update locks: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["metadata_locks".to_string()]),
        },
        timestamp: now,
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(BookMetadataLocks {
        title_lock: updated.title_lock,
        title_sort_lock: updated.title_sort_lock,
        number_lock: updated.number_lock,
        summary_lock: updated.summary_lock,
        writer_lock: updated.authors_json_lock,
        penciller_lock: updated.authors_json_lock,
        inker_lock: updated.authors_json_lock,
        colorist_lock: updated.authors_json_lock,
        letterer_lock: updated.authors_json_lock,
        cover_artist_lock: updated.authors_json_lock,
        editor_lock: updated.authors_json_lock,
        publisher_lock: updated.publisher_lock,
        imprint_lock: updated.imprint_lock,
        genre_lock: updated.genre_lock,
        language_iso_lock: updated.language_iso_lock,
        format_detail_lock: updated.format_detail_lock,
        black_and_white_lock: updated.black_and_white_lock,
        manga_lock: updated.manga_lock,
        year_lock: updated.year_lock,
        month_lock: updated.month_lock,
        day_lock: updated.day_lock,
        volume_lock: updated.volume_lock,
        count_lock: updated.count_lock,
        isbns_lock: updated.isbns_lock,
        // New Phase 6 lock fields
        book_type_lock: updated.book_type_lock,
        subtitle_lock: updated.subtitle_lock,
        authors_json_lock: updated.authors_json_lock,
        translator_lock: updated.translator_lock,
        edition_lock: updated.edition_lock,
        original_title_lock: updated.original_title_lock,
        original_year_lock: updated.original_year_lock,
        series_position_lock: updated.series_position_lock,
        series_total_lock: updated.series_total_lock,
        subjects_lock: updated.subjects_lock,
        awards_json_lock: updated.awards_json_lock,
        custom_metadata_lock: updated.custom_metadata_lock,
        cover_lock: updated.cover_lock,
    }))
}

// ============================================================================
// Book Cover Upload Endpoint
// ============================================================================

use crate::events::EntityType;
use axum::extract::Multipart;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Upload a custom cover image for a book
///
/// Accepts a multipart form with an image file. The image will be stored
/// in the uploads directory and used as the book's cover/thumbnail.
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/cover",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Cover uploaded successfully"),
        (status = 400, description = "Bad request - no image file provided or invalid image"),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn upload_book_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists and get its library_id/series_id
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get the uploaded file from multipart form
    let mut image_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read multipart field: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "cover" || name == "file" || name == "image" {
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to read file data: {}", e)))?;
            image_data = Some(data.to_vec());
            break;
        }
    }

    let image_data = image_data
        .ok_or_else(|| ApiError::BadRequest("No image file provided in request".to_string()))?;

    // Validate that it's a valid image
    image::load_from_memory(&image_data)
        .map_err(|e| ApiError::BadRequest(format!("Invalid image file: {}", e)))?;

    // Compute hash of image data for deduplication
    let image_hash = crate::utils::hasher::hash_bytes(&image_data);
    let short_hash = &image_hash[..16];

    // Create covers directory within uploads dir if it doesn't exist
    let covers_dir = state
        .thumbnail_service
        .get_uploads_dir()
        .join("covers")
        .join("books");
    fs::create_dir_all(&covers_dir)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create covers directory: {}", e)))?;

    // Use book_id and image hash for filename to avoid duplicates
    let filename = format!("{}-{}.jpg", book_id, short_hash);
    let filepath = covers_dir.join(&filename);

    // Check if this exact image already exists for this book
    if filepath.exists() {
        return Err(ApiError::BadRequest(
            "This image has already been uploaded for this book".to_string(),
        ));
    }

    let mut file = fs::File::create(&filepath)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create cover file: {}", e)))?;

    file.write_all(&image_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to write cover file: {}", e)))?;

    // Create a new custom cover record (auto-selects as primary)
    BookCoversRepository::create(
        &state.db,
        book_id,
        "custom",
        &filepath.to_string_lossy(),
        true, // is_selected
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create cover record: {}", e)))?;

    // Auto-lock cover to prevent plugins from overwriting user's custom upload
    if let Some(meta) = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten()
    {
        let mut active: crate::db::entities::book_metadata::ActiveModel = meta.into();
        active.cover_lock = sea_orm::Set(true);
        active.updated_at = sea_orm::Set(Utc::now());
        let _ = active.update(&state.db).await;
    }

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(book.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::OK)
}

// ============================================================================
// Book External IDs Endpoints
// ============================================================================

use super::super::dto::{
    BookCoverListResponse, BookExternalIdListResponse, CreateBookExternalIdRequest,
};
use crate::db::repositories::{BookCoversRepository, BookExternalIdRepository};

/// List all external IDs for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/external-ids",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "List of external IDs", body = BookExternalIdListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_book_external_ids(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookExternalIdListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let external_ids = BookExternalIdRepository::get_for_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external IDs: {}", e)))?;

    let dtos: Vec<super::super::dto::BookExternalIdDto> =
        external_ids.into_iter().map(|e| e.into()).collect();

    Ok(Json(BookExternalIdListResponse { external_ids: dtos }))
}

/// Create or update an external ID for a book
///
/// Upserts by book_id + source: if an external ID with the same source already exists,
/// it will be updated instead of creating a duplicate.
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/external-ids",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = CreateBookExternalIdRequest,
    responses(
        (status = 200, description = "External ID created or updated", body = super::super::dto::BookExternalIdDto),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn create_book_external_id(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<CreateBookExternalIdRequest>,
) -> Result<Json<super::super::dto::BookExternalIdDto>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let external_id = BookExternalIdRepository::upsert(
        &state.db,
        book_id,
        &request.source,
        &request.external_id,
        request.external_url.as_deref(),
        None, // metadata_hash
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create external ID: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["external_ids".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(external_id.into()))
}

/// Delete an external ID by ID
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/external-ids/{external_id_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("external_id_id" = Uuid, Path, description = "External ID record ID")
    ),
    responses(
        (status = 204, description = "External ID deleted"),
        (status = 404, description = "Book or external ID not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn delete_book_external_id(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, external_id_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Verify the external ID exists and belongs to this book
    let ext_id = BookExternalIdRepository::get_by_id(&state.db, external_id_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external ID: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("External ID not found".to_string()))?;

    if ext_id.book_id != book_id {
        return Err(ApiError::NotFound("External ID not found".to_string()));
    }

    let deleted = BookExternalIdRepository::delete(&state.db, external_id_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete external ID: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("External ID not found".to_string()));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["external_ids".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Book External Links Management Endpoints
// ============================================================================

use super::super::dto::{
    BookExternalLinkDto, BookExternalLinkListResponse, CreateBookExternalLinkRequest,
};
use crate::db::repositories::BookExternalLinkRepository;

/// List all external links for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/external-links",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "List of external links for the book", body = BookExternalLinkListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_book_external_links(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookExternalLinkListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let links = BookExternalLinkRepository::get_for_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external links: {}", e)))?;

    let dtos: Vec<BookExternalLinkDto> = links.into_iter().map(|l| l.into()).collect();

    Ok(Json(BookExternalLinkListResponse { links: dtos }))
}

/// Create or update an external link for a book
///
/// Upserts by book_id + source_name: if a link with the same source already exists,
/// it will be updated instead of creating a duplicate.
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/external-links",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = CreateBookExternalLinkRequest,
    responses(
        (status = 200, description = "External link created or updated", body = BookExternalLinkDto),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn create_book_external_link(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<CreateBookExternalLinkRequest>,
) -> Result<Json<BookExternalLinkDto>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let link = BookExternalLinkRepository::upsert(
        &state.db,
        book_id,
        &request.source_name,
        &request.url,
        request.external_id.as_deref(),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create external link: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["external_links".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(link.into()))
}

/// Delete an external link by source name
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/external-links/{source}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("source" = String, Path, description = "Source name (e.g., 'openlibrary', 'goodreads')")
    ),
    responses(
        (status = 204, description = "External link deleted"),
        (status = 404, description = "Book or external link not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn delete_book_external_link(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, source)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let deleted = BookExternalLinkRepository::delete_by_source(&state.db, book_id, &source)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete external link: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("External link not found".to_string()));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["external_links".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Book Covers Management Endpoints
// ============================================================================

/// List all covers for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/covers",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "List of book covers", body = BookCoverListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_book_covers(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookCoverListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let covers = BookCoversRepository::list_by_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch covers: {}", e)))?;

    let cover_dtos: Vec<super::super::dto::BookCoverDto> =
        covers.into_iter().map(|c| c.into()).collect();

    Ok(Json(BookCoverListResponse { covers: cover_dtos }))
}

/// Select a cover as the primary cover for a book
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/covers/{cover_id}/select",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("cover_id" = Uuid, Path, description = "Cover ID to select")
    ),
    responses(
        (status = 200, description = "Cover selected", body = super::super::dto::BookCoverDto),
        (status = 404, description = "Book or cover not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn select_book_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, cover_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<super::super::dto::BookCoverDto>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Select the cover (validates the cover belongs to the book)
    let cover = BookCoversRepository::select_cover(&state.db, book_id, cover_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") || e.to_string().contains("does not belong") {
                ApiError::NotFound(format!("Cover not found: {}", cover_id))
            } else {
                ApiError::Internal(format!("Failed to select cover: {}", e))
            }
        })?;

    // Auto-lock cover to prevent plugins from overwriting user's manual selection
    if let Some(meta) = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten()
    {
        let mut active: crate::db::entities::book_metadata::ActiveModel = meta.into();
        active.cover_lock = sea_orm::Set(true);
        active.updated_at = sea_orm::Set(Utc::now());
        let _ = active.update(&state.db).await;
    }

    // Delete cached thumbnail so it regenerates from the new cover
    if let Err(e) = state
        .thumbnail_service
        .delete_thumbnail(&state.db, book_id)
        .await
    {
        tracing::warn!(
            "Failed to delete cached thumbnail for book {}: {}",
            book_id,
            e
        );
    }

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(book.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(cover.into()))
}

/// Reset book cover to default (deselect all custom covers)
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/covers/selected",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 204, description = "Cover reset to default"),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn reset_book_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Deselect all covers
    BookCoversRepository::deselect_all(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reset cover: {}", e)))?;

    // Delete cached thumbnail so it regenerates from the default (embedded) cover
    if let Err(e) = state
        .thumbnail_service
        .delete_thumbnail(&state.db, book_id)
        .await
    {
        tracing::warn!(
            "Failed to delete cached thumbnail for book {}: {}",
            book_id,
            e
        );
    }

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(book.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

/// Get a specific cover image for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/covers/{cover_id}/image",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("cover_id" = Uuid, Path, description = "Cover ID")
    ),
    responses(
        (status = 200, description = "Cover image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Book or cover not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_book_cover_image(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    headers: axum::http::HeaderMap,
    Path((book_id, cover_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get the cover
    let cover = BookCoversRepository::get_by_id(&state.db, cover_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Cover not found".to_string()))?;

    // Verify cover belongs to this book
    if cover.book_id != book_id {
        return Err(ApiError::NotFound("Cover not found".to_string()));
    }

    // Get file metadata for conditional caching
    let metadata = fs::metadata(&cover.path).await.map_err(|e| {
        ApiError::Internal(format!(
            "Failed to read cover metadata from {}: {}",
            cover.path, e
        ))
    })?;

    let size = metadata.len();
    let modified_unix = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Generate ETag from cover_id + size + modified time
    let etag = format!(
        "\"{:x}-{:x}-{:x}\"",
        cover_id.as_u128(),
        size,
        modified_unix
    );

    // Check If-None-Match header for ETag validation
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH)
        && let Ok(client_etag) = if_none_match.to_str()
    {
        let client_etag = client_etag.trim().trim_start_matches("W/");
        if client_etag == etag || client_etag.trim_matches('"') == etag.trim_matches('"') {
            return Ok(Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::ETAG, &etag)
                .header(header::CACHE_CONTROL, "public, max-age=31536000")
                .body(Body::empty())
                .unwrap());
        }
    }

    // Stream the cover file
    let file = tokio::fs::File::open(&cover.path).await.map_err(|e| {
        ApiError::Internal(format!("Failed to open cover from {}: {}", cover.path, e))
    })?;
    let stream = ReaderStream::new(file);

    let last_modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(modified_unix);
    let last_modified_str = httpdate::fmt_http_date(last_modified);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CONTENT_LENGTH, size)
        .header(header::ETAG, &etag)
        .header(header::LAST_MODIFIED, last_modified_str)
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .body(Body::from_stream(stream))
        .unwrap())
}

/// Delete a specific cover for a book
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/covers/{cover_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("cover_id" = Uuid, Path, description = "Cover ID to delete")
    ),
    responses(
        (status = 204, description = "Cover deleted"),
        (status = 404, description = "Book or cover not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn delete_book_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, cover_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get the cover to verify it exists and belongs to this book
    let cover = BookCoversRepository::get_by_id(&state.db, cover_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Cover not found".to_string()))?;

    if cover.book_id != book_id {
        return Err(ApiError::NotFound("Cover not found".to_string()));
    }

    let was_selected = cover.is_selected;

    // If this is the selected cover, select another one (if available)
    if was_selected {
        let all_covers = BookCoversRepository::list_by_book(&state.db, book_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to list covers: {}", e)))?;

        let alternate = all_covers.iter().find(|c| c.id != cover_id);
        if let Some(alt_cover) = alternate {
            BookCoversRepository::select_cover(&state.db, book_id, alt_cover.id)
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("Failed to select alternate cover: {}", e))
                })?;
        }
    }

    // If this is a custom cover, delete the file
    if cover.source == "custom" {
        let path = std::path::Path::new(&cover.path);
        if path.exists() {
            let _ = fs::remove_file(path).await;
        }
    }

    // Delete the cover record
    BookCoversRepository::delete(&state.db, cover_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete cover: {}", e)))?;

    // Delete cached thumbnail so it regenerates from the new/default cover
    if was_selected
        && let Err(e) = state
            .thumbnail_service
            .delete_thumbnail(&state.db, book_id)
            .await
    {
        tracing::warn!(
            "Failed to delete cached thumbnail for book {}: {}",
            book_id,
            e
        );
    }

    // Emit cover updated event
    if was_selected {
        let event = EntityChangeEvent {
            event: EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: book_id,
                library_id: Some(book.library_id),
            },
            timestamp: Utc::now(),
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Book Error Endpoints (Enhanced with grouping and retry)
// ============================================================================

use super::super::dto::{
    BookErrorDto, BookErrorTypeDto, BookWithErrorsDto, BooksWithErrorsResponse, ErrorGroupDto,
    RetryAllErrorsRequest, RetryBookErrorsRequest, RetryErrorsResponse,
};
use crate::db::entities::book_error::{BookErrorType, parse_analysis_errors};
use crate::db::repositories::TaskRepository;
use crate::tasks::types::TaskType;

/// Query parameters for listing books with analysis errors
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct BooksWithErrorsQuery {
    /// Optional library filter
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Optional series filter
    #[serde(default)]
    pub series_id: Option<Uuid>,

    /// Filter by specific error type
    #[serde(default)]
    pub error_type: Option<BookErrorTypeDto>,

    /// Page number (1-indexed, default 1)
    #[serde(default = "default_page")]
    pub page: u64,

    /// Number of items per page (max 100, default 50)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

/// List books with errors (grouped by error type)
///
/// Returns books with errors grouped by error type, with counts and pagination.
/// This endpoint provides detailed error information including error
/// types, messages, and timestamps.
#[utoipa::path(
    get,
    path = "/api/v1/books/errors",
    params(BooksWithErrorsQuery),
    responses(
        (status = 200, description = "Books with errors grouped by type", body = BooksWithErrorsResponse,
            example = json!({
                "totalBooksWithErrors": 15,
                "errorCounts": {"parser": 5, "thumbnail": 10},
                "groups": [{
                    "errorType": "parser",
                    "label": "Parser Error",
                    "count": 5,
                    "books": []
                }],
                "page": 0,
                "pageSize": 20,
                "totalPages": 1
            })
        ),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_books_with_errors(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BooksWithErrorsQuery>,
) -> Result<Json<BooksWithErrorsResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Get error counts by type (for the summary)
    let error_counts = BookRepository::count_errors_by_type(&state.db, query.library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count errors: {}", e)))?;

    // Convert internal error type to DTO if provided
    let error_type_filter = query.error_type.map(|t| t.into());

    // Fetch books with errors (convert to 0-indexed for repository)
    let (books_with_errors, total) = BookRepository::list_with_errors(
        &state.db,
        query.library_id,
        query.series_id,
        error_type_filter,
        page - 1, // Convert 1-indexed to 0-indexed for repository
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch books with errors: {}", e)))?;

    // Convert to DTOs
    let books_models: Vec<_> = books_with_errors.iter().map(|(b, _)| b.clone()).collect();
    let book_dtos = books_to_dtos(&state.db, auth.user_id, books_models).await?;

    // Create a map from book_id to BookDto
    let book_dto_map: HashMap<Uuid, BookDto> = book_dtos.into_iter().map(|b| (b.id, b)).collect();

    // Build books with errors DTOs
    let books_with_errors_dtos: Vec<BookWithErrorsDto> = books_with_errors
        .into_iter()
        .filter_map(|(book, errors)| {
            let book_dto = book_dto_map.get(&book.id).cloned()?;
            let error_dtos: Vec<BookErrorDto> = errors
                .into_iter()
                .map(|(error_type, error)| BookErrorDto {
                    error_type: error_type.into(),
                    message: error.message,
                    details: error.details,
                    occurred_at: error.occurred_at,
                })
                .collect();
            Some(BookWithErrorsDto {
                book: book_dto,
                errors: error_dtos,
            })
        })
        .collect();

    // Group by error type
    let mut groups_map: HashMap<BookErrorType, Vec<BookWithErrorsDto>> = HashMap::new();
    for book_with_errors in &books_with_errors_dtos {
        for error in &book_with_errors.errors {
            groups_map
                .entry(error.error_type.into())
                .or_default()
                .push(book_with_errors.clone());
        }
    }

    // Build groups sorted by error type label
    let mut groups: Vec<ErrorGroupDto> = groups_map
        .into_iter()
        .map(|(error_type, books)| {
            let count = error_counts.get(&error_type).copied().unwrap_or(0);
            ErrorGroupDto {
                error_type: error_type.into(),
                label: error_type.label().to_string(),
                count,
                books,
            }
        })
        .collect();
    groups.sort_by(|a, b| a.label.cmp(&b.label));

    // Convert error counts map to string keys for JSON
    let error_counts_str: HashMap<String, u64> = error_counts
        .into_iter()
        .map(|(k, v)| {
            (
                serde_json::to_string(&k)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                v,
            )
        })
        .collect();

    let total_pages = total.div_ceil(page_size).max(1);

    Ok(Json(BooksWithErrorsResponse {
        total_books_with_errors: total,
        error_counts: error_counts_str,
        groups,
        page, // Return normalized 1-indexed page
        page_size,
        total_pages,
    }))
}

/// Retry failed operations for a specific book
///
/// Enqueues appropriate tasks based on the error types present or specified.
/// For parser/metadata/page_extraction errors, enqueues an AnalyzeBook task.
/// For thumbnail errors, enqueues a GenerateThumbnail task.
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/retry",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body(content = RetryBookErrorsRequest, content_type = "application/json",
        example = json!({"errorTypes": ["parser", "thumbnail"]})
    ),
    responses(
        (status = 200, description = "Retry tasks enqueued", body = RetryErrorsResponse,
            example = json!({"tasksEnqueued": 2, "message": "Enqueued 1 analysis task and 1 thumbnail task"})
        ),
        (status = 404, description = "Book not found"),
        (status = 400, description = "Book has no errors to retry"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn retry_book_errors(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<RetryBookErrorsRequest>,
) -> Result<Json<RetryErrorsResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get current errors for the book
    let errors = parse_analysis_errors(book.analysis_errors.as_deref());
    if errors.is_empty() {
        return Err(ApiError::BadRequest(
            "Book has no errors to retry".to_string(),
        ));
    }

    // Determine which error types to retry
    let error_types_to_retry: Vec<BookErrorType> = if let Some(types) = request.error_types {
        types.into_iter().map(|t| t.into()).collect()
    } else {
        errors.keys().cloned().collect()
    };

    // Determine which tasks to enqueue based on error types
    let needs_analysis = error_types_to_retry.iter().any(|t| {
        matches!(
            t,
            BookErrorType::Parser
                | BookErrorType::Metadata
                | BookErrorType::PageExtraction
                | BookErrorType::FormatDetection
                | BookErrorType::PdfRendering
                | BookErrorType::ZeroPages
                | BookErrorType::Other
        )
    });
    let needs_thumbnail = error_types_to_retry.contains(&BookErrorType::Thumbnail);

    let mut tasks_enqueued = 0u64;
    let mut messages = Vec::new();

    // Enqueue analysis task if needed
    if needs_analysis {
        TaskRepository::enqueue(
            &state.db,
            TaskType::AnalyzeBook {
                book_id,
                force: true,
            },
            10,   // Normal priority
            None, // No scheduled time - run immediately
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue analysis task: {}", e)))?;
        tasks_enqueued += 1;
        messages.push("1 analysis task");
    }

    // Enqueue thumbnail task if needed
    if needs_thumbnail {
        TaskRepository::enqueue(
            &state.db,
            TaskType::GenerateThumbnail {
                book_id,
                force: true,
            },
            10,   // Normal priority
            None, // No scheduled time - run immediately
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue thumbnail task: {}", e)))?;
        tasks_enqueued += 1;
        messages.push("1 thumbnail task");
    }

    let message = if messages.is_empty() {
        "No tasks enqueued".to_string()
    } else {
        format!("Enqueued {}", messages.join(" and "))
    };

    Ok(Json(RetryErrorsResponse {
        tasks_enqueued,
        message,
    }))
}

/// Retry all failed operations across all books
///
/// Enqueues appropriate tasks for all books with errors.
/// Can be filtered by error type or library.
#[utoipa::path(
    post,
    path = "/api/v1/books/retry-all-errors",
    request_body(content = RetryAllErrorsRequest, content_type = "application/json",
        example = json!({"errorType": "parser", "libraryId": null})
    ),
    responses(
        (status = 200, description = "Retry tasks enqueued", body = RetryErrorsResponse,
            example = json!({"tasksEnqueued": 15, "message": "Enqueued 10 analysis tasks and 5 thumbnail tasks"})
        ),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn retry_all_book_errors(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<RetryAllErrorsRequest>,
) -> Result<Json<RetryErrorsResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Convert error type filter if provided
    let error_type_filter: Option<BookErrorType> = request.error_type.map(|t| t.into());

    // Fetch all books with errors (unpaginated - we need all for bulk retry)
    // Use a large page size to get all results
    let (books_with_errors, _) = BookRepository::list_with_errors(
        &state.db,
        request.library_id,
        None, // No series filter for bulk retry
        error_type_filter,
        0,
        10000, // Large page size to get all
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch books with errors: {}", e)))?;

    if books_with_errors.is_empty() {
        return Ok(Json(RetryErrorsResponse {
            tasks_enqueued: 0,
            message: "No books with errors found".to_string(),
        }));
    }

    // Categorize books by what they need
    let mut needs_analysis: Vec<Uuid> = Vec::new();
    let mut needs_thumbnail: Vec<Uuid> = Vec::new();

    for (book, errors) in &books_with_errors {
        let error_types: Vec<BookErrorType> = if let Some(ref filter_type) = error_type_filter {
            // Only consider the filtered error type
            errors
                .keys()
                .filter(|t| *t == filter_type)
                .cloned()
                .collect()
        } else {
            errors.keys().cloned().collect()
        };

        let book_needs_analysis = error_types.iter().any(|t| {
            matches!(
                t,
                BookErrorType::Parser
                    | BookErrorType::Metadata
                    | BookErrorType::PageExtraction
                    | BookErrorType::FormatDetection
                    | BookErrorType::PdfRendering
                    | BookErrorType::ZeroPages
                    | BookErrorType::Other
            )
        });
        let book_needs_thumbnail = error_types.contains(&BookErrorType::Thumbnail);

        if book_needs_analysis {
            needs_analysis.push(book.id);
        }
        if book_needs_thumbnail {
            needs_thumbnail.push(book.id);
        }
    }

    let mut tasks_enqueued = 0u64;

    // Batch enqueue analysis tasks
    if !needs_analysis.is_empty() {
        let analysis_tasks: Vec<TaskType> = needs_analysis
            .into_iter()
            .map(|book_id| TaskType::AnalyzeBook {
                book_id,
                force: true,
            })
            .collect();

        let count = TaskRepository::enqueue_batch(&state.db, analysis_tasks, 10, None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to enqueue analysis tasks: {}", e)))?;
        tasks_enqueued += count;
    }

    // Batch enqueue thumbnail tasks
    if !needs_thumbnail.is_empty() {
        let thumbnail_tasks: Vec<TaskType> = needs_thumbnail
            .into_iter()
            .map(|book_id| TaskType::GenerateThumbnail {
                book_id,
                force: true,
            })
            .collect();

        let count = TaskRepository::enqueue_batch(&state.db, thumbnail_tasks, 10, None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to enqueue thumbnail tasks: {}", e)))?;
        tasks_enqueued += count;
    }

    let message = format!("Enqueued {} tasks for books with errors", tasks_enqueued);

    Ok(Json(RetryErrorsResponse {
        tasks_enqueued,
        message,
    }))
}

/// List pages for a book
///
/// Returns page metadata including dimensions for analyzed books.
/// Returns an empty array for books that haven't been analyzed yet.
/// The frontend should use the `analyzed` field from BookDto to determine
/// whether to use dynamic spread calculation (when true) or simple static
/// spreads (when false).
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/pages",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
    ),
    responses(
        (status = 200, description = "List of pages with dimensions", body = Vec<PageDto>),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn list_book_pages(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<Vec<PageDto>>, ApiError> {
    require_permission!(auth, Permission::PagesRead)?;

    // Fetch book to verify it exists and check if analyzed
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check sharing tag access for the book's series
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if !content_filter.is_book_visible(book.series_id) {
        return Err(ApiError::NotFound("Book not found".to_string()));
    }

    // If book is not analyzed, return empty array
    // Frontend will use simple static spreads in this case
    if !book.analyzed {
        return Ok(Json(vec![]));
    }

    // Fetch all pages for the book
    let pages = PageRepository::list_by_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch pages: {}", e)))?;

    // Convert to DTOs
    let page_dtos: Vec<PageDto> = pages
        .into_iter()
        .map(|page| PageDto {
            id: page.id,
            book_id: page.book_id,
            page_number: page.page_number,
            file_name: page.file_name,
            file_format: page.format,
            file_size: page.file_size,
            width: Some(page.width),
            height: Some(page.height),
        })
        .collect();

    Ok(Json(page_dtos))
}

// ============================================================================
// Book Genre Endpoints
// ============================================================================

/// Get genres for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/genres",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "List of genres for the book", body = GenreListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_book_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<GenreListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let genres = GenreRepository::get_genres_for_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;

    let dtos: Vec<GenreDto> = genres
        .into_iter()
        .map(|g| GenreDto {
            id: g.id,
            name: g.name,
            series_count: None,
            created_at: g.created_at,
        })
        .collect();

    Ok(Json(GenreListResponse { genres: dtos }))
}

/// Set genres for a book (replaces existing)
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/genres",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = SetBookGenresRequest,
    responses(
        (status = 200, description = "Genres updated", body = GenreListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn set_book_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<SetBookGenresRequest>,
) -> Result<Json<GenreListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let genres = GenreRepository::set_genres_for_book(&state.db, book_id, request.genres)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to set genres: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["genres".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    let dtos: Vec<GenreDto> = genres
        .into_iter()
        .map(|g| GenreDto {
            id: g.id,
            name: g.name,
            series_count: None,
            created_at: g.created_at,
        })
        .collect();

    Ok(Json(GenreListResponse { genres: dtos }))
}

/// Add a single genre to a book
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/genres",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = AddBookGenreRequest,
    responses(
        (status = 200, description = "Genre added", body = GenreDto),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn add_book_genre(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<AddBookGenreRequest>,
) -> Result<Json<GenreDto>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let genre = GenreRepository::add_genre_to_book(&state.db, book_id, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add genre: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["genres".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(GenreDto {
        id: genre.id,
        name: genre.name,
        series_count: None,
        created_at: genre.created_at,
    }))
}

/// Remove a genre from a book
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/genres/{genre_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("genre_id" = Uuid, Path, description = "Genre ID")
    ),
    responses(
        (status = 204, description = "Genre removed from book"),
        (status = 404, description = "Book or genre link not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn remove_book_genre(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, genre_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let removed = GenreRepository::remove_genre_from_book(&state.db, book_id, genre_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove genre: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound(
            "Genre not linked to this book".to_string(),
        ));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["genres".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Book Tag Endpoints
// ============================================================================

/// Get tags for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/tags",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "List of tags for the book", body = TagListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn get_book_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<TagListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let tags = TagRepository::get_tags_for_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let dtos: Vec<TagDto> = tags
        .into_iter()
        .map(|t| TagDto {
            id: t.id,
            name: t.name,
            series_count: None,
            created_at: t.created_at,
        })
        .collect();

    Ok(Json(TagListResponse { tags: dtos }))
}

/// Set tags for a book (replaces existing)
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/tags",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = SetBookTagsRequest,
    responses(
        (status = 200, description = "Tags updated", body = TagListResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn set_book_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<SetBookTagsRequest>,
) -> Result<Json<TagListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let tags = TagRepository::set_tags_for_book(&state.db, book_id, request.tags)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to set tags: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["tags".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    let dtos: Vec<TagDto> = tags
        .into_iter()
        .map(|t| TagDto {
            id: t.id,
            name: t.name,
            series_count: None,
            created_at: t.created_at,
        })
        .collect();

    Ok(Json(TagListResponse { tags: dtos }))
}

/// Add a single tag to a book
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/tags",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = AddBookTagRequest,
    responses(
        (status = 200, description = "Tag added", body = TagDto),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn add_book_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<AddBookTagRequest>,
) -> Result<Json<TagDto>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let tag = TagRepository::add_tag_to_book(&state.db, book_id, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add tag: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["tags".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(TagDto {
        id: tag.id,
        name: tag.name,
        series_count: None,
        created_at: tag.created_at,
    }))
}

/// Remove a tag from a book
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/tags/{tag_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("tag_id" = Uuid, Path, description = "Tag ID")
    ),
    responses(
        (status = 204, description = "Tag removed from book"),
        (status = 404, description = "Book or tag link not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Books"
)]
pub async fn remove_book_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let removed = TagRepository::remove_tag_from_book(&state.db, book_id, tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove tag: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound(
            "Tag not linked to this book".to_string(),
        ));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["tags".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}
