use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::entities::{books, series};
use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use crate::events::EventBroadcaster;
use crate::models::SeriesStrategy;

use super::strategies::{create_strategy, DetectedSeries};
use super::types::{ScanMode, ScanProgress, ScanResult, ScanStatus};

const SUPPORTED_EXTENSIONS: &[&str] = &["cbz", "cbr", "epub", "pdf"];

/// Parse allowed_formats from library and convert to lowercase extensions
/// Returns None if no restrictions (all formats allowed), or Some(Vec<String>) with allowed extensions
fn parse_allowed_formats(library: &crate::db::entities::libraries::Model) -> Option<Vec<String>> {
    library.allowed_formats.as_ref().and_then(|json| {
        serde_json::from_str::<Vec<String>>(json)
            .ok()
            .map(|formats| {
                formats
                    .iter()
                    .map(|f| f.to_lowercase())
                    .collect::<Vec<String>>()
            })
    })
}

/// Main library scanner that orchestrates the scanning process
pub async fn scan_library(
    db: &DatabaseConnection,
    library_id: Uuid,
    mode: ScanMode,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<ScanResult> {
    let scan_start = Instant::now();
    info!("Starting {} scan for library {}", mode, library_id);

    // Load library from database
    let load_start = Instant::now();
    let library = LibraryRepository::get_by_id(db, library_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;
    info!(
        "Loaded library '{}' from database in {:?}",
        library.name,
        load_start.elapsed()
    );

    // Verify library path exists
    if !Path::new(&library.path).exists() {
        return Err(anyhow::anyhow!(
            "Library path does not exist: {}",
            library.path
        ));
    }

    // Initialize progress tracking
    let mut progress = ScanProgress::new(library_id);
    progress.start();
    send_progress(&progress_tx, &progress).await;

    // Execute scan based on mode
    let result = match mode {
        ScanMode::Normal => {
            scan_normal(db, &library, &mut progress, &progress_tx, event_broadcaster).await
        }
        ScanMode::Deep => {
            scan_deep(db, &library, &mut progress, &progress_tx, event_broadcaster).await
        }
    };

    // Update progress based on result
    match &result {
        Ok(scan_result) => {
            if scan_result.has_errors() {
                progress.status = ScanStatus::Completed;
                for error in &scan_result.errors {
                    progress.add_error(error.clone());
                }
            } else {
                progress.complete();
            }
        }
        Err(e) => {
            progress.fail(e.to_string());
        }
    }

    send_progress(&progress_tx, &progress).await;

    // Update last_scanned_at timestamp
    if result.is_ok() {
        if let Err(e) = LibraryRepository::update_last_scanned(db, library_id).await {
            warn!("Failed to update last_scanned_at: {}", e);
        }
    }

    let scan_duration = scan_start.elapsed();
    let result_ref = result.as_ref();
    let files_processed = result_ref.map(|r| r.files_processed).unwrap_or(0);
    let series_created = result_ref.map(|r| r.series_created).unwrap_or(0);
    let books_created = result_ref.map(|r| r.books_created).unwrap_or(0);
    let books_updated = result_ref.map(|r| r.books_updated).unwrap_or(0);
    let books_deleted = result_ref.map(|r| r.books_deleted).unwrap_or(0);
    let books_restored = result_ref.map(|r| r.books_restored).unwrap_or(0);
    let error_count = result_ref.map(|r| r.errors.len()).unwrap_or(0);

    let files_per_sec = if scan_duration.as_secs_f64() > 0.0 {
        files_processed as f64 / scan_duration.as_secs_f64()
    } else {
        0.0
    };

    info!(
        "Scan completed for library {} in {:?} ({:.2} files/sec) - processed {} files, created {} series, {} books ({} created, {} updated, {} deleted, {} restored), {} errors",
        library_id,
        scan_duration,
        files_per_sec,
        files_processed,
        series_created,
        books_created + books_updated,
        books_created,
        books_updated,
        books_deleted,
        books_restored,
        error_count
    );

    result
}

/// Normal scan - only process new/changed files
async fn scan_normal(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<ScanResult> {
    let mut result = ScanResult::new();

    // Load existing books from database (including deleted ones for restoration)
    let load_start = Instant::now();
    let existing_books = load_existing_books(db, library.id).await?;
    info!(
        "Loaded {} existing books from database in {:?}",
        existing_books.len(),
        load_start.elapsed()
    );

    // Parse allowed formats
    let allowed_extensions = parse_allowed_formats(library);
    if let Some(ref formats) = allowed_extensions {
        info!(
            "Library '{}' has format restrictions: {:?}",
            library.name, formats
        );
    }

    // Discover all files in library (blocking I/O operation)
    let discover_start = Instant::now();
    let library_path = library.path.clone();
    let allowed_extensions_clone = allowed_extensions.clone();
    info!("Starting file discovery in library path: {}", library_path);
    let discovered_files = tokio::task::spawn_blocking(move || {
        discover_files(&library_path, allowed_extensions_clone.as_deref())
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn file discovery task: {}", e))??;
    let discover_duration = discover_start.elapsed();
    progress.files_total = discovered_files.len();
    send_progress(progress_tx, progress).await;

    info!(
        "Discovered {} files in library '{}' in {:?} ({:.2} files/sec)",
        discovered_files.len(),
        library.name,
        discover_duration,
        if discover_duration.as_secs_f64() > 0.0 {
            discovered_files.len() as f64 / discover_duration.as_secs_f64()
        } else {
            0.0
        }
    );

    // Track which file paths were seen during scan
    let mut seen_paths = std::collections::HashSet::new();
    for file in &discovered_files {
        if let Some(path_str) = file.to_str() {
            seen_paths.insert(path_str.to_string());
        }
    }

    // Create scanning strategy based on library configuration
    let series_strategy = library
        .series_strategy
        .parse::<SeriesStrategy>()
        .unwrap_or_default();
    let series_config_str = library.series_config.as_ref().map(|v| v.to_string());
    let strategy = create_strategy(series_strategy, series_config_str.as_deref())?;
    info!(
        "Using {} strategy for library '{}'",
        series_strategy, library.name
    );

    // Organize files by series using strategy
    let organize_start = Instant::now();
    let library_path = Path::new(&library.path);
    let series_map = strategy.organize_files(&discovered_files, library_path)?;
    info!(
        "Organized files into {} series in {:?}",
        series_map.len(),
        organize_start.elapsed()
    );

    // Process each series
    let series_count = series_map.len();
    let mut series_processed = 0;
    for (series_name, detected_series) in series_map {
        series_processed += 1;
        let series_start = Instant::now();
        info!(
            "Processing series {}/{}: '{}' ({} files)",
            series_processed,
            series_count,
            series_name,
            detected_series.books.len()
        );

        match process_series_with_detected(
            db,
            library,
            &detected_series,
            &existing_books,
            ScanMode::Normal,
            progress,
            progress_tx,
            &mut result,
            event_broadcaster,
        )
        .await
        {
            Ok(_) => {
                info!(
                    "Completed series '{}' in {:?}",
                    series_name,
                    series_start.elapsed()
                );
            }
            Err(e) => {
                let error_msg = format!("Error processing series '{}': {}", series_name, e);
                error!("{} (took {:?})", error_msg, series_start.elapsed());
                result.errors.push(error_msg);
            }
        }
    }

    // Detect deleted files (in DB but not on filesystem) and restore reappeared files
    let cleanup_start = Instant::now();
    let mut deleted_count = 0;
    let mut restored_count = 0;

    for (path, (_hash, book)) in existing_books {
        if !seen_paths.contains(&path) {
            // File is missing from filesystem
            if !book.deleted {
                // Mark as deleted
                debug!("Marking missing book as deleted: {}", path);
                match BookRepository::mark_deleted(db, book.id, true, event_broadcaster).await {
                    Ok(_) => {
                        deleted_count += 1;
                        result.books_deleted += 1;
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to mark book as deleted {}: {}", path, e);
                        warn!("{}", error_msg);
                        result.errors.push(error_msg);
                    }
                }
            }
        } else if book.deleted {
            // File reappeared on filesystem, restore it
            debug!("Restoring deleted book: {}", path);
            match BookRepository::mark_deleted(db, book.id, false, event_broadcaster).await {
                Ok(_) => {
                    restored_count += 1;
                    result.books_restored += 1;
                }
                Err(e) => {
                    let error_msg = format!("Failed to restore book {}: {}", path, e);
                    warn!("{}", error_msg);
                    result.errors.push(error_msg);
                }
            }
        }
    }

    if deleted_count > 0 || restored_count > 0 {
        info!(
            "Cleanup completed in {:?}: {} books marked as deleted, {} books restored",
            cleanup_start.elapsed(),
            deleted_count,
            restored_count
        );
    }

    Ok(result)
}

/// Deep scan - re-process all files
async fn scan_deep(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<ScanResult> {
    let mut result = ScanResult::new();

    // Parse allowed formats
    let allowed_extensions = parse_allowed_formats(library);
    if let Some(ref formats) = allowed_extensions {
        info!(
            "Library '{}' has format restrictions: {:?}",
            library.name, formats
        );
    }

    // Discover all files in library (blocking I/O operation)
    let discover_start = Instant::now();
    let library_path = library.path.clone();
    let allowed_extensions_clone = allowed_extensions.clone();
    info!(
        "Starting file discovery for deep scan in library path: {}",
        library_path
    );
    let discovered_files = tokio::task::spawn_blocking(move || {
        discover_files(&library_path, allowed_extensions_clone.as_deref())
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn file discovery task: {}", e))??;
    let discover_duration = discover_start.elapsed();
    progress.files_total = discovered_files.len();
    send_progress(progress_tx, progress).await;

    info!(
        "Discovered {} files for deep scan in library '{}' in {:?} ({:.2} files/sec)",
        discovered_files.len(),
        library.name,
        discover_duration,
        if discover_duration.as_secs_f64() > 0.0 {
            discovered_files.len() as f64 / discover_duration.as_secs_f64()
        } else {
            0.0
        }
    );

    // Create scanning strategy based on library configuration
    let series_strategy = library
        .series_strategy
        .parse::<SeriesStrategy>()
        .unwrap_or_default();
    let series_config_str = library.series_config.as_ref().map(|v| v.to_string());
    let strategy = create_strategy(series_strategy, series_config_str.as_deref())?;
    info!(
        "Using {} strategy for deep scan of library '{}'",
        series_strategy, library.name
    );

    // Organize files by series using strategy
    let organize_start = Instant::now();
    let library_path = Path::new(&library.path);
    let series_map = strategy.organize_files(&discovered_files, library_path)?;
    info!(
        "Organized files into {} series in {:?}",
        series_map.len(),
        organize_start.elapsed()
    );

    // For deep scan, we don't use existing books cache (process everything)
    let existing_books = HashMap::new();

    // Process each series
    let series_count = series_map.len();
    let mut series_processed = 0;
    for (series_name, detected_series) in series_map {
        series_processed += 1;
        let series_start = Instant::now();
        info!(
            "Processing series {}/{}: '{}' ({} files)",
            series_processed,
            series_count,
            series_name,
            detected_series.books.len()
        );

        match process_series_with_detected(
            db,
            library,
            &detected_series,
            &existing_books,
            ScanMode::Deep, // Always deep mode
            progress,
            progress_tx,
            &mut result,
            event_broadcaster,
        )
        .await
        {
            Ok(_) => {
                info!(
                    "Completed series '{}' in {:?}",
                    series_name,
                    series_start.elapsed()
                );
            }
            Err(e) => {
                let error_msg = format!("Error processing series '{}': {}", series_name, e);
                error!("{} (took {:?})", error_msg, series_start.elapsed());
                result.errors.push(error_msg);
            }
        }
    }

    Ok(result)
}

/// Process a single series with detected series information from strategy
#[allow(clippy::too_many_arguments)] // Internal scanner function - context params needed for recursive book processing
async fn process_series_with_detected(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    detected_series: &DetectedSeries,
    existing_books: &HashMap<String, (String, books::Model)>,
    mode: ScanMode,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
    result: &mut ScanResult,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<()> {
    // Extract file paths from detected books
    let file_paths: Vec<PathBuf> = detected_series
        .books
        .iter()
        .map(|b| b.path.clone())
        .collect();

    // Calculate series fingerprint from file paths
    let file_refs: Vec<&PathBuf> = file_paths.iter().collect();
    let fingerprint = calculate_series_fingerprint(&file_refs);

    // Use series path from detected series (should always be available from scanner)
    // Fallback to series name if not available (shouldn't happen in practice)
    let series_path = detected_series
        .path
        .as_deref()
        .unwrap_or(&detected_series.name);

    // Find or create series with fingerprint
    let series_model = find_or_create_series(
        db,
        library.id,
        &detected_series.name,
        Some(&fingerprint),
        series_path,
        event_broadcaster,
    )
    .await?;

    let is_new_series = existing_books
        .values()
        .all(|(_, book)| book.series_id != series_model.id);

    if is_new_series {
        result.series_created += 1;
        progress.increment_series();
    }

    // Process each detected book in the series
    let file_count = detected_series.books.len();
    let mut file_processed = 0;
    let mut last_progress_log = Instant::now();
    const PROGRESS_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    for detected_book in &detected_series.books {
        file_processed += 1;
        let file_start = Instant::now();

        match process_file(
            db,
            &series_model,
            &detected_book.path,
            existing_books,
            mode,
            progress,
            result,
            event_broadcaster,
        )
        .await
        {
            Ok(_) => {
                result.files_processed += 1;
                progress.files_processed += 1;

                let file_duration = file_start.elapsed();
                if file_duration.as_millis() > 1000 {
                    // Log slow files (>1s)
                    debug!(
                        "Processed file {}/{} in {:?}: {}",
                        file_processed,
                        file_count,
                        file_duration,
                        detected_book
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    );
                }

                // Send progress update every file (for real-time updates)
                send_progress(progress_tx, progress).await;

                // Log progress periodically
                if last_progress_log.elapsed() >= PROGRESS_LOG_INTERVAL {
                    let elapsed_duration = Utc::now()
                        .signed_duration_since(progress.started_at)
                        .to_std()
                        .unwrap_or(std::time::Duration::ZERO);
                    let elapsed_secs = elapsed_duration.as_secs_f64();
                    let files_per_sec = if elapsed_secs > 0.0 {
                        progress.files_processed as f64 / elapsed_secs
                    } else {
                        0.0
                    };
                    let remaining = if files_per_sec > 0.0
                        && progress.files_total > progress.files_processed
                    {
                        let remaining_files = progress.files_total - progress.files_processed;
                        std::time::Duration::from_secs_f64(remaining_files as f64 / files_per_sec)
                    } else {
                        std::time::Duration::ZERO
                    };

                    info!(
                        "Scan progress: {}/{} files ({:.1}%), {:.2} files/sec, ~{:?} remaining",
                        progress.files_processed,
                        progress.files_total,
                        if progress.files_total > 0 {
                            (progress.files_processed as f64 / progress.files_total as f64) * 100.0
                        } else {
                            0.0
                        },
                        files_per_sec,
                        remaining
                    );
                    last_progress_log = Instant::now();
                }
            }
            Err(e) => {
                let error_msg = format!(
                    "Error processing file '{}': {}",
                    detected_book.path.display(),
                    e
                );
                error!("{} (took {:?})", error_msg, file_start.elapsed());
                result.errors.push(error_msg);
            }
        }

        // Yield to the runtime after each file to allow other tasks to run
        tokio::task::yield_now().await;
    }

    Ok(())
}

/// Process a single file - FAST DETECTION PHASE (no analysis)
/// Only calculates hash and creates/updates database record without analyzing content
#[allow(clippy::too_many_arguments)] // Internal scanner function - context params needed for book creation
async fn process_file(
    db: &DatabaseConnection,
    series_model: &series::Model,
    file_path: &Path,
    existing_books: &HashMap<String, (String, books::Model)>,
    mode: ScanMode,
    progress: &mut ScanProgress,
    result: &mut ScanResult,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<()> {
    let path_str = file_path.to_string_lossy().to_string();

    // Calculate current partial hash (blocking I/O operation - fast, only first 1MB)
    let hash_start = Instant::now();
    let file_path_clone = file_path.to_path_buf();
    let current_partial_hash = tokio::task::spawn_blocking(move || {
        use crate::utils::hasher::hash_file_partial;
        hash_file_partial(&file_path_clone)
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn hash calculation task: {}", e))??;

    // Check if we should skip this file (normal mode only)
    if mode == ScanMode::Normal {
        if let Some((existing_partial_hash, existing_book)) = existing_books.get(&path_str) {
            if &current_partial_hash == existing_partial_hash {
                // Partial hash hasn't changed - file is likely unchanged
                if existing_book.analyzed {
                    debug!(
                        "Skipping unchanged analyzed file: {} (partial hash check took {:?})",
                        path_str,
                        hash_start.elapsed()
                    );
                    return Ok(());
                } else {
                    info!(
                        "File unchanged but not analyzed (will update/skip): {} - partial hash: {}",
                        path_str, current_partial_hash
                    );
                }
            } else {
                info!(
                    "File partial hash changed: {} - old: {}, new: {}",
                    path_str, existing_partial_hash, current_partial_hash
                );
            }
        } else {
            info!(
                "New file detected (not in existing_books map): {}",
                path_str
            );
        }
    }

    // Get file metadata (size and modified time) without full analysis
    let file_path_clone = file_path.to_path_buf();
    let (file_size, modified_at) =
        tokio::task::spawn_blocking(move || -> Result<(u64, DateTime<Utc>)> {
            let metadata = fs::metadata(&file_path_clone)?;
            let modified = metadata.modified()?;
            let modified_dt = DateTime::<Utc>::from(modified);
            Ok((metadata.len(), modified_dt))
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get file metadata: {}", e))??;

    // Detect format from extension
    let format = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase();

    // Check if book already exists by path and library
    let existing_book = BookRepository::get_by_path(db, series_model.library_id, &path_str).await?;
    let now = Utc::now();

    if let Some(mut existing) = existing_book {
        // Update existing book - only update hashes, size, and modified time
        // Mark as not analyzed if partial hash changed (will be verified before analysis)
        let partial_hash_changed = existing.partial_hash != current_partial_hash;

        // Check if any meaningful fields have changed (excluding updated_at which always changes)
        let size_changed = existing.file_size != file_size as i64;
        let format_changed = existing.format != format;
        let modified_changed = existing.modified_at != modified_at;
        let anything_changed =
            partial_hash_changed || size_changed || format_changed || modified_changed;

        existing.file_size = file_size as i64;
        existing.partial_hash = current_partial_hash.clone();
        // Note: file_hash (full hash) is NOT updated during scanning - only during analysis
        // This prevents the constant rehashing issue
        existing.format = format;
        existing.modified_at = modified_at;
        existing.updated_at = now;

        let was_analyzed = existing.analyzed;
        existing.analyzed = if partial_hash_changed {
            false
        } else {
            existing.analyzed
        };

        BookRepository::update(db, &existing, event_broadcaster).await?;

        // Only count as updated if something actually changed
        if anything_changed {
            result.books_updated += 1;
        }
        progress.increment_books();

        if partial_hash_changed && was_analyzed {
            info!(
                "Book marked as unanalyzed due to partial hash change: {} - old: (in DB), new: {}",
                path_str, current_partial_hash
            );
        }

        debug!(
            "Updated book (detection only): {} - analyzed: {}, partial_hash_changed: {}",
            path_str, existing.analyzed, partial_hash_changed
        );
    } else {
        // Create new book WITHOUT analysis
        // Note: title and number are now stored in book_metadata (populated during analysis)
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series_model.id,
            library_id: series_model.library_id,
            file_path: path_str.clone(),
            file_name: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_size: file_size as i64,
            file_hash: String::new(), // Will be filled during analysis phase with full hash
            partial_hash: current_partial_hash,
            format,
            page_count: 0, // Will be filled during analysis phase
            deleted: false,
            analyzed: false, // Mark as not analyzed
            analysis_error: None,
            modified_at,
            created_at: now,
            updated_at: now,
            thumbnail_path: None,         // Thumbnail not generated yet
            thumbnail_generated_at: None, // Thumbnail not generated yet
        };

        BookRepository::create(db, &book_model, event_broadcaster).await?;
        // Note: book_count is now computed dynamically via SeriesRepository::get_book_count()

        result.books_created += 1;
        progress.increment_books();

        debug!("Created book (detection only): {}", path_str);
    }

    Ok(())
}

/// Calculate a fingerprint for a series based on its books
///
/// Creates a SHA-256 hash from the normalized titles of up to 5 books
/// (sorted by filename for consistency). This fingerprint can be used
/// to detect series renames across scans.
fn calculate_series_fingerprint(file_paths: &[&PathBuf]) -> String {
    // Sort file paths by filename for consistency
    let mut sorted_paths: Vec<&PathBuf> = file_paths.to_vec();
    sorted_paths.sort_by(|a, b| {
        let a_name = a.file_name().unwrap_or_default();
        let b_name = b.file_name().unwrap_or_default();
        a_name.cmp(b_name)
    });

    // Take first 5 files (or all if fewer)
    let sample: Vec<&PathBuf> = sorted_paths.iter().take(5).copied().collect();

    // Create hash from normalized filenames
    let mut hasher = Sha256::new();
    for path in sample {
        if let Some(filename) = path.file_name() {
            if let Some(name_str) = filename.to_str() {
                // Normalize: lowercase, alphanumeric only
                let normalized: String = name_str
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect();
                hasher.update(normalized.as_bytes());
            }
        }
    }

    format!("{:x}", hasher.finalize())
}

/// Find or create a series using a 3-step matching strategy
///
/// Matching strategy (in order of priority):
/// 1. **Path match**: Same directory = same series (primary key)
/// 2. **Fingerprint match**: Directory renamed but same files = same series
/// 3. **Normalized name match**: Last resort for moved+renamed directories
/// 4. If no match, create a new series
///
/// This approach ensures that:
/// - Adding/removing files from a series directory keeps the same series (path match)
/// - Renaming a series directory keeps the same series (fingerprint match)
/// - Moving AND renaming a series directory may still match (normalized name fallback)
async fn find_or_create_series(
    db: &DatabaseConnection,
    library_id: Uuid,
    series_name: &str,
    fingerprint: Option<&str>,
    path: &str,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<series::Model> {
    use crate::db::repositories::SeriesMetadataRepository;

    // Step 1: Path match (same directory = same series)
    // This is the primary matching key - if the path matches, it's definitely the same series
    if let Some(existing) = SeriesRepository::find_by_path(db, library_id, path).await? {
        info!(
            "Matched series by path: {} -> series id {}",
            path, existing.id
        );

        // Update fingerprint and name if files changed (fingerprint may have changed)
        if let Some(fp) = fingerprint {
            if existing.fingerprint.as_ref() != Some(&fp.to_string())
                || existing.name != series_name
            {
                debug!(
                    "Updating series fingerprint/name after path match: {} (old fingerprint: {:?}, new: {})",
                    series_name,
                    existing.fingerprint,
                    fp
                );
                SeriesRepository::update_fingerprint_and_name(
                    db,
                    existing.id,
                    Some(fp.to_string()),
                    series_name,
                )
                .await?;

                // Also update series_metadata title if not locked
                if let Ok(Some(metadata)) =
                    SeriesMetadataRepository::get_by_series_id(db, existing.id).await
                {
                    if metadata.title != series_name && !metadata.title_lock {
                        SeriesRepository::update_name(db, existing.id, series_name).await?;
                    }
                }
            }
        }

        return Ok(existing);
    }

    // Step 2: Fingerprint match (directory renamed, same files)
    // The directory was renamed but files stayed the same
    if let Some(fp) = fingerprint {
        if let Some(existing) = SeriesRepository::find_by_fingerprint(db, library_id, fp).await? {
            info!(
                "Matched series by fingerprint: {} -> series id {} (path changed from {} to {})",
                series_name, existing.id, existing.path, path
            );

            // Update path and name (directory was renamed)
            SeriesRepository::update_path_and_name(db, existing.id, path.to_string(), series_name)
                .await?;

            // Also update series_metadata title if not locked
            if let Ok(Some(metadata)) =
                SeriesMetadataRepository::get_by_series_id(db, existing.id).await
            {
                if metadata.title != series_name && !metadata.title_lock {
                    info!(
                        "Detected series rename: {} -> {}",
                        metadata.title, series_name
                    );
                    SeriesRepository::update_name(db, existing.id, series_name).await?;
                }
            }

            return Ok(existing);
        }
    }

    // Step 3: Normalized name match (last resort fallback)
    // The directory was moved AND renamed, but the name is similar
    let normalized_name = SeriesRepository::normalize_name(series_name);
    if let Some(existing) =
        SeriesRepository::find_by_normalized_name(db, library_id, &normalized_name).await?
    {
        info!(
            "Matched series by normalized name: {} -> series id {} (path changed from {} to {})",
            series_name, existing.id, existing.path, path
        );

        // Update fingerprint and path
        SeriesRepository::update_fingerprint_and_path(
            db,
            existing.id,
            fingerprint.map(String::from),
            path.to_string(),
        )
        .await?;

        // Also update series.name and series_metadata title if not locked
        if existing.name != series_name {
            SeriesRepository::update_fingerprint_and_name(
                db,
                existing.id,
                fingerprint.map(String::from),
                series_name,
            )
            .await?;

            if let Ok(Some(metadata)) =
                SeriesMetadataRepository::get_by_series_id(db, existing.id).await
            {
                if !metadata.title_lock {
                    SeriesRepository::update_name(db, existing.id, series_name).await?;
                }
            }
        }

        return Ok(existing);
    }

    // Step 4: Create new series with fingerprint (title stored in series_metadata)
    info!(
        "Creating new series: {} at path {} with fingerprint {:?}",
        series_name, path, fingerprint
    );
    SeriesRepository::create_with_fingerprint(
        db,
        library_id,
        series_name,
        fingerprint.map(String::from),
        path.to_string(),
        event_broadcaster,
    )
    .await
}

/// Load existing books from database into a map
async fn load_existing_books(
    db: &DatabaseConnection,
    library_id: Uuid,
) -> Result<HashMap<String, (String, books::Model)>> {
    let series_load_start = Instant::now();
    let series_list = SeriesRepository::list_by_library(db, library_id).await?;
    info!(
        "Loaded {} series from database in {:?}",
        series_list.len(),
        series_load_start.elapsed()
    );

    let mut books_map = HashMap::new();

    for series in series_list {
        // Include deleted books so we can restore them if they reappear
        let books = BookRepository::list_by_series(db, series.id, true).await?;
        for book in books {
            // Store partial_hash for fast scanning comparison
            books_map.insert(book.file_path.clone(), (book.partial_hash.clone(), book));
        }
    }

    Ok(books_map)
}

/// Discover all supported files in library path
/// If allowed_extensions is Some, only files with those extensions will be included
/// If allowed_extensions is None, all supported formats are allowed
fn discover_files(
    library_path: &str,
    allowed_extensions: Option<&[String]>,
) -> Result<Vec<PathBuf>> {
    let start = Instant::now();
    let mut files = Vec::new();
    let mut dirs_visited = 0;
    let mut files_checked = 0;

    // Determine which extensions to check
    let extensions_to_check: Vec<&str> = if let Some(allowed) = allowed_extensions {
        // Only check allowed extensions
        allowed.iter().map(|s| s.as_str()).collect()
    } else {
        // Check all supported extensions
        SUPPORTED_EXTENSIONS.to_vec()
    };

    for entry in WalkDir::new(library_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip if not a file
        if !path.is_file() {
            dirs_visited += 1;
            continue;
        }

        files_checked += 1;

        // Check extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            // Check against allowed extensions (if specified) or all supported extensions
            if extensions_to_check.contains(&ext_str.as_str()) {
                files.push(path.to_path_buf());
            }
        }
    }

    let duration = start.elapsed();
    debug!(
        "File discovery: found {} supported files from {} files checked in {} directories, took {:?}",
        files.len(),
        files_checked,
        dirs_visited,
        duration
    );

    Ok(files)
}

/// Send progress update through channel
async fn send_progress(progress_tx: &Option<mpsc::Sender<ScanProgress>>, progress: &ScanProgress) {
    if let Some(tx) = progress_tx {
        if let Err(e) = tx.send(progress.clone()).await {
            warn!("Failed to send progress update: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_calculate_series_fingerprint_consistency() {
        // Same files in same order should produce same fingerprint
        let files1 = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
        ];
        let refs2: Vec<&PathBuf> = files2.iter().collect();

        let fp1 = calculate_series_fingerprint(&refs1);
        let fp2 = calculate_series_fingerprint(&refs2);

        assert_eq!(fp1, fp2, "Same files should produce identical fingerprints");
    }

    #[test]
    fn test_calculate_series_fingerprint_order_independence() {
        // Files in different order should produce same fingerprint (alphabetically sorted)
        let files1 = vec![
            PathBuf::from("/library/Batman/issue3.cbz"),
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
        ];
        let refs2: Vec<&PathBuf> = files2.iter().collect();

        let fp1 = calculate_series_fingerprint(&refs1);
        let fp2 = calculate_series_fingerprint(&refs2);

        assert_eq!(fp1, fp2, "File order should not affect fingerprint");
    }

    #[test]
    fn test_calculate_series_fingerprint_different_content() {
        // Different filenames should produce different fingerprints
        let files1 = vec![
            PathBuf::from("/library/Batman/Batman-001.cbz"),
            PathBuf::from("/library/Batman/Batman-002.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = vec![
            PathBuf::from("/library/Superman/Superman-001.cbz"),
            PathBuf::from("/library/Superman/Superman-002.cbz"),
        ];
        let refs2: Vec<&PathBuf> = files2.iter().collect();

        let fp1 = calculate_series_fingerprint(&refs1);
        let fp2 = calculate_series_fingerprint(&refs2);

        assert_ne!(
            fp1, fp2,
            "Different filenames should produce different fingerprints"
        );
    }

    #[test]
    fn test_calculate_series_fingerprint_limit_5_files() {
        // Should only use first 5 files (alphabetically)
        let files1 = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
            PathBuf::from("/library/Batman/issue4.cbz"),
            PathBuf::from("/library/Batman/issue5.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
            PathBuf::from("/library/Batman/issue4.cbz"),
            PathBuf::from("/library/Batman/issue5.cbz"),
            PathBuf::from("/library/Batman/issue6.cbz"), // Extra file
            PathBuf::from("/library/Batman/issue7.cbz"), // Extra file
        ];
        let refs2: Vec<&PathBuf> = files2.iter().collect();

        let fp1 = calculate_series_fingerprint(&refs1);
        let fp2 = calculate_series_fingerprint(&refs2);

        assert_eq!(
            fp1, fp2,
            "Extra files beyond first 5 should not affect fingerprint"
        );
    }

    #[test]
    fn test_calculate_series_fingerprint_path_independence() {
        // Same filenames in different paths should produce same fingerprint
        let files1 = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = vec![
            PathBuf::from("/new_location/Batman-Comics/issue1.cbz"),
            PathBuf::from("/new_location/Batman-Comics/issue2.cbz"),
        ];
        let refs2: Vec<&PathBuf> = files2.iter().collect();

        let fp1 = calculate_series_fingerprint(&refs1);
        let fp2 = calculate_series_fingerprint(&refs2);

        assert_eq!(fp1, fp2, "Same filenames in different folders should match");
    }

    #[test]
    fn test_calculate_series_fingerprint_single_file() {
        // Should work with just one file
        let files = vec![PathBuf::from("/library/Batman/standalone.cbz")];
        let refs: Vec<&PathBuf> = files.iter().collect();

        let fp = calculate_series_fingerprint(&refs);

        assert!(!fp.is_empty(), "Fingerprint should not be empty");
        assert_eq!(fp.len(), 64, "SHA-256 hex should be 64 characters");
    }
}
