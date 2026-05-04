use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, Semaphore, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::entities::{books, series};
use crate::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, TaskRepository,
};
use crate::events::{EventBroadcaster, TaskProgressEvent};
use crate::models::SeriesStrategy;
use crate::tasks::types::TaskType;

use super::strategies::{DetectedSeries, create_strategy};
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

/// Parse excluded_patterns from library into a GlobSet for efficient matching
///
/// Patterns are newline-separated and matched case-insensitively.
/// For simple patterns (no path separators), we also add a `**/pattern` variant
/// to match at any directory depth.
///
/// # Examples
/// - `.DS_Store` → matches any `.DS_Store` file at any depth
/// - `_to_filter` → matches any directory/file named `_to_filter` at any depth
/// - `*.tmp` → matches any `.tmp` file at any depth
/// - `subdir/*` → matches everything inside `subdir/` relative to library root
fn parse_excluded_patterns(library: &crate::db::entities::libraries::Model) -> Option<GlobSet> {
    library.excluded_patterns.as_ref().and_then(|patterns| {
        let mut builder = GlobSetBuilder::new();
        let mut pattern_count = 0;

        for line in patterns.lines() {
            let pattern = line.trim();
            if pattern.is_empty() {
                continue;
            }

            // Add the exact pattern (case-insensitive)
            if let Ok(glob) = GlobBuilder::new(pattern).case_insensitive(true).build() {
                builder.add(glob);
                pattern_count += 1;
            }

            // For patterns without path separators, also add **/{pattern}
            // to match at any depth in the directory tree
            if !pattern.contains('/') && !pattern.starts_with("**") {
                let deep_pattern = format!("**/{}", pattern);
                if let Ok(glob) = GlobBuilder::new(&deep_pattern)
                    .case_insensitive(true)
                    .build()
                {
                    builder.add(glob);
                }
            }
        }

        if pattern_count == 0 {
            return None;
        }

        match builder.build() {
            Ok(globset) => Some(globset),
            Err(e) => {
                warn!("Failed to build exclusion GlobSet: {}", e);
                None
            }
        }
    })
}

/// Check if a path should be excluded based on exclusion patterns
///
/// Matches both the filename and the relative path from the library root.
/// This ensures patterns like `.DS_Store` match anywhere, while patterns
/// like `subdir/*` only match relative paths.
fn should_exclude(path: &Path, library_path: &Path, excluded: &GlobSet) -> bool {
    // Check filename (for patterns like `.DS_Store`, `Thumbs.db`)
    if let Some(name) = path.file_name()
        && excluded.is_match(name)
    {
        return true;
    }

    // Check relative path from library root (for patterns like `subdir/*`)
    if let Ok(relative) = path.strip_prefix(library_path)
        && excluded.is_match(relative)
    {
        return true;
    }

    false
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
    /// Optional task id + broadcaster for emitting TaskProgressEvent
    task_id: Option<Uuid>,
    library_name: String,
    event_broadcaster: Option<Arc<EventBroadcaster>>,
}

