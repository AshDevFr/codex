//! KOReader sync progress handlers

use crate::api::error::ApiError;
use crate::api::extractors::{AuthContext, AuthState};
use crate::api::routes::koreader::dto::progress::DocumentProgressDto;
use crate::db::repositories::{BookRepository, ReadProgressRepository};
use axum::Json;
use axum::extract::{Path, State};
use std::sync::Arc;

/// GET /koreader/syncs/progress/{document}
///
/// Get reading progress for a document identified by its KOReader hash.
/// Returns the stored progress if found.
pub async fn get_progress(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(document_hash): Path<String>,
) -> Result<Json<DocumentProgressDto>, ApiError> {
    let user_id = auth.user_id;

    // Find book by koreader_hash
    let books = BookRepository::find_by_koreader_hash(&state.db, &document_hash)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to find book: {}", e)))?;

    if books.is_empty() {
        return Err(ApiError::NotFound(
            "No book found with this hash".to_string(),
        ));
    }

    if books.len() > 1 {
        return Err(ApiError::Conflict(
            "Multiple books found with the same hash".to_string(),
        ));
    }

    let book = &books[0];

    // Get reading progress for this user and book
    let progress = ReadProgressRepository::get_by_user_and_book(&state.db, user_id, book.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get progress: {}", e)))?;

    match progress {
        Some(p) => {
            // Convert internal progress to KOReader format
            // For PDF/CBZ: progress is the page number as a string
            // For EPUB: we store page number but KOReader expects DocFragment format
            let progress_str = p.current_page.to_string();
            let percentage = p
                .progress_percentage
                .unwrap_or_else(|| p.current_page as f64 / book.page_count.max(1) as f64);

            Ok(Json(DocumentProgressDto {
                document: document_hash,
                progress: progress_str,
                percentage,
                device: String::new(),
                device_id: String::new(),
            }))
        }
        None => Err(ApiError::NotFound(
            "No progress found for this book".to_string(),
        )),
    }
}

/// PUT /koreader/syncs/progress
///
/// Update reading progress for a document identified by its KOReader hash.
pub async fn update_progress(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<DocumentProgressDto>,
) -> Result<Json<DocumentProgressDto>, ApiError> {
    let user_id = auth.user_id;

    // Find book by koreader_hash
    let books = BookRepository::find_by_koreader_hash(&state.db, &request.document)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to find book: {}", e)))?;

    if books.is_empty() {
        return Err(ApiError::NotFound(
            "No book found with this hash".to_string(),
        ));
    }

    if books.len() > 1 {
        return Err(ApiError::Conflict(
            "Multiple books found with the same hash".to_string(),
        ));
    }

    let book = &books[0];

    // Parse progress string to page number
    // For PDF/CBZ: progress is the page number as a string
    // For EPUB: progress is a DocFragment XPath string, extract the index
    let current_page = parse_koreader_progress(&request.progress, &book.format);

    let completed =
        request.percentage >= 0.98 || (book.page_count > 0 && current_page >= book.page_count);

    // Update progress
    ReadProgressRepository::upsert_with_percentage(
        &state.db,
        user_id,
        book.id,
        current_page,
        Some(request.percentage),
        completed,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update progress: {}", e)))?;

    Ok(Json(request))
}

/// Parse KOReader progress string into a page number
///
/// For PDF/CBZ (pre-paginated): progress is just a page number string like "42"
/// For EPUB: progress is a DocFragment XPath like "/body/DocFragment[10]/body/div/p[1]/text().0"
///   or a TOC-based format like "#_doc_fragment_44_ c37"
fn parse_koreader_progress(progress: &str, format: &str) -> i32 {
    match format {
        "epub" => parse_epub_progress(progress),
        _ => {
            // PDF, CBZ, CBR: progress is a page number
            progress.parse::<i32>().unwrap_or(1).max(1)
        }
    }
}

/// Parse EPUB progress from KOReader format
///
/// Handles two formats:
/// 1. DocFragment[N] (1-based): "/body/DocFragment[10]/body/div/p[1]/text().0"
/// 2. _doc_fragment_N_ (0-based): "#_doc_fragment_44_ c37"
/// 3. Plain number fallback
fn parse_epub_progress(progress: &str) -> i32 {
    // Try DocFragment[N] format (1-based index)
    if let Some(start) = progress.find("DocFragment[") {
        let after = &progress[start + 12..];
        if let Some(end) = after.find(']')
            && let Ok(index) = after[..end].parse::<i32>()
        {
            return index.max(1);
        }
    }

    // Try _doc_fragment_N_ format (0-based index)
    if let Some(start) = progress.find("_doc_fragment_") {
        let after = &progress[start + 14..];
        if let Some(end) = after.find('_')
            && let Ok(index) = after[..end].parse::<i32>()
        {
            return (index + 1).max(1); // Convert 0-based to 1-based
        }
    }

    // Fallback: try parsing as plain number
    progress.parse::<i32>().unwrap_or(1).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pdf_progress() {
        assert_eq!(parse_koreader_progress("42", "cbz"), 42);
        assert_eq!(parse_koreader_progress("1", "pdf"), 1);
        assert_eq!(parse_koreader_progress("0", "cbr"), 1); // min 1
        assert_eq!(parse_koreader_progress("invalid", "pdf"), 1);
    }

    #[test]
    fn test_parse_epub_doc_fragment() {
        assert_eq!(
            parse_koreader_progress("/body/DocFragment[10]/body/div/p[1]/text().0", "epub"),
            10
        );
        assert_eq!(parse_koreader_progress("/body/DocFragment[1].0", "epub"), 1);
    }

    #[test]
    fn test_parse_epub_doc_fragment_underscore() {
        assert_eq!(
            parse_koreader_progress("#_doc_fragment_44_ c37", "epub"),
            45 // 0-based 44 -> 1-based 45
        );
        assert_eq!(
            parse_koreader_progress("#_doc_fragment_0_ c0", "epub"),
            1 // 0-based 0 -> 1-based 1
        );
    }

    #[test]
    fn test_parse_epub_plain_number() {
        assert_eq!(parse_koreader_progress("5", "epub"), 5);
    }
}
