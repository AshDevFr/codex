use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, PageRepository};
use crate::require_permission;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

/// Get page image from a book
///
/// Extracts and serves the image for a specific page from CBZ/CBR/EPUB/PDF
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/pages/{page_number}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("page_number" = i32, Path, description = "Page number (1-indexed)")
    ),
    responses(
        (status = 200, description = "Page image", content_type = "image/jpeg"),
        (status = 404, description = "Book or page not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "pages"
)]
pub async fn get_page_image(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((book_id, page_number)): Path<(Uuid, i32)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::PagesRead)?;

    // Validate page number
    if page_number < 1 {
        return Err(ApiError::BadRequest("Page number must be >= 1".to_string()));
    }

    // Fetch book from database
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check page number is valid
    if page_number > book.page_count {
        return Err(ApiError::NotFound(format!(
            "Page {} not found (book has {} pages)",
            page_number, book.page_count
        )));
    }

    // Fetch page metadata
    let page = PageRepository::get_by_book_and_number(&state.db, book_id, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch page: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Page not found".to_string()))?;

    // Extract image from book file based on format
    let image_data = extract_page_image(&book.file_path, &book.format, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract page image: {}", e)))?;

    // Determine content type from file format
    let content_type = match page.format.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    };

    // Build response with caching headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .header(header::CONTENT_LENGTH, image_data.len())
        .body(Body::from(image_data))
        .unwrap())
}

/// Extract page image from book file
async fn extract_page_image(
    file_path: &str,
    file_format: &str,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let path = std::path::Path::new(file_path);

    match file_format.to_uppercase().as_str() {
        "CBZ" => extract_from_cbz(path, page_number).await,
        #[cfg(feature = "rar")]
        "CBR" => extract_from_cbr(path, page_number).await,
        "EPUB" => extract_from_epub(path, page_number).await,
        "PDF" => extract_from_pdf(path, page_number).await,
        _ => anyhow::bail!("Unsupported format: {}", file_format),
    }
}

/// Extract image from CBZ (ZIP) archive
async fn extract_from_cbz(path: &std::path::Path, page_number: i32) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Get list of image files in archive
    let mut image_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if crate::parsers::is_image_file(&name) {
            image_files.push(name);
        }
    }

    // Sort alphabetically
    image_files.sort();

    // Get the requested page (1-indexed)
    let index = (page_number - 1) as usize;
    if index >= image_files.len() {
        anyhow::bail!("Page {} not found in archive", page_number);
    }

    // Extract image data
    let mut file = archive.by_name(&image_files[index])?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    Ok(buffer)
}

/// Extract image from CBR (RAR) archive
#[cfg(feature = "rar")]
async fn extract_from_cbr(path: &std::path::Path, page_number: i32) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;

    let mut archive = unrar::Archive::new(path).open_for_processing()?;
    let mut image_files: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(header) = archive.read_header()? {
        let entry_name = header.entry().filename.to_string_lossy().to_string();

        if crate::parsers::is_image_file(&entry_name) {
            let (data, next_archive) = header.read()?;
            archive = next_archive;
            image_files.push((entry_name, data));
        } else {
            archive = header.skip()?;
        }
    }

    // Sort by filename
    image_files.sort_by(|a, b| a.0.cmp(&b.0));

    // Get the requested page (1-indexed)
    let index = (page_number - 1) as usize;
    if index >= image_files.len() {
        anyhow::bail!("Page {} not found in archive", page_number);
    }

    Ok(image_files[index].1.clone())
}

/// Extract image from EPUB
async fn extract_from_epub(path: &std::path::Path, page_number: i32) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Get list of image files in EPUB
    let mut image_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if crate::parsers::is_image_file(&name) {
            image_files.push(name);
        }
    }

    // Sort alphabetically
    image_files.sort();

    // Get the requested page (1-indexed)
    let index = (page_number - 1) as usize;
    if index >= image_files.len() {
        anyhow::bail!("Page {} not found in EPUB", page_number);
    }

    // Extract image data
    let mut file = archive.by_name(&image_files[index])?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    Ok(buffer)
}

/// Extract image from PDF
async fn extract_from_pdf(_path: &std::path::Path, _page_number: i32) -> anyhow::Result<Vec<u8>> {
    // PDF image extraction is complex and requires rendering
    // This is a placeholder - full implementation would use pdf rendering libraries
    anyhow::bail!("PDF image extraction not yet implemented")
}
