use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex, Semaphore};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::entities::{books, series};
use crate::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, TaskRepository,
};
use crate::events::EventBroadcaster;
use crate::models::SeriesStrategy;
use crate::tasks::types::TaskType;

use super::strategies::{create_strategy, DetectedSeries};
use super::types::{ScanMode, ScanProgress, ScanResult, ScanStatus, ScannerConfig};

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

/// Result from processing a single series
///
/// Contains counts and errors from processing all books in a series.
#[derive(Debug, Default)]
struct SeriesProcessResult {
    /// Number of files scanned (processed/checked)
    files_scanned: usize,
    /// Number of books created
    books_created: usize,
    /// Number of books updated
    books_updated: usize,
    /// Number of analysis tasks queued
    tasks_queued: usize,
    /// Errors encountered during processing
    errors: Vec<String>,
}

impl SeriesProcessResult {
    fn new() -> Self {
        Self::default()
    }
}

/// Thread-safe wrapper for shared scan state during parallel series processing
///
/// This allows multiple series to be processed concurrently while still
/// maintaining accurate counts and error tracking.
struct SharedScanState {
    /// Thread-safe progress tracking
    progress: Arc<Mutex<ScanProgress>>,
    /// Thread-safe result accumulation
    result: Arc<Mutex<ScanResult>>,
    /// Progress channel sender
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
}

impl SharedScanState {
    fn new(library_id: Uuid, progress_tx: Option<mpsc::Sender<ScanProgress>>) -> Self {
        let mut progress = ScanProgress::new(library_id);
        progress.start();
        Self {
            progress: Arc::new(Mutex::new(progress)),
            result: Arc::new(Mutex::new(ScanResult::new())),
            progress_tx,
        }
    }

    /// Set the total file count for progress tracking
    async fn set_total_files(&self, total: usize) {
        let mut progress = self.progress.lock().await;
        progress.files_total = total;
    }

    /// Merge results from a completed series into the shared state
    async fn merge_series_result(&self, series_result: SeriesProcessResult, is_new_series: bool) {
        {
            let mut result = self.result.lock().await;
            result.books_created += series_result.books_created;
            result.books_updated += series_result.books_updated;
            result.files_processed += series_result.files_scanned;
            result.tasks_queued += series_result.tasks_queued;
            result.errors.extend(series_result.errors);

            if is_new_series {
                result.series_created += 1;
            }
        }

        {
            let mut progress = self.progress.lock().await;
            progress.books_found += series_result.books_created + series_result.books_updated;
            progress.files_processed += series_result.files_scanned;

            if is_new_series {
                progress.series_found += 1;
            }
        }
    }

    /// Send current progress through the channel
    async fn send_progress(&self) {
        if let Some(ref tx) = self.progress_tx {
            let progress = self.progress.lock().await.clone();
            if let Err(e) = tx.send(progress).await {
                warn!("Failed to send progress update: {}", e);
            }
        }
    }

    /// Add an error to the result
    async fn add_error(&self, error: String) {
        self.result.lock().await.errors.push(error);
    }

    /// Mark deleted book count
    async fn add_deleted(&self, count: usize) {
        self.result.lock().await.books_deleted += count;
    }

    /// Mark restored book count
    async fn add_restored(&self, count: usize) {
        self.result.lock().await.books_restored += count;
    }

    /// Get the final results
    async fn into_result(self) -> (ScanResult, ScanProgress) {
        let result = match Arc::try_unwrap(self.result) {
            Ok(mutex) => mutex.into_inner(),
            Err(arc) => arc.lock().await.clone(),
        };
        let progress = match Arc::try_unwrap(self.progress) {
            Ok(mutex) => mutex.into_inner(),
            Err(arc) => arc.lock().await.clone(),
        };
        (result, progress)
    }
}

impl Clone for SharedScanState {
    fn clone(&self) -> Self {
        Self {
            progress: Arc::clone(&self.progress),
            result: Arc::clone(&self.result),
            progress_tx: self.progress_tx.clone(),
        }
    }
}

/// Batch accumulator for pending book operations
///
/// Collects books to create or update until the batch is full,
/// then flushes them to the database in a single operation.
struct BookBatch {
    /// Books to create (new files)
    to_create: Vec<books::Model>,
    /// Books to update (changed files)
    to_update: Vec<books::Model>,
    /// Book IDs that need analysis tasks queued
    needs_analysis: Vec<Uuid>,
    /// Maximum batch size before auto-flush
    capacity: usize,
    /// Whether to force re-analysis (Deep scan mode)
    force_analysis: bool,
}

