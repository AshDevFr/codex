//! KOReader sync progress handlers
//!
//! Converts between KOReader's DocFragment format and Codex's R2Progression
//! (Readium standard) so that progress is shared across all clients (web reader,
//! KOReader, OPDS apps).

use crate::api::error::ApiError;
use crate::api::extractors::{AuthContext, AuthState};
use crate::api::permissions::Permission;
use crate::api::routes::koreader::dto::progress::DocumentProgressDto;
use crate::db::entities::books;
use crate::db::repositories::{BookRepository, ReadProgressRepository};
use crate::parsers::EpubPosition;
use axum::Json;
use axum::extract::{Path, State};
use std::sync::Arc;

/// GET /koreader/syncs/progress/{document}
///
/// Get reading progress for a document identified by its KOReader hash.
/// Converts stored R2Progression back to KOReader's DocFragment format.
pub async fn get_progress(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(document_hash): Path<String>,
) -> Result<Json<DocumentProgressDto>, ApiError> {
    auth.require_permission(&Permission::ProgressRead)?;
    let user_id = auth.user_id;

    let book = find_book_by_hash(&state, &document_hash).await?;

    let progress = ReadProgressRepository::get_by_user_and_book(&state.db, user_id, book.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get progress: {}", e)))?;

    match progress {
        Some(p) => {
            let percentage = p
                .progress_percentage
                .unwrap_or_else(|| p.current_page as f64 / book.page_count.max(1) as f64);

            // Convert R2Progression back to KOReader format
            let progress_str = if book.format == "epub" {
                r2_progression_to_koreader(&p.r2_progression, &book)
                    .unwrap_or_else(|| p.current_page.to_string())
            } else {
                p.current_page.to_string()
            };

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
/// Converts KOReader's DocFragment format to R2Progression for unified storage.
pub async fn update_progress(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<DocumentProgressDto>,
) -> Result<Json<DocumentProgressDto>, ApiError> {
    auth.require_permission(&Permission::ProgressWrite)?;
    let user_id = auth.user_id;

    tracing::debug!(
        koreader_hash = %request.document,
        progress = %request.progress,
        percentage = request.percentage,
        device = %request.device,
        device_id = %request.device_id,
        "KOReader progress update request"
    );

    let book = find_book_by_hash(&state, &request.document).await?;

    // Parse progress string to page number
    let current_page = parse_koreader_progress(&request.progress, &book.format);

    // For EPUB: convert KOReader progress to R2Progression JSON
    let r2_progression = if book.format == "epub" {
        koreader_to_r2_progression(&request.progress, request.percentage, &book)
    } else {
        None
    };

    let completed =
        request.percentage >= 0.98 || (book.page_count > 0 && current_page >= book.page_count);

    ReadProgressRepository::upsert_with_percentage(
        &state.db,
        user_id,
        book.id,
        current_page,
        Some(request.percentage),
        completed,
        r2_progression,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update progress: {}", e)))?;

    Ok(Json(request))
}

/// Find a single book by KOReader hash, returning appropriate errors.
async fn find_book_by_hash(state: &AuthState, hash: &str) -> Result<books::Model, ApiError> {
    let books = BookRepository::find_by_koreader_hash(&state.db, hash)
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

    Ok(books.into_iter().next().unwrap())
}

/// Get the unique hrefs (spine items) from epub_positions, preserving order.
fn get_spine_hrefs(positions: &[EpubPosition]) -> Vec<&str> {
    let mut hrefs: Vec<&str> = Vec::new();
    for p in positions {
        if hrefs.last().is_none_or(|last| *last != p.href.as_str()) {
            hrefs.push(&p.href);
        }
    }
    hrefs
}

/// Parse the book's epub_positions JSON.
fn parse_epub_positions(book: &books::Model) -> Option<Vec<EpubPosition>> {
    book.epub_positions
        .as_ref()
        .and_then(|json| serde_json::from_str::<Vec<EpubPosition>>(json).ok())
}

/// Convert KOReader DocFragment progress to R2Progression JSON.
///
/// Maps DocFragment index (1-based spine item) to the corresponding EPUB href
/// from the book's positions list, then builds an R2Progression object.
fn koreader_to_r2_progression(
    progress: &str,
    percentage: f64,
    book: &books::Model,
) -> Option<String> {
    let doc_fragment_index = parse_epub_progress(progress);
    let positions = parse_epub_positions(book)?;
    let hrefs = get_spine_hrefs(&positions);

    // DocFragment index is 1-based, convert to 0-based
    let spine_index = (doc_fragment_index - 1).max(0) as usize;
    let href = hrefs.get(spine_index)?;

    let r2 = serde_json::json!({
        "locator": {
            "href": href,
            "type": "application/xhtml+xml",
            "locations": {
                "totalProgression": percentage
            }
        },
        "device": {
            "id": "koreader",
            "name": "KOReader"
        },
        "modified": chrono::Utc::now().to_rfc3339()
    });

    serde_json::to_string(&r2).ok()
}

/// Convert stored R2Progression back to KOReader DocFragment format.
///
/// Finds which spine index the stored href corresponds to, then returns
/// `/body/DocFragment[N].0` where N is the 1-based index.
fn r2_progression_to_koreader(r2_json: &Option<String>, book: &books::Model) -> Option<String> {
    let json_str = r2_json.as_ref()?;
    let r2: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let href = r2.get("locator")?.get("href")?.as_str()?;

    let positions = parse_epub_positions(book)?;
    let hrefs = get_spine_hrefs(&positions);

    // Find which spine index this href corresponds to (with suffix matching)
    let href_clean = href.split('#').next().unwrap_or(href);
    let href_decoded = urlencoding::decode(href_clean).unwrap_or_else(|_| href_clean.into());

    let spine_index = hrefs.iter().position(|h| {
        *h == href_decoded.as_ref()
            || href_decoded.ends_with(*h)
            || h.ends_with(href_decoded.as_ref())
    })?;

    // Convert 0-based spine index to 1-based DocFragment index
    Some(format!("/body/DocFragment[{}].0", spine_index + 1))
}

/// Parse KOReader progress string into a page number
///
/// For PDF/CBZ (pre-paginated): progress is just a page number string like "42"
/// For EPUB: progress is a DocFragment XPath, extract the spine index
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

    #[test]
    fn test_get_spine_hrefs() {
        let positions = vec![
            EpubPosition {
                href: "ch1.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                progression: 0.0,
                position: 1,
                total_progression: 0.0,
            },
            EpubPosition {
                href: "ch1.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                progression: 0.5,
                position: 2,
                total_progression: 0.1,
            },
            EpubPosition {
                href: "ch2.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                progression: 0.0,
                position: 3,
                total_progression: 0.5,
            },
        ];
        let hrefs = get_spine_hrefs(&positions);
        assert_eq!(hrefs, vec!["ch1.xhtml", "ch2.xhtml"]);
    }

    /// Helper to create a test book model with given epub_positions.
    fn test_book(positions: &[EpubPosition]) -> books::Model {
        books::Model {
            id: uuid::Uuid::new_v4(),
            library_id: uuid::Uuid::new_v4(),
            series_id: uuid::Uuid::new_v4(),
            file_path: String::new(),
            file_name: String::new(),
            format: "epub".to_string(),
            file_size: 0,
            file_hash: String::new(),
            partial_hash: String::new(),
            koreader_hash: None,
            page_count: positions.len() as i32,
            deleted: false,
            analyzed: true,
            analysis_error: None,
            analysis_errors: None,
            modified_at: chrono::Utc::now(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            epub_positions: Some(serde_json::to_string(positions).unwrap()),
            epub_spine_items: None,
        }
    }

    fn pos(href: &str, position: i32, total_progression: f64) -> EpubPosition {
        EpubPosition {
            href: href.to_string(),
            media_type: "application/xhtml+xml".to_string(),
            progression: 0.0,
            position,
            total_progression,
        }
    }

    #[test]
    fn test_roundtrip_doc_fragment_format() {
        // 3 spine items, each with 1 position
        let positions = vec![
            pos("OEBPS/ch1.xhtml", 1, 0.0),
            pos("OEBPS/ch2.xhtml", 2, 0.33),
            pos("OEBPS/ch3.xhtml", 3, 0.66),
        ];
        let book = test_book(&positions);

        // Test each DocFragment index roundtrips correctly
        for i in 1..=3 {
            let input = format!("/body/DocFragment[{}]/body/div/p[1]/text().0", i);
            let r2 = koreader_to_r2_progression(&input, i as f64 / 3.0, &book);
            assert!(r2.is_some(), "Failed to create R2 for DocFragment[{}]", i);

            let result = r2_progression_to_koreader(&r2, &book);
            assert_eq!(
                result.unwrap(),
                format!("/body/DocFragment[{}].0", i),
                "Roundtrip failed for DocFragment[{}]",
                i
            );
        }
    }

    #[test]
    fn test_roundtrip_doc_fragment_underscore_format() {
        let positions = vec![pos("ch1.xhtml", 1, 0.0), pos("ch2.xhtml", 2, 0.5)];
        let book = test_book(&positions);

        // _doc_fragment_0_ is 0-based -> DocFragment[1] (1-based)
        let r2 = koreader_to_r2_progression("#_doc_fragment_0_ c37", 0.1, &book);
        assert!(r2.is_some());
        let result = r2_progression_to_koreader(&r2, &book);
        assert_eq!(result.unwrap(), "/body/DocFragment[1].0");

        // _doc_fragment_1_ -> DocFragment[2]
        let r2 = koreader_to_r2_progression("#_doc_fragment_1_ c10", 0.6, &book);
        assert!(r2.is_some());
        let result = r2_progression_to_koreader(&r2, &book);
        assert_eq!(result.unwrap(), "/body/DocFragment[2].0");
    }

    #[test]
    fn test_roundtrip_multiple_positions_per_spine() {
        // ch1 has 3 positions, ch2 has 2 positions (like a real EPUB)
        let positions = vec![
            pos("OEBPS/ch1.xhtml", 1, 0.0),
            pos("OEBPS/ch1.xhtml", 2, 0.1),
            pos("OEBPS/ch1.xhtml", 3, 0.2),
            pos("OEBPS/ch2.xhtml", 4, 0.5),
            pos("OEBPS/ch2.xhtml", 5, 0.7),
        ];
        let book = test_book(&positions);

        // DocFragment[1] -> ch1.xhtml
        let r2 = koreader_to_r2_progression("/body/DocFragment[1].0", 0.1, &book);
        let result = r2_progression_to_koreader(&r2, &book);
        assert_eq!(result.unwrap(), "/body/DocFragment[1].0");

        // DocFragment[2] -> ch2.xhtml
        let r2 = koreader_to_r2_progression("/body/DocFragment[2].0", 0.6, &book);
        let result = r2_progression_to_koreader(&r2, &book);
        assert_eq!(result.unwrap(), "/body/DocFragment[2].0");
    }

    #[test]
    fn test_roundtrip_preserves_percentage_in_r2() {
        let positions = vec![pos("ch1.xhtml", 1, 0.0)];
        let book = test_book(&positions);

        let r2_json = koreader_to_r2_progression("/body/DocFragment[1].0", 0.42, &book).unwrap();
        let r2: serde_json::Value = serde_json::from_str(&r2_json).unwrap();

        let tp = r2["locator"]["locations"]["totalProgression"]
            .as_f64()
            .unwrap();
        assert!((tp - 0.42).abs() < f64::EPSILON);
    }

    #[test]
    fn test_no_epub_positions_returns_none() {
        let mut book = test_book(&[]);
        book.epub_positions = None;

        let r2 = koreader_to_r2_progression("/body/DocFragment[1].0", 0.5, &book);
        assert!(r2.is_none());

        let result = r2_progression_to_koreader(&Some("{}".to_string()), &book);
        assert!(result.is_none());
    }

    #[test]
    fn test_web_reader_r2_to_koreader() {
        // Simulates: web reader stores R2Progression, KOReader reads it back
        let positions = vec![
            pos("OEBPS/ch1.xhtml", 1, 0.0),
            pos("OEBPS/ch2.xhtml", 2, 0.5),
            pos("OEBPS/ch3.xhtml", 3, 0.8),
        ];
        let book = test_book(&positions);

        // Web reader stores R2Progression with href
        let web_r2 = serde_json::to_string(&serde_json::json!({
            "locator": {
                "href": "OEBPS/ch2.xhtml",
                "type": "application/xhtml+xml",
                "locations": {
                    "totalProgression": 0.55
                }
            }
        }))
        .unwrap();

        // KOReader should get DocFragment[2] back
        let result = r2_progression_to_koreader(&Some(web_r2), &book);
        assert_eq!(result.unwrap(), "/body/DocFragment[2].0");
    }
}
