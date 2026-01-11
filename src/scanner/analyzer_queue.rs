use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{prelude::Decimal, DatabaseConnection};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::{book_metadata_records, books, pages};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, PageRepository, SeriesMetadataRepository,
    SeriesRepository, TaskRepository,
};
use crate::events::EventBroadcaster;
use crate::scanner::analyze_file;
use crate::tasks::types::TaskType;

use super::types::ScanProgress;

/// Result of analyzing a batch of books
#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub books_analyzed: usize,
    pub errors: Vec<String>,
}

/// Analyze a single book
///
/// # Arguments
/// * `force` - If true, bypass full hash check and force re-analysis even if file hasn't changed
/// * `event_broadcaster` - Optional event broadcaster for emitting entity change events
pub async fn analyze_book(
    db: &DatabaseConnection,
    book_id: Uuid,
    force: bool,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<AnalysisResult> {
    let analysis_start = Instant::now();
    info!("Starting analysis for book {} (force={})", book_id, force);

    // Get the book
    let book = BookRepository::get_by_id(db, book_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Book not found"))?;

    let mut result = AnalysisResult::default();

    match analyze_single_book(db, book, None, force, event_broadcaster).await {
        Ok(_) => {
            result.books_analyzed = 1;
            // Clear any previous analysis error on success
            if let Err(e) = BookRepository::set_analysis_error(db, book_id, None).await {
                warn!("Failed to clear analysis error for book {}: {}", book_id, e);
            }
            info!(
                "Analysis completed for book {} in {:?}",
                book_id,
                analysis_start.elapsed()
            );
        }
        Err(e) => {
            let error_msg = format!("Failed to analyze book {}: {}", book_id, e);
            error!("{}", error_msg);
            // Store the analysis error for UI display
            if let Err(set_err) =
                BookRepository::set_analysis_error(db, book_id, Some(error_msg.clone())).await
            {
                warn!(
                    "Failed to set analysis error for book {}: {}",
                    book_id, set_err
                );
            }
            result.errors.push(error_msg);
        }
    }

    Ok(result)
}

/// Analyze a single book and update the database
async fn analyze_single_book(
    db: &DatabaseConnection,
    mut book: books::Model,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
    force: bool,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<()> {
    let analyze_start = Instant::now();
    let file_path = PathBuf::from(&book.file_path);

    debug!("Analyzing book: {} (force={})", book.file_path, force);

    // FULL HASH VERIFICATION PHASE:
    // Before expensive analysis, verify the file actually changed using full hash
    // This catches false positives from partial hash changes (e.g., Docker mount issues)
    // Skip this check if force=true
    if !force && !book.file_hash.is_empty() {
        // Book was previously analyzed and has a full hash
        // Compute full hash to verify the file actually changed
        let file_path_clone = file_path.clone();
        let current_full_hash = tokio::task::spawn_blocking(move || {
            use crate::utils::hasher::hash_file;
            hash_file(&file_path_clone)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn full hash calculation: {}", e))??;

        if current_full_hash == book.file_hash {
            // Full hash unchanged - false positive from partial hash
            // Update partial_hash to match current state and skip analysis
            let file_path_clone2 = file_path.clone();
            let current_partial_hash = tokio::task::spawn_blocking(move || {
                use crate::utils::hasher::hash_file_partial;
                hash_file_partial(&file_path_clone2)
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to spawn partial hash calculation: {}", e))??;

            book.partial_hash = current_partial_hash;
            book.analyzed = true; // Mark as analyzed since nothing changed
            book.updated_at = chrono::Utc::now();
            BookRepository::update(db, &book, event_broadcaster).await?;

            debug!(
                "Skipping analysis - full hash unchanged (false positive from partial hash): {}",
                book.file_path
            );
            return Ok(());
        } else {
            debug!(
                "Full hash verification confirmed change: {} - proceeding with analysis",
                book.file_path
            );
        }
    }

    // Analyze the file (blocking I/O operation)
    let file_path_clone = file_path.clone();
    let metadata = tokio::task::spawn_blocking(move || analyze_file(&file_path_clone))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn file analysis task: {}", e))?
        .with_context(|| format!("Failed to analyze file: {}", book.file_path))?;

    let analyze_duration = analyze_start.elapsed();

    if analyze_duration.as_millis() > 500 {
        debug!(
            "File analysis took {:?}: {}",
            analyze_duration,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    // Compute partial hash to keep both hashes in sync
    let file_path_clone2 = file_path.clone();
    let partial_hash = tokio::task::spawn_blocking(move || {
        use crate::utils::hasher::hash_file_partial;
        hash_file_partial(&file_path_clone2)
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn partial hash calculation: {}", e))??;

    // Update book with analyzed metadata
    let now = Utc::now();

    // Track if cover is becoming available (page_count going from 0 to positive)
    let cover_now_available = book.page_count == 0 && metadata.page_count > 0;

    // Extract title from metadata, or fall back to filename without extension
    // Handle both None and empty string cases
    book.title = metadata
        .comic_info
        .as_ref()
        .and_then(|ci| ci.title.clone())
        .filter(|title| !title.is_empty()) // Filter out empty strings
        .or_else(|| {
            // Fallback to filename without extension
            let file_name = &book.file_name;
            if let Some(pos) = file_name.rfind('.') {
                Some(file_name[..pos].to_string())
            } else {
                Some(file_name.clone())
            }
        });
    book.number = metadata.comic_info.as_ref().and_then(|ci| {
        ci.number
            .as_ref()
            .and_then(|n| n.parse::<f64>().ok())
            .map(|v| Decimal::from_f64_retain(v).unwrap_or_default())
    });
    book.file_size = metadata.file_size as i64;
    book.file_hash = metadata.file_hash.clone();
    book.partial_hash = partial_hash;
    book.format = format!("{:?}", metadata.format).to_lowercase();
    book.page_count = metadata.page_count as i32;
    book.modified_at = metadata.modified_at;
    book.analyzed = true; // Mark as analyzed
    book.analysis_error = None; // Clear any previous error on successful analysis
    book.updated_at = now;

    BookRepository::update(db, &book, event_broadcaster).await?;

    // Emit CoverUpdated event and queue thumbnail generation if cover became available
    if cover_now_available {
        if let Some(broadcaster) = event_broadcaster {
            // Get library_id from series
            if let Ok(Some(series)) = SeriesRepository::get_by_id(db, book.series_id).await {
                use crate::events::{EntityChangeEvent, EntityEvent, EntityType};

                let event = EntityChangeEvent {
                    event: EntityEvent::CoverUpdated {
                        entity_type: EntityType::Book,
                        entity_id: book.id,
                        library_id: Some(series.library_id),
                    },
                    user_id: None,
                    timestamp: Utc::now(),
                };

                match broadcaster.emit(event) {
                    Ok(count) => {
                        debug!(
                            "Emitted CoverUpdated event to {} subscribers for book: {}",
                            count, book.id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to emit CoverUpdated event for book {}: {:?}",
                            book.id, e
                        );
                    }
                }

                // Queue thumbnail generation task for this book's library
                // This will batch with other books analyzed around the same time
                let task_type = TaskType::GenerateThumbnails {
                    library_id: Some(series.library_id),
                };

                match TaskRepository::enqueue(db, task_type, 0, None).await {
                    Ok(task_id) => {
                        debug!(
                            "Queued thumbnail generation task {} for library {} after book analysis",
                            task_id, series.library_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to queue thumbnail generation task for library {}: {:?}",
                            series.library_id, e
                        );
                    }
                }
            }
        }
    }

    // Save ComicInfo metadata to book_metadata_records table if available
    if let Some(comic_info) = &metadata.comic_info {
        // Convert ISBNs Vec<String> to JSON string for storage
        let isbns_json = if !metadata.isbns.is_empty() {
            Some(serde_json::to_string(&metadata.isbns).unwrap_or_default())
        } else {
            None
        };

        // Parse black_and_white and manga fields
        let black_and_white =
            comic_info
                .black_and_white
                .as_ref()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "yes" | "true" | "1" => Some(true),
                    "no" | "false" | "0" => Some(false),
                    _ => None,
                });

        let manga = comic_info
            .manga
            .as_ref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "yes" | "true" | "1" | "yesandrighttoleft" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            });

        // Check if metadata already exists to preserve the ID
        let existing_metadata = BookMetadataRepository::get_by_book_id(db, book.id).await?;
        let metadata_id = existing_metadata
            .as_ref()
            .map(|m| m.id)
            .unwrap_or_else(Uuid::new_v4);

        let metadata_record = book_metadata_records::Model {
            id: metadata_id,
            book_id: book.id,
            summary: comic_info.summary.clone(),
            writer: comic_info.writer.clone(),
            penciller: comic_info.penciller.clone(),
            inker: comic_info.inker.clone(),
            colorist: comic_info.colorist.clone(),
            letterer: comic_info.letterer.clone(),
            cover_artist: comic_info.cover_artist.clone(),
            editor: comic_info.editor.clone(),
            publisher: comic_info.publisher.clone(),
            imprint: comic_info.imprint.clone(),
            genre: comic_info.genre.clone(),
            web: comic_info.web.clone(),
            language_iso: comic_info.language_iso.clone(),
            format_detail: comic_info.format.clone(),
            black_and_white,
            manga,
            year: comic_info.year,
            month: comic_info.month,
            day: comic_info.day,
            volume: comic_info.volume,
            count: comic_info.count,
            isbns: isbns_json,
            created_at: now,
            updated_at: now,
        };

        BookMetadataRepository::upsert(db, &metadata_record).await?;
        debug!(
            "Saved metadata for book: {} ({} fields)",
            book.file_path,
            count_non_null_fields(&metadata_record)
        );

        // Populate series metadata from the first book if not already populated
        if let Ok(Some(series)) = SeriesRepository::get_by_id(db, book.series_id).await {
            // Get series metadata to check if it needs population
            if let Ok(Some(metadata)) =
                SeriesMetadataRepository::get_by_series_id(db, book.series_id).await
            {
                // Only populate if series metadata doesn't have summary, publisher, or year yet
                // and the fields are not locked
                let should_populate = metadata.summary.is_none()
                    && metadata.publisher.is_none()
                    && metadata.year.is_none()
                    && !metadata.summary_lock
                    && !metadata.publisher_lock
                    && !metadata.year_lock;

                if should_populate
                    && (comic_info.summary.is_some()
                        || comic_info.publisher.is_some()
                        || comic_info.year.is_some())
                {
                    // Populate series metadata from book's ComicInfo
                    use crate::db::entities::series_metadata;
                    use sea_orm::{ActiveModelTrait, Set};

                    let mut metadata_active: series_metadata::ActiveModel = metadata.into();

                    if let Some(ref summary) = comic_info.summary {
                        metadata_active.summary = Set(Some(summary.clone()));
                    }
                    if let Some(ref publisher) = comic_info.publisher {
                        metadata_active.publisher = Set(Some(publisher.clone()));
                    }
                    if let Some(year) = comic_info.year {
                        metadata_active.year = Set(Some(year));
                    }
                    metadata_active.updated_at = Set(Utc::now());

                    metadata_active.update(db).await?;
                    info!(
                        "Populated series '{}' metadata from book: {}",
                        series.name, book.file_path
                    );
                }
            }
        }
    }

    // Save page information to pages table
    if !metadata.pages.is_empty() {
        // Delete existing pages first to handle re-analysis
        PageRepository::delete_by_book(db, book.id).await?;

        // Convert PageInfo to pages::Model
        let page_models: Vec<pages::Model> = metadata
            .pages
            .iter()
            .map(|page_info| pages::Model {
                id: Uuid::new_v4(),
                book_id: book.id,
                page_number: page_info.page_number as i32,
                file_name: page_info.file_name.clone(),
                format: format!("{:?}", page_info.format).to_lowercase(),
                width: page_info.width as i32,
                height: page_info.height as i32,
                file_size: page_info.file_size as i64,
                created_at: now,
            })
            .collect();

        // Batch insert all pages for efficiency
        PageRepository::create_batch(db, &page_models).await?;
        debug!(
            "Saved {} pages for book: {}",
            page_models.len(),
            book.file_path
        );
    }

    debug!(
        "Analyzed and updated book: {} (took {:?})",
        book.file_path, analyze_duration
    );

    // Send progress update if channel is provided
    if let Some(tx) = progress_tx {
        // Note: You may want to update ScanProgress to track analysis progress
        // For now, we just send an update to indicate activity
        let _ = tx.send(ScanProgress::new(Uuid::nil())).await;
    }

    Ok(())
}

/// Helper function to count non-null fields in metadata for logging
fn count_non_null_fields(metadata: &book_metadata_records::Model) -> usize {
    let mut count = 0;
    if metadata.summary.is_some() {
        count += 1;
    }
    if metadata.writer.is_some() {
        count += 1;
    }
    if metadata.penciller.is_some() {
        count += 1;
    }
    if metadata.inker.is_some() {
        count += 1;
    }
    if metadata.colorist.is_some() {
        count += 1;
    }
    if metadata.letterer.is_some() {
        count += 1;
    }
    if metadata.cover_artist.is_some() {
        count += 1;
    }
    if metadata.editor.is_some() {
        count += 1;
    }
    if metadata.publisher.is_some() {
        count += 1;
    }
    if metadata.imprint.is_some() {
        count += 1;
    }
    if metadata.genre.is_some() {
        count += 1;
    }
    if metadata.web.is_some() {
        count += 1;
    }
    if metadata.language_iso.is_some() {
        count += 1;
    }
    if metadata.format_detail.is_some() {
        count += 1;
    }
    if metadata.black_and_white.is_some() {
        count += 1;
    }
    if metadata.manga.is_some() {
        count += 1;
    }
    if metadata.year.is_some() {
        count += 1;
    }
    if metadata.month.is_some() {
        count += 1;
    }
    if metadata.day.is_some() {
        count += 1;
    }
    if metadata.volume.is_some() {
        count += 1;
    }
    if metadata.count.is_some() {
        count += 1;
    }
    if metadata.isbns.is_some() {
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_non_null_fields() {
        let metadata = book_metadata_records::Model {
            id: Uuid::new_v4(),
            book_id: Uuid::new_v4(),
            summary: Some("Test summary".to_string()),
            writer: Some("Test Writer".to_string()),
            penciller: None,
            inker: None,
            colorist: None,
            letterer: None,
            cover_artist: None,
            editor: None,
            publisher: Some("Test Publisher".to_string()),
            imprint: None,
            genre: Some("Action".to_string()),
            web: None,
            language_iso: Some("en".to_string()),
            format_detail: None,
            black_and_white: None,
            manga: Some(false),
            year: Some(2024),
            month: None,
            day: None,
            volume: None,
            count: None,
            isbns: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(count_non_null_fields(&metadata), 7);
    }

    /// Test filename extraction logic (unit test for the title fallback logic)
    #[test]
    fn test_extract_title_from_filename() {
        // Helper function that mimics the filename extraction logic
        fn extract_title_from_filename(file_name: &str) -> String {
            if let Some(pos) = file_name.rfind('.') {
                file_name[..pos].to_string()
            } else {
                file_name.to_string()
            }
        }

        // Test standard filename with extension
        assert_eq!(extract_title_from_filename("my_book.cbz"), "my_book");
        assert_eq!(extract_title_from_filename("comic.epub"), "comic");
        assert_eq!(extract_title_from_filename("document.pdf"), "document");

        // Test filename with multiple dots (should use last dot)
        assert_eq!(extract_title_from_filename("book.vol.1.cbz"), "book.vol.1");
        assert_eq!(
            extract_title_from_filename("my.comic.book.cbz"),
            "my.comic.book"
        );

        // Test filename with no extension
        assert_eq!(extract_title_from_filename("noextension"), "noextension");
        assert_eq!(extract_title_from_filename("book"), "book");

        // Test filename starting with dot
        assert_eq!(extract_title_from_filename(".hidden"), "");

        // Test filename ending with dot
        assert_eq!(extract_title_from_filename("book."), "book");

        // Test empty filename
        assert_eq!(extract_title_from_filename(""), "");

        // Test filename with only extension
        assert_eq!(extract_title_from_filename(".cbz"), "");
    }
}