impl BookBatch {
    fn new(capacity: usize, force_analysis: bool) -> Self {
        Self {
            to_create: Vec::with_capacity(capacity),
            to_update: Vec::with_capacity(capacity),
            needs_analysis: Vec::with_capacity(capacity),
            capacity,
            force_analysis,
        }
    }

    /// Check if the batch is full and should be flushed
    fn is_full(&self) -> bool {
        self.to_create.len() + self.to_update.len() + self.needs_analysis.len() >= self.capacity
    }

    /// Check if the batch has any items
    fn is_empty(&self) -> bool {
        self.to_create.is_empty() && self.to_update.is_empty() && self.needs_analysis.is_empty()
    }

    /// Add a book to create
    fn add_create(&mut self, book: books::Model, needs_analysis: bool) {
        if needs_analysis {
            self.needs_analysis.push(book.id);
        }
        self.to_create.push(book);
    }

    /// Add a book to update
    fn add_update(&mut self, book: books::Model, needs_analysis: bool) {
        if needs_analysis {
            self.needs_analysis.push(book.id);
        }
        self.to_update.push(book);
    }

    /// Flush the batch to the database
    ///
    /// Returns (created_count, updated_count, tasks_queued, errors)
    async fn flush(&mut self, db: &DatabaseConnection) -> (usize, usize, usize, Vec<String>) {
        let mut errors = Vec::new();
        let mut created = 0;
        let mut updated = 0;
        let mut tasks_queued = 0;

        // Batch create new books
        if !self.to_create.is_empty() {
            match BookRepository::create_batch(db, &self.to_create).await {
                Ok(count) => {
                    created = count as usize;
                    debug!("Batch created {} books", created);
                }
                Err(e) => {
                    errors.push(format!("Failed to batch create books: {}", e));
                }
            }
        }

        // Batch update existing books
        if !self.to_update.is_empty() {
            match BookRepository::update_batch(db, &self.to_update).await {
                Ok(count) => {
                    updated = count as usize;
                    debug!("Batch updated {} books", updated);
                }
                Err(e) => {
                    errors.push(format!("Failed to batch update books: {}", e));
                }
            }
        }

        // Enqueue analysis tasks for books that need it
        if !self.needs_analysis.is_empty() {
            let force = self.force_analysis;
            let tasks: Vec<TaskType> = self
                .needs_analysis
                .iter()
                .map(|book_id| TaskType::AnalyzeBook {
                    book_id: *book_id,
                    force,
                })
                .collect();

            match TaskRepository::enqueue_batch(db, tasks, 0, None).await {
                Ok(count) => {
                    tasks_queued = count as usize;
                    debug!("Enqueued {} analysis tasks", tasks_queued);
                }
                Err(e) => {
                    errors.push(format!("Failed to enqueue analysis tasks: {}", e));
                }
            }
        }

        // Clear the batch
        self.to_create.clear();
        self.to_update.clear();
        self.needs_analysis.clear();

        (created, updated, tasks_queued, errors)
    }
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

    // Execute optimized batched scan (handles both Normal and Deep modes)
    // The batched scan manages its own progress tracking internally
    let result = scan_batched(db, &library, mode, progress_tx.clone(), event_broadcaster).await;

    // Send final progress update
    let mut final_progress = ScanProgress::new(library_id);
    match &result {
        Ok(scan_result) => {
            final_progress.files_processed = scan_result.files_processed;
            final_progress.files_total = scan_result.files_processed;
            final_progress.series_found = scan_result.series_created;
            final_progress.books_found = scan_result.books_created + scan_result.books_updated;

            if scan_result.has_errors() {
                final_progress.status = ScanStatus::Completed;
                for error in &scan_result.errors {
                    final_progress.add_error(error.clone());
                }
            } else {
                final_progress.complete();
            }
        }
        Err(e) => {
            final_progress.fail(e.to_string());
        }
    }

