use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{prelude::Decimal, DatabaseConnection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::entities::{books, series};
use crate::db::repositories::{BookRepository, LibraryRepository, SeriesRepository};
use crate::scanner::analyze_file;

use super::types::{ScanMode, ScanProgress, ScanResult, ScanStatus};

const HASH_READ_SIZE: usize = 1024 * 1024; // 1MB for partial hash
const SUPPORTED_EXTENSIONS: &[&str] = &["cbz", "cbr", "epub", "pdf"];

/// Main library scanner that orchestrates the scanning process
pub async fn scan_library(
    db: &DatabaseConnection,
    library_id: Uuid,
    mode: ScanMode,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
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
        ScanMode::Normal => scan_normal(db, &library, &mut progress, &progress_tx).await,
        ScanMode::Deep => scan_deep(db, &library, &mut progress, &progress_tx).await,
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

    // Discover all files in library (blocking I/O operation)
    let discover_start = Instant::now();
    let library_path = library.path.clone();
    info!("Starting file discovery in library path: {}", library_path);
    let discovered_files = tokio::task::spawn_blocking(move || discover_files(&library_path))
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

    // Organize files by series (folder structure)
    let organize_start = Instant::now();
    let series_map = organize_by_series(&discovered_files, &library.path);
    info!(
        "Organized files into {} series in {:?}",
        series_map.len(),
        organize_start.elapsed()
    );

    // Process each series
    let series_count = series_map.len();
    let mut series_processed = 0;
    for (series_name, file_paths) in series_map {
        series_processed += 1;
        let series_start = Instant::now();
        info!(
            "Processing series {}/{}: '{}' ({} files)",
            series_processed,
            series_count,
            series_name,
            file_paths.len()
        );

        match process_series(
            db,
            library,
            &series_name,
            &file_paths,
            &existing_books,
            ScanMode::Normal,
            progress,
            progress_tx,
            &mut result,
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
                match BookRepository::mark_deleted(db, book.id, true).await {
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
            match BookRepository::mark_deleted(db, book.id, false).await {
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
) -> Result<ScanResult> {
    let mut result = ScanResult::new();

    // Discover all files in library (blocking I/O operation)
    let discover_start = Instant::now();
    let library_path = library.path.clone();
    info!(
        "Starting file discovery for deep scan in library path: {}",
        library_path
    );
    let discovered_files = tokio::task::spawn_blocking(move || discover_files(&library_path))
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

    // Organize files by series (folder structure)
    let organize_start = Instant::now();
    let series_map = organize_by_series(&discovered_files, &library.path);
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
    for (series_name, file_paths) in series_map {
        series_processed += 1;
        let series_start = Instant::now();
        info!(
            "Processing series {}/{}: '{}' ({} files)",
            series_processed,
            series_count,
            series_name,
            file_paths.len()
        );

        match process_series(
            db,
            library,
            &series_name,
            &file_paths,
            &existing_books,
            ScanMode::Deep, // Always deep mode
            progress,
            progress_tx,
            &mut result,
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

/// Process a single series with its files
async fn process_series(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    series_name: &str,
    file_paths: &[PathBuf],
    existing_books: &HashMap<String, (String, books::Model)>,
    mode: ScanMode,
    progress: &mut ScanProgress,
    progress_tx: &Option<mpsc::Sender<ScanProgress>>,
    result: &mut ScanResult,
) -> Result<()> {
    // Calculate series fingerprint from file paths
    let file_refs: Vec<&PathBuf> = file_paths.iter().collect();
    let fingerprint = calculate_series_fingerprint(&file_refs);

    // Extract series path (relative to library root)
    let series_path = if !file_paths.is_empty() {
        let library_path = Path::new(&library.path);
        let first_file = &file_paths[0];
        if let Ok(relative) = first_file.strip_prefix(library_path) {
            // Get the parent directory (series folder)
            relative
                .parent()
                .and_then(|p| p.components().next())
                .map(|c| c.as_os_str().to_string_lossy().to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Find or create series with fingerprint
    let series_model = find_or_create_series(
        db,
        library.id,
        series_name,
        Some(&fingerprint),
        series_path.as_deref(),
    )
    .await?;

    let is_new_series = existing_books
        .values()
        .all(|(_, book)| book.series_id != series_model.id);

    if is_new_series {
        result.series_created += 1;
        progress.increment_series();
    }

    // Process each file in the series
    let file_count = file_paths.len();
    let mut file_processed = 0;
    let mut last_progress_log = Instant::now();
    const PROGRESS_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    for file_path in file_paths {
        file_processed += 1;
        let file_start = Instant::now();

        match process_file(
            db,
            &series_model,
            file_path,
            existing_books,
            mode,
            progress,
            result,
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
                        file_path.file_name().unwrap_or_default().to_string_lossy()
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
                let error_msg = format!("Error processing file '{}': {}", file_path.display(), e);
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
async fn process_file(
    db: &DatabaseConnection,
    series_model: &series::Model,
    file_path: &Path,
    existing_books: &HashMap<String, (String, books::Model)>,
    mode: ScanMode,
    progress: &mut ScanProgress,
    result: &mut ScanResult,
) -> Result<()> {
    let path_str = file_path.to_string_lossy().to_string();

    // Calculate current hash (blocking I/O operation)
    let hash_start = Instant::now();
    let file_path_clone = file_path.to_path_buf();
    let current_hash = tokio::task::spawn_blocking(move || calculate_file_hash(&file_path_clone))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn hash calculation task: {}", e))??;

    // Check if we should skip this file (normal mode only)
    if mode == ScanMode::Normal {
        if let Some((existing_hash, existing_book)) = existing_books.get(&path_str) {
            if &current_hash == existing_hash {
                debug!(
                    "Skipping unchanged file: {} (hash check took {:?})",
                    path_str,
                    hash_start.elapsed()
                );

                // If the book was previously analyzed and hash hasn't changed, no need to reanalyze
                if existing_book.analyzed {
                    return Ok(());
                }
            }
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

    // Check if book already exists by path
    let existing_book = BookRepository::get_by_path(db, &path_str).await?;
    let now = Utc::now();

    if let Some(mut existing) = existing_book {
        // Update existing book - only update hash, size, and modified time
        // Mark as not analyzed if hash changed
        let hash_changed = existing.file_hash != current_hash;

        // Check if any meaningful fields have changed (excluding updated_at which always changes)
        let size_changed = existing.file_size != file_size as i64;
        let format_changed = existing.format != format;
        let modified_changed = existing.modified_at != modified_at;
        let anything_changed = hash_changed || size_changed || format_changed || modified_changed;

        existing.file_size = file_size as i64;
        existing.file_hash = current_hash;
        existing.format = format;
        existing.modified_at = modified_at;
        existing.updated_at = now;
        existing.analyzed = if hash_changed {
            false
        } else {
            existing.analyzed
        };

        BookRepository::update(db, &existing).await?;

        // Only count as updated if something actually changed
        if anything_changed {
            result.books_updated += 1;
        }
        progress.increment_books();

        debug!(
            "Updated book (detection only): {} - analyzed: {}",
            path_str, existing.analyzed
        );
    } else {
        // Create new book WITHOUT analysis
        let book_model = books::Model {
            id: Uuid::new_v4(),
            series_id: series_model.id,
            title: None,  // Will be filled during analysis phase
            number: None, // Will be filled during analysis phase
            file_path: path_str.clone(),
            file_name: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_size: file_size as i64,
            file_hash: current_hash,
            format,
            page_count: 0, // Will be filled during analysis phase
            deleted: false,
            analyzed: false, // Mark as not analyzed
            modified_at,
            created_at: now,
            updated_at: now,
        };

        BookRepository::create(db, &book_model).await?;
        SeriesRepository::increment_book_count(db, series_model.id).await?;

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
    let mut sorted_paths: Vec<&PathBuf> = file_paths.iter().copied().collect();
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

/// Find or create a series by name, with optional fingerprint matching
async fn find_or_create_series(
    db: &DatabaseConnection,
    library_id: Uuid,
    series_name: &str,
    fingerprint: Option<&str>,
    path: Option<&str>,
) -> Result<series::Model> {
    let series_list = SeriesRepository::list_by_library(db, library_id).await?;

    // 1. Try fingerprint match first (most reliable for rename detection)
    if let Some(fp) = fingerprint {
        if let Some(existing) = series_list
            .iter()
            .find(|s| s.fingerprint.as_ref().map(|f| f == fp).unwrap_or(false))
        {
            info!(
                "Matched series by fingerprint: {} -> {}",
                series_name, existing.name
            );

            // Update name if changed (series was renamed)
            if existing.name != series_name {
                info!(
                    "Detected series rename: {} -> {}",
                    existing.name, series_name
                );
                SeriesRepository::update_name(db, existing.id, series_name).await?;

                // Return updated series
                return SeriesRepository::get_by_id(db, existing.id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Series not found after update"));
            }

            return Ok(existing.clone());
        }
    }

    // 2. Fallback to normalized name match
    let normalized_search = series_name.to_lowercase();
    if let Some(existing) = series_list
        .iter()
        .find(|s| s.normalized_name.to_lowercase() == normalized_search)
    {
        // Update fingerprint if missing
        if existing.fingerprint.is_none() && fingerprint.is_some() {
            info!("Adding fingerprint to existing series: {}", existing.name);
            SeriesRepository::update_fingerprint(db, existing.id, fingerprint.map(String::from))
                .await?;
        }

        return Ok(existing.clone());
    }

    // 3. Create new series with fingerprint
    SeriesRepository::create_with_fingerprint(
        db,
        library_id,
        series_name,
        fingerprint.map(String::from),
        path.map(String::from),
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
    let mut total_books = 0;

    for series in series_list {
        // Include deleted books so we can restore them if they reappear
        let books = BookRepository::list_by_series(db, series.id, true).await?;
        total_books += books.len();
        for book in books {
            books_map.insert(book.file_path.clone(), (book.file_hash.clone(), book));
        }
    }

    Ok(books_map)
}

/// Discover all supported files in library path
fn discover_files(library_path: &str) -> Result<Vec<PathBuf>> {
    let start = Instant::now();
    let mut files = Vec::new();
    let mut dirs_visited = 0;
    let mut files_checked = 0;

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
            if SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
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

/// Organize files by series based on folder structure
/// Strategy: Direct child folders of library = series
fn organize_by_series(files: &[PathBuf], library_path: &str) -> HashMap<String, Vec<PathBuf>> {
    let mut series_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let library_path = Path::new(library_path);

    for file_path in files {
        // Get the series name from folder structure
        let series_name = extract_series_name(file_path, library_path);

        series_map
            .entry(series_name)
            .or_insert_with(Vec::new)
            .push(file_path.clone());
    }

    series_map
}

/// Extract series name from file path based on library root
/// Direct child folder of library = series name
fn extract_series_name(file_path: &Path, library_path: &Path) -> String {
    // Get relative path from library root
    let relative = file_path.strip_prefix(library_path).unwrap_or(file_path);

    // Get first component (direct child folder)
    let components: Vec<_> = relative.components().collect();

    if components.len() > 1 {
        // Use first folder as series name
        components[0].as_os_str().to_string_lossy().to_string()
    } else {
        // File is directly in library root
        "Unsorted".to_string()
    }
}

/// Calculate SHA-256 hash of file (partial - first 1MB for performance)
/// NOTE: This is a synchronous function - wrap in spawn_blocking when calling from async context
fn calculate_file_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_READ_SIZE];

    let bytes_read = file.read(&mut buffer)?;
    hasher.update(&buffer[..bytes_read]);

    Ok(format!("{:x}", hasher.finalize()))
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
    fn test_extract_series_name() {
        let library = Path::new("/library");

        // File in series folder
        let path = PathBuf::from("/library/Batman/issue1.cbz");
        assert_eq!(extract_series_name(&path, library), "Batman");

        // File in nested folder
        let path = PathBuf::from("/library/Batman/Year One/issue1.cbz");
        assert_eq!(extract_series_name(&path, library), "Batman");

        // File directly in library
        let path = PathBuf::from("/library/standalone.cbz");
        assert_eq!(extract_series_name(&path, library), "Unsorted");
    }

    #[test]
    fn test_organize_by_series() {
        let files = vec![
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Superman/issue1.cbz"),
            PathBuf::from("/library/standalone.cbz"),
        ];

        let series_map = organize_by_series(&files, "/library");

        assert_eq!(series_map.len(), 3);
        assert_eq!(series_map.get("Batman").unwrap().len(), 2);
        assert_eq!(series_map.get("Superman").unwrap().len(), 1);
        assert_eq!(series_map.get("Unsorted").unwrap().len(), 1);
    }

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
