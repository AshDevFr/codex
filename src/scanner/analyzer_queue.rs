use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{prelude::Decimal, DatabaseConnection};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::entities::book_error::{BookError, BookErrorType};
use crate::db::entities::{book_metadata, books, pages};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, PageRepository,
    SeriesMetadataRepository, SeriesRepository, TaskRepository,
};
use crate::events::EventBroadcaster;
use crate::models::{BookStrategy, NumberStrategy};
use crate::scanner::analyze_file;
use crate::scanner::strategies::{
    create_book_strategy, create_number_strategy, BookMetadata, BookNamingContext, NumberContext,
    NumberMetadata,
};
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
            // Clear all analysis-related errors on success (parser, metadata, page_extraction, format_detection)
            // Note: We don't clear thumbnail errors here - those are handled by thumbnail generation
            for error_type in [
                BookErrorType::Parser,
                BookErrorType::Metadata,
                BookErrorType::PageExtraction,
                BookErrorType::FormatDetection,
                BookErrorType::Other,
            ] {
                if let Err(e) = BookRepository::clear_error(db, book_id, error_type).await {
                    warn!(
                        "Failed to clear {:?} error for book {}: {}",
                        error_type, book_id, e
                    );
                }
            }
            info!(
                "Analysis completed for book {} in {:?}",
                book_id,
                analysis_start.elapsed()
            );
        }
        Err(e) => {
            // Categorize the error based on its type
            let (error_type, error_msg) = categorize_analysis_error(&e);
            let full_error_msg = format!("Failed to analyze book {}: {}", book_id, e);
            error!("{}", full_error_msg);

            // Store the categorized error for UI display
            let book_error = BookError::new(error_msg);
            if let Err(set_err) =
                BookRepository::set_error(db, book_id, error_type, book_error).await
            {
                warn!(
                    "Failed to set {:?} error for book {}: {}",
                    error_type, book_id, set_err
                );
            }
            result.errors.push(full_error_msg);
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

    // Get book number from metadata (for use in title resolution)
    let metadata_number: Option<f32> = metadata
        .comic_info
        .as_ref()
        .and_then(|ci| ci.number.as_ref())
        .and_then(|n| n.parse::<f32>().ok());

    // Resolve book number using the library's number strategy
    let resolved_number = resolve_book_number(db, &book, &metadata, metadata_number).await;

    // Resolve book title using the library's book naming strategy
    // Note: title and number are now stored in book_metadata, not books table
    let resolved_title = resolve_book_title(db, &book, &metadata, resolved_number).await;
    let resolved_number_decimal =
        resolved_number.map(|n| Decimal::from_f64_retain(n as f64).unwrap_or_default());

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

    // Queue thumbnail generation for this book
    // - For new books (cover_now_available): generate thumbnail (force=false since it doesn't exist)
    // - For force re-analysis: regenerate thumbnail (force=true to replace existing)
    // - For file changes: regenerate thumbnail (force=true to replace existing)
    // Note: We only reach this point if the file actually changed or force=true,
    // since unchanged files return early after full hash verification
    let should_generate_thumbnail = cover_now_available || force || metadata.page_count > 0;
    let thumbnail_force = force || !cover_now_available; // Force if re-analyzing, not if new book

    if should_generate_thumbnail {
        // Queue per-book thumbnail generation task
        let task_type = TaskType::GenerateThumbnail {
            book_id: book.id,
            force: thumbnail_force,
        };

        match TaskRepository::enqueue(db, task_type, 0, None).await {
            Ok(task_id) => {
                debug!(
                    "Queued thumbnail generation task {} for book {} (force={})",
                    task_id, book.id, thumbnail_force
                );
            }
            Err(e) => {
                warn!(
                    "Failed to queue thumbnail generation task for book {}: {:?}",
                    book.id, e
                );
            }
        }

        // Emit CoverUpdated event if cover became available
        if cover_now_available {
            if let Some(broadcaster) = event_broadcaster {
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

        // Check if metadata already exists to preserve the ID and respect locks
        let existing_metadata = BookMetadataRepository::get_by_book_id(db, book.id).await?;
        let metadata_id = existing_metadata
            .as_ref()
            .map(|m| m.id)
            .unwrap_or_else(Uuid::new_v4);

        // Build metadata record, respecting locks on existing fields
        let metadata_record = if let Some(ref existing) = existing_metadata {
            // Only update fields that are not locked
            book_metadata::Model {
                id: metadata_id,
                book_id: book.id,
                // Display fields (title, title_sort, number)
                title: if existing.title_lock {
                    existing.title.clone()
                } else {
                    Some(resolved_title.clone())
                },
                title_sort: if existing.title_sort_lock {
                    existing.title_sort.clone()
                } else {
                    None // title_sort is typically user-set, not auto-generated
                },
                number: if existing.number_lock {
                    existing.number
                } else {
                    resolved_number_decimal
                },
                summary: if existing.summary_lock {
                    existing.summary.clone()
                } else {
                    comic_info.summary.clone()
                },
                writer: if existing.writer_lock {
                    existing.writer.clone()
                } else {
                    comic_info.writer.clone()
                },
                penciller: if existing.penciller_lock {
                    existing.penciller.clone()
                } else {
                    comic_info.penciller.clone()
                },
                inker: if existing.inker_lock {
                    existing.inker.clone()
                } else {
                    comic_info.inker.clone()
                },
                colorist: if existing.colorist_lock {
                    existing.colorist.clone()
                } else {
                    comic_info.colorist.clone()
                },
                letterer: if existing.letterer_lock {
                    existing.letterer.clone()
                } else {
                    comic_info.letterer.clone()
                },
                cover_artist: if existing.cover_artist_lock {
                    existing.cover_artist.clone()
                } else {
                    comic_info.cover_artist.clone()
                },
                editor: if existing.editor_lock {
                    existing.editor.clone()
                } else {
                    comic_info.editor.clone()
                },
                publisher: if existing.publisher_lock {
                    existing.publisher.clone()
                } else {
                    comic_info.publisher.clone()
                },
                imprint: if existing.imprint_lock {
                    existing.imprint.clone()
                } else {
                    comic_info.imprint.clone()
                },
                genre: if existing.genre_lock {
                    existing.genre.clone()
                } else {
                    comic_info.genre.clone()
                },
                web: if existing.web_lock {
                    existing.web.clone()
                } else {
                    comic_info.web.clone()
                },
                language_iso: if existing.language_iso_lock {
                    existing.language_iso.clone()
                } else {
                    comic_info.language_iso.clone()
                },
                format_detail: if existing.format_detail_lock {
                    existing.format_detail.clone()
                } else {
                    comic_info.format.clone()
                },
                black_and_white: if existing.black_and_white_lock {
                    existing.black_and_white
                } else {
                    black_and_white
                },
                manga: if existing.manga_lock {
                    existing.manga
                } else {
                    manga
                },
                year: if existing.year_lock {
                    existing.year
                } else {
                    comic_info.year
                },
                month: if existing.month_lock {
                    existing.month
                } else {
                    comic_info.month
                },
                day: if existing.day_lock {
                    existing.day
                } else {
                    comic_info.day
                },
                volume: if existing.volume_lock {
                    existing.volume
                } else {
                    comic_info.volume
                },
                count: if existing.count_lock {
                    existing.count
                } else {
                    comic_info.count
                },
                isbns: if existing.isbns_lock {
                    existing.isbns.clone()
                } else {
                    isbns_json.clone()
                },
                // Preserve existing lock states
                title_lock: existing.title_lock,
                title_sort_lock: existing.title_sort_lock,
                number_lock: existing.number_lock,
                summary_lock: existing.summary_lock,
                writer_lock: existing.writer_lock,
                penciller_lock: existing.penciller_lock,
                inker_lock: existing.inker_lock,
                colorist_lock: existing.colorist_lock,
                letterer_lock: existing.letterer_lock,
                cover_artist_lock: existing.cover_artist_lock,
                editor_lock: existing.editor_lock,
                publisher_lock: existing.publisher_lock,
                imprint_lock: existing.imprint_lock,
                genre_lock: existing.genre_lock,
                web_lock: existing.web_lock,
                language_iso_lock: existing.language_iso_lock,
                format_detail_lock: existing.format_detail_lock,
                black_and_white_lock: existing.black_and_white_lock,
                manga_lock: existing.manga_lock,
                year_lock: existing.year_lock,
                month_lock: existing.month_lock,
                day_lock: existing.day_lock,
                volume_lock: existing.volume_lock,
                count_lock: existing.count_lock,
                isbns_lock: existing.isbns_lock,
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            // No existing metadata, create new with all locks set to false
            book_metadata::Model {
                id: metadata_id,
                book_id: book.id,
                // Display fields
                title: Some(resolved_title.clone()),
                title_sort: None, // title_sort is typically user-set
                number: resolved_number_decimal,
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
                // All locks default to false for new records
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
                web_lock: false,
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
                created_at: now,
                updated_at: now,
            }
        };

        BookMetadataRepository::upsert(db, &metadata_record).await?;
        debug!(
            "Saved metadata for book: {} ({} fields)",
            book.file_path,
            count_non_null_fields(&metadata_record)
        );

        // Populate series metadata from the first book if not already populated
        if let Ok(Some(series_metadata_model)) =
            SeriesMetadataRepository::get_by_series_id(db, book.series_id).await
        {
            use crate::db::entities::series_metadata;
            use sea_orm::{ActiveModelTrait, Set};

            let series_title = series_metadata_model.title.clone();
            let mut needs_update = false;
            let mut metadata_active: series_metadata::ActiveModel =
                series_metadata_model.clone().into();

            // Update title_sort with title if not set and not locked
            if series_metadata_model.title_sort.is_none() && !series_metadata_model.title_sort_lock
            {
                metadata_active.title_sort = Set(Some(series_title.clone()));
                needs_update = true;
                debug!("Setting title_sort to '{}' for series", series_title);
            }

            // Only populate other fields if series metadata doesn't have summary, publisher, or year yet
            // and the fields are not locked
            let should_populate = series_metadata_model.summary.is_none()
                && series_metadata_model.publisher.is_none()
                && series_metadata_model.year.is_none()
                && !series_metadata_model.summary_lock
                && !series_metadata_model.publisher_lock
                && !series_metadata_model.year_lock;

            if should_populate
                && (comic_info.summary.is_some()
                    || comic_info.publisher.is_some()
                    || comic_info.year.is_some())
            {
                // Populate series metadata from book's ComicInfo
                if let Some(ref summary) = comic_info.summary {
                    metadata_active.summary = Set(Some(summary.clone()));
                }
                if let Some(ref publisher) = comic_info.publisher {
                    metadata_active.publisher = Set(Some(publisher.clone()));
                }
                if let Some(year) = comic_info.year {
                    metadata_active.year = Set(Some(year));
                }
                needs_update = true;
            }

            if needs_update {
                metadata_active.updated_at = Set(Utc::now());
                metadata_active.update(db).await?;
                info!(
                    "Updated series '{}' metadata from book: {}",
                    series_title, book.file_path
                );
            }
        }
    } else {
        // No ComicInfo, but we still need to store the resolved title and number
        let existing_metadata = BookMetadataRepository::get_by_book_id(db, book.id).await?;
        let metadata_id = existing_metadata
            .as_ref()
            .map(|m| m.id)
            .unwrap_or_else(Uuid::new_v4);

        let metadata_record = if let Some(ref existing) = existing_metadata {
            // Only update title/number if not locked
            book_metadata::Model {
                id: metadata_id,
                book_id: book.id,
                title: if existing.title_lock {
                    existing.title.clone()
                } else {
                    Some(resolved_title.clone())
                },
                title_sort: existing.title_sort.clone(),
                number: if existing.number_lock {
                    existing.number
                } else {
                    resolved_number_decimal
                },
                // Keep all existing values
                summary: existing.summary.clone(),
                writer: existing.writer.clone(),
                penciller: existing.penciller.clone(),
                inker: existing.inker.clone(),
                colorist: existing.colorist.clone(),
                letterer: existing.letterer.clone(),
                cover_artist: existing.cover_artist.clone(),
                editor: existing.editor.clone(),
                publisher: existing.publisher.clone(),
                imprint: existing.imprint.clone(),
                genre: existing.genre.clone(),
                web: existing.web.clone(),
                language_iso: existing.language_iso.clone(),
                format_detail: existing.format_detail.clone(),
                black_and_white: existing.black_and_white,
                manga: existing.manga,
                year: existing.year,
                month: existing.month,
                day: existing.day,
                volume: existing.volume,
                count: existing.count,
                isbns: existing.isbns.clone(),
                // Keep all lock states
                title_lock: existing.title_lock,
                title_sort_lock: existing.title_sort_lock,
                number_lock: existing.number_lock,
                summary_lock: existing.summary_lock,
                writer_lock: existing.writer_lock,
                penciller_lock: existing.penciller_lock,
                inker_lock: existing.inker_lock,
                colorist_lock: existing.colorist_lock,
                letterer_lock: existing.letterer_lock,
                cover_artist_lock: existing.cover_artist_lock,
                editor_lock: existing.editor_lock,
                publisher_lock: existing.publisher_lock,
                imprint_lock: existing.imprint_lock,
                genre_lock: existing.genre_lock,
                web_lock: existing.web_lock,
                language_iso_lock: existing.language_iso_lock,
                format_detail_lock: existing.format_detail_lock,
                black_and_white_lock: existing.black_and_white_lock,
                manga_lock: existing.manga_lock,
                year_lock: existing.year_lock,
                month_lock: existing.month_lock,
                day_lock: existing.day_lock,
                volume_lock: existing.volume_lock,
                count_lock: existing.count_lock,
                isbns_lock: existing.isbns_lock,
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            // No existing metadata, create new with just title and number
            book_metadata::Model {
                id: metadata_id,
                book_id: book.id,
                title: Some(resolved_title.clone()),
                title_sort: None,
                number: resolved_number_decimal,
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
                web: None,
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
                web_lock: false,
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
                created_at: now,
                updated_at: now,
            }
        };

        BookMetadataRepository::upsert(db, &metadata_record).await?;
        debug!(
            "Saved metadata for book (no ComicInfo): {} - title: {:?}",
            book.file_path, metadata_record.title
        );

        // Update series title_sort if not set and not locked (even without ComicInfo)
        if let Ok(Some(series_metadata_model)) =
            SeriesMetadataRepository::get_by_series_id(db, book.series_id).await
        {
            if series_metadata_model.title_sort.is_none() && !series_metadata_model.title_sort_lock
            {
                use crate::db::entities::series_metadata;
                use sea_orm::{ActiveModelTrait, Set};

                let series_title = series_metadata_model.title.clone();
                let mut metadata_active: series_metadata::ActiveModel =
                    series_metadata_model.into();
                metadata_active.title_sort = Set(Some(series_title.clone()));
                metadata_active.updated_at = Set(Utc::now());
                metadata_active.update(db).await?;
                debug!(
                    "Setting title_sort to '{}' for series (no ComicInfo)",
                    series_title
                );
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

/// Resolve book title using the library's book naming strategy
async fn resolve_book_title(
    db: &DatabaseConnection,
    book: &books::Model,
    file_metadata: &crate::parsers::BookMetadata,
    book_number: Option<f32>,
) -> String {
    // Get library to determine book naming strategy
    let library = match LibraryRepository::get_by_id(db, book.library_id).await {
        Ok(Some(lib)) => lib,
        Ok(None) | Err(_) => {
            // Fallback to filename if library not found
            warn!(
                "Library not found for book {}, using filename strategy",
                book.id
            );
            return filename_fallback(&book.file_name);
        }
    };

    // Parse book strategy from library
    let book_strategy = library
        .book_strategy
        .parse::<BookStrategy>()
        .unwrap_or(BookStrategy::Filename);
    let book_config_str = library.book_config.as_ref().map(|v| v.to_string());
    let strategy = create_book_strategy(book_strategy, book_config_str.as_deref());

    // Get series name from metadata for context
    let series_name = match SeriesMetadataRepository::get_by_series_id(db, book.series_id).await {
        Ok(Some(m)) => m.title,
        Ok(None) | Err(_) => {
            warn!(
                "Series metadata not found for book {}, using filename strategy",
                book.id
            );
            return filename_fallback(&book.file_name);
        }
    };

    // Get total books in series for padding calculation
    let total_books = match BookRepository::count_by_series(db, book.series_id).await {
        Ok(count) => count as usize,
        Err(_) => 1,
    };

    // Build metadata from comic info
    let metadata = file_metadata.comic_info.as_ref().map(|ci| BookMetadata {
        title: ci.title.clone().filter(|t| !t.is_empty()),
        number: book_number,
    });

    // Build naming context
    let context = BookNamingContext {
        series_name,
        book_number,
        volume: None, // Could be extracted from metadata/path for series_volume_chapter
        chapter_number: None, // Could be extracted from metadata/path
        total_books,
    };

    // Resolve title using strategy
    strategy.resolve_title(&book.file_name, metadata.as_ref(), &context)
}

/// Fallback: extract title from filename (without extension)
fn filename_fallback(file_name: &str) -> String {
    if let Some(pos) = file_name.rfind('.') {
        file_name[..pos].to_string()
    } else {
        file_name.to_string()
    }
}

/// Resolve book number using the library's number strategy
async fn resolve_book_number(
    db: &DatabaseConnection,
    book: &books::Model,
    _file_metadata: &crate::parsers::BookMetadata,
    metadata_number: Option<f32>,
) -> Option<f32> {
    // Get library to determine number strategy
    let library = match LibraryRepository::get_by_id(db, book.library_id).await {
        Ok(Some(lib)) => lib,
        Ok(None) | Err(_) => {
            // Fallback to file_order (default) if library not found
            warn!(
                "Library not found for book {}, using file_order strategy",
                book.id
            );
            // We don't have file order position here, so fall back to metadata
            return metadata_number;
        }
    };

    // Parse number strategy from library
    let number_strategy = library
        .number_strategy
        .parse::<NumberStrategy>()
        .unwrap_or(NumberStrategy::FileOrder);
    let number_config_str = library.number_config.as_ref().map(|v| v.to_string());
    let strategy = create_number_strategy(number_strategy, number_config_str.as_deref());

    // For file_order strategy, we need the book's position in the series
    // Get total books in series and calculate position
    let (file_order_position, total_books) =
        match get_book_position_in_series(db, book.series_id, &book.file_name).await {
            Ok((pos, total)) => (pos, total),
            Err(_) => {
                // Fallback: use 1 as position if we can't determine
                warn!(
                    "Could not determine file order position for book {}, using 1",
                    book.id
                );
                (1, 1)
            }
        };

    // Build number metadata
    let number_metadata = NumberMetadata {
        number: metadata_number,
    };

    // Build number context
    let context = NumberContext::new(file_order_position, total_books);

    // Resolve number using strategy
    strategy.resolve_number(&book.file_name, Some(&number_metadata), &context)
}

/// Get the position of a book within its series (sorted alphabetically by filename)
async fn get_book_position_in_series(
    db: &DatabaseConnection,
    series_id: Uuid,
    file_name: &str,
) -> Result<(usize, usize)> {
    // Get all books in the series (not including deleted)
    let books = BookRepository::list_by_series(db, series_id, false).await?;

    let total = books.len();

    // Sort by filename to match file_order strategy behavior
    let mut sorted_names: Vec<&str> = books.iter().map(|b| b.file_name.as_str()).collect();
    sorted_names.sort();

    // Find position of this book (1-indexed)
    let position = sorted_names
        .iter()
        .position(|name| *name == file_name)
        .map(|p| p + 1) // Convert to 1-indexed
        .unwrap_or(1); // Fallback to 1 if not found

    Ok((position, total))
}

/// Helper function to count non-null fields in metadata for logging
fn count_non_null_fields(metadata: &book_metadata::Model) -> usize {
    let mut count = 0;
    if metadata.title.is_some() {
        count += 1;
    }
    if metadata.title_sort.is_some() {
        count += 1;
    }
    if metadata.number.is_some() {
        count += 1;
    }
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

/// Categorize an analysis error into a specific BookErrorType
/// Returns the error type and a user-friendly error message
fn categorize_analysis_error(error: &anyhow::Error) -> (BookErrorType, String) {
    let error_string = error.to_string().to_lowercase();
    let root_cause = error.root_cause().to_string();

    // Check for format detection errors (check first as it's very specific)
    if error_string.contains("unsupported format")
        || error_string.contains("unsupported file format")
        || error_string.contains("unknown format")
    {
        return (
            BookErrorType::FormatDetection,
            format!("Unsupported or unrecognized file format: {}", root_cause),
        );
    }

    // Check for PDF rendering errors (check before page extraction, as both may mention "page")
    if error_string.contains("pdfium")
        || (error_string.contains("pdf") && error_string.contains("render"))
    {
        return (
            BookErrorType::PdfRendering,
            format!("PDF rendering error: {}", root_cause),
        );
    }

    // Check for metadata parsing errors
    if error_string.contains("comicinfo")
        || error_string.contains("invalid metadata")
        || (error_string.contains("metadata") && !error_string.contains("no metadata"))
    {
        return (
            BookErrorType::Metadata,
            format!("Failed to extract metadata: {}", root_cause),
        );
    }

    // Check for page extraction errors (check before parser errors)
    if error_string.contains("no images")
        || error_string.contains("empty archive")
        || (error_string.contains("page") && error_string.contains("extract"))
        || (error_string.contains("page") && error_string.contains("decode"))
        || (error_string.contains("image") && error_string.contains("decode"))
    {
        return (
            BookErrorType::PageExtraction,
            format!("Failed to extract pages: {}", root_cause),
        );
    }

    // Check for parser/archive errors
    if error_string.contains("zip")
        || error_string.contains("rar")
        || error_string.contains("archive")
        || error_string.contains("corrupted")
        || (error_string.contains("failed to open") && !error_string.contains("page"))
        || (error_string.contains("failed to extract") && !error_string.contains("page"))
    {
        return (
            BookErrorType::Parser,
            format!("Failed to parse archive: {}", root_cause),
        );
    }

    // Default to "Other" for unrecognized errors
    (BookErrorType::Other, root_cause)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_non_null_fields() {
        let metadata = book_metadata::Model {
            id: Uuid::new_v4(),
            book_id: Uuid::new_v4(),
            // Display fields (moved from books table)
            title: Some("Test Title".to_string()),
            title_sort: None,
            number: None,
            // Content fields
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
            // All locks default to false
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
            web_lock: false,
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(count_non_null_fields(&metadata), 8); // title, summary, writer, publisher, genre, language_iso, manga, year
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

    #[test]
    fn test_categorize_analysis_error_format_detection() {
        let error = anyhow::anyhow!("Unsupported format: test.txt");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::FormatDetection);

        let error = anyhow::anyhow!("Unknown format for file");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::FormatDetection);
    }

    #[test]
    fn test_categorize_analysis_error_parser() {
        let error = anyhow::anyhow!("Failed to open ZIP archive");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Parser);

        let error = anyhow::anyhow!("Invalid archive: corrupted header");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Parser);

        let error = anyhow::anyhow!("RAR extraction failed");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Parser);
    }

    #[test]
    fn test_categorize_analysis_error_metadata() {
        let error = anyhow::anyhow!("Failed to parse ComicInfo.xml");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Metadata);

        let error = anyhow::anyhow!("Invalid metadata in book");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Metadata);

        let error = anyhow::anyhow!("Book metadata is corrupted");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Metadata);
    }

    #[test]
    fn test_categorize_analysis_error_page_extraction() {
        let error = anyhow::anyhow!("No images found in archive");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::PageExtraction);

        let error = anyhow::anyhow!("Empty archive with no content");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::PageExtraction);

        let error = anyhow::anyhow!("Failed to extract page from book");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::PageExtraction);

        let error = anyhow::anyhow!("Failed to decode image data");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::PageExtraction);
    }

    #[test]
    fn test_categorize_analysis_error_pdf_rendering() {
        let error = anyhow::anyhow!("PDFium library not available");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::PdfRendering);

        let error = anyhow::anyhow!("Failed to render PDF page");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::PdfRendering);
    }

    #[test]
    fn test_categorize_analysis_error_other() {
        let error = anyhow::anyhow!("Unknown error occurred");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Other);

        let error = anyhow::anyhow!("Something unexpected happened");
        let (error_type, _msg) = categorize_analysis_error(&error);
        assert_eq!(error_type, BookErrorType::Other);
    }
}