    send_progress(&progress_tx, &final_progress).await;

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
/// Unified batched scan implementation for both Normal and Deep modes
///
/// This optimized version:
/// - Processes multiple series concurrently (configurable via scanner.parallel_series)
/// - Hashes files in parallel within each series (configurable via scanner.parallel_hashing)
/// - Uses batch DB operations (configurable via scanner.batch_size)
/// - Queues analysis tasks immediately during scan (workers can start early)
/// - Uses thread-safe shared state for progress tracking
async fn scan_batched(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    mode: ScanMode,
    progress_tx: Option<mpsc::Sender<ScanProgress>>,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<ScanResult> {
    // Load scanner configuration from database settings
    let config = ScannerConfig::load(db).await;
    info!(
        "Scanner config: batch_size={}, parallel_hashing={}, parallel_series={}",
        config.batch_size, config.parallel_hashing, config.parallel_series
    );

    // Create shared state for thread-safe progress tracking
    let shared_state = SharedScanState::new(library.id, progress_tx);

    // Always load existing books from database
    // - Normal mode: used for change detection (skip unchanged files)
    // - Deep mode: used to detect updates vs creates, and for series detection
    let load_start = Instant::now();
    let existing_books_with_hash = load_existing_books(db, library.id).await?;

    // Create a simpler map without the hash for the new batched processing
    let existing_books_map: HashMap<String, books::Model> = existing_books_with_hash
        .iter()
        .map(|(path, (_, book))| (path.clone(), book.clone()))
        .collect();

    info!(
        "Loaded {} existing books from database in {:?}",
        existing_books_map.len(),
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
    info!(
        "Starting file discovery for {} scan in library path: {}",
        mode, library_path
    );
    let discovered_files = tokio::task::spawn_blocking(move || {
        discover_files(&library_path, allowed_extensions_clone.as_deref())
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn file discovery task: {}", e))??;
    let discover_duration = discover_start.elapsed();

    // Update progress with total files
    shared_state.set_total_files(discovered_files.len()).await;
    shared_state.send_progress().await;

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

    // Track which file paths were seen during scan (for deleted file detection)
    let seen_paths: HashSet<String> = discovered_files
        .iter()
        .filter_map(|f| f.to_str().map(String::from))
        .collect();

    // Create scanning strategy based on library configuration
    let series_strategy = library
        .series_strategy
        .parse::<SeriesStrategy>()
        .unwrap_or_default();
    let series_config_str = library.series_config.as_ref().map(|v| v.to_string());
    let strategy = create_strategy(series_strategy, series_config_str.as_deref())?;
    info!(
        "Using {} strategy for {} scan of library '{}'",
        series_strategy, mode, library.name
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

    // Process series in parallel with semaphore control
    let series_semaphore = Arc::new(Semaphore::new(config.parallel_series));
    let series_count = series_map.len();

    let series_futures: Vec<_> = series_map
        .into_iter()
        .map(|(series_name, detected_series)| {
            let sem = Arc::clone(&series_semaphore);
            let state = shared_state.clone();
            let db = db.clone();
            let library = library.clone();
            let existing_books_map = existing_books_map.clone();
            let config = config.clone();
            let event_broadcaster = event_broadcaster.cloned();

            async move {
                let _permit = match sem.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        let error = format!(
                            "Failed to acquire series semaphore for '{}': {}",
                            series_name, e
                        );
                        error!("{}", error);
                        state.add_error(error).await;
                        return;
                    }
                };

                let series_start = Instant::now();
                info!(
                    "Processing series '{}' ({} files)",
                    series_name,
                    detected_series.books.len()
                );

                match process_series_batched(
                    &db,
                    &library,
                    &detected_series,
                    &existing_books_map,
                    mode,
                    &config,
                    event_broadcaster.as_ref(),
                )
                .await
                {
                    Ok((series_result, is_new_series)) => {
                        info!(
                            "Completed series '{}' in {:?} (created: {}, updated: {}, tasks: {})",
                            series_name,
                            series_start.elapsed(),
                            series_result.books_created,
                            series_result.books_updated,
                            series_result.tasks_queued
                        );
                        state
                            .merge_series_result(series_result, is_new_series)
                            .await;
                        state.send_progress().await;
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Error processing series '{}': {} (took {:?})",
                            series_name,
                            e,
                            series_start.elapsed()
                        );
                        error!("{}", error_msg);
                        state.add_error(error_msg).await;
                    }
                }
            }
        })
        .collect();

    info!(
        "Starting parallel series processing ({} series, {} concurrent)",
        series_count, config.parallel_series
    );

    // Wait for all series to complete
    join_all(series_futures).await;

    // Handle deleted/restored files (Normal mode only)
    if mode == ScanMode::Normal {
        let cleanup_start = Instant::now();
        let mut deleted_count = 0;
        let mut restored_count = 0;

        for (path, (_, book)) in &existing_books_with_hash {
            if !seen_paths.contains(path) {
                // File is missing from filesystem
                if !book.deleted {
                    // Mark as deleted
                    debug!("Marking missing book as deleted: {}", path);
                    match BookRepository::mark_deleted(db, book.id, true, event_broadcaster).await {
                        Ok(_) => {
                            deleted_count += 1;
                        }
                        Err(e) => {
                            let error_msg =
                                format!("Failed to mark book as deleted {}: {}", path, e);
                            warn!("{}", error_msg);
                            shared_state.add_error(error_msg).await;
                        }
                    }
                }
            } else if book.deleted {
                // File reappeared on filesystem, restore it
                debug!("Restoring deleted book: {}", path);
                match BookRepository::mark_deleted(db, book.id, false, event_broadcaster).await {
                    Ok(_) => {
                        restored_count += 1;
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to restore book {}: {}", path, e);
                        warn!("{}", error_msg);
                        shared_state.add_error(error_msg).await;
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
            shared_state.add_deleted(deleted_count).await;
            shared_state.add_restored(restored_count).await;
        }
    }

    // Extract final results
    let (result, _progress) = shared_state.into_result().await;
    Ok(result)
}

/// Result from hashing a file
struct FileHashResult {
    path: PathBuf,
    path_str: String,
    partial_hash: String,
    file_size: u64,
    modified_at: DateTime<Utc>,
    format: String,
}

/// Hash a single file and get its metadata
///
/// This is designed to be called concurrently with semaphore control.
async fn hash_file_with_metadata(file_path: PathBuf) -> Result<FileHashResult> {
    let path_str = file_path.to_string_lossy().to_string();

    // Calculate current partial hash (blocking I/O operation - fast, only first 1MB)
    let file_path_clone = file_path.clone();
    let current_partial_hash = tokio::task::spawn_blocking(move || {
        use crate::utils::hasher::hash_file_partial;
        hash_file_partial(&file_path_clone)
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn hash calculation task: {}", e))??;

    // Get file metadata (size and modified time)
    let file_path_clone = file_path.clone();
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

    Ok(FileHashResult {
        path: file_path,
        path_str,
        partial_hash: current_partial_hash,
        file_size,
        modified_at,
        format,
    })
}

/// Process a batch of files in parallel and collect them for batch DB operations
///
/// Returns a list of file hash results that can be used to create/update books.
async fn hash_files_parallel(
    files: Vec<PathBuf>,
    parallel_hashing: usize,
) -> Vec<Result<FileHashResult>> {
    let semaphore = Arc::new(Semaphore::new(parallel_hashing));

    let hash_futures: Vec<_> = files
        .into_iter()
        .map(|file_path| {
            let sem = Arc::clone(&semaphore);
            async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to acquire semaphore: {}", e))?;
                hash_file_with_metadata(file_path).await
            }
        })
        .collect();

    join_all(hash_futures).await
}

/// Process a single series using batch operations
///
/// This is the optimized version that:
/// - Hashes files in parallel
/// - Batch creates/updates books in the database
/// - Immediately queues analysis tasks
///
/// Returns a SeriesProcessResult instead of mutating state directly.
#[allow(clippy::too_many_arguments)]
async fn process_series_batched(
    db: &DatabaseConnection,
    library: &crate::db::entities::libraries::Model,
    detected_series: &DetectedSeries,
    existing_books_map: &HashMap<String, books::Model>,
    mode: ScanMode,
    config: &ScannerConfig,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<(SeriesProcessResult, bool)> {
    let mut result = SeriesProcessResult::new();

    // Extract file paths from detected books
    let file_paths: Vec<PathBuf> = detected_series
        .books
        .iter()
        .map(|b| b.path.clone())
        .collect();

    // Calculate series fingerprint from file paths
    let file_refs: Vec<&PathBuf> = file_paths.iter().collect();
    let fingerprint = calculate_series_fingerprint(&file_refs);

    // Use series path from detected series
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

    // Check if this is a new series
    let is_new_series = existing_books_map
        .values()
        .all(|book| book.series_id != series_model.id);

    // Create batch accumulator
    // Deep scan forces re-analysis of all books, Normal scan only analyzes new/changed books
    let force_analysis = mode == ScanMode::Deep;
    let mut batch = BookBatch::new(config.batch_size, force_analysis);

    // Process files in chunks for parallel hashing
    let now = Utc::now();
    let total_files = file_paths.len();
    let mut files_processed = 0;

    for chunk in file_paths.chunks(config.batch_size) {
        let chunk_start = Instant::now();

        // Hash all files in this chunk in parallel
        let hash_results = hash_files_parallel(chunk.to_vec(), config.parallel_hashing).await;

        for hash_result in hash_results {
            files_processed += 1;

            match hash_result {
                Ok(file_hash) => {
                    // Count every successfully hashed file as scanned
                    result.files_scanned += 1;

                    // Check if we should skip this file (normal mode only)
                    if mode == ScanMode::Normal {
                        if let Some(existing_book) = existing_books_map.get(&file_hash.path_str) {
                            if existing_book.partial_hash == file_hash.partial_hash {
                                // Partial hash hasn't changed - file is likely unchanged
                                if existing_book.analyzed {
                                    debug!(
                                        "Skipping unchanged analyzed file: {}",
                                        file_hash.path_str
                                    );
                                    continue;
                                }
                            }
                        }
                    }

                    // Check if book already exists by path
                    if let Some(existing_book) = existing_books_map.get(&file_hash.path_str) {
                        // Update existing book
                        let partial_hash_changed =
                            existing_book.partial_hash != file_hash.partial_hash;
                        let size_changed = existing_book.file_size != file_hash.file_size as i64;
                        let format_changed = existing_book.format != file_hash.format;
                        let modified_changed = existing_book.modified_at != file_hash.modified_at;
                        let anything_changed = partial_hash_changed
                            || size_changed
                            || format_changed
                            || modified_changed;

                        if anything_changed {
                            let mut updated_book = existing_book.clone();
                            updated_book.file_size = file_hash.file_size as i64;
                            updated_book.partial_hash = file_hash.partial_hash;
                            updated_book.format = file_hash.format;
                            updated_book.modified_at = file_hash.modified_at;
                            updated_book.updated_at = now;

                            // Mark as unanalyzed if hash changed
                            let needs_analysis = if partial_hash_changed {
                                updated_book.analyzed = false;
                                true
                            } else {
                                // For deep scan, always re-analyze; for normal scan, only if not analyzed
                                force_analysis || !existing_book.analyzed
                            };

                            batch.add_update(updated_book, needs_analysis);
                        } else if !existing_book.analyzed || force_analysis {
                            // No changes but needs analysis (or deep scan forces re-analysis)
                            batch.needs_analysis.push(existing_book.id);
                        }
                    } else {
                        // Create new book
                        let book_model = books::Model {
                            id: Uuid::new_v4(),
                            series_id: series_model.id,
                            library_id: library.id,
                            file_path: file_hash.path_str.clone(),
                            file_name: file_hash
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            file_size: file_hash.file_size as i64,
                            file_hash: String::new(), // Filled during analysis
                            partial_hash: file_hash.partial_hash,
                            format: file_hash.format,
                            page_count: 0, // Filled during analysis
                            deleted: false,
                            analyzed: false,
                            analysis_error: None,
                            modified_at: file_hash.modified_at,
                            created_at: now,
                            updated_at: now,
                            thumbnail_path: None,
                            thumbnail_generated_at: None,
                        };

                        batch.add_create(book_model, true);
                    }
                }
                Err(e) => {
                    result.errors.push(format!("Failed to hash file: {}", e));
                }
            }

            // Flush batch if full
            if batch.is_full() {
                let (created, updated, tasks_queued, errors) = batch.flush(db).await;
                result.books_created += created;
                result.books_updated += updated;
                result.tasks_queued += tasks_queued;
                result.errors.extend(errors);
            }
        }

        debug!(
            "Processed chunk of {} files in {:?} ({}/{} total)",
            chunk.len(),
            chunk_start.elapsed(),
            files_processed,
            total_files
        );
    }

    // Flush any remaining items in the batch
    if !batch.is_empty() {
        let (created, updated, tasks_queued, errors) = batch.flush(db).await;
        result.books_created += created;
        result.books_updated += updated;
        result.tasks_queued += tasks_queued;
        result.errors.extend(errors);
    }

    Ok((result, is_new_series))
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