impl SharedScanState {
    fn new(
        library_id: Uuid,
        library_name: String,
        progress_tx: Option<mpsc::Sender<ScanProgress>>,
        task_id: Option<Uuid>,
        event_broadcaster: Option<Arc<EventBroadcaster>>,
    ) -> Self {
        let mut progress = ScanProgress::new(library_id);
        progress.start();
        Self {
            progress: Arc::new(Mutex::new(progress)),
            result: Arc::new(Mutex::new(ScanResult::new())),
            progress_tx,
            task_id,
            library_name,
            event_broadcaster,
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

    /// Send current progress through the channel and emit a TaskProgressEvent
    /// if a task id and broadcaster are available.
    async fn send_progress(&self) {
        let progress = self.progress.lock().await.clone();

        if let (Some(task_id), Some(broadcaster)) = (self.task_id, self.event_broadcaster.as_ref())
        {
            let total = progress.files_total;
            let current = progress.files_processed.min(total.max(1));
            let message = if total == 0 {
                format!("Scanning {} (discovering files…)", self.library_name)
            } else {
                format!(
                    "Scanning {} ({}/{} files, {} series, {} books)",
                    self.library_name, current, total, progress.series_found, progress.books_found,
                )
            };
            let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                task_id,
                "scan_library",
                current,
                total.max(current),
                Some(message),
                Some(progress.library_id),
                None,
                None,
            ));
        }

        if let Some(ref tx) = self.progress_tx
            && let Err(e) = tx.send(progress).await
        {
            warn!("Failed to send progress update: {}", e);
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
            task_id: self.task_id,
            library_name: self.library_name.clone(),
            event_broadcaster: self.event_broadcaster.clone(),
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

            match TaskRepository::enqueue_batch(db, tasks, None).await {
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
    task_id: Option<Uuid>,
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
    let result = scan_batched(
        db,
        &library,
        mode,
        progress_tx.clone(),
        event_broadcaster,
        task_id,
    )
    .await;

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
    if result.is_ok()
        && let Err(e) = LibraryRepository::update_last_scanned(db, library_id).await
    {
        warn!("Failed to update last_scanned_at: {}", e);
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
    task_id: Option<Uuid>,
) -> Result<ScanResult> {
    // Load scanner configuration from database settings
    let config = ScannerConfig::load(db).await;
    info!(
        "Scanner config: batch_size={}, parallel_hashing={}, parallel_series={}",
        config.batch_size, config.parallel_hashing, config.parallel_series
    );

    // Create shared state for thread-safe progress tracking
    let shared_state = SharedScanState::new(
        library.id,
        library.name.clone(),
        progress_tx,
        task_id,
        event_broadcaster.cloned(),
    );

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

    // Parse excluded patterns
    let excluded_patterns = parse_excluded_patterns(library);
    if let Some(ref patterns) = excluded_patterns {
        info!(
            "Library '{}' has {} exclusion patterns configured",
            library.name,
            patterns.len()
        );
    }

    // Discover all files in library (blocking I/O operation)
    let discover_start = Instant::now();
    let library_path = library.path.clone();
    let allowed_extensions_clone = allowed_extensions.clone();
    let excluded_patterns_clone = excluded_patterns.clone();
    info!(
        "Starting file discovery for {} scan in library path: {}",
        mode, library_path
    );
    let discovered_files = tokio::task::spawn_blocking(move || {
        discover_files(
            &library_path,
            allowed_extensions_clone.as_deref(),
            excluded_patterns_clone.as_ref(),
        )
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

    // Collect all series paths for duplicate detection during fingerprint matching
    // This allows us to distinguish between a rename (old path not in scan) vs copy (both paths in scan)
    let all_series_paths: HashSet<String> =
        series_map.values().filter_map(|s| s.path.clone()).collect();
    let all_series_paths = Arc::new(all_series_paths);

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
            let all_series_paths = Arc::clone(&all_series_paths);
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
                    &all_series_paths,
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

    // Handle deleted/restored files
    let cleanup_start = Instant::now();
    let mut deleted_count = 0;
    let mut restored_count = 0;
    let mut affected_series_ids: HashSet<Uuid> = HashSet::new();

    for (path, (_, book)) in &existing_books_with_hash {
        if !seen_paths.contains(path) {
            // File is missing from filesystem
            if !book.deleted {
                // Mark as deleted
                debug!("Marking missing book as deleted: {}", path);
                match BookRepository::mark_deleted(db, book.id, true, event_broadcaster).await {
                    Ok(_) => {
                        deleted_count += 1;
                        affected_series_ids.insert(book.series_id);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to mark book as deleted {}: {}", path, e);
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
                    affected_series_ids.insert(book.series_id);
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

    // Renumber books in series affected by deletions or restorations
    // This ensures book numbers stay contiguous and deleted books get cleared
    for series_id in &affected_series_ids {
        match super::renumber_series_books(db, *series_id, library.id).await {
            Ok(count) => {
                if count > 0 {
                    debug!(
                        "Renumbered {} books in series {} after delete/restore cleanup",
                        count, series_id
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Failed to renumber books in series {} after cleanup: {}",
                    series_id, e
                );
            }
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
    koreader_hash: Option<String>,
    file_size: u64,
    modified_at: DateTime<Utc>,
    format: String,
}

/// Hash a single file and get its metadata
///
/// This is designed to be called concurrently with semaphore control.
async fn hash_file_with_metadata(file_path: PathBuf) -> Result<FileHashResult> {
    let path_str = file_path.to_string_lossy().to_string();

    // Calculate current partial hash and KOReader hash (blocking I/O)
    let file_path_clone = file_path.clone();
    let (current_partial_hash, koreader_hash) = tokio::task::spawn_blocking(move || {
        use crate::utils::hasher::{hash_file_koreader, hash_file_partial};
        let partial = hash_file_partial(&file_path_clone)?;
        let koreader = hash_file_koreader(&file_path_clone).ok();
        Ok::<_, std::io::Error>((partial, koreader))
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
        koreader_hash,
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
    all_series_paths: &HashSet<String>,
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

    debug!(
        "Processing detected series: name='{}', path='{}', fingerprint='{}', file_count={}",
        detected_series.name,
        series_path,
        &fingerprint[..16], // First 16 chars of fingerprint for brevity
        file_paths.len()
    );

    // Get preprocessing rules from library
    let preprocessing_rules = LibraryRepository::get_preprocessing_rules(library);

    // Find or create series with fingerprint
    let series_model = find_or_create_series(
        db,
        library.id,
        &detected_series.name,
        Some(&fingerprint),
        series_path,
        all_series_paths,
        &preprocessing_rules,
        event_broadcaster,
    )
    .await?;

    debug!(
        "Series resolution: detected='{}' -> db_id={}, db_name='{}', db_path='{}'",
        detected_series.name, series_model.id, series_model.name, series_model.path
    );

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
                    if mode == ScanMode::Normal
                        && let Some(existing_book) = existing_books_map.get(&file_hash.path_str)
                        && existing_book.partial_hash == file_hash.partial_hash
                    {
                        // Partial hash hasn't changed - file is likely unchanged
                        if existing_book.analyzed {
                            debug!("Skipping unchanged analyzed file: {}", file_hash.path_str);
                            continue;
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
                        // Check if book needs to move to a different series (deep scan only)
                        let series_changed =
                            force_analysis && existing_book.series_id != series_model.id;
                        let anything_changed = partial_hash_changed
                            || size_changed
                            || format_changed
                            || modified_changed
                            || series_changed;

                        // Always update koreader_hash (it may have been computed
                        // with a corrected algorithm or may be newly available)
                        let koreader_hash_changed =
                            existing_book.koreader_hash != file_hash.koreader_hash;
                        let anything_changed = anything_changed || koreader_hash_changed;

                        if anything_changed {
                            let mut updated_book = existing_book.clone();
                            updated_book.file_size = file_hash.file_size as i64;
                            updated_book.partial_hash = file_hash.partial_hash;
                            updated_book.koreader_hash = file_hash.koreader_hash;
                            updated_book.format = file_hash.format;
                            updated_book.modified_at = file_hash.modified_at;
                            updated_book.updated_at = now;

                            // Update series_id if book moved to a different folder (deep scan only)
                            if series_changed {
                                debug!(
                                    "Book '{}' moved from series {} to series {}",
                                    file_hash.path_str, existing_book.series_id, series_model.id
                                );
                                updated_book.series_id = series_model.id;
                            }

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
                            analysis_errors: None,
                            modified_at: file_hash.modified_at,
                            created_at: now,
                            updated_at: now,
                            thumbnail_path: None,
                            thumbnail_generated_at: None,
                            koreader_hash: file_hash.koreader_hash,
                            epub_positions: None,
                            epub_spine_items: None,
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

    // Renumber all books in the series when the series composition changed
    // (new books added to an existing series).
    // This ensures book numbers reflect the correct natural sort order
    // even for books that weren't re-analyzed in this scan.
    if result.books_created > 0 && !is_new_series {
        match super::renumber_series_books(db, series_model.id, library.id).await {
            Ok(count) => {
                if count > 0 {
                    debug!(
                        "Renumbered {} books in series '{}' after adding new books",
                        count, series_model.name
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Failed to renumber books in series '{}': {}",
                    series_model.name, e
                );
            }
        }
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
        if let Some(filename) = path.file_name()
            && let Some(name_str) = filename.to_str()
        {
            // Normalize: lowercase, alphanumeric only
            let normalized: String = name_str
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect();
            hasher.update(normalized.as_bytes());
        }
    }

    format!("{:x}", hasher.finalize())
}

/// Find or create a series using a 3-step matching strategy
///
/// Matching strategy (in order of priority):
/// 1. **Path match**: Same directory = same series (primary key)
/// 2. **Fingerprint match**: Directory renamed but same files = same series
///    - Only accepts match if old path is NOT in current scan (true rename vs copy)
/// 3. **Normalized name match**: Last resort for moved+renamed directories
/// 4. If no match, create a new series
///
/// This approach ensures that:
/// - Adding/removing files from a series directory keeps the same series (path match)
/// - Renaming a series directory keeps the same series (fingerprint match)
/// - Copying a series directory creates a NEW series (old path still in scan)
/// - Moving AND renaming a series directory may still match (normalized name fallback)
///
/// The `all_series_paths` parameter contains paths of all series detected in the current scan.
/// This is used to distinguish between a rename (old path not in scan) vs a copy (old path in scan).
///
/// The `preprocessing_rules` parameter allows applying title preprocessing rules when creating
/// a new series. The original name is preserved in `series.name` for file recognition, while
/// the preprocessed title is stored in `series_metadata.title` for display and search.
#[allow(clippy::too_many_arguments)]
async fn find_or_create_series(
    db: &DatabaseConnection,
    library_id: Uuid,
    series_name: &str,
    fingerprint: Option<&str>,
    path: &str,
    all_series_paths: &HashSet<String>,
    preprocessing_rules: &[crate::services::metadata::preprocessing::PreprocessingRule],
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<series::Model> {
    use crate::db::repositories::SeriesMetadataRepository;
    use crate::services::metadata::preprocessing::apply_rules;

    debug!(
        "find_or_create_series: name='{}', path='{}', fingerprint={:?}",
        series_name,
        path,
        fingerprint.map(|f| &f[..16.min(f.len())])
    );

    // Apply preprocessing rules to get the metadata title
    let preprocessed_title = if preprocessing_rules.is_empty() {
        series_name.to_string()
    } else {
        apply_rules(series_name, preprocessing_rules)
    };
    let title_was_preprocessed = preprocessed_title != series_name;

    if title_was_preprocessed {
        debug!(
            "Title preprocessed: '{}' -> '{}'",
            series_name, preprocessed_title
        );
    }

    // Step 1: Path match (same directory = same series)
    // This is the primary matching key - if the path matches, it's definitely the same series
    debug!("Step 1: Searching by path='{}'", path);
    if let Some(existing) = SeriesRepository::find_by_path(db, library_id, path).await? {
        info!(
            "Matched series by path: {} -> series id {}",
            path, existing.id
        );

        // Update fingerprint and name if files changed (fingerprint may have changed)
        if let Some(fp) = fingerprint
            && (existing.fingerprint.as_ref() != Some(&fp.to_string())
                || existing.name != series_name)
        {
            debug!(
                "Updating series fingerprint/name after path match: {} (old fingerprint: {:?}, new: {})",
                series_name, existing.fingerprint, fp
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
                && metadata.title != series_name
                && !metadata.title_lock
            {
                SeriesRepository::update_name(db, existing.id, series_name).await?;
            }
        }

        return Ok(existing);
    }

    // Step 2: Fingerprint match (directory renamed, same files)
    // The directory was renamed but files stayed the same
    // IMPORTANT: Only accept fingerprint match if the OLD path is NOT in the current scan.
    // This distinguishes between a rename (old path not in scan) vs a copy/duplicate (both paths in scan).
    debug!("Step 1 failed, trying Step 2: fingerprint match");
    if let Some(fp) = fingerprint {
        debug!(
            "Step 2: Searching by fingerprint='{}'",
            &fp[..16.min(fp.len())]
        );
        if let Some(existing) = SeriesRepository::find_by_fingerprint(db, library_id, fp).await? {
            // Check if the old series path is also being scanned in this run
            // If the old path is in all_series_paths, it means both directories exist (copy/duplicate)
            // If the old path is NOT in all_series_paths, it means the directory was renamed
            let old_path_in_scan = all_series_paths.contains(&existing.path);

            if !old_path_in_scan {
                info!(
                    "Matched series by fingerprint: {} -> series id {} (path changed from {} to {})",
                    series_name, existing.id, existing.path, path
                );

                // Update path and name (directory was renamed)
                SeriesRepository::update_path_and_name(
                    db,
                    existing.id,
                    path.to_string(),
                    series_name,
                )
                .await?;

                // Also update series_metadata title if not locked
                if let Ok(Some(metadata)) =
                    SeriesMetadataRepository::get_by_series_id(db, existing.id).await
                    && metadata.title != series_name
                    && !metadata.title_lock
                {
                    info!(
                        "Detected series rename: {} -> {}",
                        metadata.title, series_name
                    );
                    SeriesRepository::update_name(db, existing.id, series_name).await?;
                }

                return Ok(existing);
            } else {
                debug!(
                    "Step 2: Fingerprint matched series '{}' but old path '{}' is also in current scan (copy, not rename), skipping",
                    existing.id, existing.path
                );
            }
        }
    }

    // Step 3: Normalized name match (last resort fallback)
    // The directory was moved AND renamed, but the name is similar
    debug!("Step 2 failed, trying Step 3: normalized name match");
    let normalized_name = SeriesRepository::normalize_name(series_name);
    debug!("Step 3: Searching by normalized_name='{}'", normalized_name);
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
                && !metadata.title_lock
            {
                SeriesRepository::update_name(db, existing.id, series_name).await?;
            }
        }

        return Ok(existing);
    }

    // Step 4: Create new series with fingerprint (title stored in series_metadata)
    debug!("Step 3 failed, proceeding to Step 4: create new series");
    info!(
        "Creating new series: name='{}', title='{}' at path {} with fingerprint {:?}",
        series_name, preprocessed_title, path, fingerprint
    );

    // Create series with preprocessed title
    // - series.name = original directory name (for file recognition)
    // - series_metadata.title = preprocessed title (for display and search)
    SeriesRepository::create_with_fingerprint_and_title(
        db,
        library_id,
        series_name,
        fingerprint.map(String::from),
        path.to_string(),
        if title_was_preprocessed {
            Some(&preprocessed_title)
        } else {
            None
        },
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
///
/// # Arguments
/// - `library_path`: Root path of the library to scan
/// - `allowed_extensions`: If Some, only files with these extensions will be included.
///   If None, all supported formats are allowed.
/// - `excluded_patterns`: If Some, files/directories matching these patterns will be skipped.
///   Uses WalkDir's `filter_entry` to prune entire directories, improving performance.
fn discover_files(
    library_path: &str,
    allowed_extensions: Option<&[String]>,
    excluded_patterns: Option<&GlobSet>,
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

    let library_path_buf = Path::new(library_path);

    // Build WalkDir iterator with optional exclusion filter
    // Using filter_entry prunes entire directories, avoiding traversal of excluded subtrees
    let walker = WalkDir::new(library_path).follow_links(false);

    let entries = if let Some(excluded) = excluded_patterns {
        // With exclusion patterns: use filter_entry for directory pruning
        walker
            .into_iter()
            .filter_entry(|entry| {
                let path = entry.path();
                // Don't exclude the root library path itself
                if path == library_path_buf {
                    return true;
                }
                !should_exclude(path, library_path_buf, excluded)
            })
            .filter_map(|e| e.ok())
            .collect::<Vec<_>>()
    } else {
        // Without exclusion patterns: simple iteration
        walker
            .into_iter()
            .filter_map(|e| e.ok())
            .collect::<Vec<_>>()
    };

    for entry in entries {
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
    if excluded_patterns.is_some() {
        debug!(
            "File discovery: found {} supported files from {} files checked in {} directories (with exclusion patterns), took {:?}",
            files.len(),
            files_checked,
            dirs_visited,
            duration
        );
    } else {
        debug!(
            "File discovery: found {} supported files from {} files checked in {} directories, took {:?}",
            files.len(),
            files_checked,
            dirs_visited,
            duration
        );
    }

    Ok(files)
}

/// Send progress update through channel
async fn send_progress(progress_tx: &Option<mpsc::Sender<ScanProgress>>, progress: &ScanProgress) {
    if let Some(tx) = progress_tx
        && let Err(e) = tx.send(progress.clone()).await
    {
        warn!("Failed to send progress update: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_calculate_series_fingerprint_consistency() {
        // Same files in same order should produce same fingerprint
        let files1 = [
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = [
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
        let files1 = [
            PathBuf::from("/library/Batman/issue3.cbz"),
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = [
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
        let files1 = [
            PathBuf::from("/library/Batman/Batman-001.cbz"),
            PathBuf::from("/library/Batman/Batman-002.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = [
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
        let files1 = [
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
            PathBuf::from("/library/Batman/issue3.cbz"),
            PathBuf::from("/library/Batman/issue4.cbz"),
            PathBuf::from("/library/Batman/issue5.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = [
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
        let files1 = [
            PathBuf::from("/library/Batman/issue1.cbz"),
            PathBuf::from("/library/Batman/issue2.cbz"),
        ];
        let refs1: Vec<&PathBuf> = files1.iter().collect();

        let files2 = [
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
        let files = [PathBuf::from("/library/Batman/standalone.cbz")];
        let refs: Vec<&PathBuf> = files.iter().collect();

        let fp = calculate_series_fingerprint(&refs);

        assert!(!fp.is_empty(), "Fingerprint should not be empty");
        assert_eq!(fp.len(), 64, "SHA-256 hex should be 64 characters");
    }

    // Helper to create a minimal library model for testing
    fn create_test_library(
        excluded_patterns: Option<String>,
    ) -> crate::db::entities::libraries::Model {
        use chrono::Utc;
        crate::db::entities::libraries::Model {
            id: Uuid::new_v4(),
            name: "Test Library".to_string(),
            path: "/test/library".to_string(),
            series_strategy: "series_volume".to_string(),
            series_config: None,
            book_strategy: "filename".to_string(),
            book_config: None,
            number_strategy: "file_order".to_string(),
            number_config: None,
            scanning_config: None,
            default_reading_direction: "ltr".to_string(),
            allowed_formats: None,
            excluded_patterns,
            title_preprocessing_rules: None,
            auto_match_conditions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_scanned_at: None,
        }
    }

    // ==================== parse_excluded_patterns tests ====================

    #[test]
    fn test_parse_excluded_patterns_none() {
        let library = create_test_library(None);
        let result = parse_excluded_patterns(&library);
        assert!(result.is_none(), "None patterns should return None");
    }

    #[test]
    fn test_parse_excluded_patterns_empty_string() {
        let library = create_test_library(Some("".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(result.is_none(), "Empty string should return None");
    }

    #[test]
    fn test_parse_excluded_patterns_whitespace_only() {
        let library = create_test_library(Some("   \n  \n   ".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(
            result.is_none(),
            "Whitespace-only patterns should return None"
        );
    }

    #[test]
    fn test_parse_excluded_patterns_single_pattern() {
        let library = create_test_library(Some(".DS_Store".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(result.is_some(), "Single pattern should return Some");
        let globset = result.unwrap();
        assert!(!globset.is_empty(), "GlobSet should have patterns");
    }

    #[test]
    fn test_parse_excluded_patterns_multiple_patterns() {
        let library = create_test_library(Some(".DS_Store\nThumbs.db\n@eaDir".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(result.is_some(), "Multiple patterns should return Some");
        let globset = result.unwrap();
        // Each pattern adds 2 entries (exact + **/)
        assert!(
            globset.len() >= 3,
            "GlobSet should have at least 3 patterns"
        );
    }

    #[test]
    fn test_parse_excluded_patterns_trims_whitespace() {
        let library = create_test_library(Some("  .DS_Store  \n  Thumbs.db  ".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(
            result.is_some(),
            "Patterns with whitespace should still work"
        );
        let globset = result.unwrap();
        // Should match trimmed patterns
        assert!(globset.is_match(".DS_Store"));
        assert!(globset.is_match("Thumbs.db"));
    }

    #[test]
    fn test_parse_excluded_patterns_path_pattern() {
        let library = create_test_library(Some("subdir/*".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(result.is_some(), "Path patterns should work");
        let globset = result.unwrap();
        // Path patterns with / should NOT get ** prefix added
        assert!(globset.is_match(Path::new("subdir/file.cbz")));
    }

    #[test]
    fn test_parse_excluded_patterns_glob_wildcards() {
        let library = create_test_library(Some("*.tmp\n*.bak".to_string()));
        let result = parse_excluded_patterns(&library);
        assert!(result.is_some(), "Glob patterns should work");
        let globset = result.unwrap();
        assert!(globset.is_match("file.tmp"));
        assert!(globset.is_match("backup.bak"));
        assert!(!globset.is_match("file.cbz"));
    }

    // ==================== should_exclude tests ====================

    #[test]
    fn test_should_exclude_exact_filename() {
        let library = create_test_library(Some(".DS_Store".to_string()));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        assert!(should_exclude(
            Path::new("/library/.DS_Store"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/subdir/.DS_Store"),
            library_path,
            &globset
        ));
    }

    #[test]
    fn test_should_exclude_case_insensitive() {
        let library = create_test_library(Some(".DS_Store".to_string()));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        // Case variations should match
        assert!(should_exclude(
            Path::new("/library/.ds_store"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/.DS_STORE"),
            library_path,
            &globset
        ));
    }

    #[test]
    fn test_should_exclude_directory_name() {
        let library = create_test_library(Some("_to_filter".to_string()));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        // Directory itself should be excluded
        assert!(should_exclude(
            Path::new("/library/_to_filter"),
            library_path,
            &globset
        ));
        // Nested directory should also be excluded
        assert!(should_exclude(
            Path::new("/library/series/_to_filter"),
            library_path,
            &globset
        ));
    }

    #[test]
    fn test_should_exclude_glob_pattern() {
        let library = create_test_library(Some("*.tmp".to_string()));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        assert!(should_exclude(
            Path::new("/library/file.tmp"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/subdir/another.tmp"),
            library_path,
            &globset
        ));
        assert!(!should_exclude(
            Path::new("/library/file.cbz"),
            library_path,
            &globset
        ));
    }

    #[test]
    fn test_should_exclude_relative_path_pattern() {
        let library = create_test_library(Some("subdir/*".to_string()));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        // Files in subdir/ should be excluded
        assert!(should_exclude(
            Path::new("/library/subdir/file.cbz"),
            library_path,
            &globset
        ));
        // Files NOT in subdir/ should not be excluded
        assert!(!should_exclude(
            Path::new("/library/other/file.cbz"),
            library_path,
            &globset
        ));
    }

    #[test]
    fn test_should_exclude_non_matching() {
        let library = create_test_library(Some(".DS_Store\nThumbs.db".to_string()));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        // Regular files should not be excluded
        assert!(!should_exclude(
            Path::new("/library/Batman/issue1.cbz"),
            library_path,
            &globset
        ));
        assert!(!should_exclude(
            Path::new("/library/series.epub"),
            library_path,
            &globset
        ));
    }

    #[test]
    fn test_should_exclude_common_patterns() {
        // Test common macOS/Windows/NAS patterns
        let library = create_test_library(Some(
            ".DS_Store\nThumbs.db\n@eaDir\n__MACOSX\n.Spotlight-V100".to_string(),
        ));
        let globset = parse_excluded_patterns(&library).unwrap();
        let library_path = Path::new("/library");

        assert!(should_exclude(
            Path::new("/library/.DS_Store"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/comics/Thumbs.db"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/@eaDir"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/__MACOSX"),
            library_path,
            &globset
        ));
        assert!(should_exclude(
            Path::new("/library/.Spotlight-V100"),
            library_path,
            &globset
        ));
    }

    // ==================== discover_files with exclusion tests ====================

    #[test]
    fn test_discover_files_no_exclusions() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create test files
        File::create(library_path.join("book1.cbz")).unwrap();
        File::create(library_path.join("book2.epub")).unwrap();

        let result = discover_files(library_path.to_str().unwrap(), None, None).unwrap();

        assert_eq!(result.len(), 2, "Should find 2 files without exclusions");
    }

    #[test]
    fn test_discover_files_with_file_exclusion() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create test files
        File::create(library_path.join("book1.cbz")).unwrap();
        File::create(library_path.join(".DS_Store")).unwrap();

        let library = create_test_library(Some(".DS_Store".to_string()));
        let excluded = parse_excluded_patterns(&library);

        let result =
            discover_files(library_path.to_str().unwrap(), None, excluded.as_ref()).unwrap();

        assert_eq!(
            result.len(),
            1,
            "Should find only 1 file (excluding .DS_Store)"
        );
        assert!(result[0].ends_with("book1.cbz"));
    }

    #[test]
    fn test_discover_files_with_directory_exclusion() {
        use std::fs::{self, File};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create directory structure
        fs::create_dir(library_path.join("series")).unwrap();
        fs::create_dir(library_path.join("_to_filter")).unwrap();

        // Create test files
        File::create(library_path.join("series/book1.cbz")).unwrap();
        File::create(library_path.join("_to_filter/book2.cbz")).unwrap();

        let library = create_test_library(Some("_to_filter".to_string()));
        let excluded = parse_excluded_patterns(&library);

        let result =
            discover_files(library_path.to_str().unwrap(), None, excluded.as_ref()).unwrap();

        assert_eq!(
            result.len(),
            1,
            "Should find only 1 file (excluding _to_filter directory)"
        );
        assert!(result[0].to_string_lossy().contains("series"));
    }

    #[test]
    fn test_discover_files_with_glob_exclusion() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create test files
        File::create(library_path.join("book1.cbz")).unwrap();
        File::create(library_path.join("temp.tmp")).unwrap();
        File::create(library_path.join("backup.bak")).unwrap();

        let library = create_test_library(Some("*.tmp\n*.bak".to_string()));
        let excluded = parse_excluded_patterns(&library);

        let result =
            discover_files(library_path.to_str().unwrap(), None, excluded.as_ref()).unwrap();

        assert_eq!(
            result.len(),
            1,
            "Should find only 1 file (excluding *.tmp and *.bak)"
        );
        assert!(result[0].ends_with("book1.cbz"));
    }

    #[test]
    fn test_discover_files_nested_directory_exclusion() {
        use std::fs::{self, File};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create nested directory structure
        fs::create_dir_all(library_path.join("series/good")).unwrap();
        fs::create_dir_all(library_path.join("series/_to_filter/deep")).unwrap();

        // Create test files
        File::create(library_path.join("series/good/book1.cbz")).unwrap();
        File::create(library_path.join("series/_to_filter/book2.cbz")).unwrap();
        File::create(library_path.join("series/_to_filter/deep/book3.cbz")).unwrap();

        let library = create_test_library(Some("_to_filter".to_string()));
        let excluded = parse_excluded_patterns(&library);

        let result =
            discover_files(library_path.to_str().unwrap(), None, excluded.as_ref()).unwrap();

        // Should only find book1.cbz - the _to_filter directory and all its contents should be excluded
        assert_eq!(
            result.len(),
            1,
            "Should find only 1 file (directory pruning should exclude nested files)"
        );
        assert!(result[0].to_string_lossy().contains("good"));
    }

    #[test]
    fn test_discover_files_multiple_exclusions() {
        use std::fs::{self, File};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Create directory structure
        fs::create_dir(library_path.join("series")).unwrap();
        fs::create_dir(library_path.join("@eaDir")).unwrap();

        // Create test files
        File::create(library_path.join("series/book1.cbz")).unwrap();
        File::create(library_path.join("series/.DS_Store")).unwrap();
        File::create(library_path.join("@eaDir/metadata.db")).unwrap();

        let library = create_test_library(Some(".DS_Store\n@eaDir\nThumbs.db".to_string()));
        let excluded = parse_excluded_patterns(&library);

        let result =
            discover_files(library_path.to_str().unwrap(), None, excluded.as_ref()).unwrap();

        assert_eq!(
            result.len(),
            1,
            "Should find only 1 file with multiple exclusions"
        );
        assert!(result[0].ends_with("book1.cbz"));
    }

    #[test]
    fn test_discover_files_caltrash_dot_prefix_exclusion() {
        use std::fs::{self, File};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path();

        // Reproduce Calibre library structure with .caltrash and .calnotes
        fs::create_dir_all(library_path.join(".caltrash/b/1")).unwrap();
        fs::create_dir_all(library_path.join(".caltrash/b/2")).unwrap();
        fs::create_dir_all(library_path.join(".calnotes")).unwrap();
        fs::create_dir_all(library_path.join("Author Name/Book Title")).unwrap();

        // Create files in .caltrash (should be excluded)
        File::create(library_path.join(".caltrash/b/1/book.epub")).unwrap();
        File::create(library_path.join(".caltrash/b/2/book.epub")).unwrap();
        // Create files in .calnotes (should be excluded)
        File::create(library_path.join(".calnotes/notes.db")).unwrap();
        // Create legitimate book (should be found)
        File::create(library_path.join("Author Name/Book Title/book.epub")).unwrap();

        let library = create_test_library(Some(".caltrash\n.calnotes".to_string()));
        let excluded = parse_excluded_patterns(&library);

        let result =
            discover_files(library_path.to_str().unwrap(), None, excluded.as_ref()).unwrap();

        assert_eq!(
            result.len(),
            1,
            "Should find only 1 file (excluding .caltrash and .calnotes directories). Found: {:?}",
            result
        );
        assert!(
            result[0].to_string_lossy().contains("Author Name"),
            "Should only find the legitimate book"
        );
    }
}
